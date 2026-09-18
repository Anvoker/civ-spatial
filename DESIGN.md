# CivSpatial — Design

**An eval measuring how the *encoding* of a 4X game board affects an LLM's ability to
reason about it spatially — and what that reasoning *costs* in tokens and latency.**

This document is the durable design record. It assumes the framing in
`spatial-encoding-eval-continuity.md` (the original project brief) and records the
decisions made on top of it. Where the two disagree, this file wins — but the
divergences are called out explicitly.

---

## 1. What this is (and is not)

We freeze a real Freeciv board, describe it to a model several different ways
(*encodings*), ask the same programmatically-generated spatial questions about each, and
score answers against **computed** ground truth. The finding is **which encoding best
preserves spatial information for an LLM, at what token/latency cost, across question
difficulty and model tier.**

- **Frozen-state eval, not an agent.** No Freeciv game runs in the loop. Iterates in
  seconds; deterministic scoring; no wall-clock/$ blow-up.
- **Deterministic scoring.** Exact-match against ground truth computed by non-LLM code.
  No LLM-judge, no rater variance.
- **Three separated roles** (the load-bearing separation):
  - **parser** — `.sav` → neutral `Board`. The only Freeciv-aware code.
  - **encoder** — `Board` → a *board-access strategy* (a prompt string, or later a query
    surface). **The independent variable.**
  - **scorer** — model answer → correct/incorrect vs. computed ground truth. Independent
    of both parser and encoder.

### Divergences from the original brief (deliberate)
1. **Language: Rust**, not Python. Core is Rust; peripheral analysis/plots may use
   whatever fits (they consume JSONL/Parquet and are downstream).
2. **Cost is a first-class dependent variable.** The brief measured accuracy. We also
   measure **tokens, latency, and call count**, and map an **accuracy-vs-cost frontier**,
   not just an accuracy ranking. Recorded from day one, even for single-shot T0.
3. **Question difficulty is tiered T0–T3** (below), and T2/T3 introduce *valuation*
   (threat, defensibility) — which the brief deliberately fenced out as a separate
   failure mode. We admit them, under strict discipline (§7), because without them the
   realistic 4X board is decorative: pure-geometry questions could run on a random grid.
   **T0/T1 are built first; T2/T3 are architected-for but designed separately.**
4. **Interactive/queryable encodings are in scope** (later). A model querying
   deterministic tools over a *frozen* board is **not** "a game in the loop" — it stays
   frozen-state and deterministically scored. This is where token savings live.

---

## 2. Grid semantics (the single source of truth for "spatial")

The eval defines its **own** clean geometry, deliberately simpler than Freeciv's native
topology. The save is only a source of a *realistic terrain layout*; it does not dictate
movement rules. Both encoders and ground-truth solvers import one `geometry` module, so
they cannot drift — which is what makes exact-match scoring valid.

- **8-connected Moore neighborhood** — the 8 compass directions.
- **North is toward `y == 0`** (top row). N decreases y; S increases y; E increases x;
  W decreases x.
- **Distance is Chebyshev**: `max(|dx|, |dy|)` — king/8-direction steps. Matches "how
  many tiles to move" for a unit that steps diagonally.
- **Bounded, NON-wrapping rectangle.** No pole/edge wrap.

Every question prompt states the rules it depends on; the model is scored against ground
truth computed under exactly those rules.

---

## 3. The neutral Board (parser output)

`Board` is the boundary between Freeciv and everything else. Nothing downstream imports
anything Freeciv-specific; a `Board` could equally come from another platform's telemetry.

Coordinate convention: `x` east (column), `y` south (row), `tiles[y][x]`.

**Lean model (T0/T1):** per-tile `terrain` + `extras` set + territory `owner`; `City`
(x, y, id, name, owner, size); `Unit` (x, y, id, kind, owner); `Player` (id, name, nation).

**Rich model (lights up at T2/T3):** units grow `veteran` level and `hp`; tiles already
carry the full `extras` set. **The data is already in the save** (the unit table has
`veteran`/`hp` columns; extras are fully decoded), so this is a model extension, not a new
data source — the "budget for telling the LLM how strong a unit is" seam.

