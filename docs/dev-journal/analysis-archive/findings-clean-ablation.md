# Findings — clean-harness ablation: the calculator is the accuracy lever, enumeration is a pure EFFICIENCY lever

**Date:** 2026-08-01 · the first dispersed-kind ablation run on the **bug-fixed harness** (after the B1 unit-stacking
contract fix + the B2/B3/B4/B6 harness batch). Board: T677 `--fog 2` (98% explored, dense late-game known map).
Kinds: nearest-owned, reachable-nearest, region-count. DeepSeek V4 Flash think-off, per-kind 8, **3 different-seed
reps, n=72/arm**. Arms use dead-op-free "trimmed" surfaces (`raw-calc`, `raw-calc-enum`, branch `maxops-enum`).
Data: `results-clean-p2-seed{1,2,3}.jsonl` (maxops-enum worktree). Supersedes the enumeration-accuracy claims in
`findings-scale-ceiling.md` and `findings-enum-residual.md`, both of which were measured on the BUGGY harness.

## The three arms
- **`raw`** (no-ops) — full board, no calculator. Perception only.
- **`raw-calc`** — full board + `{distance, travel_turns, count_terrain, count_resource, reach_turns}`. The minimal
  correct calculator for these kinds (no dead combat/valuation ops).
- **`raw-calc-enum`** — full board + `{distance, travel_turns, reach_turns, list_tiles, list_owned_cities,
  list_owned_units}` (counts retired). Calculator + positional enumeration.

## Headline table (pooled 3 seeds, n=72/arm)

| arm | acc | 95% Wilson CI | gen_tok | latency |
|---|---|---|---|---|
| raw (no-ops) | 0.542 | [0.427, 0.652] | 741 | 7.2 s |
| raw-calc | 0.833 | [0.731, 0.902] | 6,209 | 52.7 s |
| raw-calc-enum | 0.845 | [0.743, 0.911] | 2,617 | 24.1 s |

Per-kind (correct / n):

| kind | raw | raw-calc | raw-calc-enum |
|---|---|---|---|
| nearest-owned | 14/24 | 15/24 | 12/23 |
| reachable-nearest | 14/24 | 22/24 | 24/24 |
| region-count | 11/24 | 23/24 | 24/24 |

(raw-calc-enum n=71: one `error`-status row — a failed completion caught by B4 and excluded from the denominator.)

## Finding 1 — the calculator (arithmetic) is the accuracy lever; enumeration adds ~none

no-ops → calc is **+0.29** (0.542 → 0.833). calc → calc-enum is **+0.012** (0.833 → 0.845) — deep in the noise,
CIs almost fully overlapping. On the fixed harness, **enumeration buys no accuracy over the plain calculator.**
`reach_turns` drives reachable-nearest (14 → 22/24 — the unit-move-rate op `raw-ops` never had), and count/list
solves region-count (11 → 23/24). The arithmetic deficit is the real one; free measurement closes it.

## Finding 2 — enumeration's value is PURE EFFICIENCY (this is the whole story now)

At equal accuracy, `raw-calc-enum` runs at **24.1 s / 2,617 gen-tok** vs `raw-calc` **52.7 s / 6,209 gen-tok** —
~2× faster, ~2.4× fewer generated tokens. Enumeration lets the model locate entities in one call instead of
narrating/eyeballing, so it converges in fewer turns. That win is real, method-independent, and it is the ONLY
thing enumeration buys on the clean harness.

## Finding 3 — the prior enumeration ACCURACY lift was substantially a render-bug artifact

On the buggy harness (`findings-enum-residual.md`), `raw-maxops-enum` led its calculator control by **+0.055**
(and the baseline by +0.097). On the clean harness that lift is **+0.012 (gone)**. Mechanism: the old render
(pre-B1) showed only **1 of N stacked units**, so the calculator arm literally could not eyeball its own
cities/units on a dense board — enumeration (`list_owned_cities`/`list_tiles`) was the only way to *locate* them,
which looked like an accuracy lift. Once B1 makes the render show **all** units with ids, the calculator arm
locates directly from the board block and enumeration's edge evaporates. **The "enumeration breaks the locating
wall" claim was largely enumeration compensating for the broken render.** Pausing to fix the harness before
re-running is what surfaced this.

