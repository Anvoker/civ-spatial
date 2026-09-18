# Findings — T677 scale-crossover (does raw>adjacency invert? does hierarchical pay off?)

**Date:** 2026-07-28 · **Board:** `testcontroller_T677.sav`, full board (84×56, dense: 10 players,
118 cities, 682 units), no crop, difficulty `hard`, seed 1, **think-off**, per-kind 6 → **n=54/encoding**.
Model: `deepseek/deepseek-v4-flash` (OpenRouter, concurrency 8, cache on). File:
`results-t677-crossover.jsonl`. Accuracies carry a 95% Wilson CI.

This is the pivotal experiment teed up in `CONTINUITY-2026-07-27.md`: on a **dense** board, does `raw`'s
T50 advantage over `adjacency` **invert**, and does the relational/chunked `hierarchical` encoder finally
**pay off** at scale (the SAGA / NLGraph / Lost-in-Aggregation prediction)?

---

## Headline table — accuracy [95% Wilson CI] and cost, by encoding

| Encoding | acc [95% CI] | mean prompt tok | vs raw tokens |
|---|---|---|---|
| **hierarchical** | **0.852 [0.734, 0.923]** | ~79.7k | 1.4× |
| raw | 0.778 [0.651, 0.868] | ~56.3k | 1.0× |
| ascii | 0.778 [0.651, 0.868] | ~50.7k | 0.9× |
| adjacency | 0.778 [0.651, 0.868] | ~242.4k | **4.3×** |

n=54/encoding, think-off. Board-block (chars/4) estimates for reference: adjacency ~145.8k · hierarchical
~50.9k · raw ~36.8k · ascii ~33.7k model-tok; the "mean prompt tok" column is the model's own reported count.

## Finding 1 — **no inversion.** `raw` still strictly dominates `adjacency` at scale.
On T50, `raw` (0.893) beat `adjacency` (0.869) on accuracy *and* cost. On dense T677 the accuracy gap
closes to **zero** — both sit at **0.778** — but `adjacency` pays **4.3× the tokens** (242k vs 56k) for
that identical accuracy. So the relational encoding did **not** overtake `raw` at scale; it remains
**strictly dominated on the cost frontier** (equal accuracy, far more expensive). The scale hypothesis for
adjacency is **answered: no.** → adjacency is now retired from default runs (opt-in via `--encoding
adjacency`; code + oracle gate retained).

## Finding 2 — **hierarchical crosses over.** The T50 null becomes the T677 leader (directional).
`hierarchical` was **null at T50** (`findings-hierarchical.md`: tied accuracy, +27% tokens → dominated).
On dense T677 it is the **only** encoder to break the three-way 0.778 pack: **0.852**, **+7.4 points over
raw**, at **1.4× raw's tokens** (not adjacency's 4.3×). This is the predicted scale effect — a chunked,
region-summary representation starts to help exactly when the flat board gets big and dense.

**Caveat — this is directional, not established.** The Wilson CIs overlap (hierarchical [0.734, 0.923] vs
raw [0.651, 0.868]); the gap is 4 questions (46/54 vs 42/54); single board, single seed, think-off only.
It flips the *sign* of the T50 result, which is the notable part, but the magnitude needs a confirmation
run before it's a headline. **→ It got one, and it did NOT replicate — see "CONFIRMATION" below. Read this
Finding 2 together with that section: the seed-1 "leader" cell was sampling noise; hierarchical ties raw,
it does not beat it.**

## Finding 3 — the edge lives in the discriminating kinds.
Per-category (n=6 each; wide CIs — read as direction):

| Category | adjacency | ascii | hierarchical | raw | note |
|---|---|---|---|---|---|
| region-count | 1/6 | 2/6 | **3/6** | **3/6** | the wall — hierarchical top-tied, still only 0.5 |
| nearest | 3/6 | 3/6 | **5/6** | **5/6** | hierarchical & raw lead |
| terrain | 6/6 | 5/6 | **6/6** | 4/6 | raw *dips* on the dense board; hierarchical perfect |
| unit-strength (T2a) | 4/6 | 3/6 | 3/6 | 3/6 | valuation still hard for all |
| city-defense (T2b) | 4/6 | 5/6 | 5/6 | 4/6 | — |
| direction / distance / reachability / adjacency-Q | 6/6 | 6/6 | 6/6 | 6/6 | **saturated — no discrimination** |

