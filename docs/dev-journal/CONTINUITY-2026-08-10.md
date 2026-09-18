# CONTINUITY — 2026-08-10 EOD

Dated handoff snapshot. Durable state + command reference live in `RESUME.md`. Full experiment writeup:
`analysis/findings-region-summary-arms.md`.

## Headline
A long perception-surface session on **interactive-maxops**, all merged to master (`69b1197`), every
merge `just check` green (clippy · tests · verify-oracle ×4). Split perception into an **occupant layer
(front-loaded roster)** and a **terrain layer (`scan_region`)**, then consolidated the terrain tools
**B-style** (dropped `get_tile`+`scan`, kept `scan_grid`). Also: constraint-site hardened to Variant C,
a real `--withhold` confound found+fixed, and a viewer batch.

## What landed today (all on master, all `just check` green)

| Commit(s) | Change |
|---|---|
| `0988c65`,`0ab4025` → `a989c2f` | **Viewer batch 1** — tool-call analytics relocated to the bottom of Aggregated Stats (per-side in Compare); new **cross-turn repeat-call** metric (% of questions where a tool is called in ≥2 DIFFERENT turns); Prev/Next qnav moved under the Questions list. |
| `dd10933`…`c2c755b` → `41d8d83` | **Viewer batch 2 (compactness)** — Aggregated Stats to a fixed **2-col** grid via a container query (2→1 col under 600px panel width, works in Compare), density trims, and a **6th "Tool calls per question"** stat block. |
| `241966e` → `5d4eb2b` | **constraint-site Variant C** — region search (not a 4-tile shortlist) + a non-bundled **spacing** constraint (≥3 Chebyshev from every city, `citymindist`) that `site_check` does NOT report → defeats the "call site_check and read off" shortcut. Oracle-100% held. **NEEDS A LIVE RUN to confirm it shifts behavior** (not inferable offline). |
| `704b49b`,`d040158` → `437638b` | **Experiment arm machinery** — enriched `region_summary` description (+legacy toggle), dropped `list_owned_*` from the interactive-enum family, added composable `--legacy-rs-desc` / `--withhold` / `--frontload-roster` modifiers. |
| `44a45cb` | Data-analysis docs (agenda items 1/2/5) + constraint-site redesign proposal. |
| `548ff64` → `f1ef766` | **Withhold prose-scrub FIX** — `--withhold` was filtering only the JSON tool array, leaving the tool still *described* in the prompt → the model tried to call a phantom tool and thrashed, confounding the ablation. Now scrubbed from preamble+overview (test asserts zero mentions). |
| `211eec4` → `64074f6` | **Perception redesign** — roster front-loaded as STANDARD on interactive-maxops (cities carry size+walls), quadrant **double-listing killed**, `region_summary`→**`scan_region`** (bounded 10×10 per-tile terrain+resource list, `[city]`/`[unit]` markers, Unknown tiles skipped but hidden-count always in the header), `list_cities`/`list_units` verbs removed. Scoped to interactive-maxops; plain `interactive` untouched. |
| `a827086`,`c4477a5` → `ae67625` | **Terrain-tool consolidation (B-STYLE)** — interactive-maxops **19→17 tools**: dropped `get_tile`+`scan`, kept `scan_grid`+`scan_region`. Execute logic for dropped verbs retained for other surfaces + the parity gate. |
| `69b1197` | Findings doc + experiment scripts. |

## The experiment results (DeepSeek, corpus_medium board, fog p1, n=39/arm/seed; `findings-region-summary-arms.md`)
- **region_summary uptake=0** (smoke) was the OLD sector-form design, not stubbornness — the arbitrary-center
  redesign broke it; description wording is a marginal lever.
- **`--withhold` confound:** the confounded read said "removing scan_grid explodes get_tile 231→534." Un-confounded
  it's flat (~236); the model just uses `scan` instead. Item-2 verdict FLIPPED from "scan_grid clearly earns its
  place" to ambiguous.
- **Roster front-loading:** real efficiency win — clean isolation −33% get_tile, −17% turns, accuracy-neutral.
- **scan_region 3-arm A/B (2 seeds):** pooled acc A 0.795 / B 0.782 / C 0.769 (all within noise). Decision came
  from PER-KIND mechanism: **forward-posting** uses get_tile heavily but *wastefully* (pooled C=A) → drop it;
  **hidden-force** consistently loses 1/8 without scan_grid (wide-area fog survey, scan_grid's niche) → keep it.
- **Model won't self-select the efficient tool:** 406 get_tile calls even with scan_region available. Removing the
  crutch is accuracy-neutral.

## Caveats
n=39/arm/seed, one board, 2 seeds — directional. Decisions rest on mechanism + efficiency + cross-seed
consistency, not the within-noise aggregate accuracy. hidden-force's −1 is 2 items (16/16 vs 14/16), small but
repeatable — the reason scan_grid was kept.

## Next session (owner to prioritize)
1. **constraint-site Variant C live run** — confirm it actually moves models off call+report (offline can't tell).
2. **The blog headline** (§THE BLOG PLAN #3: raw-maxops-enum vs interactive-maxops-enum) — still zero data,
   biggest open work; needs a budget top-up.
3. **Replication** — the scan_region/consolidation findings on a 2nd board before any write-up.
4. Housekeeping: 4 old worktrees remain (pre-session, on merged branches); untracked at root are pre-existing
   (`analysis/new 174.txt`, `package-lock.json`, `smoke-nonsat-export.json`) + a scratch `arm-D-roster-export.json`.