### Freeciv `.sav` format (verified against `data/saves/myagent_T50.sav`)
Plaintext, INI-like. `[section]` headers; `key=value`; quoted strings; comma vectors;
multi-line tables `key={"c0","c1",... \n rows \n }`.
- `[savefile]`: `terrident` table (identifier char → terrain name); `extras_vector`
  (ordered extra names).
- `[map]`:
  - `t<YYYY>="…"` — one char per tile; decode via `terrident`. (Ocean is the space char.)
  - `e<NN>_<YYYY>="…"` — **extras bit-planes.** Plane `NN` packs four extras as one hex
    digit per tile: bit 1 → `extras[4*NN+0]`, 2 → `+1`, 4 → `+2`, 8 → `+3`. (Verified:
    digit `2` in plane `e03` = River.)
  - `owner<YYYY>="a,b,…"` — per-tile territory owner: a player id or `-`.
  - Map width = length of a terrain row; height = number of terrain rows (78×52 here).
- `[player<i>]`: `name`, `nation`, city table `c={…}` (keyed **y,x,id,…,size,…,name**),
  unit table `u={…}` (keyed **id,x,y,…,type_by_name,…,veteran,hp,…**).
- Compressed saves (`.sav.xz`/`.sav.zst`) must be decompressed to plaintext first.

Board provenance (source filename) is carried on `Board.source` for result traceability.

---

## 4. The two-axis principle (why encodings differ)

Encodings vary along **two independent axes** that trade off:

- **Spatial fidelity** — how well 2D layout survives (ASCII map: high; coordinate list: ~0).
- **Information capacity** — how much per-tile detail is representable (coordinate list:
  everything; single-glyph ASCII: ~1 symbol/tile, lossy).

**Load-bearing rule — information-completeness:**
> For any question posed under an encoding, that encoding **must be information-complete
> with respect to that question.** Otherwise the eval measures representational *capacity*,
> not spatial *reasoning*. The encoding comparison is only valid when **information content
> is held constant** and we vary how **spatial/relational structure** is presented (and
> how much relational computation is pre-done).

Consequence: a single glyph cannot hold `terrain + Road + Iron + Veteran Spearman + HP`.
The honest, non-strawman ASCII encoding is therefore a **glyph grid + coordinate-keyed
legend** (layout from the grid, richness from the legend); the glyph shows only the most
salient occupant by fixed precedence (unit > city > notable-extra > terrain), the legend
disambiguates the rest. This "how to serialize a full tile" problem is **cross-cutting** —
every encoding answers it — so we define one canonical rendering (§5) and let each encoder
choose *where* the facts go.

---

## 5. TileFacts — canonical tile-content rendering

`TileFacts` is the neutral, complete description of what occupies a tile (terrain, ordered
extras, owner, any city, any unit with its rich attributes). Every encoder renders the
*same* `TileFacts`; they differ only in **placement**:

- **raw** — a flat list, one line per (notable) tile: `(x,y): <TileFacts>`.
- **annotated ASCII** — glyph in the grid + full `TileFacts` in the coordinate-keyed legend.
- **adjacency** — tiles as nodes with precomputed neighbor relations; `TileFacts` on nodes.
- **layered ASCII** (optional) — one aligned glyph grid per low-cardinality dimension;
  high-cardinality attributes still fall to a legend. Anchors the high-fidelity/high-token
  corner of the cost axis.

Holding `TileFacts` constant across encoders is what enforces information-completeness by
construction.

---

## 6. Question → Answer → EvalItem pipeline

Three levels, kept distinct (the abstract question has **no** answer; the answer is
*produced* by a solver; only then is it bundled for grading):

```
QuestionKind         a family + its solver: knows how to GENERATE questions for a board
   │  generate(board, rng, cfg)      (sample params; apply admissibility filters)
   ▼
Question             abstract: what's asked + Referents/params. NO answer. Stable id.
   │  solve(question, board)         the SOLVER: robust non-LLM ground-truth computation
   ▼
Answer               typed ground truth: Int | Direction | Terrain | Bool
   │                                    | Choice{value, options} | Coord
   │  bundle(question, answer, spec, provenance)
   ▼
EvalItem             question + expected Answer + AnswerSpec + provenance. Frozen dataset row.
   │  render(encoder) → prompt ;  model.answer(prompt) → reply
   ▼
Scorer::score(expected: &Answer, reply, ctx) → Score
                     SEPARATE module. Compares expected vs. actual (+ context). Never
                     re-runs the solver. Independent of encoder AND solver.
```

