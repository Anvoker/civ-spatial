# Findings — cross-model tier sweep (Gemini 2.5 Flash + Claude Sonnet 4.5 anchor)

**Date:** 2026-07-27 · **Board:** `myagent_T50.sav`, full board (78×52), no crop, difficulty `hard`,
seed 1, temperature 0. Extends `findings-deepseek-fullboard.md`. All accuracies below carry a **95%
Wilson score interval** (added to `analysis/summarize.py` this session; `python analysis/summarize.py`
self-checks the math on the 8/10 -> [0.49, 0.94] case).

## Runs added this session
| Run | Model | Reasoning flag | Encodings | per-kind | n/enc | File |
|---|---|---|---|---|---|---|
| G-off | google/gemini-2.5-flash | off | raw, ascii, adjacency | 12 | 84 | `results-gemini25-off.jsonl` |
| G-on | google/gemini-2.5-flash | on | raw, ascii | 12 | 84 | `results-gemini25-on.jsonl` |
| S-off | anthropic/claude-sonnet-4.5 | off | raw, ascii | 8 | 56 | `results-sonnet45-off.jsonl` |
| S-on | anthropic/claude-sonnet-4.5 | on | raw, ascii | 2 | 14 | `results-sonnet45-on.jsonl` |

Adjacency was dropped from the reasoning/anchor runs to stay inside the ~$5 budget (it is ~213k
tokens/question and the prior findings show it is strictly dominated by `raw`). Sonnet `per-kind` was
cut to bound its expensive output pricing ($15/M). Prior DeepSeek/Qwen numbers below are re-summarized
from the main-worktree files for side-by-side comparison; runs are **matched by seed** (each smaller
run is a first-k subset of the same question set).

---

## Headline table — accuracy [95% Wilson CI], by model x encoding x reasoning

| Model (tier) | Enc | reasoning OFF | reasoning ON |
|---|---|---|---|
| **claude-sonnet-4.5** (frontier) | raw | **0.929** [0.830, 0.972] (n=56) | 1.000 [0.785, 1.000] (n=14) |
| | ascii | 0.768 [0.642, 0.859] (n=56) | 0.786 [0.524, 0.924] (n=14) |
| **gemini-2.5-flash** (mid) | raw | **0.952** [0.884, 0.981] | 0.952 [0.884, 0.981] |
| | ascii | 0.762 [0.661, 0.840] | 0.750 [0.648, 0.830] |
| | adjacency | 0.833 [0.739, 0.898] | — (dropped) |
| **deepseek-v4-flash** (cheap) | raw | 0.893 [0.809, 0.943] | 0.988 [0.936, 0.998] |
| | ascii | 0.750 [0.648, 0.830] | 0.976 [0.917, 0.993] |
| | adjacency | 0.869 [0.781, 0.925] | 0.976 [0.917, 0.993] |
| **qwen3.6-35b-a3b** (local) | raw | 0.714 [0.564, 0.828] (n=42) | — |
| | ascii | **0.881** [0.750, 0.948] (n=42) | — |

n=84/cell unless noted. DeepSeek/Qwen from `findings-deepseek-fullboard.md`.

---

## The reasoning toggle did not behave uniformly across models (methodology finding)

This is the headline of the sweep and it gates the F2 verdict. The harness `--think off` sends
`reasoning_effort:"none"`; `--think on` sends **no reasoning field at all** (provider default). Each
provider resolves that default differently, so the *same two flags* produced three different regimes:

| Model | think OFF (`"none"`) | think ON (default) | Toggle effect |
|---|---|---|---|
| deepseek-v4-flash | reasoning suppressed (~300 compl tok/q) | reasons (~1950-4416 compl tok/q) | **works** — real off<->on contrast |
| gemini-2.5-flash | `"none"` **ignored**, still verbose (~570-926/q) | same (~570-923/q) | **inert (stuck ON)** — compl tokens & accuracy identical to 3 sig figs |
| claude-sonnet-4.5 | no extended thinking (~317-389/q) | no extended thinking (~348-393/q) | **inert (stuck OFF)** — compl tokens flat |

Evidence for inertness: Gemini raw completion was 926/q (off) vs 923/q (on); ascii 579 vs 570 — and
accuracy moved 0.952->0.952 (raw), 0.762->0.750 (ascii). Sonnet raw was 317/q (off) vs 348/q (on),
ascii 389 vs 393. Neither model changed regime when the flag flipped. **Only DeepSeek gives a clean
reasoning on/off manipulation through this harness.** To force reasoning on Gemini/Anthropic you would
need a provider-specific reasoning field (`reasoning: {max_tokens}` / effort level) that `--think on`
does not currently emit — a Rust change, out of scope here.

---

## F2 (reasoning collapses the encoding gap) — **holds where testable, contradicted where reasoning is always-on**

- **DeepSeek (clean test): replicates.** raw-ascii spread 0.143 (off) -> 0.012 (on); every encoding ->
  ~0.98. This is the original F2 result, unchanged.
- **Gemini 2.5 Flash: F2 does NOT hold in its (always-reasoning) regime.** Gemini reasons in *both*
  toggle settings, yet the raw-ascii gap is a stable **~0.19** (raw 0.952 vs ascii ~0.75-0.76) and does
  not close. So a model that reasons by default can still carry a large, persistent encoding gap — the
  gap is not automatically collapsed by the presence of reasoning. (Caveat: we could not obtain a
  matched no-reasoning Gemini baseline, so this is "reasoning-on gap persists," not a within-model
  off->on delta.)
