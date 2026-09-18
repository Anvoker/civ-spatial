# Finding — hierarchical region-summary encoder: null at T50 (scale is the open variable)

**Date:** 2026-07-27 · **Board:** `myagent_T50.sav` (78×52), full board, DeepSeek V4 Flash,
seed 1, per-kind 12, difficulty hard. Encoder: `hierarchical` (fixed-partition region histograms
as the surface + complete tile leaves as the backing; deterministic, no LLM; passes the round-trip
reconstruction gate + Oracle-100%).

## Result — pre-aggregation did NOT dent the aggregation wall here
Matched questions, current generator (region-count excludes the default terrain):

| | region-count | terrain | overall | mean prompt tok |
|---|---|---|---|---|
| **think-OFF** hierarchical | 10/12 | 8/12 | 0.907 | ~34.0k |
| **think-OFF** raw | 9/12 | 10/12 | 0.898 | ~26.7k |
| **think-ON** hierarchical | 12/12 | 11/12 | (0.944 incl. T2) | ~34.0k |
| **think-ON** raw (baseline) | 11/12 | 12/12 | 0.988 | ~26.6k |

**Verdict: hierarchical is *dominated* on T50** — accuracy tied-to-slightly-worse than `raw`
(region-count edges are ±1 question = noise at n=12; hierarchical is actually *worse* on terrain,
plausibly because the region summary distracts from locating a specific tile's leaf) at **~1.27×
the tokens**. Same shape as `adjacency`: extra structure, no accuracy payoff, more cost.

## Why the expected headroom vanished
The pre-registered hypothesis assumed raw region-count ≈ 7/12 (our old think-off number). But that
7/12 was inflated by the **Ocean count-by-exclusion** questions, which the **default-terrain fix
already removed** — lifting raw to 9/12 on its own. For *listed* terrains at radius 3–5 on a small
board, the aggregation window is small enough that raw's explicit tile list suffices; there's little
left for pre-chunking to fix.

## Caveats + the real test
- **n = 12/cell — accuracy differences here are within noise.** The only clean signal is the +27%
  token cost.
- Hierarchical's thesis was always *"chunking helps aggregation **at scale**."* T50 is small, so the
  scan raw must do is cheap. **This null result is exactly what the difficulty/scale-crossover
  experiment is for** — hierarchical (and adjacency) may only earn their tokens on a large, dense
  board (e.g. T677: 84×56, ~682 units, 118 cities) where raw must scan hundreds of tiles.

**Conclusion:** do not invest further in hierarchical until the scale-crossover can test it where it
might pay off. Getting the **T677** board is the unblock. Result recorded; encoder kept (opt-in via
`--encoding hierarchical`).