- **`Answer` is a typed enum** so scoring is **type-directed** (int compare vs.
  direction-parse vs. closed-set membership). Closed-option variants also let the scorer
  flag a reply naming something *not on the board* as an **invalid/format failure** —
  distinct from *wrong* (tracked separately; methodological hygiene).
- **`Referent`** = a handle to a board thing (`Tile(x,y)` | `City(id)` | `Unit(id)` |
  `Landmark`) that the question refers to. The `Question` stores the referent, not a
  literal string; each encoder implements `render_referent` so the *same* question reads
  as "tile (34,20)" (raw), "3 E / 2 N of your capital" (egocentric), or "tile T-1420"
  (labeled). Keeps the question identical across encodings; earns its keep at
  egocentric/adjacency encodings.

### Question corpus principle — rooted in real play, reachability-bounded (2026-07-29)
The corpus is defined by **what a regular FreeCiv player actually does**, not by an internal
menu of spatial primitives. This is a *fairness* commitment: "is this slate fair to encoder X?"
is a question we would have to adjudicate (and a skeptic distrusts); "is this the real
distribution of player activity?" is answerable against an **external, citable source** (the
FreeCiv autoplayer's decision points, CivRealm's task taxonomy, strategy guides). Grounding the
corpus *externalizes* the fairness judgment — it does not eliminate it, so the activity list must
be anchored to such a source, never to our intuition of "what players do."

Consequences, all deliberate:
- **Spatial scope is bounded by the player's REACHABLE SPACE, not the whole map.** A player asks
  "what iron can I *get to* soon," not "where is the single nearest iron among all 4,700 tiles."
  Scope is a **terrain-aware travel-cost horizon** from a *controlled entity* (a city or a unit):
  beyond it the target effectively does not exist (answer `none`). This reuses the `reach_turns`
  BFS built for the T3 threat field, and unifies `nearest-owned` (reach from your cities) with a
  future scout task (reach from a unit) under one idea — **egocentric reachability** (origin = an
  entity you control; region = its reachable neighborhood). That framing is what motivates testing
  an **egocentric + interactive** encoder (origin-relative overview + fetch outward).
- **Bounded ≠ small ≠ an automatic win for any encoder.** A fast unit over several turns on open
  terrain reaches a large fraction of the map. So we bound the task *because that is real play*,
  then **measure** which encoder wins the bounded version — we never assume it.
- **Only the deterministically-scorable slice of activity is admissible.** Many real decisions
  (declare war? which tech?) have no ground truth. The corpus lives in the intersection of "what
  players do" and "what we can score"; T3 dominance (§7) widens that intersection to cover *some*
  genuine judgment ("is this settle site a blunder?"). Name what is excluded — do not imply full
  coverage of play.
- **Two layers.** Activity-grounded questions are the **headline** (fair + decision-relevant) but
  confound several spatial skills per question. The T0/T1 spatial primitives, and the *unbounded*
  global search that activity-grounding removes, are **retained as DIAGNOSTICS** — the primitives
  isolate *which* skill an encoder helps, and unbounded `nearest` keeps the interactive failure
  mode *visible* rather than defined away. Demote them; do not delete them.

### Admissibility filters (at generation time)
In-bounds; **unambiguous**; and for later tiers, **decisive margin** — the solver computes
the top-2 candidates and *discards* the instance if they are within epsilon, keeping only
questions any reasonable model of the predicate agrees on. This neutralizes "that answer
is arguable" for valuation questions before they ever ship.

### Adding a new question type
Write one `QuestionKind` impl: (1) declare category + tier; (2) sample referents
(deterministic, seeded RNG); (3) compute the answer via a pure solver over `board` +
`geometry` (T2/T3: over the stated combat `rules`); (4) set `AnswerSpec`; (5) render the
template; (6) apply admissibility. Register it. Runner/scorer/analysis pick it up
**generically** (dispatch on `AnswerSpec`, not category) — one file touched.

### Verifying a question type — two cheap levels
1. **Unit test on a hand-built mini-board** — asserts the solver returns the known answer.
   Verifies *the ground truth itself*.
2. **The Oracle model must score 100% end-to-end** — a **self-consistency invariant of the
   harness**: generation → prompt → extraction → scoring all agree. Oracle ≠ 100% ⇒ the
   `AnswerSpec`/extractor disagrees with the `Answer` type — a bug caught **before any API
   call**. Every new question type must pass this gate.

