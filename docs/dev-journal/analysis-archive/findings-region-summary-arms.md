# Findings — interactive-maxops perception redesign (region_summary → scan_region, roster, tool consolidation)

**Date:** 2026-08-10. **Model:** DeepSeek V4 Flash, think-off. **Board:** `corpus_medium_a6_s1337-…-auto.sav`,
fog p1. **Config:** `--per-kind 8 --difficulty hard --cache on`. **Perception-active kinds only:**
forward-posting, fogged-assault, hidden-force, region-count, surprise-strike (constraint-site &
compare-two-attacks excluded — ~0 perception calls). n = 39/arm/seed unless noted. **Noise:** single-arm
Wilson CI ≈ ±0.13 at n=39; treat sub-0.10 accuracy gaps as ties.

Motivated by the owner's agenda items 1 (region_summary underused), 2 (scan_grid skepticism), 4 (roster
front-loading). Baseline from the 2026-08-07 smoke: **region_summary called 0 times** in 55
interactive-maxops trajectories; get_tile dominated (210 calls, 70% in forward-posting).

## Round 1 — 4-arm region_summary uptake (seed 1) — CONFOUNDED, see fix
A legacy desc · B enriched desc · C −scan_grid · D +roster (each adds one lever).

| arm | acc | get_tile | scan_grid | region_summary |
|---|---|---|---|---|
| A legacy | 0.82 | 239 | 126 | 13 |
| B enriched | 0.79 | 231 | 161 | 18 |
| C −scan_grid (confounded) | 0.82 | **534** | 32† | 49 |
| D +roster (confounded) | 0.82 | 186 | 27† | 66 |

†phantom. **The arbitrary-center region_summary redesign alone broke the zero** (A uses it 13× vs smoke's 0);
description enrichment (A→B) nudged it (13→18) but did not cut get_tile. **Confound discovered:** the
`--withhold` modifier filtered only the JSON tool array, NOT the prompt prose — the withheld tool was still
*described*, so the model tried to call it, failed, and thrashed. This inflated get_tile in arm C. Fixed by
scrubbing withheld tools from the assembled system prompt (preamble + overview).

## Round 2 — un-confounded C/D re-run
| arm | acc | turns | calls | get_tile | scan_grid | scan |
|---|---|---|---|---|---|---|
| C −scan_grid (fixed) | 0.74 | 6.79 | 17.4 | 236 | **0** | 101 |
| D +roster (fixed) | 0.79 | 5.62 | 13.0 | 159 | **0** | 88 |

- **Item 2 flipped.** The confounded "removing scan_grid explodes get_tile 231→534" was an artifact. Clean:
  get_tile stays ~236 (baseline); the model substitutes `scan`, not get_tile spam. Removing scan_grid is
  efficiency-neutral-to-positive with a possible small accuracy dip (0.82→0.74, within noise). "scan_grid
  clearly earns its place" is dead.
- **Item 4 confirmed.** Clean roster isolation (C-fixed→D-fixed, differ only by roster): get_tile −33%
  (236→159), turns −17%, calls −25%, accuracy flat-to-up. Roster-as-standard justified (magnitude −33%, not
  the confound-inflated −65%).

## The redesign (merged)
Split perception into two layers: **occupants front-loaded** (roster: units + cities with size/walls) and
**terrain on demand**. `region_summary` (occupants + coarse terrain) → **`scan_region`** (bounded 10×10
per-tile terrain+resource list, `[city]`/`[unit]` markers, Unknown tiles skipped but hidden-count in the
header). Roster made standard on interactive-maxops; quadrant "double-listing" of every city killed;
`list_cities`/`list_units` verbs removed. Scoped to interactive-maxops (plain `interactive` untouched → the
clean interactive-vs-raw arm survives).

## Round 3 — scan_region consolidation A/B (seeds 1 & 2)
A full menu (`scan_region`+`scan_grid`+`scan`+`get_tile`) · B `scan_region`+`scan_grid` (drop pinpoint) ·
C `scan_region` only. Roster front-loaded in all three.

| arm | acc S1 | acc S2 | pooled (n=78) | notes |
|---|---|---|---|---|
| A full | 0.79 | 0.79 | **0.795** | get_tile still 406/224 despite scan_region — habit crutch |
| B +scan_grid | 0.77 | 0.79 | 0.782 | |
| C scan_region only | 0.79 | 0.74 | 0.769 | |

All three within noise pooled. Decision came from **per-kind mechanism**, not the aggregate:
- **forward-posting — tool-neutral.** Uses get_tile heavily in A (189/124 calls) but *wastefully* (A acc 0.38/0.50).
  Pooled C = A (7/16 = 0.44); removing get_tile shifts work to scan_region + calculator ops with no accuracy
  change. get_tile is a crutch, not a lever.
- **hidden-force — scan_grid-dependent.** Consistent across both seeds: scan_grid present (A, B) → 8/8;
  scan_grid absent (C) → 7/8. Pooled A/B 16/16 vs C 14/16. Mechanistically clean — hidden-force is a
  wide-area fogged-region survey, scan_grid's unique niche, which bounded `scan_region` can't fully cover.

## Decision — B-style consolidation (merged, master `ae67625`)
Drop `get_tile` + `scan` (used heavily but wastefully), **keep `scan_grid`** (earns its place on
hidden-force's wide-area survey). interactive-maxops menu **19 → 17 tools** (`scan_region` + `scan_grid` +
15 ops). Execute logic for the dropped verbs retained for other surfaces + the run_tool parity gate.

## Takeaways
1. **region_summary uptake=0 was the old sector-form design + "qualitative" framing**, not model stubbornness;
   the arbitrary-center redesign fixed it. Description wording is a marginal lever.
2. **Withhold ablations must scrub prose, not just the tool array** — else the model thrashes on a phantom
   tool and confounds the result. (General harness lesson.)
3. **Roster front-loading is a real efficiency win** (−33% get_tile, −17% turns), accuracy-neutral.
4. **Tool value is kind-specific.** get_tile = universally over-used crutch that doesn't buy accuracy;
   scan_grid = niche but genuine value on wide-area fog kinds. Aggregate accuracy hides both.
5. **Models won't self-select the efficient tool** (get_tile 406 even with scan_region present) — you must
   remove the crutch; removal is accuracy-neutral.

## Caveats
- n=39/arm/seed, one board, 2 seeds — directional, not firm. Decisions rest on mechanism + efficiency +
  cross-seed consistency, not the (within-noise) aggregate accuracy.
- hidden-force's −1 for scan_region-only is 2 items total (16/16 vs 14/16); small but repeatable and
  mechanistically credible — the reason scan_grid was kept.

## Related (separate track)
constraint-site was redesigned to Variant C (region search + non-bundled spacing constraint) to defeat the
"call site_check and report" shortcut (agenda item 5) — merged; **needs a live run to confirm it actually
shifts model behavior** (not inferable offline).
