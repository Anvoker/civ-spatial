# CivSpatial — Continuity / Handoff, 2026-07-28

Where we left off. Read this first, then `RESUME.md` (evergreen state + commands), `DESIGN.md`,
`RELATED-WORK.md` (prior art), the design docs (`interactive-board-access-design.md`,
`T1g-T3-questions-draft.md`, `T2-T3-design.md`), and the `analysis/findings-*.md` results (newest:
**`findings-interactive.md`**). Supersedes `CONTINUITY-2026-07-27.md`.

> **The headline of this session's second half: the INTERACTIVE (queryable) board-access mode is
> built and it WINS.** See the dedicated section below and `findings-interactive.md`.

## Repo state
- **Branch `master`.** Latest commit **`22272f6`** (the `best-site` siting question). Key commits this
  session: `54b46f4` minimal harness, `c8d972b` hierarchical confirmation, `5046f76` interactive design,
  `f8e37f9` interactive offline parts, `e35fce8` interactive tool-loop, `fad7cf8` interactive findings,
  `22272f6` siting question. `just check` green (tests + clippy `-D warnings` on default **and**
  `--features remote` + Oracle-100%).
- **Boards:** `myagent_T50.sav` (78×52 fixture) and `testcontroller_T677.sav` (84×56, dense headline).
- **Board access = 5 static encoders + 1 interactive surface:** raw, ascii, adjacency, egocentric,
  hierarchical (static) + **`interactive`** (a multi-turn tool loop — see below). `run` defaults to
  raw+ascii+hierarchical; adjacency/egocentric opt-in; `--encoding interactive` needs `--features remote`.
- **Question kinds:** T0/T1 (terrain, adjacency, direction, distance, nearest, region-count, reachability),
  T2 (unit-strength, city-defense), **+ `best-site` (T1 siting)** — "which candidate tile has the most
  {terrain} in its work radius" (best-of-K `Choice`). `--kinds` selects any subset.
- **Secrets:** OpenRouter key in the gitignored **`.env`** (`OPENROUTER_API_KEY=…`); `scripts/eval-min.sh`
  auto-sources it. `.env` confirmed untracked.

## What this session did (2026-07-28)
1. **Ran the T677 scale-crossover** (the pivotal experiment paused yesterday). DeepSeek think-off, per-kind 6,
   n=54/enc, all 9 kinds. Findings in `analysis/findings-t677-crossover.md`:
   - **No inversion.** raw and adjacency *tie* at 0.778, but adjacency costs **4.3× the tokens** (242k vs
     56k) for identical accuracy → raw still **strictly dominates** adjacency on the cost frontier. The
     relational encoding does not pay off even at scale.
   - **Hierarchical looked like a crossover on seed 1** (0.852 vs raw 0.778) **but the confirmation
     killed the magnitude** (see step 4). Corrected verdict: it *de-dominates* at scale (T50 dominated →
     T677 accuracy-tied with raw) but does **not** beat raw, and at 1.4× raw's tokens it stays off the
     cost frontier. raw remains the frontier encoder.
2. **Retired adjacency from default runs** (per user). Code, registration, and the `verify-oracle` /
   oracle-test coverage all **retained** — it just no longer runs unless you pass `--encoding adjacency`.
   Rationale: dominated at both scales; it was ~50% of a run's spend for zero accuracy gain.
3. **Built the minimal harness** (the 80/20 daily loop):
   - Added a **`--kinds a,b,c`** category filter to `run`/`verify-oracle`/`gen-questions` (validated
     against the 9 category names; a typo errors instead of silently emptying the set).
   - **`scripts/eval-min.sh`**: raw+ascii+hierarchical × {terrain, region-count, nearest} × think-off on
     DeepSeek, T677, cache on, concurrency 8, auto-sourcing `.env`. **The saving is fewer QUESTIONS, not
     fewer encoders** (per user): it keeps the whole working set of interest and you add new encoders here.
     The pruned parts are the saturated/duplicate *kinds* (direction/distance/reachability/adjacent-terrain)
     — the always-green floor-check, off the daily loop.
4. **Ran a hierarchical confirmation** — seed 2, per-kind 12, raw/ascii/hierarchical, think-off, n=108/enc
   (`results-t677-hier-confirm.jsonl`). **It did NOT replicate the seed-1 edge:** raw **0.870** =
   hierarchical **0.870**, ascii 0.815. The seed-1 "hierarchical leads" cell was sampling noise (raw's
   unlucky terrain dip). Pooled +2.4 pts, inside the noise. → **Hierarchical NOT promoted**; raw stays the
   frontier. Full walk-back in `findings-t677-crossover.md` (CONFIRMATION section).

## THE BIG BUILD — interactive (queryable) board access, and it WINS

The "when does hierarchical win?" thread led to a realization: the static `hierarchical` encoder
front-loads *every* leaf, so it is structurally always more tokens than raw and can never reach
hierarchy's real payoff — **not paying for detail you don't fetch**. That payoff needs a **tool loop**.
So we designed and built it (`interactive-board-access-design.md`).

