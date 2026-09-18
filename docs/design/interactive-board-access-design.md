# CivSpatial — Interactive (Queryable) Board-Access Design + v1 Spec

**Status:** design, ready to build v1. Written 2026-07-28. Companion to `DESIGN.md`,
`findings-t677-crossover.md` (why: static hierarchical can't win on cost), and
`T2-T3-design.md` (question tiers — orthogonal to this).

**One-line:** add a **board-access mode where the model queries the board coarse-to-fine through
tools** instead of receiving it all upfront — the only mode that can be *cheaper* than `raw`, the one
where hierarchy's real payoff lives, and (bonus) the one that dissolves the "teaching-to-the-test"
validity problem.

---

## 0. TL;DR of decisions

1. **Interactive is a new board-access mode, not a new text encoder.** It sits alongside
   raw/ascii/hierarchical as another `encoding` value in the same comparison, but it's driven by a
   **multi-turn tool loop**, not a one-shot `render`.
2. **Why it matters:** the static `hierarchical` encoder front-loads *every* leaf, so it is
   structurally **always more tokens than raw** — it can never reach hierarchy's real advantage,
   which is *not paying for detail you don't fetch* (`findings-t677-crossover.md`). Only a query loop
   captures that.
3. **The coarse level is deliberately lossy and qualitative** — deterministic map-like *descriptions*
   ("predominantly hilly, sea along the eastern edge, ~a quarter of the region"), **not** exact
   counts. Compute exactly, describe coarsely. Generated **without an LLM** (§3).
4. **Tools are observation-only, never answer-shaped** (no `count_in_radius`, `nearest`, `best_site`).
   The model composes observations into answers — same discipline as static hierarchical (§2).
5. **The metric is cost-to-decision:** total billed tokens across the whole trajectory, at cache
   pricing, vs. accuracy. Interactive is the only mode that can undercut raw's cost (§5).
6. **Tool outputs count against cost/budget** — that's what creates the economic pressure that makes
   the experiment meaningful; not counting them collapses interactive into "raw, but slow" (§5).
7. **MAX_TURNS is a safety backstop (generous, ~16), not the economic lever;** a per-question token
   budget is the real pressure (§4).
8. **Prompt the mechanics, never the strategy** — teach tool use + cost tradeoff + answer format;
   never what to look for. Strategy coaching = leakage (§6).
9. **Completeness gate = reconstruct-the-board-via-tools** (`get_tile`/`scan` parity). The Oracle-100%
   invariant stays on the static encoders; interactive gets the parity gate instead (§7).
10. **Whack-a-mole dissolves:** with a fixed, domain-neutral verb set, *the model* chooses what to
    fetch — we no longer curate "the ideal summary," we measure navigation efficiency (§9).

---

## 1. Why interactive (the arc)

Static `hierarchical` = PART 1 summaries **+** PART 2 (every non-default leaf), all in one prompt. So
its token cost is `raw` + a summary layer → **always > raw, by construction** (measured: tied accuracy,
+40% tokens, permanently off the cost frontier). The theoretical value of hierarchy has two halves:

- **(A) coarse-to-fine reasoning** — survey, then focus. An *accuracy* lever. Static gets it, weakly.
- **(B) don't load/pay for detail you don't need** — a *cost* lever. Static **cannot** get it.

(B) is where hierarchy would actually beat raw, and it needs a **tool loop**: overview first, then the
model *issues queries* to pull only what it needs, paying only for that. Humans don't stay at one level
of representation; they zoom. This mode lets the model zoom.

---

## 2. The queryable board surface (the tools)

A fixed **multi-resolution pyramid** of observation verbs. Fixed granularities + explicit coordinates
only; **no answer-shaped verbs**.

```
overview()                     # given upfront, cheap, always in context:
                               #   board dims, default terrain, the partition scheme, and a
                               #   COARSE qualitative description per top-level cell (quadrants).

region_summary(sector="R2C4")  # a fixed 10x10 sector's QUALITATIVE description (§3): dominant
                               #   terrain + coarse share, water/edge concentration, occupants,
                               #   ownership tilt. NO exact per-terrain counts.

scan(x0,y0,x1,y1)              # every non-default tile's FULL facts in a box. Area-capped
                               #   (v1: <= 12x12) so "scan the whole board" costs visibly and
                               #   takes many calls. (PART 2, on demand.)

get_tile(x,y)                  # one tile's complete facts. <- parity anchor
```

**Deliberately excluded** (would answer the question): `count_terrain_in_radius`, `nearest`,
`best_site`, `distance`, arbitrary-window histograms. Summaries describe **fixed** sectors/quadrants
only — "hills within radius 2 of T" still forces the model to pick sectors, scan partial overlaps, and
reason. The tools give *sight at a chosen resolution*; the reasoning stays the model's job.

Deferred to v2: `list_cities(region?)`, `list_units(region?, owner?)` (v1 surfaces occupants inside
overview/region_summary/scan, which is enough).

---

## 3. The deterministic region-description generator (its own testable module)

New module `civ-eval/src/describe.rs`. `fn describe_region(board, x0,y0,x1,y1) -> String`. **Compute
exactly, emit coarsely** — the histogram is internal; the output is bucketed and map-like, so no exact
count leaks and the coarse level stays honestly lossy.

**Pipeline**
1. Terrain histogram over the box (exact, internal); water fraction (Ocean/Deep Ocean/Lake).
2. **Dominant terrain + share → quantifier:** >0.65 "predominantly {T}", 0.45–0.65 "mostly {T}",
   top-two within 0.15 "a mix of {A} and {B}", else "varied terrain, chiefly {T}".
3. **Coverage fraction → coarse bucket:** <0.10 "a sliver", 0.10–0.20 "a small part", 0.20–0.30
   "about a quarter", 0.30–0.42 "about a third", 0.42–0.58 "about half", 0.58–0.80 "most", >0.80
   "nearly all".
4. **Spatial concentration:** split the box into a 3×3 cell grid, compute each notable terrain's mass
   per cell, label where it concentrates → {throughout, in the center, along the {N/S/E/W} edge, in the
   {NE/NW/SE/SW} corner}. (E.g. Ocean mass in the east column → "sea along the eastern edge.")
5. **Occupants / ownership:** cities (count + owners), units (presence + owners), tile-ownership tilt
   → "holds 1 city of player 0", "contested (units of players 0 and 2)", "unclaimed".
6. **Render** a 1–2 sentence template from the pieces.

Example → *"Predominantly hilly, with sea along the eastern edge (about a quarter of the region). Holds
1 city of player 0; a few of its units nearby."*

**Correct by construction** (derived from exact data), so testing is about the *buckets and phrasing*,
not ground-truth mismatch. **Unit tests** on hand-built regions assert: the named dominant terrain is
the true plurality; the coverage bucket matches the true fraction's bucket; the named edge matches the
computed concentration; occupant/owner phrases match. Start simple (dominant + water + concentration +
occupancy); enrich messy multi-terrain cases iteratively. The description is **lossy → not
reconstructable**; parity comes only from `get_tile`/`scan` (§7).

---

## 4. The tool-loop runner + turn/budget policy

```
messages = [ system(overview + rules_block + tool instructions §6), user(question) ]
usage_total = 0 ; turns = 0 ; tool_calls = 0
loop:
    if turns >= MAX_TURNS or usage_total >= TOKEN_BUDGET:
        answer = force_answer(messages)                 # "answer now with what you have"
        break
    reply = model.chat(messages, TOOLS)                 # ONE API round-trip
    usage_total += reply.usage ; turns += 1
    if reply.tool_calls:
        for call in reply.tool_calls:
            result = surface.execute(board, call)        # run the verb; result is text
            messages.push(assistant(tool_call), tool(result)) ; tool_calls += 1
        continue
    else:
        answer = extract("Answer:", reply.text) ; break
record(answer, usage_total, turns, tool_calls)
```

- **MAX_TURNS = 16 (v1)** — a *backstop against loops*, not the economic lever. Set generous so we
  measure navigation efficiency, not "performance under a tight cap." **Log the turn-count
  distribution**; if most questions finish in <8, the cap isn't binding (good). Tighten only if nothing
  approaches it.
- **TOKEN_BUDGET (v1) = 3× the board's raw token size** (~170k on T677) — the runaway backstop. The
  real economic pressure is that every fetch counts (§5), not this cap.
- **force_answer** = one final turn with "you've used your budget; give your best answer now as
  `Answer: <X>`" so every question yields a scorable reply.

---

## 5. Cost metric + budget accounting

**Metric = total billed tokens across the trajectory (at cache pricing), vs. accuracy.** Interactive
becomes a new row in the same table as raw/ascii/hierarchical; the headline question is **does
interactive reach raw's accuracy at fewer total tokens?** — the 2-D frontier.

**Tool outputs COUNT** (toward both the reported cost and the budget cap). Why:
- They *are* billed regardless — appended tool results are input tokens on every later turn.
- Counting them is the **economic pressure**: a `region_summary` (a sentence) is cheap; a 12×12 `scan`
  (up to 144 tile lines) is expensive → the model is incentivized to use coarse descriptions and fetch
  detail sparingly, i.e. to actually use the hierarchy.
- It makes the vs-raw comparison **fair and one-currency**: raw pays for the whole board upfront;
  interactive pays for what it pulls; both in total billed tokens-to-correct-answer.
- **Not** counting them → the model scans everything for free → interactive collapses into "raw, but
  slow"; the cost advantage becomes invisible. (Only useful as a *secondary* "reasoning-cost-only"
  analysis, never the headline.)