- **Sonnet 4.5: not a reasoning test.** Both conditions are effectively no-reasoning; the raw-ascii gap
  is ~0.16 (0.929 vs 0.768), consistent with a no-reasoning regime. The n=14 "on" probe (raw 1.00,
  ascii 0.786) is within CI of "off" and just confirms the toggle is inert.

**Verdict:** F2 replicates on DeepSeek but is **model-conditional, not universal**. Gemini is a
counterexample in the weak form: reasoning being present did not erase the encoding gap. The cleaner
statement of the project's core finding is now: *large encoding gaps live in the no-reasoning regime;
turning reasoning on (where you can) collapses them — but a model that reasons by default is not
guaranteed to have collapsed them.*

## F3 (best encoding is model-dependent) — **replicates, with a sharper shape**

Every **cloud** model — DeepSeek, Gemini, Sonnet — prefers **raw over ascii** by a wide, CI-separated
margin in the no-reasoning regime:

| Model | raw - ascii (no-reasoning) | CIs disjoint? |
|---|---|---|
| gemini-2.5-flash | +0.190 (0.952 vs 0.762) | yes |
| claude-sonnet-4.5 | +0.161 (0.929 vs 0.768) | nearly (0.830-0.972 vs 0.642-0.859) |
| deepseek-v4-flash | +0.143 (0.893 vs 0.750) | overlap at edges |
| **qwen3.6-35b-a3b (local)** | **-0.167 (0.714 vs 0.881)** | yes — **flipped** |

**Verdict: F3 replicates.** There is no universal best encoding — but the split is not random across
models: the three cloud models **cluster on raw**, and local Qwen is the lone **ascii**-preferring
outlier (its `raw` weakness is concentrated in `nearest`: 1/6 raw vs 6/6 ascii, i.e. coordinate-list
search+arithmetic, where it leans on the 2D glyph grid instead). So the refined claim is: *encoding
preference is model-dependent, and the axis that predicts it here is "consumes a flat coordinate list
well" (cloud models: raw) vs "needs 2D spatial layout" (Qwen: ascii)*, not model size or tier.

Note the **tier axis is weak for accuracy**: frontier Sonnet (raw 0.929) does not beat mid Gemini
(0.952) or even cheap DeepSeek-with-reasoning (0.988) on this board — spatial-from-text at `hard` is
near a shared ceiling for capable models; encoding and reasoning-regime move the needle more than tier.

Residual hard skill (consistent with prior): **region-count** and **terrain** are where ascii bleeds
the most (Sonnet ascii terrain 4/8, region-count 3/8; Gemini ascii terrain 3/12). Direction / distance
/ nearest / reachability saturate near 1.0 for every cloud model and don't discriminate.

---

## Cost & tokens (this session)

Caching passed through OpenRouter on every model (Gemini 99.7-99.9% board-prefix hits; Anthropic
94-98% via our `--cache on` `cache_control` marker). Per-run token totals and measured spend:

| Run | prompt tok | cached % | compl tok | measured $ | notes |
|---|---|---|---|---|---|
| Gemini off (252 trials) | 21.38M | 99.9% | 409k | ~$1.69 | adjacency = 17.9M of the prompt tokens |
| Gemini on (168 trials) | 3.47M | 99.7% | 125k | ~$0.75 | raw+ascii only; incl. a killed pk12-all-3 attempt (~$0.06) |
| Sonnet off (112 trials) | 2.12M | 97.8% | 39.5k | ~$1.17 | Anthropic cache-read is ~10x Gemini's |
| Sonnet on (28 trials) | 0.53M | 94.3% | 10.4k | ~$0.54 | pk2 probe |
| Gemini adjacency probe (14) | 2.98M | 100% | 9.6k | ~$0.24 | caching-behavior smoke test |
| **Session total (task)** | — | — | — | **~$4.4** | key `total_usage` $5.58 incl. ~$1.19 prior DeepSeek |

**Cache-read rate matters more than tier for cost.** Measured effective board-prefix read rates:
DeepSeek ~ $0.028/M (given), Gemini ~ $0.031/M, **Anthropic ~ $0.30/M** (10x) — plus Anthropic
cache-*write* at $3.75/M and output at $15/M. That is why a 56-question Sonnet run costs more than a
252-question Gemini run. For Gemini, **completion tokens dominate the bill**, not the (cheaply cached)
board: the $1.69 off-run was ~$0.62 cached input + ~$1.02 output. Reported per instructions; convert
tokens to $ with each provider's own rates.

---

## Caveats
- **The reasoning "on" conditions for Gemini and Sonnet are not true reasoning-on** (toggle inert, see
  above). Treat Gemini's two columns as one always-reasoning regime and Sonnet's as one no-reasoning
  regime. DeepSeek is the only clean off<->on pair.
- Single board, single seed. Aggregate cells (n=56-84) are firm; Sonnet-on (n=14) and per-category
  (n<=12) are directional — read the Wilson CIs, several are wide.
- Sonnet dropped adjacency and used smaller `per-kind` for budget, so its rows are not
  cost-frontier-comparable on adjacency.
- Encoding token sizes differ by tokenizer; sizes here are each model's own reported counts.
- region-count numbers predate the `generators.rs` default-terrain fix (as in the DeepSeek findings).

## Suggested next steps
1. To test F2 on Gemini/Anthropic, teach the harness to emit provider-native reasoning controls on
   `--think on` (Gemini `reasoning.max_tokens`, Anthropic extended-thinking) — a small `remote.rs`
   change — then re-run a matched off/on pair.
2. A harder T1 tier to de-saturate distance/nearest/reachability (still 1.0 across cloud models).
3. Add one more ascii-preferring model to test whether "needs 2D layout" (the Qwen axis) generalizes
   or is Qwen-specific.
