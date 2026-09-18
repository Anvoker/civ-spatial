# constraint-site Variant C — live confirmation (2026-08-11)

**Verdict: Variant C defeats the "call `site_check` and read off `ok`" shortcut. Confirmed live.**

## Setup
- DeepSeek V4 Flash, think-off, interactive-maxops, fog p1, `--difficulty hard`, `--cache on`.
- Board: `corpus_medium_a6_s1337-T0200-Y01490-auto.sav` (same as the perception-surface sweeps).
- Kind: `constraint-site` only, `--per-kind 16` (15 scored, 1 API error).
- Script: `scripts/exp-constraint-site-live.sh`. Results: `results-constraint-site-live.jsonl`,
  traces `traces/results-constraint-site-live-1786433164803/`.

Variant C recap: model is handed a rectangular REGION and must return any tile satisfying FOUR
constraints — defensible ∧ within-2-of-water ∧ not-enemy-territory ∧ spaced-≥`CITY_MIN_SPACING`
(3, Chebyshev) from every city — or certify `"none"`. `site_check(x,y,player)` reports only the
FIRST THREE (bundled) and explicitly NOT the spacing rule. So a model that trusts `site_check.ok`
alone will place cities too close and answer wrong on spacing-binding items.

## Result
- **Accuracy 15/15** (1.000, Wilson [0.80, 1.00]); 1 API error (not a wrong answer).
- Aggregate tool calls: **`site_check` 415 · `distance` 84 · `scan_region` 22 · `scan_grid` 19**.
- **`distance` calls are concentrated on the "some-answer" items** (a candidate coordinate is expected)
  and ~0 on most "none" items — the model spacing-checks exactly when `site_check` leaves survivors,
  and skips it when `site_check` already rules everything out. Correctly adaptive.

## Mechanism (the smoking gun)
On some-answer items the model's own trace shows the intended two-step reasoning, e.g.
`Basilios_74-7` (30 `distance` calls):
> "Candidates that passed site_check (defensible, within_water, not_enemy): (76,4), (77,4), …
> Let me check distances from these to the nearest cities. (76,4) to (76,20) = 16 — far enough …
> All distances are well over 3."

It enumerates cities from the **front-loaded roster**, then computes Chebyshev `distance` from each
`site_check`-passing candidate to every nearby city and applies the `≥3` rule ITSELF. This is the
exact opposite of reading off `ok`.

## Why the accuracy is *earned*, not luck
Spacing only ever REMOVES candidates. A model ignoring spacing would (a) falsely report a
too-close-but-`ok` tile as valid → wrong "some", and (b) never convert a would-be "none" correctly.
15/15 including all 9 none-items means it is not falsely accepting `site_check`-passing-but-too-close
tiles. The separate spacing step is doing real work.

## Caveat
One board, n=15, one model/seed. The mechanism is unambiguous in-trace, but the *rate* at which
weaker/less-agentic models fall for the shortcut is untested — a cross-model pass (e.g. Gemini, which
we know engages tools less; see the tool-engagement thread) would show whether Variant C's difficulty
is model-dependent. Directional, not a saturation claim.
