# Findings — a calculator beats reasoning: the dispersed-integration deficit was ARITHMETIC

**Date:** 2026-07-31 · basic primitives-only **calculator** (oracle measurement operators as tool calls;
commits `bf9b2ca`/`f42ee10`, the `raw-ops` / `interactive-ops` modes). DeepSeek V4 Flash. Corpus: T677
masked to **player4 (`--fog 4`, 6.8% explored, 319 known tiles)** — the same early-game fogged board as the
over-fetch work — restricted to the three **dispersed** kinds that carried every prior deficit
(`nearest-owned`, `reachable-nearest`, `region-count`). Per-kind 8 × **3 reps** = 24/kind/arm, 72/arm, **360
trials** across 5 arms. Items are **paired across arms** (identical `item_id`s), so per-item comparisons are
valid. Results: `results-calc-*.jsonl`; numbers from `python analysis/summarize.py results-calc-*.jsonl`.

> **Why this run:** the fog work isolated the interactive dispersed-integration deficit to a
> **reasoning/integration limit** (think-on closed it, directory verbs did not — `findings-fog.md`). The
> "maximal calculator" thesis makes a sharper prediction: that limit is *arithmetic*, not spatial. If so,
> handing the model **free, exact measurement operators** (distance / travel-turns / counts / nearest) should
> close the deficit **without any reasoning at all** — and do it better than turning reasoning on. This is that
> test, and the basic (primitives-only) calculator's job was to settle it before we invest in the maximal
> decision-corpus.

## Headline: the calculator hits the ceiling; reasoning doesn't

DeepSeek [nothink] unless the row says `think-ON`. Pooled over 3 reps (n=72/arm):

| arm | perception | compute | **acc** | 95% Wilson CI | tot tok | gen tok | latency | turns | tool-calls | cache% |
|---|---|---|---|---|---|---|---|---|---|---|
| **raw-ops** | full board | **calculator** | **0.986** | [0.925, 0.998] | 59,508 | 3,833 | 51.6 s | 7.0 | 20.0 | 88% |
| raw `think-ON` | full board | reasoning | 0.875 | [0.779, 0.933] | 12,483 | 7,157 | 83.2 s | 1 | 0 | 91% |
| interactive | query loop | none | 0.819 | [0.715, 0.891] | 89,089 | 6,675 | 92.5 s | 9.8 | 42.0 | 73% |
| interactive-ops | query loop | **calculator** | 0.806 | [0.700, 0.880] | 103,784 | 5,315 | 78.4 s | 11.2 | 36.3 | 78% |
| raw (baseline) | full board | none | 0.792 | [0.684, 0.869] | 5,679 | 353 | 5.6 s | 1 | 0 | 96% |

`raw-ops` (full board + calculator, **think-off**) lands at **0.986** — near-perfect on exactly the kinds that
had resisted every previous lever, and **higher than turning reasoning on** (raw `think-ON` 0.875). It is also
the **most stable** arm across reps (below), while the two query-loop arms and the reasoning arm all sit in the
noisy 0.79–0.88 band.

## Per-kind (pooled, n=24/cell) — the calculator fixes the two kinds that never held

| kind | raw off | raw `think-ON` | **raw-ops** | interactive | interactive-ops |
|---|---|---|---|---|---|
| nearest-owned | 23/24 | 23/24 | **24/24** | 18/24 | 20/24 |
| reachable-nearest | 15/24 | 16/24 | **23/24** | 17/24 | 15/24 |
| region-count | 19/24 | 24/24 | **24/24** | 24/24 | 23/24 |
| **overall** | **57/72 = 0.792** | **63/72 = 0.875** | **71/72 = 0.986** | **59/72 = 0.819** | **58/72 = 0.806** |

`reachable-nearest` is the tell: it sat at **15/24 (raw off)** and barely moved with reasoning (**16/24**), but
the calculator lifts it to **23/24**. `region-count` — the historic counting wall for static encoders (raw off
19/24) — goes to **24/24** once a `count` operator is available. The calculator's *only* miss across all three
kinds is a single `reachable-nearest` non-answer.