**What's built (all committed, `just check` green, both features):**
- `describe.rs` — deterministic, LLM-free **qualitative region descriptions** ("predominantly Hills,
  sea along the eastern edge, ~a quarter of the region"). Compute exactly, describe coarsely; no exact
  counts/coords leak, so the coarse level stays honestly lossy.
- `QueryableSurface` + `Interactive` (in `encoders.rs`) — an **overview** (per-quadrant descriptions) +
  three **observation-only** verbs: `region_summary(sector)` (qualitative), `scan(x0,y0,x1,y1)` (exact
  leaves, capped 12×12), `get_tile(x,y)`. No answer-shaped verbs (no count/nearest/best-site). Parity
  gate = reconstruct-the-board-via-tools (in `tests/reconstruction.rs`).
- `json.rs` — a dependency-free recursive **JSON value parser** (reads nested `tool_calls`).
- `model.rs` — `ToolDef`/`ToolCall`/`Role`/`Message`/`ChatReply` + the **`ChatModel`** trait (multi-turn).
- `remote.rs` — `ChatModel` for `OpenAiCompatible`: builds the `tools`+`messages` request (cache_control
  on the stable system prefix), parses the response. Tested offline (body shape + response parse).
- `runner.rs` — **`run_interactive`**: strategy-free mechanics system prompt, MAX_TURNS backstop (16) +
  token budget (200k) + force-answer, trajectory token **sums**, cache-aware concurrency. `ResultRow`
  gained `n_turns`/`n_tool_calls`. Offline end-to-end test drives a scripted mock `ChatModel`.
- CLI: `--encoding interactive` (routes to run_interactive; needs `remote`) + `--max-turns`/`--token-budget`.

**The result (first live runs, DeepSeek think-off, T677 — `findings-interactive.md`):**
- **Tool calling works** — DeepSeek drove 214 tool calls in the 6-question smoke; caching across turns
  works. (Web-checked: DeepSeek V4 Flash + LM Studio/Qwen both support OpenAI-format tools; we're non-
  streaming, avoiding the known Hermes streaming bugs.)
- **Stage B head-to-head (n=12/mode, terrain + region-count): interactive won BOTH axes.** 12/12 accuracy
  vs 9/12 (raw and hierarchical); **~8× fewer tokens** than raw (6.9k vs 55.9k); **~4.5× cheaper per
  correct answer**. And it **cracked region-count** (6/6 vs raw 5/6, hierarchical 3/6) — the counting wall
  — by tallying fetched leaves over a focused context instead of eyeballing a 56k-token board dump.
- **Failure mode: `nearest`** (unbounded global search) blows up — 17 turns (hit the cap), ~90–99 tool
  calls, 200–280k tokens. **Interactive wins on LOCAL/BOUNDED questions, loses on GLOBAL SEARCH.**
- **Whack-a-mole dissolved:** with a fixed, domain-neutral verb set, the *model* chooses what to fetch;
  we measure navigation efficiency, not our summary-tuning.

**Caveats:** small n (12/mode); the two Stage-B kinds favor interactive; single board/model/think-off;
wall-clock is interactive's real cost (many sequential round-trips), not dollars.

## Why these questions/encoders (the pruning rationale, for the next session)
- **Discriminating kinds:** `terrain` (ascii bleeds), `region-count` (the wall — survives reasoning),
  `nearest` (the cross-model flipper: Qwen raw 1/6 vs ascii 6/6). These three carry the minimal harness.
- **Saturated / redundant kinds:** `direction`, `distance`, `reachability` (all ~1.0 across cloud models,
  think on/off) and `adjacency`-question (a near-duplicate of `terrain` — "terrain at a 1-step offset").
  Not worthless — they're the floor-check (like unit tests that always pass) — just off the daily loop.
- **Encoders:** frontier is **raw** (dominates) + **ascii** (the model-flipper). Adjacency is dominated;
  **hierarchical de-dominates at scale but ties raw (doesn't beat it) and costs 1.4× — still off the
  frontier** (see above). egocentric untested at scale.

## ⏸️ WHERE WE LEFT OFF (end of 2026-07-28) — START HERE TOMORROW
The siting comparison **is DONE and interactive won again** (`findings-interactive.md`, Stage C; n=24/mode,
best-site + terrain + region-count, DeepSeek think-off, T677):
- **interactive 24/24** vs raw 16/24, hierarchical 18/24; ~3× fewer tokens, ~2.5× cheaper/correct.
- On `best-site` specifically: **raw 4/8 (chance), hierarchical 6/8, interactive 8/8.** Front-loaded
  encodings fail at spatial *decisions* needing local aggregation; the query loop nails them.
- New caveat: a **cost fat-tail** — most best-site trajectories are cheap (2–5 turns) but one blew up to
  196 tool calls / 197k tokens (still correct).

**The consolidated result (three question types now):** querying beats front-loading on local, bounded,
AND decision-shaped questions — cheaper *and* more accurate — losing only on unbounded global search
(`nearest`). Nothing is running; `.env` holds the key.

**Pick up tomorrow with one of (all cheap, all need `cargo build -p civ-cli --features remote` first):**
1. **Larger-n siting confirmation** — per-kind 16 (maybe seed 2) on `best-site,terrain,region-count` to firm
   up the 8/8-vs-4/8 gap + the cost fat-tail. ~$0.30. (Same command as `RESUME.md`, bump `--per-kind`.)
2. **Think-on interactive pass** — does reasoning lift the static encoders toward interactive? Would sharpen
   the claim to "board *access* matters most when reasoning is off/cheap." Add `--think on`.
3. **`nearest` / fat-tail mitigation** — exclude global-search kinds from interactive, or cap/penalize the
   over-fetch tail (a bounded search verb is possible but must not be answer-shaped).

### (superseded) the earlier open thread — the siting comparison itself
The immediate experiment above was "run raw + hierarchical + interactive on `best-site` ± terrain,
region-count." It ran; results are Stage C. Kept here as a breadcrumb.

Note on `best-site`: it's a **best-of-K** `Choice`, not an open-board search. An open-board argmax→`Coord`
form was prototyped and **dropped** — unique global maxima are rare (supply ~8) and the all-tiles scan was
slow (~20–28s/gen). Best-of-K is fast (0.08s) + high-supply but tests "evaluate K" rather than "survey the
whole board to find the spot." A real *survey* test would need a real-valued desirability score (rarely
ties) to revive the open form — deferred.

### Background: the "when does hierarchical win?" thread (mostly answered by interactive)
The static-hierarchical magnitude question is settled: it **ties** raw (0.870=0.870), doesn't beat it, costs
1.4×. The reason it doesn't win is the **question mix** (our questions are point/local — flat-list territory).
Hierarchy's payoff needed *querying*, not a better static summary — which is exactly what the interactive
mode delivered. The old idea of a "global aggregation tier" (quadrant counts, territory control) is still a
valid static-hierarchy test but is now lower priority than the interactive direction.

Secondary thread — **reasoning**:
- **think-on pass on raw/ascii/hierarchical at T677.** Does reasoning collapse the raw–ascii gap (and move
  hierarchical) the way it did on T50 DeepSeek (F2, `findings-deepseek-fullboard.md`)? Quick discriminating-
  set version: `THINK=on ENCODINGS="raw ascii hierarchical" bash scripts/eval-min.sh`. For the full-battery
  aggregate (all 9 kinds, matching the 0.870 numbers), run a direct `civ run` with those three `--encoding`s,
  `--think on`, and **no** `--kinds`. ~6.4× slower, cheap in $.
- If think-on collapses raw>ascii at scale too, the "encoding matters most when reasoning is off" story
  holds on the dense board and the project's core claim is board-robust.

## Backlog
- **Interactive follow-ups:** larger-n confirmation of the Stage-B win; a **think-on** interactive pass;
  add the deferred `list_cities`/`list_units` verbs; consider unit *type* (not just presence) in
  `describe.rs`; the open-board `Coord` siting form with a real-valued score.
- **Balanced count sweep** — turn the count-curve into a measurement (balanced true counts 0–20+, fixed
  model, raw-vs-ascii). Cheap, high-value.
- **T3 valuation** — `Answer::ChoiceSet` dominance tier (best-of-K siting is the natural home; the
  single-objective `best-site` is built, the multi-axis withheld-weighting T3 is not). Design approved in
  `T2-T3-design.md`; needs the `ChoiceSet` variant + a negative Oracle test. See `T1g-T3-questions-draft.md`.
- **Scene-graph / landmark-graph** static encoders (SAGA-style); **global aggregation tier** (the static-
  hierarchy test); **factored labeling axis**; **tokenization/delimiter ASCII ablation**; **harder T1 tier**.

## Ops notes / gotchas (2026-07-28)
- **⚠️ Rebuild remote before every cloud run — bit us THREE times this session.** `just check` / any
  `cargo run|build -p civ-cli` rebuilds `target/debug/civ.exe` **without** `--features remote`; a subsequent
  cloud `run` then dies with "needs a live provider" (cost-free — fails at dispatch). Always
  `cargo build -p civ-cli --features remote` immediately before launching a cloud run.
- **Interactive is slow in wall-clock, cheap in $.** Each question is N sequential round-trips (region-count
  used 20–53 tool calls; `nearest` hit the 16-turn cap). Use `--concurrency` to fan questions out. The
  `--max-turns`/`--token-budget` knobs are safety backstops, not the economic lever (per-fetch cost is).
- **Interactive gotcha to watch on the next run:** `nearest` (unbounded search) is pathological — keep it
  out of interactive comparisons, or expect 200k+ tokens/question and force-answer invalids.
- **Cost:** DeepSeek cache-read ~$0.028/M. Crossover/confirmation ~$0.3–0.6 each; interactive Stage A ~$0.03,
  Stage B ~$0.06. Budget comfortable.
- **`.env` for the key** — never on the command line; the scripts source it. Confirmed gitignored/untracked.
