# Findings — tool–task fit & the tool-parity gap (why "more tools" isn't the lever)

**Date:** 2026-08-02 · two linked results on the bug-fixed (post-B1) harness. Board: T677 `--fog 2` (98% explored,
dense late-game known map). DeepSeek V4 Flash, think-off. Data: `results-survivor-dense-seed{1,2,3}.jsonl`
(survivor experiment) + `results-nearowned-filter-seed{1,2,3}.jsonl` + `traces/` (failure analysis). Companion to
`findings-clean-ablation.md` (the dispersed-kind result this generalizes).

## TL;DR (the mini blog result)
Two findings that together say the lever is **tool–task fit**, not tool count:
1. **On decision kinds, generic spatial ops don't help — only the *matching* (combat/valuation) ops do.**
   no-ops (0.64) ≈ spatial-ops (0.61) ≪ maxops (0.92). Adding the wrong tools even *hurts* (more cap-outs).
2. **A tool surface must be parity-complete with the ground-truth solver, or the model pays an unaffordable
   composition cost and shortcuts to a wrong heuristic.** The residual `nearest-owned` errors are not a model-
   capability wall and not a harness bug — they are a **missing primitive**: the solver does a *multi-source* BFS
   from all own cities, but the exposed `travel_turns` is *point-to-point*, so matching it costs O(tiles × cities)
   tool calls the model can't afford.

---

## Result 1 — tool–task fit (survivor experiment)

6 decision kinds × 3 arms × 3 seeds, T677/p2 dense, think-off, n=24/kind/arm (n=144/arm total).

| kind | raw (no-ops) | raw-calc (spatial ops) | raw-maxops (full calc) |
|---|---|---|---|
| adv-assault-target | 0.62 | 0.65 | **1.00** |
| settle-site | 0.58 | 0.62 | **1.00** |
| cf-vacate | 0.83 | 0.83 | **0.96** |
| triage-reinforce | 0.58 | 0.33 | **0.92** |
| compare-two-attacks | 0.58 | 0.62 | **0.88** |
| constraint-site | 0.62 | 0.58 | **0.79** |
| **arm total (n=144)** | **0.64** | **0.61** | **0.92** |