## The failure MODE, by arm — this is the whole mechanism

| arm | correct | **wrong** | **non-answer (invalid)** |
|---|---|---|---|
| raw (baseline) | 57 | **15** | 0 |
| raw `think-ON` | 63 | 9 | 0 |
| **raw-ops** | 71 | **0** | 1 |
| interactive | 59 | 4 | **9** |
| interactive-ops | 58 | 3 | **11** |

The failure modes are cleanly separated by arm, and they name the causes:
- **raw baseline sees everything and gets 15 answers WRONG, zero non-answers** — it has all the tiles in one
  block and still miscomputes. That residual is **arithmetic**, by construction (perception is complete).
- **Reasoning attacks the wrong-answer pile:** raw `think-ON` cuts wrong 15 → 9 (still not zero).
- **The calculator ELIMINATES it:** raw-ops has **0 wrong answers** — every arithmetic slip the reasoning arm
  still made is gone. Its lone failure is 1 non-answer.
- **The query-loop arms fail a different way — non-answers (cap-outs), not wrong answers** (9 and 11 invalid
  vs ≤4 wrong). This is the exact over-fetch signature from `findings-fog.md`: over-fetch → failure-to-converge,
  not incorrectness.

## Claim 1 — a calculator beats reasoning; the deficit was ARITHMETIC, not spatial

**raw-ops (0.986, think-off) > raw think-ON (0.875).** This is the prediction the calculator thesis made: the
persistent dispersed-integration deficit was **arithmetic**. Given the *same* full-board perception, exact
measurement operators beat switching reasoning on — and beat it while think is **off**, so no spatial
chain-of-thought is doing the work.

*Rigor.* The pooled 95% CIs graze (overlap only in ~[0.925, 0.933]), so this is not a slam-dunk on pooled CIs
alone. But the paired, per-rep picture is unambiguous:

| rep | raw-ops | raw `think-ON` |
|---|---|---|
| 1 | 0.958 | 0.958 |
| 2 | 1.000 | 0.875 |
| 3 | 1.000 | 0.792 |

raw-ops **wins twice and ties once, never loses**, and does it by driving **wrong answers to zero** (vs think-on's
9). Reasoning is also *noisier* (per-rep range 0.792–0.958, ~0.17 spread — the ~12–18% single-run flip noise in
full view) while raw-ops is pinned at the ceiling (0.958–1.000). The mechanism is decisive even where the CIs are
cautious: **reasoning reduces arithmetic errors; the calculator removes them.**

## Claim 2 — perception and compute are SEPARABLE levers; the win is cheap full perception + compute

**raw-ops (0.986) ≫ interactive-ops (0.806)** — CIs disjoint ([0.925, 0.998] vs [0.700, 0.880]). And bolting the
identical operators onto the **query loop did essentially nothing**: interactive-ops **0.806** vs plain
interactive **0.819** (CIs fully overlapping; if anything a touch lower, and with *more* non-answers, 11 vs 9).

So the calculator's payoff is **contingent on the perception substrate it sits on**, and the two levers are
independent:
- On **full-board perception** (raw), adding compute is transformative: 0.792 → 0.986.
- On the **query loop** (interactive), adding the same compute is inert: 0.819 → 0.806.

The reason is the failure-mode table: interactive's binding constraint is **convergence/over-fetch**
(non-answers), not arithmetic. A calculator can only help a run that *reaches* the arithmetic step; interactive
arms cap out before they get there (and the operators add yet another thing to fetch, nudging non-answers *up*).
**The win is the combination — cheap complete perception AND compute — not either alone, and not the query loop
under any compute.**

## Claim 3 — this dovetails with the over-fetch finding