Two things confirm prior work: (a) the four saturated kinds (direction, distance, reachability, the
adjacent-terrain question) are 6/6 for *every* encoding — they still don't discriminate, validating the
minimal-harness pruning; (b) **region-count is still the wall** even for the winner (0.5). Hierarchical's
advantage concentrates in `region-count` + `nearest` + `terrain` — precisely the discriminating set. Note
`raw`'s **terrain** dip to 4/6 here: scanning a flat coordinate list for one tile gets harder on a bigger,
denser board, where the hierarchical region-summary reads it off directly.

---

## CONFIRMATION (seed 2, per-kind 12, n=108/enc) — **the edge does NOT replicate**
An independent replication (fresh question sample, 2× the power, raw/ascii/hierarchical, think-off;
`results-t677-hier-confirm.jsonl`) **walks back Finding 2's magnitude**:

| Encoding | seed-1 acc (n=54) | **seed-2 acc [95% CI] (n=108)** | pooled (n=162) |
|---|---|---|---|
| raw | 0.778 | **0.870 [0.794, 0.921]** | 0.840 |
| hierarchical | 0.852 | **0.870 [0.794, 0.921]** | 0.864 |
| ascii | 0.778 | 0.815 [0.731, 0.877] | 0.802 |

On seed 2, **hierarchical and raw are exactly tied (0.870 = 0.870)** — the +7.4-pt seed-1 gap vanished.
The seed-1 result was largely a **sampling artifact**: raw took an unlucky `terrain` dip (4/6) on seed-1's
questions and recovered on seed 2; hierarchical's seed-1 `region-count` edge also reversed (it was the
*worst* of the three on seed-2 region-count, 7/12 vs raw/ascii 8/12). Pooled over both seeds hierarchical
leads raw by a slim **+2.4 pts (0.864 vs 0.840)** — well inside the noise; CIs overlap heavily.

**Corrected verdict.** Hierarchical went from **dominated at T50** (worse cost, tied accuracy) to
**tied-with-raw-on-accuracy at T677**, but it does **not** convincingly *beat* raw — and it still costs
**1.4× raw's tokens**, so on the accuracy-vs-cost frontier it remains **dominated by raw** (same category
as adjacency, milder token penalty). What replicates cleanly across both seeds: **ascii is the weakest of
the three at scale** (raw > ascii holds), and **raw is the frontier encoder**. The honest headline is
"hierarchical de-dominates at scale (accuracy catches up) but doesn't earn its extra tokens," not
"hierarchical wins."

## Implications
1. **Adjacency: retired from default runs.** Confirmed dominated at both scales. `run` now defaults to
   `raw+ascii`; adjacency stays in the code, registered, and covered by `verify-oracle` + the oracle test.
2. **Hierarchical: NOT promoted.** The confirmation shows it ties raw on accuracy at scale (not beats) and
   still costs 1.4× the tokens → off the cost frontier. It's a genuine *de-domination* vs T50 (worth noting
   in the writeup as "chunking closes the gap at scale"), but it does not earn a default slot. raw stays the
   frontier encoder. A **think-on pass** is the remaining question (does reasoning move any of this?).
3. **Minimal-harness choice reinforced.** `raw+ascii` is the right cost-minimal loop: raw is the frontier
   and hierarchical does not beat it. No change needed.

## Caveats
- Two seeds now (crossover n=54 + confirmation n=108). The confirmation is the more-powered, so weight it:
  the seed-1 "hierarchical leads" cell was noise. Still single board, single model, think-off.
- think-off is the cleanest encoding-separation regime; a think-on pass would test whether reasoning
  collapses hierarchical's edge the way it collapsed the raw–ascii gap on DeepSeek (`findings-deepseek-fullboard.md` F2).
- region-count numbers are post the `generators.rs` default-terrain fix (unlike the T50 findings files).