Status: raw-maxops 133 correct / 9 wrong / 2 invalid. raw-calc 86 / 41 / **15 invalid** / 2 error (the spatial
calculator brute-forces decision kinds it can't express → cap-outs). Cost: raw-maxops $0.0067/item — *cheaper* per
item than raw-calc ($0.0085) because it stalls less. Dense survivor total spend: **$2.50**.

**The relation is not a monotonic ladder.** It is **no-ops ≈ spatial-ops ≪ maxops**:
- The spatial calculator (`raw-calc`) is *below* no-ops on decisions and racks up cap-outs (triage-reinforce
  collapses to 0.33). Generic spatial ops are the **wrong** tool for combat/valuation decisions.
- Only `raw-maxops` — with the combat/valuation ops that *match* the questions — wins (+0.28 over no-ops).

This is the counterpart to the dispersed-kind result (`findings-clean-ablation.md`), where those same spatial ops
*were* the right tool and lifted accuracy +0.29. **Same ops, opposite outcome, depending on whether they match the
question.** Tool–task fit is the lever, not tool count.

### Survivor sub-finding — which decision kinds stay discriminating under free measurement
- **Collapse / saturate to ceiling under maxops:** `adv-assault-target`, `settle-site` (→ 1.00); `cf-vacate`
  (0.96), `triage-reinforce` (0.92) nearly so. Free measurement makes these trivial.
- **Survive (still discriminating with full tooling):** `compare-two-attacks` (0.88), and **`constraint-site`
  (0.79) is the hardest** — the best candidate for a "still-hard-even-with-tools" headline.

Caveat: per-kind n=24 (~3-item resolution, noisy); arm totals (n=144) are solid. Noise floor ~12–18%/item.

---

## Result 2 — the tool-parity gap (nearest-owned failure analysis)

`nearest-owned` (nearest resource tile > Chebyshev-2 from any own city, answered as min land-path travel-turns from
the nearest own city, horizon 6, else "none") is the one kind the calculator arms don't crack. A verified trace
analysis of all **7 wrong `raw-calc-enum`** answers (solver re-implemented in Python from `question.rs:652-669` +
`rules.rs:790-843`, reproduced all 7 expected answers; every tool value the model received replayed against an
independent BFS and matched) found:

**All 7 are model reasoning/effort errors — not a harness bug.** `travel_turns` and the `list_tiles` exclusion
filter are faithful wherever called. The dominant failure (**6/7**) is **incomplete argmin**: the model measured a
handful of (tile × city) pairs and stopped, always **overshooting**. The values it *failed* to fetch were exactly
the low ones that flip the answer (e.g. tile (74,9) measured from two cities at 6 turns each, never from city
(77,10) which is 3). Sub-flavors: right tile / wrong origin city; fixating on a wrong "looks-closest" tile. Two
cases fell back to **Chebyshev-eyeballing** after exhausting the call budget; one skipped the exclusion filter and
made a Manhattan-vs-Chebyshev slip.

**Root cause = a tool-parity gap.** The ground-truth solver runs a **multi-source BFS from all own cities at once**
(`ReachField::from_origins`). The exposed tool surface has only **point-to-point** `travel_turns` and no
"distance-from-nearest-own-city" primitive. To match the solver the model must loop `travel_turns` over
O(eligible tiles × own cities) — ~30 × ~20–35 here — which blows its turn/call budget, so it guesses a nearest city
(wrong) or reverts to Chebyshev. **The tool surface is not parity-complete with the solver, and the model pays the
difference in an unaffordable composition it can't complete.**

### The general lesson (the mini blog point)
> When a question's ground truth is computed by an aggregate/multi-source operation, exposing only the
> point-wise primitive makes the task *look* solvable but forces an O(n·m) tool loop no budget-bounded model can
> run. It will shortcut to a heuristic and be wrong — and this reads as a "model capability" ceiling when it is
> really a **tool-ergonomics / parity** deficiency. Tool surfaces should expose primitives at the *same
> granularity the solver uses*.

This is the same principle as Result 1 from the other side: Result 1 = the tool must match the *question type*;
Result 2 = the tool must match the *solver's operation granularity*. Both are "tool–task fit."

### Fix (IMPLEMENTED + CONFIRMED)
Exposed the multi-source operation the solver already computes: `travel_turns_from_owned(player, tx, ty)` — min
land-turns from the player's nearest own city, backed by `ReachField::from_origins`, horizon baked in at 6 for
strict one-shot parity with the solver's inner op. Kept SPLIT from the exclusion filter so the outer argmin stays
under test. Added to the enum op group (`raw-calc-enum`, `raw-maxops-enum`, `interactive-maxops-enum`).

**Confirmation re-run** (nearest-owned, T677/p2, 3 seeds, DeepSeek think-off; enum arm carries the new tool, raw
and raw-calc are controls without it):

| arm | before (filter only) | after (+ travel_turns_from_owned) |
|---|---|---|
| raw (no-ops, control) | 13/24 = 0.542 | 14/24 = 0.583 |
| raw-calc (control, no new tool) | 12/23 = 0.522 | 16/24 = 0.667 |
| **raw-calc-enum (has new tool)** | **16/24 = 0.667, 7 wrong** | **23/23 = 1.000, 0 wrong** |

The residual hard kind is **solved**: `raw-calc-enum` went from 0.667 (7 wrong, all incomplete-argmin) to **perfect
(23/23)**, and it did so **cheaper** ($0.22 vs $0.27) and in **fewer turns** (4.0 vs 4.9) — the model stops
brute-forcing once the tool matches the solver's granularity. This is the mini blog result demonstrated end-to-end:
**close the tool-parity gap and the apparent "capability ceiling" disappears.**