`findings-fog.md`'s tracer result: interactive over-fetches (redundant fog re-scans, no fetch-memory) and
**over-fetch predicts NON-answers, not wrong answers** (cap-outs from failure-to-converge). This run shows the
same signature from the other side of the ledger: the two query-loop arms carry **all 20 of the run's
non-answers** (9 + 11) and burn the most tokens/turns/round-trips to do it (interactive 42 tool-calls / 9.8
turns / 92.5 s; interactive-ops 36 calls / 11.2 turns; both ~73–78% cached, so the unclosed cost is **latency**,
not dollars). The static arms — which fetch the board **once** — have **zero** non-answers. Handing the query
loop a calculator does not touch its over-fetch problem, which is exactly why interactive-ops flatlines.

## Cost / latency — the calculator is ~10× the tokens of raw, and worth it

- **raw-ops costs ~10× the tokens of raw** (59.5k vs 5.7k tot) and ~9× the latency (51.6 s vs 5.6 s), for
  **+0.194 accuracy and zero wrong answers**. The extra tokens are the tool-loop re-send across ~7 turns / ~20
  operator calls (the board itself is ~5k, sent each turn); 88% is cached, so the dollar multiple is far below
  10×, but latency is real (sequential round-trips).
- **raw think-ON** is the *cheapest way to buy accuracy in tokens* (12.5k) but is **slow** (83.2 s, reasoning
  is sequential generation) and still leaves 9 wrong.
- **The query loop is the worst on BOTH axes now — cost and accuracy.** interactive-ops is the single most
  expensive arm (103.8k tok) at the second-lowest accuracy (0.806); interactive is 89.1k tok / 92.5 s at 0.819.
  On this small early board the calculator-on-full-board route dominates the query loop on every axis that
  matters except raw-token count vs raw-baseline.

## Caveats
- **Single model** (DeepSeek V4 Flash). Cross-model validity (esp. a **think-ON cross-model pass** to equalize
  in-loop effort, per the Gemini agentic-effort confound in `findings-fog.md`) and a **frontier anchor** are
  separate in-flight workstreams, not settled here.
- **n and noise.** n=72/arm pooled (24/kind). The ~12–18% per-item single-run flip noise is visible in the
  per-rep spreads (raw off 0.708–0.833; think-on 0.792–0.958). Headline numbers are **pooled over 3 reps**; per-rep
  figures are given where a claim rests on them (Claim 1). raw-ops's near-perfection is the one arm the noise
  floor can't move — it is pinned at the ceiling in all three reps.
- **Corpus-specific.** This is the **three dispersed kinds on an early-game (6.8%) fogged board**. The calculator
  result is a statement about **dispersed-integration on a sparse board**, the exact regime where the deficit
  lived — not a claim about local-aggregation kinds, dense/late boards, or the full 6-kind slate.
- **The operators are oracle-exact by construction.** raw-ops's 0 wrong answers is *because* the measurements
  are ground-truth; the finding is that once measurement is free the model composes it correctly, i.e. the
  residual was arithmetic — not that the model can measure.
- The `raw-ops` gain vs `raw think-ON` has grazing pooled CIs; the claim leans on the paired per-rep dominance
  and the failure-mode decomposition (see Claim 1).

## What this means for the pivot

The basic (primitives-only) calculator **did its job**: it settled the mechanism question the fog work left
open. The persistent dispersed-integration deficit was **arithmetic** — free exact operators close it (0.986,
0 wrong) better than reasoning does (0.875, 9 wrong), and they close it on **full-board perception**, not the
query loop (which is over-fetch-bound and gets nothing from compute). Perception and compute are separable
levers; the winning combination is cheap complete perception + compute.

That is precisely the greenlight the **maximal-calculator decision corpus** needs. If primitives-only operators
already sweep the argmax/aggregation kinds (nearest-owned, reachable-nearest, region-count all → 23–24/24),
then those kinds are **solved** and no longer discriminate — which is the design premise of
`maximal-calculator-corpus.md`: **make every measurement free and keep only the kinds that survive** (the
withheld-weighting (S), counterfactual (C), perspective (P), predicate-composition (I), and partial-order
ranking (J) reasoning kinds). This run confirms the "collapse list" empirically from below — the single-axis /
free-operator kinds collapse once you hand over a calculator — so building the maximal decision corpus is the
right next investment. Next builds: `cf-vacate`, `adv-assault-target`, `triage-reinforce`, `compare-two-attacks`
(the ranked shortlist).

