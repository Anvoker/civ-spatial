# Findings — cross-model think-ON + frontier anchor: the early-game interactive deficit survives reasoning, and it's an *engagement* asymmetry (not a think-flag artifact)

**Date:** 2026-07-31. DeepSeek / Gemini / GPT via OpenRouter. T677 `--fog 4` (6.8% explored, early-game),
fair 6-kind corpus (settle-site, best-site, region-count, terrain, nearest-owned, reachable-nearest),
per-kind 8 (n≈48/cell; single-rep → wide Wilson CIs, noise floor ~12–18%). Results:
`results-thinkon-gemini-int.jsonl`, `results-thinkon-gemini-raw-reco.jsonl`, `results-frontier-t677-p4.jsonl`.
Context/baselines: `analysis/findings-fog.md` (think-off cross-model + DeepSeek think-on).

## Why
findings-fog.md left two threads open. (1) The think-off cross-model comparison was **confounded**: at the
same `[nothink]`, DeepSeek worked the tool loop ~13× harder than Gemini, so "DeepSeek interactive > Gemini
interactive" might be *effort*, not skill. The proposed clean fix was a **think-ON** pass to equalize in-loop
effort. (2) No frontier-tier anchor point existed. This run does both.

## 1. Think-ON does NOT rescue interactive early-game on Gemini — and does NOT equalize effort

Gemini 2.5 Flash, think-ON, T677/p4:

| encoder | acc | 95% Wilson CI | turns | tool-calls |
|---|---|---|---|---|
| raw | **0.915** | [0.801, 0.966] | 1 | – |
| interactive | **0.729** | [0.590, 0.834] | 3.4 | 2.8 |

Interactive still trails raw by ~0.19 **with reasoning ON**. The deficit is entirely in dispersed-integration/
travel: nearest-owned 4/8 vs raw 7/8, reachable-nearest 2/8 vs 6/7, settle-site 5/8 vs 6/8; local aggregation
ties (best-site, region-count, terrain all 8/8 both).

**The key negative result:** turning reasoning ON did NOT make Gemini work the loop. Gemini think-on interactive
still bails after **3.4 turns / 2.8 tool-calls** — vs **DeepSeek think-on's 16.1 turns / 64.3 calls** (~23×
more). So the agentic-effort asymmetry flagged in findings-fog is **not a think-flag artifact** — it persists
at both settings. Gemini structurally under-engages multi-turn tool loops regardless of the reasoning flag.

**Confound verdict:** the think-ON pass *resolves* the question, but not the way we predicted. It does not
equalize in-loop effort; it shows the DeepSeek-vs-Gemini interactive gap is a durable **engagement** difference,
not a reasoning-flag confound. Report "DeepSeek interactive > Gemini interactive" as an agentic-engagement
difference, and the robust cross-model claim as the within-model result below.

## 2. Robust cross-model finding: interactive trails raw early-game on BOTH models; reasoning rescues it only where the model engages the loop

| model | think | interactive | raw | gap | loop effort (turns/calls) |
|---|---|---|---|---|---|
| DeepSeek V4 Flash | off | 0.819 | 0.931 | −0.11 | 11.9 / 50.8 |
| DeepSeek V4 Flash | ON (dispersed only) | 0.778 | 0.833 | −0.055 (nearly closed) | 16.1 / 64.3 |
| Gemini 2.5 Flash | off | 0.694 | 0.806 | −0.11 | 3.8 / 7.3 |
| Gemini 2.5 Flash | ON | 0.729 | 0.915 | −0.19 (stays wide) | 3.4 / 2.8 |

(DeepSeek think-on row = the dispersed-kinds subset from findings-fog; other rows = full 6-kind corpus — not
tile-for-tile comparable, read the ranking within each row.)

**Interactive is behind raw early-game on both lineages at both think settings.** Reasoning-on closes the
dispersed deficit on DeepSeek (which iterates the loop) but NOT on Gemini (which won't). The lever is
**engagement × reasoning, not reasoning alone**: a query loop only pays off if the model actually works it.
This is the sharpest statement yet of the early-game interactive story.

## 3. Frontier anchor: a bigger model does NOT crack the dispersed deficit on raw

openai/gpt-5.6-sol, raw, think-off, T677/p4 (n=48):

| model (raw, think-off) | acc | 95% Wilson CI |
|---|---|---|
| GPT-5.6-sol | **0.771** | [0.635, 0.867] |
| Gemini 2.5 Flash | 0.806 | – |
| DeepSeek V4 Flash | 0.792 | – |

The frontier model lands in the **same 0.77–0.81 band** as the two cheaper models on raw think-off — no
capability-tier jump. Per-kind it still misses the dispersed kinds (reachable-nearest 4/8, settle-site 5/8)
while acing terrain (8/8).

**This corroborates the calculator thesis (`findings-calc.md`) from the model-size axis.** The
dispersed-integration deficit is NOT closed by a bigger model (GPT-5.6 raw ≈ DeepSeek raw ≈ 0.77–0.80); it is
closed by the **calculator** (raw-ops 0.986). Three models across three lineages all stall at ~0.8 on raw
think-off; the operator tool — not scale, not (on a non-engaging model) reasoning — is what breaks through.
The deficit is arithmetic/computational, not capability-tier.

## Caveats
- n=8/kind (n≈48/cell), single-rep → wide Wilson CIs, no majority vote (noise floor ~12–18%). Point values
  are suggestive; the **ranking/direction** is the claim.
- Masked early-game corpus (T677/p4, 6.8%) — specific to the phase where interactive is weakest; not a
  whole-game verdict.
- Frontier point is raw-only, think-off only (budget); `openai/gpt-5.6-sol` slug/pricing per OpenRouter at
  run time. Frontier think-on / interactive left unrun (low value given the calculator result).
- DeepSeek think-on row is dispersed-kinds-only (from findings-fog), not the full 6-kind corpus.

## Cost
Gemini think-on ≈ **$1.89**; frontier ≈ **$0.10** (tiny output, cached board block) → **this run ≈ $2.0,
within the $4 cap.** (JSONL records tokens, not dollars; cost computed from OpenRouter prices — Gemini
$0.30/$2.50 per-M in/out, $0.075/M cached; GPT output was negligible at ~32 gen-tok/q.)

## Data hygiene
Canonical frontier file = `results-frontier-t677-p4.jsonl` (48 trials, all 6 kinds). The
`results-frontier-k-*.jsonl` per-kind files + `results-frontier-smoke.jsonl` are the same/partial data split
out during the run — do NOT combine them with t677-p4 in `summarize.py` (double-counts, as an interim tally did).

## Next
- A truly clean cross-model think-on comparison needs a model that BOTH reasons AND engages the loop (DeepSeek
  does; Gemini doesn't), or an interactive harness that enforces minimum engagement. Given `findings-calc.md`
  (raw-ops ≫ interactive), interactive's early-game story is now lower-priority than the calculator /
  decision-corpus direction.