---

## 7. Difficulty tiers

- **T0 — perception:** terrain/extra lookup, adjacency. Pure grid.
- **T1 — geometric derived:** distance, nearest, counting-in-region, path/reachability.
  Pure geometry.
- **T2 — pointwise valuation:** unit-vs-unit strength, a single city's defense. Needs the
  stated combat model, but local.
- **T3 — spatial × valuation:** "most threatened city", "weakest flank". Geometry +
  valuation + aggregation.

The **result is the degradation curve up the ladder, per encoding** (and per model tier) —
a stronger finding than any single aggregate. T2–T3 are exactly the questions a chessboard
cannot ask.

**T2/T3 discipline (so ground truth stays defensible):**
- **Give the model the same rules you score against.** A small explicit combat/strength
  rules block goes in the prompt (`Prompt.rules_block`), fed from the **same `rules`
  module** the solver uses. This keeps it a *spatial-reasoning* eval over stated semantics,
  not a "did it memorize Freeciv's constants" eval — and keeps the encoding the only
  varying thing.
- **Keep answers discrete** (a city, a direction, A/B) → exact-match survives.
- **Only emit decisive-margin instances** (above).
- **Do not reimplement Freeciv's real combat model.** A simple, principled, *stated* model
  is the goal; its simplicity is a feature because the model is told it too.

T2/T3 semantics are designed in a **separate** effort (`T2-T3-valuation-continuity.md`)
that works *within* the stable contract here (§6) — it adds `QuestionKind`s and the `rules`
module; it does not redesign `Question`/`Answer`/`AnswerSpec`/`EvalItem`/`Prompt`.

---

## 8. Encoders as board-access strategies

An encoding is generalized from `Board → String` to a **board-access strategy** — "how the
model perceives the board." Two delivery modes behind one role:

- **Static** (v1): produces a prompt string (the whole board up front). The board block is
  the cacheable prefix; questions are cheap. Encoders: **raw**, **annotated ASCII**,
  **adjacency** (v1 trio); **egocentric**, **scene-graph**, **layered ASCII** later.
- **Interactive/queryable** (later milestone): produces a **tool surface** —
  `get_tile(x,y)`, `units(faction)`, `region_summary(bbox)`, `nearest(resource, from)` —
  deterministic pure functions over the frozen `Board`. The model pulls only what it needs;
  **tokens become dynamic and question-dependent** (local questions cheap; global scans
  expensive — that dependence *is* the finding). Multi-turn, bounded by a call budget.
  Final-answer scoring stays deterministic; the *trajectory* is where tracing (§10) earns
  its keep.

The `EvalItem` is identical across modes; only delivery differs. v1 implements the static
branch; the interactive branch is a named seam, not a rewrite.

---

## 9. Answer elicitation