**Bottom line:** a calculator beats reasoning on the dispersed kinds because the deficit was arithmetic; it pays
off only with full-board perception, not the over-fetch-bound query loop; and it collapses exactly the
primitives-solvable kinds — clearing the way for the maximal-calculator corpus.

---

## Scale test — does raw-ops hold on a LARGE known map? (added 2026-07-31)

The 0.986 above is a **small-map** result: T677 masked to player4 at **6.8% explored** (~319 known
tiles, raw board block ~5k tokens). To test whether the calculator's near-ceiling win survives when
perception is *expensive*, re-ran `raw-ops` vs `interactive-ops` on **T677 `--fog 2` (98% explored — a
large, dense, well-explored late-game known map)**, same 3 dispersed kinds, per-kind 8, think-off,
DeepSeek, **3 reps (seeds 1-3, n=72/arm)**. Data: `results-calc-scale-p2{,-rep2,-rep3}.jsonl`.

### The single-rep "inversion" was noise
The first rep looked dramatic — interactive-ops 0.875 vs raw-ops 0.708 (a full inversion). It did
**not** replicate. Pooled over 3 reps:

| arm | 6.8% explored (small) | 98% explored (large), 3-rep pooled |
|---|---|---|
| raw-ops | **0.986** [0.925, 0.998] | **0.736** [0.624, 0.824] |
| interactive-ops | 0.806 | **0.750** [0.639, 0.836] |

At 98% the two arms are **tied** (0.750 vs 0.736, CIs overlap heavily). The scout's 0.875-vs-0.708 gap
was single-rep noise (the documented ~12-18% per-item flip); pooling collapsed it. Lesson again: never
ship a 1-rep gap.

### The robust finding: raw-ops does NOT hold at scale
raw-ops drops **0.986 -> 0.736** (~0.25) from the small to the large known map, while interactive-ops is
roughly board-size-invariant (0.806 -> 0.750). The calculator's near-ceiling win is **board-size-
dependent**: on a large board raw-ops loses its decisive lead and the two ops arms **converge to ~0.75**.
This mirrors the static-dumps-anti-scale / query-loops-scale crossover from `findings-fog.md`, now
reappearing **with the calculator on both sides**.

### Per-kind split (a clue to the mechanism)
At 98% the two dispersed kinds trade wins:
- **nearest-owned**: interactive-ops 18/24 vs raw-ops 12/24 (interactive-ops better)
- **reachable-nearest**: interactive-ops 12/24 vs raw-ops 18/24 (raw-ops better)
- **region-count**: ~24/24 both (tied)

They cancel -> overall tie. The opposite per-kind preferences suggest the two arms fail for *different*
reasons at scale (not yet diagnosed).

### Corrected thesis
The calculator eliminates the **arithmetic** bottleneck on small boards (raw-ops 0.986, 0 wrong). At
scale a **second bottleneck re-emerges — locating the right entities/tiles to point the operators at
within a large board** — and it caps *both* ops arms at ~0.75. Richer operators alone are unlikely to
break this ceiling; this directly cautions the maximal-calculator direction (`raw-maxops` = full board +
more operators probably hits the same wall).

### Open — the key next investigation
**Why the ~0.75 ceiling at scale?** A failure-mode analysis of the 98% traces (both arms, all 3 reps):
where does each arm go wrong — wrong-coordinate selection, entity not found, mis-aggregation, early
bail, turn cap? Scoped but **not yet run**.

Caveats: the 3 reps use **different seeds** (a broader item sample, pooled n=72 — not repeated-item
majority vote), single model (DeepSeek think-off), n=24/kind pooled. Cost: scout $0.41 + reps 2-3 $0.82
= ~$1.23.
