# Findings — the real-play corpus: interactive's advantage is modest and kind-dependent

**Date:** 2026-07-29 · **Boards:** T677 (dense, 84×56) + **T50** (sparse, 78×52) · DeepSeek V4 Flash,
think-off, difficulty hard, seed 1, per-kind 12 · encoders raw / hierarchical / interactive · kinds =
`settle-site` (T3), `best-site`, `region-count`, `terrain`, `nearest-owned` (travel-cost), `reachable-nearest`
(scout). Results: `results-t3-final-t677.jsonl`, `results-t3-final-t50.jsonl`, `results-failuremode.jsonl`.

**Why this run exists.** It replaces the curated 3-kind siting slate (where interactive went 48/48,
`findings-interactive.md` Stage D) with a broader corpus **rooted in real player activities + reachability-
bounded scope** (`DESIGN.md` §6) — deliberately including the *dispersed-integration* and *travel* tasks
the curated slate omitted. The goal was to answer the fairness critique honestly: is "interactive beats the
pants off everything" real, or an artifact of testing the questions interactive is built to win?

## Headline: it was largely an artifact

On the fair corpus, interactive's edge is **modest and kind-dependent**, and **nearly vanishes on the sparse
board**:

| board | interactive | raw | hierarchical |
|---|---|---|---|
| **T677** (dense) | **0.736** | 0.625 | 0.569 |
| **T50** (sparse) | **0.736** | 0.708 | 0.681 |

On T50 the three encoders are within each other's 95% CIs. "Beats the pants off everything" held only on the
curated local-aggregation slate; on a fair mix it is a ~11-pt edge on the dense board and a coin-flip on the
sparse one.

## Per-kind, both boards (the real signal)

| kind | | interactive | raw | hierarchical |
|---|---|---|---|---|
| **terrain** | T677 / T50 | **12/12 · 12/12** | 9 · 10 | 11 · 9 |
| **best-site** | T677 / T50 | **12/12 · 12/12** | 7 · 8 | 9 · 7 |
| **region-count** | T677 / T50 | **11/12 · 12/12** | 9 · 9 | 6 · 6 |
| **settle-site (T3)** | T677 / T50 | 9 · 7 | 6 · 8 | 5 · **10** |
| **nearest-owned** | T677 / T50 | 5 · 5 | **9 · 6** | 6 · 9 |
| **reachable-nearest** | T677 / T50 | 5 · 5 | **5 · 10** | 4 · 8 |

Three robust patterns:

1. **Interactive DOMINATES local aggregation** — `terrain`, `best-site`, `region-count`: 11–12/12 on *both*
   boards. This is the real, replicated win: tally/decide over a focused window beats eyeballing a big dump.
2. **Interactive LOSES dispersed-entity integration + travel** — `nearest-owned`, `reachable-nearest`: 5/12
   on *both* boards, with raw/hier ahead. **The bounded reframe did NOT rescue it.** Bounding the search
   space doesn't help when the task requires integrating *scattered* cities/units + resources + a travel-cost
   computation — the fetch loop does that poorly think-off (and racks up force-answer invalids: 7 on T677).
   This is the honest failure mode the curated slate hid, and it is *not* just "unbounded search."
3. **T3 settle-site is board-dependent** — interactive wins on dense T677 (9/12; the threat field is a live
   local-aggregation signal there) but *loses* on sparse T50 (7/12; **hierarchical 10/12**), where threat is
   thin and siting is terrain-driven — the static encoders' strength.

## Why the edge collapses on T50

On the small/sparse board the static encoders are **already cheap** (raw ~28k tokens, hier ~35k vs T677's
57k / 80k) and the model can see the whole board at once. Interactive's core advantage — *not paying for the
whole board* — has little to buy when the board is cheap, so its over-fetching and many round-trips (89 s mean
latency vs raw's 12 s) mostly add error surface. **Interactive's advantage scales with board size/density.**

## Failure-mode run confirms it (`results-failuremode.jsonl`, T677, raw vs interactive, n=6/kind)

| | interactive | raw |
|---|---|---|
| `nearest` (unbounded) | 3/6 | 6/6 |
| `nearest-owned` (bounded) | 2/6 | 5/6 |
| overall | 0.417, **217k tok, 109 s** | 0.917, 56k tok, 10 s |

Both the unbounded *and* the bounded resource-search favor the static full-board view — the bounded version
is not a win for interactive, it's just less catastrophic than the unbounded blow-up.

## What it means for the thesis

The fair corpus resolves the fairness critique honestly: **interactive is not a universal winner.** Its true
shape is:
- **WIN:** local aggregation & bounded decisions (count / site / terrain over a focused window) — robust
  across boards, and where the T3 dominance metric also favors it (dense board).
- **LOSE:** dispersed-entity integration & travel reasoning (find + route to scattered targets), and
  threat-light valuation on sparse boards.

The defensible claim shrinks to: *"On dense boards, interactive is the most cost-effective access strategy for
local-aggregation and bounded-decision questions; it loses on tasks requiring integration across dispersed
entities or travel-cost reasoning, and its overall edge is board-size-dependent."*

## First live T3 data
`settle-site` runs end-to-end live and the **blunder-rate metric separates the encoders** (interactive best on
dense, hierarchical best on sparse) — the tier behaves as designed.

## Open (unchanged by this run)
- **Cross-model** (single model here; think-on rescued static encoders on siting and likely narrows
  interactive's local-aggregation edge too).
- **Think-on** on this corpus.
- `reachable-nearest` is *hard for everyone* (4–5/12 think-off) — may be a reasoning-gated kind.
- **Total session spend: $1.76.**
