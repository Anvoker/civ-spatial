# Findings — full-board encoding eval (DeepSeek V4 Flash + Qwen a3b)

**Date:** 2026-07-27 · **Board:** `myagent_T50.sav`, full board (78×52), no crop, difficulty `hard`.

## Runs
| Run | Model | Reasoning | Encodings | per-kind | File |
|---|---|---|---|---|---|
| A | deepseek/deepseek-v4-flash | off | raw, ascii, adjacency | 12 (84 q) | `results-deepseek-fullboard-hard.jsonl` |
| B | deepseek/deepseek-v4-flash | **on** | raw, ascii, adjacency | 12 (84 q) | `results-deepseek-fullboard-hard-thinkon.jsonl` |
| C | qwen3.6-35b-a3b-mtp (local, calibrated) | off | raw, ascii | 6 (42 q) | `results-qwen-fullboard-hard.jsonl` |

All runs: seed 1, temperature 0, 0 invalid/0 failed. Run C is a matched *subset* of A/B's question set
(same seed → first 6/category). Adjacency is cloud-only — 181k tokens won't fit local 32k context.

---

## Finding 1 — the ceiling is broken (no-reasoning, run A)
Full board (vs the earlier cropped runs where all encodings ceilinged at ~1.0), DeepSeek no-think:
- **raw 0.893 · adjacency 0.869 · ascii 0.750** — a ~14-point spread. ASCII is the weakest text encoding.
- **raw strictly dominates adjacency**: higher accuracy at **~6.8× fewer tokens** (26.6k vs 181.6k). The
  fancy relational encoding buys nothing; the naive grid (ascii) is the *worst*. Real accuracy-vs-cost
  frontier is **raw vs ascii**; adjacency is off it.

## Finding 2 — reasoning collapses the encoding gap (run B vs A) ← headline
Turning reasoning **on** lifts every encoding to ~0.98 and erases the spread:

| Encoding | reasoning OFF | reasoning ON | Δ |
|---|---|---|---|
| raw | 0.893 | 0.988 | +9.5 |
| **ascii** | 0.750 | 0.976 | **+22.6** |
| adjacency | 0.869 | 0.976 | +10.7 |

raw−ascii spread: **+0.143 → +0.012**. ASCII — worst without reasoning — gains most and nearly ties.
**Interpretation:** the no-think encoding differences were largely a *reasoning substitute*. A good encoding
compensates for a model that can't reason; give it reasoning and it extracts what it needs from any
encoding. **The encoding effect is conditional on reasoning budget** — strong when reasoning is off/cheap,
near-zero when it's on. This reframes the project's core question.

## Finding 3 — the best encoding is model-dependent (run C vs A, matched 42 q)
On the **identical 42 questions**, no reasoning:

| Encoding | Qwen a3b (calibrated) | DeepSeek V4 |
|---|---|---|
| **raw** | 0.714 | **0.905** |
| **ascii** | **0.881** | 0.810 |

**The preference flips.** DeepSeek prefers `raw` (flat coordinate list); Qwen prefers `ascii` (2D glyph
grid) — a ~19-point swing in opposite directions. Evidence *against* "there is a universally best encoding."
Qwen's `raw` weakness is concentrated in **nearest (raw 1/6 vs ascii 6/6)** and region-count — plausibly it
leans on 2D layout and struggles with coordinate-list search+arithmetic, where DeepSeek is strong at both.
(n=6/cell — the 42-q aggregate flip is the robust part; per-category is directional.)

## Finding 4 — region-count is the residual hard skill
The only category that survives reasoning: run B leaves it at raw 11/12, ascii 10/12, adjacency 10/12
(everything else → 12/12). No-think it was the worst everywhere (raw 7, adj 4, ascii 3). Counting terrain
in a 7×7–11×11 window is a genuine aggregation limit, not an encoding/perception artifact — reasoning helps
but doesn't fully solve it. This is what the `--difficulty hard` lever is for, and it's working.

---

## Cost & latency (caching validated: 98–99% board-prefix hits)
| Run | in tokens (cached) | out tokens | est. $ | wall-clock | mean lat/call |
|---|---|---|---|---|---|
| A (off) | 18.2M (98.5%) | 74.9k | **~$0.54** | ~16 min | 3.5–4.6 s |
| B (on) | 18.2M (99%) | 698k | **~$0.65** | ~103 min | 16–39 s |
| C (Qwen off) | local | — | $0 | ~54 min | raw 16 s / ascii 45 s |

Reasoning is **cheap in dollars (~+20%) but ~6.4× slower**. ASCII reasons hardest (4416 completion tok/call
vs ~1950 for raw/adjacency) — it works to decode its own grid. (Correction: an earlier note estimated run A
at $0.20 assuming ~0.1× cache-read pricing; the real effective cache-read rate is ~$0.028/M, so ~$0.54.)

## Known issue — region-count default-terrain asymmetry (FIXED)
`raw`/`adjacency` **omit** the board's default terrain (here Ocean, 1365/4056 tiles) and only state it in a
header, so counting the *default* is a count-by-exclusion — harder, and encoding-specific — while counting a
*listed* terrain is a simple tally. Evidence in run A: raw nailed listed terrains (Plains 29✓, Grassland 21✓)
but missed Ocean (58→94); Deep Ocean (which *is* listed) came close (30→28). **Fix applied:** region-count
generation now excludes the default terrain (`generators.rs`), so the category measures the same skill across
all encodings. Not a validity bug (the info was disclosed), but a real confound now removed. Runs A–C predate
the fix; their region-count numbers include some default-terrain questions.

## Caveats
- Single board, single seed. n=84/encoding (runs A/B) is firm at the aggregate; per-category (n=12) and all of
  run C (n=6) are directional. No Wilson/bootstrap CIs yet.
- distance/nearest/reachability saturate at 12/12 (runs A/B) — they don't discriminate; a harder T1 tier should
  target them.
- Qwen calibration effect is **not isolable** from these runs (the only prior full-board ascii number, 0.643,
  was confounded by 16k truncation, now fixed at 32k; and it was think-on vs C's think-off).

## Suggested next steps
1. **Wilson intervals** in `summarize.py` before treating any cell as real.
2. **Tier sweep** (Gemini 2.5 Flash + a frontier anchor) to test Findings 2–3 across more models.
3. A **harder T1 tier** to de-saturate distance/nearest/reachability.
4. Re-run region-count post-fix to get the clean, cross-encoding-comparable number.
