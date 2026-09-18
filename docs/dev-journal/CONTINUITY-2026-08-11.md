# CONTINUITY — 2026-08-11 EOD

Dated handoff snapshot. Durable state + command reference live in `RESUME.md`.

## Headline
A live-validation + budget-awareness session. Confirmed **constraint-site Variant C** live; ran a
**2nd-board replication** of the scan_region consolidation; found and corrected a mechanism
mis-attribution; **implemented + shipped a budget-aware interactive loop** (up-front turn limit +
priority order + running `[turn n/N]` stamps); then a **scan_grid KEEP/DROP re-baseline** that reverses
the mid-session doubt and confirms **scan_grid stays** and **17-tool interactive-maxops stands**.
All code `just check` green; budget-aware prompt verified live in traces.

## What landed
| Item | Result |
|---|---|
| Housekeeping | 4 merged worktrees pruned + branches deleted; stray `analysis/new 174.txt` (a wrong paste of YouTube URLs) deleted. |
| **constraint-site Variant C — LIVE ✓** | 15/15; trace proves the model computes the hidden spacing constraint as a SEPARATE step (415 `site_check` + 84 `distance`, distance concentrated on some-answer items). The "call site_check and read off" shortcut is defeated. `analysis/findings-constraint-site-variantC-live.md`. |
| **2nd-board replication** | Large board `corpus_large_a3_s42-T0220`, scan_region A/B/C. Accuracy-when-answered neutral (replicates). Initially read as "tool removal raises non-answers"; drill-down CORRECTED it: forward-posting is budget-marginal at scale BY DESIGN (adversarial 1-ply search, no shortcut primitive; median 13/16 turns even with the full menu). `analysis/findings-scanregion-board2-replication.md`. |
| **Budget-aware interactive loop — SHIPPED** | `runner.rs`: system prompt now leads with the actual `max_turns`, the priority order (never-run-out > correct > efficient), and every tool result is stamped `[turn n/N]`. New test `system_prompt_states_budget_and_tool_results_are_turn_stamped`. `just check` green (verify-oracle 708). Verified live in traces (HARD LIMIT text + `[turn 1..N/16]` stamps present). |
| **scan_grid KEEP/DROP re-baseline** | 4 arms (KEEP vs `--withhold scan_grid` × medium/large) under the new prompt. `analysis/findings-scangrid-drop-budgetaware.md`. |
| **scan_region FOG-FIDELITY FIX** | `scan_region_text` was dropping fogged-but-explored tiles as "Unknown"; now LISTS them (terrain/owner/`(fogged)`, live units hidden). New test `scan_region_lists_fogged_tiles_with_terrain_and_territory_hiding_units`. Verified live. |
| **scan_grid DROPPED → 16 tools** | Removed from interactive-maxops/-enum menus + preambles/overview/CLI help; execute/dispatch retained for plain surfaces + parity gate. Menu tests updated. |

## The re-baseline + the fog fix (this is where the session's biggest result landed)
1. **Budget-awareness works (SHIPPED).** Same tool set, large-board forward-posting: non-answers 3→1,
   turn median ~14→~12, cap never hit; accuracy-when-answered held (6/7 vs 4/5). Converted non-answers
   into mostly-CORRECT answers, not rushed wrong ones. New standard (re-baseline accepted — prior
   interactive numbers carry an old-prompt asterisk).
2. **scan_grid's hidden-force edge was a scan_region FOG-FIDELITY GAP → fixed → scan_grid DROPPED (16
   tools).** Pre-fix, dropping scan_grid cost hidden-force 8/8→6/8 both boards ("earns its place"). The
   owner pushed: *was scan_grid actually USED on the items DROP got wrong?* Matched-item + trace analysis:
   used on 3/4, and the mechanism was decisive — KEEP saw a fogged area as "deep enemy territory, fogged
   land to mass on"; DROP saw the SAME area as "mostly ocean, nowhere to mass." Root cause (code-confirmed,
   `scan_region_text`): scan_region treated any FOGGED tile as `Unknown` and dropped it
   (`known = terrain!="Unknown" && !is_fogged`), discarding legitimately-known fogged terrain/territory.
   **Fixed** scan_region to LIST fogged tiles (terrain/owner/`(fogged)`, live units hidden; only
   never-explored omitted). **Re-test: DROP now MATCHES KEEP on hidden-force both boards (MED 8=8, LRG
   7=7)** → scan_grid earns nothing → **dropped** from interactive-maxops/-enum (execute retained for the
   plain surfaces + parity gate). The scan_grid read went keep→maybe-drop→keep→**drop**; the flip-flops
   were the process — an n=8 accuracy delta is untrustworthy until traced to a mechanism, and here the
   mechanism was a real fidelity bug whose fix improved the harness AND simplified the surface.

**Open thread:** the large-board forward-posting non-answer margin (KEEP 1–2 vs DROP 4) is unresolved —
likely a `max_turns` budget matter (forward-posting is budget-marginal by design), not a scan_grid need.
A higher-budget forward-posting run would settle it.

## Caveats
n=8/kind/arm, single seed throughout. The load-bearing result is hidden-force PARITY post-fix (DROP=KEEP,
both boards) — that decided the scan_grid drop; the forward-posting convergence margin stays open. The
scan_region fog fix is a general fidelity win, independent of the scan_grid decision. constraint-site is
one board/model.

## Next session (owner to prioritize)
1. **Cross-model constraint-site** — does a weaker-tool-engagement model (Gemini) fall for the site_check
   shortcut Variant C defeats? Tests whether the difficulty is model-dependent.
2. **Higher-`max_turns` forward-posting run (large board)** — settles the one open thread: if the DROP
   non-answer margin (KEEP 1–2 vs DROP 4) vanishes with a bigger budget, it was budget, not scan_grid
   (confirming the 16-tool drop was clean). Also firms frontier-kind measurability at scale.
3. **Blog headline #3 (raw-maxops-enum vs interactive-maxops-enum) — DEPRIORITIZED** (see RESUME). The
   current, nearly-complete story is tool–task fit + frontier-resistance + won't-self-select + now
   budget-awareness; #3 is a cost-frontier appendix, budget-permitting.
4. Frontier kinds (forward-posting, fogged-assault) may want a higher `max_turns`/`token_budget` at
   large-board scale to be fully measurable — orthogonal to the tool menu.

## Files (this session)
Code: `civ-eval/src/runner.rs` (budget-aware loop), `civ-eval/tests/interactive_loop.rs`,
`civ-eval/src/encoders.rs` (scan_region fog fix + scan_grid drop + 2 tests), `civ-cli/src/main.rs` (help).
Findings: `analysis/findings-constraint-site-variantC-live.md`,
`analysis/findings-scanregion-board2-replication.md`, `analysis/findings-scangrid-drop-budgetaware.md`.
Scripts: `scripts/exp-constraint-site-live.sh`, `scripts/exp-scanregion-abc-board2.sh`,
`scripts/exp-scangrid-drop-budgetaware.sh`, `scripts/exp-scangrid-drop-fogfix.sh`.
