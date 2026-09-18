# scan_region A/B/C — 2nd-board replication (2026-08-11)

**Board:** `corpus_large_a3_s42-T0220-Y01595-auto.sav` (late, LARGE — biggest size-axis contrast to the
original medium board). Same arms/kinds/fog/seed as `exp-scanregion-abc.sh`.
DeepSeek V4 Flash, think-off, interactive-maxops, fog p1, `--difficulty hard`, `--per-kind 8`.
Script `scripts/exp-scanregion-abc-board2.sh`; results `results-sr-board2-{A-full,B-safe,C-scanregiononly}.jsonl`.

Arms: **A** full (scan_region+scan+scan_grid+get_tile) · **B** safe (drop get_tile+scan) ·
**C** bold (also drop scan_grid → scan_region only).

Original (medium) board = `findings-region-summary-arms.md`: pooled acc A 0.795 / B 0.782 / C 0.769;
"get_tile a wasteful crutch (pooled C=A)"; "scan_grid earns its place on hidden-force (−1/8 without it)".

## Headline
Accuracy-neutrality of the consolidation **replicates for accuracy-when-answered**. The large board
reveals two things the medium board did not: **(1) scan_grid's hidden-force accuracy justification does
NOT replicate; and (2) forward-posting becomes budget-marginal at large-board scale — but the drill-down
shows this is INTRINSIC to the frontier kind's 1-ply adversarial search, NOT caused by the tool
consolidation** (the removed tools were barely/never used; see Finding 2). Net: the consolidation is not
implicated; the scan_grid justification is weakened.

## Per-arm (n=8/kind; surprise-strike yielded 0 decisive instances on this board)
| kind | A-full | B-safe | C-only | note |
|---|---|---|---|---|
| region-count | 8/8 | 8/8 | 7/8 (+1 invalid) | flat |
| hidden-force | 7/8 | 7/8 | **8/8** | scan_grid removal did NOT hurt |
| forward-posting | 5/7 (**1 err**) | 4/5 (**3 err**) | 3/3 (**5 err**) | errors ↑ as tools removed |
| fogged-assault | 0/3 | 0/3 | 0/3 | frontier-resist kind, n=3, uninformative |
| **aggregate** | 0.769 (n26) | 0.792 (n24) | 0.818 (n22) | rising, CIs overlap |

> **⚠️ SUPERSEDED (2026-08-11):** Finding 1 below ("scan_grid's hidden-force niche does NOT replicate")
> was based on a single n=8 arm under the blind-budget prompt. A follow-up 4-arm KEEP/DROP run on BOTH
> boards under the budget-aware prompt REVERSES it — scan_grid helps hidden-force +2/8 on each board AND
> cuts forward-posting non-answers (KEEP 1 vs DROP 4). The lone 8/8-without-scan_grid here was
> noise-floor luck. **scan_grid stays KEPT.** See `findings-scangrid-drop-budgetaware.md`.

## Finding 1 — scan_grid's hidden-force niche does NOT replicate [SUPERSEDED — see note above]
Medium board kept scan_grid solely because hidden-force lost 1/8 without it (16/16→14/16 across seeds).
On the large board hidden-force is **7/8 (A) / 7/8 (B) / 8/8 (C)** — removing scan_grid did not hurt;
scan_region alone handled the wide-area fog survey (8/8, scan_region=31 calls, no thrash). The medium
board's −1 looks like noise. **The single accuracy-based justification for keeping scan_grid is weakened.**

## Finding 2 — forward-posting is budget-marginal at large-board scale BY DESIGN (not a tool-removal effect)
forward-posting non-answers rise A=1 → B=3 → C=5 (of 8) — empty `err`, empty `got`, `n_turns` 11–15
(max_turns=16): budget-exhaustion non-answers, not API errors. **But drilling into the mechanism shows
tool removal is NOT the dominant cause** (this corrects the first draft of this doc):

- **It is budget-marginal even with ALL tools.** Arm A (full menu) forward-posting already burns a
  **median 13/16 turns** ([12,12,12,13,13,13,13,14]); B median 13; C median 14. It sits at the cliff
  edge before any tool is dropped.
- **The removed tools were barely/never used.** `get_tile`=0 and `scan`=0 across ALL arms including A —
  so the A→B jump (1→3) cannot be causal (dropping unused tools). `scan_grid` was used only 1–3×/traj.
  n=8 makes 1-vs-3 pure noise; only scan_grid (B→C) is a plausible thin contributor.
- **Why the kind is intrinsically expensive:** scoring is an adversarial 1-PLY search — per candidate
  tile, which enemies come into attack reach (PRESSURE) and how hard can the enemy reposition to punish
  (SURVIVAL). The kind is designed so the static `threat`/ThreatField **structurally under-reads** this,
  so the model cannot shortcut it — it must iterate `reach_turns`/`win_prob`/`att_eff`/`distance`
  **per enemy × per candidate**. On the large board (more enemies, wider candidate spread) that
  O(candidates×enemies) manual search nearly exhausts the 16-turn budget on its own.

**Corrected conclusion:** the large-board non-answers are a **max_turns/token-budget phenomenon for an
intrinsically expensive frontier kind**, not evidence the consolidation hurts. This is the
frontier-resistance design working as intended: no primitive collapses the 1-ply search, so at scale it
surfaces as non-convergence rather than wrong answers. The fix (if we want to measure forward-posting at
scale) is a higher turn/token budget for this kind, NOT keeping more perception tools.

## Finding 3 — the get_tile "crutch" claim is board-specific
On the large board the model **never used get_tile even when offered** (arm A get_tile=0); it preferred
scan_region/scan_grid for wide-area work. The medium board's "get_tile is a wasteful crutch (406 calls)"
was board-specific behavior; on a large board get_tile isn't the tool of choice at all.

## Implication for the B-style consolidation (17 tools)
**The consolidation is NOT implicated on the large board.** Accuracy-when-answered is neutral (replicates
board1), and the non-answer rise on forward-posting is an intrinsic frontier-kind cost, not a tool-count
effect (Finding 2). The one real update: **scan_grid's sole accuracy justification (hidden-force −1/8)
did not replicate** — scan_grid may be droppable, making a "scan_region-only" (16-tool) surface worth a
confirmatory look. Separately, the frontier kinds (forward-posting, fogged-assault) may need a **higher
max_turns/token_budget at large-board scale** to be measurable, independent of the tool menu. This
mirrors the earlier scale caveat (raw-ops 0.986 small → 0.736 large; a second bottleneck at scale).

## Caveats
n=8/kind, single board, single seed; API/convergence errors reduce effective n; CIs wide (all aggregate
CIs overlap). surprise-strike=0 and fogged-assault n=3 are uninformative here. Directional — the
mechanism (empty-err budget-exhaustion, monotone with tool removal) is the evidence, not the aggregate.
A 2nd seed on this board and a mid-size board would firm up Findings 1 & 2.