**Caching softens it:** with prefix caching the accumulated context is re-read at the cache rate
(~$0.028/M on DeepSeek) on later turns — billed near-full-price roughly once, cheap thereafter. That's
what makes multi-turn viable. **Fairness knob:** keep tool output formats **terse** (comparable to
raw's per-tile format) so we don't inflate interactive's cost artificially.

---

## 6. Prompting policy — mechanics, never strategy

The system prompt teaches only **how to operate**:
- you cannot see the whole board; here are the tools + their args;
- **summaries are cheap and approximate; `scan`/`get_tile` are exact but you pay per tile**;
- a typical flow: survey with `overview` → zoom into promising regions with `region_summary` → pull
  exact detail only where needed with `scan`/`get_tile` → then reply `Answer: <X>`.

It **never** teaches domain strategy ("check safety before settling", "count the hills in the radius").
Coaching *what to look for* is teaching-to-the-test. Mechanics = fair; strategy = leakage. (How much
even the mechanics-prompt helps is itself a variable — keep it fixed across encodings and note it.)

---

## 7. Completeness / parity gate

The tool surface must hide nothing. **Gate:** rebuild the exact board using only the tools — tile the
board with `scan` (or `get_tile` every coord) and assert the recovered board equals the true board
(same discipline as `Encoder::reconstruct`; this is the backlog's `get_tile`-parity check). Guarantees
the model *could* obtain everything; it just shouldn't need to.

**Oracle-100% invariant:** stays on the static encoders (unchanged). It doesn't map cleanly onto a
trajectory (there's no single prompt), so v1 relies on: the parity gate (surface completeness) + the
region-description unit tests (§3) + the **unchanged** offline solvers/scorer (the final-answer
extraction and scoring path is shared with the static runner). A scripted "oracle navigator" that
issues the minimal tool calls and checks the answer is a v2 nicety, not a v1 gate.

---

## 8. Code integration points

- **`civ-eval/src/describe.rs`** (new) — `describe_region` (§3) + unit tests.
- **Queryable surface trait** (new, e.g. `src/surface.rs`):
  ```rust
  pub struct ToolDef { pub name: &'static str, pub description: String, pub params_schema: String }
  pub struct ToolCall { pub id: String, pub name: String, pub args_json: String }
  pub trait QueryableSurface {
      fn name(&self) -> &str;                                   // "interactive"
      fn overview(&self, board: &Board) -> String;             // cheap initial context
      fn tools(&self) -> Vec<ToolDef>;                         // schemas for the API
      fn execute(&self, board: &Board, call: &ToolCall) -> String;  // run one verb -> text
      fn reconstruct_via_tools(&self, board: &Board) -> RecoveredBoard; // parity gate
  }
  ```
- **`Model` multi-turn extension** (behind `remote`):
  ```rust
  pub struct Message { pub role: Role, pub content: String, pub tool_call_id: Option<String>,
                       pub tool_calls: Vec<ToolCall> }
  pub struct ChatReply { pub text: Option<String>, pub tool_calls: Vec<ToolCall>,
                         pub usage: Usage, pub latency_ms: u64 }
  pub trait ChatModel { fn chat(&self, messages: &[Message], tools: &[ToolDef]) -> ChatReply; }
  ```
  The Oracle does not implement it; interactive is a `remote`-only path.
- **`remote.rs`** — add the OpenAI `tools` array to the request body; parse `tool_calls` from the
  response; support `role:"tool"` and assistant-with-tool_calls messages. Move the `cache_control`
  breakpoint to the end of the stable prefix (overview + rules + tool instructions). Standard
  function-calling; DeepSeek/OpenRouter support it — smoke-test caching across a growing conversation
  before any big run.
- **`runner.rs`** — a `run_interactive` path driving the loop (§4); reuses `score`/extraction unchanged.
- **`ResultRow`** — add `n_turns`, `n_tool_calls`; `prompt/completion/cached_tokens` become **sums over
  the trajectory**. `encoding` = "interactive". Summarizer picks these up for free.
- **CLI** — `--encoding interactive` (routes to `run_interactive`); reuse `--concurrency` (across
  questions), `--cache`, `--model`, `--think`.

---

## 9. Validity — how this dissolves whack-a-mole

The static-encoder worry was: *if we choose the summary content to match our questions, "hierarchical
wins" is circular.* Interactive removes the choice from us:
- We provide a **fixed, domain-neutral verb set** (observations at fixed granularities + leaves). We do
  **not** curate per-question summaries.
- **The model** decides what to fetch. We measure **how efficiently it navigates** to a correct answer.
- So the result generalizes: it's not "an encoder that pre-computed the answer wins," it's "a query
  surface lets the model reach the answer at lower cost than dumping everything" — a claim about
  *access*, not about *our summary-tuning*.

What could still bias it, to control: (a) tool-output verbosity (keep terse, §5); (b) the mechanics
prompt (keep fixed, strategy-free, §6); (c) the sector size / partition (a stated hyperparameter, not
tuned per question). None of these encode answers.

---

## 10. v1 SPEC — the smallest first version

**Scope (build exactly this):**
- `describe.rs` with `describe_region` (§3) + unit tests.
- `QueryableSurface` impl `Interactive` with **four verbs**: `overview`, `region_summary(sector)`,
  `scan(x0,y0,x1,y1)` (cap 12×12), `get_tile(x,y)`. Sector = the existing 10×10 partition.
- `ChatModel` + `remote.rs` tool plumbing; `run_interactive` loop; `MAX_TURNS=16`,
  `TOKEN_BUDGET=3×raw-board-tokens`, force-answer fallback.
- `ResultRow`: `n_turns`, `n_tool_calls`, trajectory token sums; `--encoding interactive`.

**Gates:** parity (`reconstruct_via_tools` == board) on T50 + T677; `describe_region` unit tests;
`just check` stays green (static encoders + Oracle unchanged).

**Test run:** on **T677**, compare **interactive vs raw vs hierarchical(static)**, think-off, on the
question kinds where hierarchy should help — **region-count** and the **siting** kinds (best-hills-site
once T1g exists; until then region-count + terrain/nearest). Include a couple of point-lookups
(`terrain`) as a floor/sanity. Small per-kind first (e.g. 4–6) to calibrate turn distribution + cost
before scaling.

**Success criterion (the whole point):** interactive reaches raw's accuracy on region-count/siting at
**fewer total billed tokens** (or ties tokens at higher accuracy) → hierarchy finally pays off, for the
right reason. If interactive costs *more* than raw for the same accuracy → the query overhead doesn't
beat front-loading at this board scale; report that plainly.

**Explicitly NOT in v1:** entity verbs (`list_cities/units`), T1g siting questions (separate build),
T3, a scripted oracle navigator, tunable sector size. Add after v1 shows signal.

---

## 11. Risks / open questions

- **Wall-clock.** N sequential round-trips per question. Mitigate with concurrency across questions +
  a small calibration run first. Interactive runs are slower and cost more in $ — price it in.
- **Caching across a growing multi-turn conversation** is finickier than the single-prefix case —
  smoke-test cache-hit rates before a big run (a broken cache makes interactive look far more expensive
  than it is).
- **Non-determinism** of trajectories — the parity gate + fixed solvers hold; but report token/turn
  *distributions*, not just means.
- **Force-answer confound** — a model cut off by the budget answers weakly; track how often force-answer
  fires (if it's common, the budget is too tight or the surface too coarse).
- **Tool-output verbosity fairness** — the single biggest lever on interactive's measured cost; fix the
  formats terse and identical in spirit to raw's per-tile line.

## 12. Build order

1. `describe.rs` + tests (pure, offline, no API). 
2. `QueryableSurface::Interactive` + parity gate (offline).
3. `ChatModel` + `remote.rs` tool plumbing + `run_interactive` (needs `remote`).
4. Wire `--encoding interactive`, `ResultRow` fields, summarizer.
5. Calibration run (small per-kind) on T677 → check turn distribution, cache hits, force-answer rate.
6. Scaled run: interactive vs raw vs hierarchical on region-count/siting → the cost-to-decision verdict.