## Finding 4 — the old ~0.75 "locating ceiling" lifted to ~0.83–0.85

The old ops arms capped at ~0.75; the clean calc arms hit 0.833/0.845. A real chunk of the old ceiling was the
harness bugs — the render dropping stacked units (perception), the operators resolving the wrong stacked unit
(B1), and the extractor fabricating `got` (B2) — not a fundamental locating wall. The "second bottleneck =
locating" framing in `findings-scale-ceiling.md` overstated a fundamental limit; much of it was fixable harness
error.

## Finding 5 — nearest-owned is the residual hard kind, and enumeration makes it WORSE (over-engagement)

nearest-owned is the one kind nothing cracks — raw 14, calc 15, enum **12/23 (below even no-ops)**. The failure
mode *shifts* across arms, which is the whole mechanism:

| arm | nearest-owned | wrong | invalid/error | avg tool calls |
|---|---|---|---|---|
| raw (no-ops) | 14/24 | 10 | 0 | 0 |
| raw-calc | 15/24 | 5 | 4 | 19.5 |
| raw-calc-enum | 12/23 | 4 | **8** | **36.9 (max 72)** |

nearest-owned needs the **"within-2-of-my-cities" exclusion** (drop resource tiles within Chebyshev-2 of any of
your cities, then min travel-turns from the nearest city). Enumeration hands the model every city and every
resource tile, which tempts it to **brute-force the pairwise exclusion by hand**. Confirmed in the capped Oasis
trace: **~178 `distance` calls** vs 8 `travel_turns` — the model is checking every (resource × city) pair,
explodes past the turn/token budget, is force-answered, and emits an **empty** result → `invalid`. no-ops does
*better* precisely because it can't take that path: it one-shot-guesses a complete (if off-by-one) answer. So
**more tools → more brute-forcing → more cap-outs**; the failure mode moves from "wrong" (no-ops) to "never
finishes" (enum).

The fix is NOT more raw positions — it is a **positional-FILTER operator** (`list_tiles(resource=X,
exclude_within=2_of_my_cities)`, or a `nearest_owned_city` index) that does the exclusion *inside the tool* and
returns only eligible tiles. That collapses the 178-call flail into one call. This is the same "positional filter"
`findings-enum-residual.md` recommended, now with a clean-harness demonstration of exactly why it's needed.

## Revised thesis (what to carry forward)
- **Accuracy:** the calculator (free arithmetic + `reach_turns`) is the lever; it lifts dispersed-kind accuracy
  from ~0.54 to ~0.83 on a large dense board. Enumeration adds no accuracy.
- **Efficiency:** enumeration is a ~2× cost/latency win at equal accuracy — its sole, but real, contribution.
- **The remaining hard kind is nearest-owned**, and its bottleneck is exclusion+argmin *logic*, which raw
  positions make worse (over-engagement → cap-out). It needs a predicate-filter operator, not more positions.
- **The prior "enumeration breaks the locating ceiling" story was largely a render-bug artifact** and is retired.

## Caveats
- n=72/arm, 3 **different-seed** reps (pooling, not repeated-item majority vote); single model (DeepSeek think-off);
  the calc-vs-enum CIs overlap heavily (that overlap IS the finding — no accuracy difference).
- Not tile-for-tile comparable to the buggy-harness ablation (different op sets + fixed harness) — read directions,
  not deltas.
- One `error` row (enum) excluded from its denominator per B4.
- Board-specific: dispersed kinds on one large late-game fogged board (T677/p2). The small-board calculator result
  (`findings-calc.md`, 6.8% explored, raw-ops 0.986) has little stacking and is not implicated by the render bug.
- The B1 render-all-units fix enlarged the T677 board block, so these runs are ~3–7× slower per seed than the old
  ablation — the expected latency cost of complete perception.
