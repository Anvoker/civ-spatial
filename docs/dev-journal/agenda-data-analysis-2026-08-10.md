# Agenda data analysis — 2026-08-10 (read-only, from existing traces)

**Source:** `traces/results-smoke-nonsat-1786114275460/` (110 trajectory JSONs + `run.json`).
**Surfaces present:** only `interactive-maxops` (55) and `raw-maxops` (55). There is **no plain
`interactive` surface in this run** — all interactive data is `-maxops`. `raw-maxops` dumps the
whole board, so it issues **zero perception calls** (get_tile/scan/scan_grid/region_summary); it
only calls calculators/predicates. **All perception-tool analysis below is `interactive-maxops`.**
**Kinds (interactive-maxops, 8 each except fogged-assault=7):** compare-two-attacks, constraint-site,
fogged-assault, forward-posting, hidden-force, region-count, surprise-strike.

**VINTAGE CAVEAT (applies throughout):** this run predates `88b2583` (region_summary → arbitrary-center;
here it is the OLD `RrCc` sector form) and `e68e242` (M1/M2/M4 merges; `site_check` did not exist,
constraint-site used the three separate conjuncts). Behavioral ratios can shift on a fresh run.

---

## Item 1 — get_tile vs region_summary usage

**Method:** counted every `tool_calls[].name` across all 55 interactive-maxops trajectories; broke
out perception verbs per kind. Confirmed `region_summary` **is** offered in the menu (it is tool #1
of 25 in every request's `tools` list) — so a zero count is a choice, not an absence.

**Headline numbers (interactive-maxops, all kinds pooled):**

| tool | calls |
|---|---|
| get_tile | 210 |
| scan_grid | 130 |
| scan | 26 |
| **region_summary** | **0** |
| list_units | 18 |
| list_cities | 14 |

- **get_tile : region_summary = 210 : 0** (ratio undefined / infinite).
- **all tile-verbs (get_tile+scan+scan_grid) : region_summary = 366 : 0.**

**Per-kind perception usage:**

| kind | get_tile | scan_grid | scan | region_summary | list_* |
|---|---|---|---|---|---|
| compare-two-attacks | 9 | 0 | 0 | 0 | 0 |
| constraint-site | 0 | 0 | 0 | 0 | 0 |
| fogged-assault | 23 | 6 | 4 | 0 | 11 |
| forward-posting | **147** | 13 | 22 | 0 | 15 |
| hidden-force | 13 | 45 | 0 | 0 | 6 |
| region-count | 4 | 8 | 0 | 0 | 0 |
| surprise-strike | 14 | 58 | 0 | 0 | 0 |

**Findings:**
- The model **never used region_summary once** in 55 trajectories despite it being available. The
  baseline the owner wants to move (region_summary up, get_tile down) currently starts at **zero
  region_summary**, so the target ratio can only improve.
- `forward-posting` is the get_tile sink: **147 of 210** get_tile calls (70%) live there — a single
  kind dominates the point-fetch behavior.
- scan_grid concentrates in `surprise-strike` (58) and `hidden-force` (45).

**Verdict / flag:** Pre-change baseline is **get_tile 210 : region_summary 0**. The redesign
(`88b2583`) plausibly changes whether the model reaches for region_summary at all, so the
*behavioral* ratio on a fresh run is **NOT inferable from this data — NEEDS A FRESH LIVE RUN** to
measure post-redesign uptake. What we *can* bank: (1) region_summary uptake was zero under the old
sector form; (2) get_tile reduction efforts should target `forward-posting` first.

---

## Item 2 — is scan_grid pulling its weight?

**Owner hypothesis:** scan_grid gives less info than region_summary → more turns.

**Method:** split interactive-maxops trajectories by whether they used scan_grid; compared mean
n_turns, mean tool-calls, and accuracy — pooled and per perception-kind.

**scan_grid footprint:** 194 total calls (130 scan_grid + related), used in 37/55 trajectories.
By kind: surprise-strike 8/8, hidden-force 8/8, region-count 8/8, forward-posting 8/8,
fogged-assault 5/7; **zero** in compare-two-attacks and constraint-site.

| group | n | mean turns | mean calls | accuracy |
|---|---|---|---|---|
| scan_grid USERS | 37 | 6.89 | 18.1 | 86% (32/37) |
| scan_grid NON-users | 18 | 3.89 | 10.1 | 89% (16/18) |

Per perception-kind (users vs non-users):
- fogged-assault: users 7.40 turns / 100% acc (5) vs non-users 7.50 turns / 0% acc (2)
- forward-posting: users **11.75 turns, 50.1 calls, 38% acc** (8); no non-users
- hidden-force: users 4.62 turns / 100% (8); no non-users
- region-count: users 3.25 turns / 100% (8); no non-users
- surprise-strike: users 7.62 turns / 100% (8); no non-users

**Why this CANNOT confirm or refute the hypothesis:**
1. **The comparison the hypothesis asks for is untestable here: region_summary was used 0 times**, so
   there is no scan_grid-vs-region_summary contrast to make. Only scan_grid-vs-nothing exists.
2. The pooled "users have more turns" (6.89 vs 3.89) is **fully confounded by kind**: the non-user
   group is dominated by the two easiest kinds (compare-two-attacks, constraint-site) that need
   little perception, while scan_grid users are exactly the perception-heavy kinds. Turns track
   task difficulty, not the tool.
3. Within a kind, scan_grid use is near-universal (mostly 8/8), so there is **no within-kind control
   arm** to isolate scan_grid's effect.
4. `forward-posting` is the one alarming kind (11.75 turns, 50 calls, 38% acc) — but there it is
   tangled with heavy get_tile (147) too, so scan_grid can't be blamed in isolation.

**Verdict: NEEDS-A-B (inconclusive from existing data; do not run it here).**

**Designed cheap A/B (do not execute — flagged NEEDS A FRESH LIVE RUN):**
- **Arms:** identical kinds/boards/items, two menus — (A) control (full menu), (B) scan_grid
  withheld from the menu (region_summary + get_tile + scan remain). Optional third arm (C) with
  get_tile *and* scan_grid withheld to force region_summary, to directly test the owner's real
  question (region_summary sufficiency).
- **Scope:** the 5 perception-heavy kinds only (fogged-assault, forward-posting, hidden-force,
  region-count, surprise-strike). Reuse the existing ~39 interactive-maxops items in those kinds.
- **Metrics:** mean n_turns, mean tool-calls, accuracy, wrong-answer rate; paired per item.
- **Decision rule (respect the fidelity-merge bar):** cut scan_grid only if arm B matches A on
  accuracy (within noise) AND does not raise turns — i.e. scan_grid is removable without cost.
  Keep it if withholding it drops accuracy or inflates turns.
- **Cost estimate:** ~39 items × 2 arms = 78 runs (×3 = 117 with arm C); at this run's ~5.9
  turns/item this is a single small smoke, well under an hour of local model time — cheap.

**Lean verdict:** **KEEP for now, but run the A/B** — current data cannot justify a cut, and the only
signal against scan_grid (forward-posting bloat) is confounded with get_tile.

---

## Item 5 — is constraint-site edifying, or "call 3 tools and read off"?

**Method:** parsed candidate coords from each `item_id`
(`constraint-site:<leader>:x0,y0,x1,y1,...`), counted conjunct-tool calls
(`is_defensible` / `within_water_radius` / `not_enemy_territory`), any other calls (perception,
geometry, calculators), distinct candidates probed, and whether the trajectory is "pure call+report"
(**only** the 3 conjunct tools, nothing else).

**Every constraint-site item has exactly 4 candidates.** Results (all 8 interactive-maxops):

| candidates | conjunct calls | other calls | perception | pure call+report? | correct? |
|---|---|---|---|---|---|
| 4 | 9–12 (mean 11.6) | **0** | **0** | **yes** | yes |

- **Pure call+report: 8/8 = 100%.** No trajectory made a single non-conjunct call — no perception
  (get_tile/scan/region_summary = 0), no distance/geometry, no candidate-to-candidate comparison.
- **7/8 ran the full brute grid** (3 tools × 4 candidates = 12 calls); 1 short-circuited slightly
  (9 calls / 3 candidates probed).
- **Accuracy 8/8 correct.** Expected answers split 4× "none" (no candidate satisfies all three) and
  4× a specific winner — but even the "none" case is just "call the tools on all four, observe all
  fail," not spatial search.

**Finding:** constraint-site as run is **pure tool-calling, not spatial reasoning** — it confirms the
owner's concern exactly. The task reduces to "call `is_defensible`, `within_water_radius`,
`not_enemy_territory` on each of the 4 given candidates and AND the booleans." The M2 merge
(`site_check` bundling all three) shortens the shortcut further: post-merge this becomes **1 call per
candidate (4 total)**, i.e. even less reasoning.

**Verdict: a HARDER variant is warranted.** Recommended difficulty levers (any/all):
1. **Real search, not a shortlist:** stop handing 4 candidates. Give a *region* and require the model
   to *find* a satisfying site (or prove none exists), forcing perception + geometry instead of
   4 predicate lookups.
2. **Decoy conjunct / over-provision candidates:** many candidates (e.g. 8–12) with several near-
   misses, so the model must actually rule out via geometry rather than exhaustively probe — and add
   a constraint that `site_check` does NOT bundle (e.g. a distance/adjacency condition) so a single
   merged call cannot settle it.
3. **None-of-them proof requiring search:** "none" answers should require demonstrating coverage of a
   space, not four failed lookups.

Any harder variant that relies on post-merge `site_check` behavior or the redesigned region_summary
**NEEDS A FRESH LIVE RUN to validate — do not infer** difficulty from this pre-change trace.

---

## Cross-cutting flags
- No plain-`interactive` surface exists in this run; everything interactive is `-maxops`.
- `region_summary` usage = 0 everywhere → Items 1 & 2 both blocked from measuring the
  region_summary contrast; both need a fresh post-`88b2583` run to establish the new baseline.