**Free reasoning, then a strict final-answer line** (`Answer: <x>`), parsed by the
type-directed extractor. Grounded in the format-restriction literature ("Let Me Speak
Freely?", Tam et al. 2024, and follow-ups): forcing full structured/JSON output degrades
*reasoning* ~10–30% (it tends to emit the answer before the reasoning, collapsing CoT).
The consensus fix is "separate thinking from formatting." A minimal *trailing* JSON
(reason first, answer last) is an acceptable later variant; full JSON-mode is not,
especially for local models.

**Held identical across all encodings** so the format tax is a constant, not a confound in
an *encoding* comparison. Invalid/unparseable replies are tracked as a distinct outcome
from wrong.

`Prompt` shape: `{ board_block, rules_block: Option<String>, question_block }`. `rules_block`
is `None` for T0/T1; filled from the `rules` module at T2.

---

## 10. Models, results, cost

- **`Model` trait** (sync for v1): `answer(&self, prompt) -> Reply { text, usage }`.
  - **`OracleModel`** — returns computed ground truth; offline, deterministic, $0. Proves
    the pipeline and powers the 100% invariant.
  - **`OpenAiCompatible`** (behind a `remote` feature flag) — one impl for **local Qwen**
    (Ollama/vLLM/LM Studio) over http **and** cloud (**OpenRouter**, OpenAI) over https;
    base-URL in config, swapping model = flag not code. Built on **`ureq` + rustls** (blocking,
    matching the sync `Model` trait; pure-Rust TLS so no OpenSSL on Windows), confined to
    `remote.rs`; the offline core stays dependency-free. Supports `--provider` pinning and
    `--cache` (a `cache_control` breakpoint on the board prefix; prompts split into
    `Prompt{cacheable_prefix, tail}`). The runner has a cache-aware **concurrent** path
    (`--concurrency N`, cloud-only): per encoding it warms the board-prefix cache with one serial
    call, then fans the rest across a bounded thread pool — ~5× wall-clock with cache hits
    preserved. (An earlier plan named the `genai` crate; the hand-rolled ureq client proved
    simpler and kept the dependency surface minimal.)
- **Model selection is config-driven** (`models.toml`): named profiles
  `{ name, kind, base_url, model_id, env_key, params, provider_pin }`. Runner iterates the
  matrix **models × encodings × question-set**; every result row is tagged with the
  profile + temperature + seed.
- **Results = JSONL** (source of truth), optionally **Parquet** later. **Cost fields are
  first-class from v1**: `prompt_tokens, completion_tokens, cached_tokens, calls, latency_ms`
  — even for single-shot Oracle runs (Oracle reports zeros/synthetic but the columns
  exist). Analysis via **DuckDB** SQL over the result files
  (`avg(correct)`, `avg(correct)/avg(tokens)` per condition, Wilson/bootstrap CIs).

### Deferred backlog (explicitly NOT in the first build)
- **OpenTelemetry** (`tracing` + OTLP) emission and **LangFuse** (or Arize Phoenix) trace
  UI for browsing individual prompt→response→score cases and, later, multi-turn query
  trajectories. Blessed *to evaluate live*, not yet to marry. The interactive encoding is
  what makes it genuinely valuable.
- **Layered-ASCII**, **egocentric**, **scene-graph** encoders.
- **Interactive/queryable** board-access mode + bounded tool-loop runner.
- **Statistical treatment** (Wilson intervals / bootstrap) in the analysis layer. *(Still
  pending — the first cloud runs use raw counts; small per-category n needs CIs before any cell
  is treated as real.)*
- Additional boards from the CivRealm corpus (difficulty gradient: sparse early → dense
  turn-677). v1 uses the single T50 board. *(T677 is now specifically wanted as the T2/T3
  headline-measurement board — see below.)*

### T2/T3 valuation — design DELIVERED (separate track, 2026-07-27)
A separate agent completed the valuation design in **`T2-T3-design.md`** (working within the §6–§9
contract). Key user-approved decisions now handed to the core track to implement:
- **`Answer::ChoiceSet { acceptable, options }`** for T3 dominance scoring — `acceptable` is
  precomputed at generation so the scorer stays a pure set-membership comparator (never sees the
  board). Add a *negative* Oracle self-test (a deliberately-dominated pick must score `wrong`).
- **`City.improvements` decode** in `civ-core` (parser extension) — the per-city `improvements`
  bitstring vs `improvement_vector` (City Walls / Great Wall / Coastal Defense / Palace).
- **Rich-rendering seam** — encoders surface walls/veteran/HP **only for T2/T3**, leaving T0/T1
  board blocks byte-identical (so in-flight T0/T1 runs are undisturbed).
- Combat model = the `classic` ruleset's own stat tables + a simple stated scalar; needs the
  **T677** board for the dense headline run. Reframing: tiers are a *composition-depth* ladder
  (they stress the encoding's information-capacity axis), not a new cognition type.

### Known limitation — cropping vs. experimental power
Small local-model context windows (e.g. LM Studio at 16k) force us to **crop** the board so
denser encodings (`raw`, `adjacency`) fit. Cropping is a fast-iteration convenience with a
real cost: **a small board is easy to read in *any* encoding, so it risks a ceiling effect
that compresses out the between-encoding signal** — precisely the signal the encoding
comparison exists to measure. So a cropped run is a *smoke test of the hypothesis*, not the
publishable measurement. Mitigations for the real run: use the *largest* crop that still fits
the target encodings (crop size is a difficulty knob); aggregate *multiple/sliding crops* for
full-board coverage at small prompt size; push question parameters harder (larger radii/paths);
and do the honest **full-board run on a big-context (cloud) model**. The model × thinking
comparison, run on the full board, retains its power; only the encoding comparison is affected.

**Update (2026-07-27): resolved.** Cloud Flash models (~1M ctx) fit the full board in every
encoding incl. adjacency — no crop — so the encoding comparison was run at full power and the
ceiling broke (encodings separated; see `analysis/findings-deepseek-fullboard.md`). Multiple/
sliding crops were considered and **rejected**: each crop is self-contained and loses the
long-range questions (far-tile distance, out-of-window nearest) we care about. Cropping remains
only a convenience for the small-context *local* path (which still can't fit adjacency).

---

## 11. Repository layout

```
civ-core/     Board, Tile, Unit, City, Player; geometry; Freeciv .sav parser. No deps.
civ-eval/     TileFacts; encoders (board-access); Question/QuestionKind/Answer/AnswerSpec;
              T0/T1 QuestionKinds + solvers; EvalItem; prompt assembly; Scorer; Model +
              Oracle (+ remote behind a feature); runner; result schema (JSONL).
civ-cli/      clap CLI: run, verify-oracle, gen-questions, dump-board.
data/saves/   Freeciv boards (myagent_T50.sav to start).
justfile      task runner (test, lint, eval, check → verify-oracle).
```

Build-order discipline: get one end-to-end path working (crappy encoder + real scorer +
Oracle) **before** polishing encoders. If you can't score, you can't tell whether an
encoding helped.

---

## 12. Build order (milestones)

1. **v1 (this build):** civ-core (parser/board/geometry + tests) → civ-eval (TileFacts,
   raw/ASCII/adjacency encoders, T0 [+T1] questions/solvers, scorer, Oracle, runner,
   JSONL) → civ-cli → **tests green incl. Oracle-100% invariant** → offline oracle sweep
   producing `results.jsonl` with cost fields. **No OTel/LangFuse.**
2. Wire `remote` model (local Qwen first) behind the feature; first real numbers.
3. Add encodings (egocentric, scene-graph, layered ASCII); DuckDB analysis + CIs.
4. Interactive/queryable board-access + tool-loop runner; then OTel→LangFuse tracing.
5. T2/T3 valuation (separate design) once T0/T1 is solid; scale to more boards.

---

## 13. Related work & positioning (see `RELATED-WORK.md`)

A literature scan (2026-07) found that **several of our headline framings are already established** —
cite them as *foundation*, not discovery: that *encoding choice materially changes LLM accuracy and
is model/task-dependent* (**Talk like a Graph**, Fatemi et al., ICLR 2024 — the direct methodological
twin, "freeze the object, vary the encoding, score vs computed truth"); that *representation quality,
not reasoning, is the bottleneck* (**Text2Space**; **Lost in Aggregation**); that *text/coords beat
images* (**SpatialEval**, NeurIPS 2024); and that *counting/aggregation is an architectural wall*
("Why LLMs Struggle to Count Letters"; **Lost in Aggregation**). CivRealm-**SAGA** (2026) independently
argues the bottleneck is "information architecture" but measures only through game score.

**Narrowed, defensible novelty (the intersection):** a **real frozen game board** (not toy grids/
graphs), **breadth of structurally-distinct encodings + a queryable tool surface** on one **accuracy-
vs-token frontier**, the **model-dependent winning encoding as a first-class result**, and
**perception isolated from strategy** on a real strategy game. Position as *"Talk-like-a-Graph
extended to real 2D spatial game state, with a perception isolate and a cost frontier."*

**Prior-art-motivated roadmap additions** (details + citations in `RELATED-WORK.md`):
- **Difficulty/scale-crossover** *(prioritized)* — does raw's advantage over adjacency invert as the
  board gets harder/bigger? (quadruple-motivated).
- **Region-count accuracy vs. true-count reanalysis** *(prioritized; free from existing JSONL)* —
  predicts a sharp cliff; first mechanistic view of the aggregation wall.
- **Factored labeling axis** — from Talk-like-a-Graph's **node-labeling × edge-phrasing** factoring:
  isolate labeling (integer coordinates vs named/semantic tile IDs) as an independent knob.
- **Tokenization/delimiter ASCII ablation** *(cheap follow-up)* — likely explains local-Qwen→ascii.
- **Scene-graph: bucketed-vs-exact two-arm** (kept information-complete, unlike SAGA's lossy graph).
