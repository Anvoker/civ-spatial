# scan_grid drop + budget-aware loop + a scan_region fog-fidelity fix (2026-08-11)

This is the full arc of a single investigation that changed direction twice under scrutiny. Bottom line:
**(1) shipped a budget-aware interactive loop; (2) found and fixed a `scan_region` fog-fidelity gap;
(3) dropped `scan_grid` → interactive-maxops is now a 16-tool surface.** DeepSeek V4 Flash, think-off,
interactive-maxops, fog p1, `--difficulty hard`, `--per-kind 8`, seed 1. Boards:
`corpus_medium_a6_s1337-T0200`, `corpus_large_a3_s42-T0220`. Arms: KEEP (scan_grid present) vs DROP
(`--withhold scan_grid`) × {medium, large}. Scripts `scripts/exp-scangrid-drop-budgetaware.sh`
(pre-fix, `results-sgd-*`) and `scripts/exp-scangrid-drop-fogfix.sh` (post-fix, `results-sgd2-*`).

## Finding 1 — budget-awareness works (SHIPPED as the new standard)
The interactive loop now (a) states the actual `max_turns` in the system prompt, (b) gives the priority
order **never-run-out > correct > efficient**, (c) stamps every tool result `[turn n/N]`. Clean test =
SAME tool set, old blind prompt vs new prompt, forward-posting on the LARGE board:

| | non-answers | err turn-counts | accuracy-when-answered |
|---|---|---|---|
| OLD blind (board2 arm B) | 3/8 | [11,13,13] | 4/5 = 0.80 |
| NEW budget-aware (LRG-KEEP) | **1/8** | [12] | 6/7 = 0.857 |

Turn counts shifted DOWN (large forward-posting median ~14→~12; nothing hits the cap of 16). It converted
budget-exhaustion non-answers into mostly-CORRECT answers, not rushed wrong ones. **Re-baseline accepted**
— prior interactive numbers carry an old-prompt asterisk. (`runner.rs` + `interactive_loop.rs` test.)

## Finding 2 — scan_grid's hidden-force edge was a scan_region FOG-FIDELITY GAP, not a real capability
Pre-fix, dropping scan_grid cost hidden-force accuracy 8/8→6/8 on BOTH boards, which read as "scan_grid
earns its place." But a matched per-item check (prompted by the owner: *was scan_grid actually used on
the items DROP got wrong?*) showed it was used on 3 of 4 discriminating items — and the mechanism trace
was decisive. On a hidden-force item (which fogged area hides the largest enemy force):

- **KEEP** (scan_grid) saw (38,22) as *"deep in enemy territory… fogged land tiles where enemy units
  could be massing"* → correct.
- **DROP** (scan_region only) saw the SAME area as *"mostly Ocean… nowhere to mass troops"* → wrong.

Root cause, confirmed in code (`scan_region_text`): `scan_region` treated any FOGGED tile as
`Unknown` and DROPPED it (`known = terrain != "Unknown" && !is_fogged`), so it listed only the visible
edge-ocean and reported *"77 tiles Unknown"*. `scan_grid` correctly showed the fogged tiles' terrain +
enemy territory (`Hills <territory Basilios> (fogged)`). Under three-state fog a fogged tile's
terrain/ownership ARE known — only live units are hidden (the whole hidden-force premise) — so
`scan_region` was discarding legitimately-known substrate. **A tool-preference number was really a
fidelity bug.**

## The fix + re-test
Fixed `scan_region` to LIST fogged-but-explored tiles (terrain/resource/owner/`[city]`, tagged
`(fogged)`, live units hidden); only never-explored tiles are omitted now. Verified live. Re-ran KEEP/DROP:

| board | hidden-force KEEP pre→post | hidden-force DROP pre→post | KEEP–DROP gap |
|---|---|---|---|
| medium | 8/8 → 8/8 | **6/8 → 8/8** | 2 → **0** |
| large | 8/8 → 7/8 | **6/8 → 7/8** | 2 → **0** |

**Post-fix, DROP matches KEEP on both boards.** scan_grid's hidden-force advantage was entirely the
scan_region gap; with scan_region fog-aware, scan_region-only reasons about hidden force just as well.

## Decision — drop scan_grid (16-tool interactive-maxops)
With the hidden-force justification gone, the only residual scan_grid signal was a large-board
forward-posting non-answer margin (KEEP 1–2 vs DROP 4) — small n and confounded with forward-posting
being intrinsically budget-marginal (a turn/token-budget matter, plausibly not a tool need). Owner
decision: **drop scan_grid now.** interactive-maxops / -enum menus lose scan_grid (execute/dispatch
retained for the plain surfaces + parity gate); `scan_region` is the sole terrain-perception verb.

## The honest trail
scan_grid read: keep (board1) → maybe-drop (board2, noise) → keep (this run, +2/8) → **drop** (once the
+2/8 was traced to the scan_region fog gap and fixed). The flip-flops were the process working: an n=8
accuracy delta is untrustworthy until you trace it to a mechanism. Here the mechanism turned out to be a
real fidelity bug, whose fix both improved the harness AND simplified the surface.

## Caveats
n=8/kind/arm, single seed. hidden-force parity post-fix (MED 8=8, LRG 7=7) is the load-bearing result;
the forward-posting convergence margin is unresolved (a higher-`max_turns` test would settle whether it
is a budget or a tool matter). The scan_region fog fix is a general fidelity improvement independent of
the scan_grid decision, and benefits every fog kind.
