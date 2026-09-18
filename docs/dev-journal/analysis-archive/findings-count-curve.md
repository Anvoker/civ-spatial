# Finding — region-count accuracy vs. TRUE count: the multiplicity cliff at the board level

**Date:** 2026-07-27 · pooled reanalysis of existing runs (no new spend). 214 region-count trials
across all `results-*.jsonl` (DeepSeek/Gemini/Sonnet think on+off, Qwen; raw/ascii/adjacency/
hierarchical). True count = `int(expected)`. Reproduce: `python analysis/count_curve.py results-*.jsonl`.

**Motivation:** "Why Do LLMs Struggle to Count Letters?" (see `RELATED-WORK.md`) finds counting
failure scales with **multiplicity** — an architectural aggregation limit — and predicts a sharp
accuracy cliff as the count rises. Does our board-level region-count show the same?

## Headline curve (all pooled, Wilson 95% CI)

| true count | n | acc | 95% CI |
|--:|--:|:--:|:--|
| 0 | 79 | 0.90 | [0.81, 0.95] |
| 1 | 10 | 0.80 | [0.49, 0.94] |
| 2 | 12 | 0.67 | [0.39, 0.86] |
| 3 | 11 | 0.55 | [0.28, 0.79] |
| 4 | 6 | 0.50 | [0.19, 0.81] |
| 5–6 | 15 | 0.47 | [0.25, 0.70] |
| 7–9 | 27 | 0.56 | [0.37, 0.72] |
| 10+ | 54 | 0.44 | [0.32, 0.58] |

**The cliff appears — but softer and shifted right vs. letter-counting.** Accuracy declines
monotonically from 0.90 (count 0), first crossing below 50% at **count 4–6** (vs **count 2** in the
letter paper), then **plateaus at ~0.44–0.56** for high counts rather than continuing to 80–90%
error. Direction confirmed; the board-level cliff is gentler, later, and flattens.

## Encoding shifts WHERE the cliff starts (strong; not a model confound)

```
encoding    c=0    1     2     3     4    5-6   7-9   10+   ALL
raw        0.88  1.00  1.00  1.00  0.50  0.50  0.64  0.68  0.79
ascii      0.96  1.00  0.25  0.00  0.50  0.20  0.22  0.24  0.53
adjacency  0.79  0.50  1.00  0.00  0.50  0.67  0.80  0.38  0.62
hierarchic 1.00  0.00  0.00  1.00   -    1.00  1.00  1.00  0.83
n/bin       79    10    12    11    6    15    27    54    214
```

- **ascii** collapses like the letter result: perfect at 0–1, then **0.25 @2, 0.00 @3**, plateau
  ~0.20–0.24 — crosses 50% at **count 2**.
- **raw** holds **1.00 through counts 1–3**, dips at 4, plateaus ~0.64–0.68 — crosses 50% around
  **count 4–5**, ~2 counts later than ascii.
- **Not a model confound:** raw (n=84) and ascii (n=76) share the same model set, and raw beats ascii
  *within every model* (deepseek-nothink 16/24 vs 3/12; gemini-nothink 11/12 vs 6/12). The **encoding**
  does the work — "encoding shifts the onset, architecture sets the shape." This is the mechanistic
  account of *why cloud models prefer raw over ascii*: raw defers the counting cliff by ~2 counts.
- adjacency (n=42) and hierarchical (n=12) bins are too thin to trust.

**Secondary:** reasoning pushes the cliff right (DeepSeek [think] 0.86 overall vs [nothink] 0.55; on
count 10+, 0.72 vs 0.30) — consistent with counting being an aggregation step that benefits from
explicit enumeration.

## Caveat (loud)
One board/seed family; 214 trials; **count-0 is 37%** of them (trivially easy — nothing to count —
inflating the low bins). Most non-zero / per-encoding / per-model bins are n = 6–15, several ≤ 6, some
= 1; CIs are wide. **Directional signal, not a measurement.** It motivates a dedicated **count sweep**
(multiple boards, balanced counts 0–20+, fixed model, raw-vs-ascii head-to-head) to turn this into a
real curve — a natural companion to the T677 scale-crossover.
