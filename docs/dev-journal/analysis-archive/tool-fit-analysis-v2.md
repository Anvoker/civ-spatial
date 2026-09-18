# Tool-fit analysis v2 — the WIDE historical + kind-complete structural pass

**Date:** 2026-08-08 · **Author:** second-pass tool-fit audit (worktree off master `f34f41d`).
**Purpose:** recommend a reduced, parity-safe tool roster using (1) a KIND-COMPLETE structural map of
what each solver actually needs and (2) a HISTORICAL usage aggregation across **all 30 trace runs / 1183
per-question traces**, not the single 7-kind smoke the v1 pass leaned on.

**Method / reproducibility.** All numbers below come from a Python walk of every subdir under `traces/`
(`scratchpad/aggregate.py` → `agg_out.txt`); each trace is a per-question JSON with `encoding`,
`category`, `result.status`, and per-turn `response.tool_calls`. Structural claims come from reading
`civ-eval/src/question.rs` (Question enum L121-316, `kind_id` L336-359, solvers) +
`civ-eval/src/generators.rs` (`all_kinds` L141-167, per-kind `generate`/scoring) +
`civ-eval/src/encoders.rs` (tool rosters). No source tool definitions were changed.

---

## 0. The roster under review (and how it EVOLVED)

The tool surface is not one fixed set — it grew over the runs, which is why single-run usage is
misleading. Three op groups, assembled per surface (`encoders.rs`):

- **Perception (6)** — `region_summary, scan, scan_grid, get_tile, list_cities, list_units`
  (`Interactive::tools` L1459-1522). Present on every *interactive* (query-loop) surface; **absent on
  raw** surfaces (raw dumps the whole board, so no fetch verbs).
- **Spatial primitives (4)** — `distance, travel_turns, count_terrain, count_resource`
  (`operator_tools` L1791-1830).
- **Maximal operators (15)** — `att_eff, def_eff, def_eff_base, win_prob, city_defense, city_fall_prob,
  garrison_defense, threat, site_axes, tile_cover, support, reach_turns, is_defensible,
  within_water_radius, not_enemy_territory` (`maximal_operator_tools` L2113-2263).
- **Enumeration group (4)** — `list_tiles, list_owned_cities, list_owned_units, travel_turns_from_owned`
  (`enumeration_operator_tools` L2715-2755). Enum surfaces only; `count_terrain`/`count_resource` are
  RETIRED when this group is present (subsumed by `list_tiles`, L2931/2942).

**Current maxops roster = 25 distinct tools** (`interactive-maxops` = 6+4+15; `raw-maxops` = 4+15 = 19,
no perception). Add the 4 enum tools and the full universe is **29** — the "~29 tools" of the brief.

**Era detection (critical — the maxops surface itself grew):**

| era | surface | #tools | evidence |
|---|---|---|---|
| **survivor era** (T1785…, 2026-08-02) | `raw-maxops` | **11** | `distance,travel_turns,count_terrain,count_resource,att_eff,def_eff,city_defense,garrison_defense,threat,site_axes,reach_turns` — NO `def_eff_base/win_prob/city_fall_prob/tile_cover/support/is_defensible/within_water_radius/not_enemy_territory` |
| **current era** (T1786…, 2026-08-07) | `raw-maxops` | **19** | full measurement set (survivor 11 + the 8 above) |
| **current era** | `interactive-maxops` | **25** | perception 6 + the 19 |

So 8 of the 15 maximal ops (`def_eff_base, win_prob, city_fall_prob, tile_cover, support, is_defensible,
within_water_radius, not_enemy_territory`) have usage evidence from **exactly one run** — the current-era
`results-smoke-nonsat-1786114275460` (n=111, 7 kinds). That single-run dependence is the main caveat on
every usage-only claim below and the reason the structural map does the load-bearing work.

---

## 1. Per-run historical aggregation (era + kind coverage + bias)

30 non-empty run dirs, 1183 traces. Grouped by what they actually exercise. Every run named; cite by dir.

| run dir | model | surfaces (era) | kinds covered | n | what it's good for |
|---|---|---|---|---|---|
| `rawcount` | DeepSeek | raw | terrain | 1 | (probe, ignore) |
| `results-tracediag-1785429227705` | DeepSeek | interactive | best-site, nearest-owned | 13 | perception verbs |
| `results-tracediag-1785429846953` | DeepSeek | interactive | best-site, nearest-owned, reachable-nearest, settle-site | 32 | perception verbs |
| `results-thinkon-gemini-smoke-1785485803697` | Gemini think | interactive | 6 basic kinds | 6 | perception verbs |
| `results-thinkon-gemini-int-1785486761310` | Gemini think | interactive | 6 basic kinds | 48 | **region_summary/scan_grid/get_tile** heavy |
| `results-thinkon-gemini-t677-p4-1785485928998` | Gemini think | raw | 6 basic kinds | 47 | raw baseline (no tools) |
| `results-frontier-smoke-1785487340235` | Opus 4.8 | raw | 6 basic kinds | 6 | raw baseline |
| `results-frontier-smoke-1785487603711` | GPT-5.6 | raw | 6 basic kinds | 6 | raw baseline |
| `results-frontier-k-*` (6 dirs) | GPT-5.6 | raw | one kind each: best-site, nearest-owned, reachable-nearest, region-count, settle-site, terrain | 8 each | raw baseline per-kind |
| `results-frontier-t677-p4-*` (3 dirs) | Opus/GPT | raw | 6 basic kinds | 48 each | raw baseline (high invalid) |
| `results-calc-scale-p2-*` (3 dirs) | DeepSeek | raw-ops(4)+interactive-ops(10) | nearest-owned, reachable-nearest, region-count | 48 each | **spatial primitives** (distance/travel_turns/count/region_summary) |
| `results-nearowned-filter-seed{1,2,3}` | DeepSeek | raw, raw-calc(5), raw-calc-enum(7) | nearest-owned | 24 each | **enum group + list_tiles** |
| `results-nearowned-ttfo-seed{1,2,3}` | DeepSeek | raw, raw-calc, raw-calc-enum(7) | nearest-owned | 24 each | **travel_turns_from_owned** (parity precedent) |
| `results-survivor-dense-seed{1,2,3}` | DeepSeek | raw, raw-calc(5), **raw-maxops(11)** | adv-assault-target, cf-vacate, compare-two-attacks, constraint-site, settle-site, triage-reinforce | 144 each | **combat/valuation ops** (att_eff/def_eff/city_defense/garrison_defense/threat/site_axes/reach_turns) |
| `results-smoke-nonsat-1786114275460` | DeepSeek | **raw-maxops(19), interactive-maxops(25)** | compare-two-attacks, constraint-site, fogged-assault, forward-posting, hidden-force, region-count, surprise-strike | 111 | **the only run with the CURRENT full 25-roster** (win_prob/city_fall_prob/tile_cover/def_eff_base/is_defensible/within_water_radius/not_enemy_territory) |

**Combined kind coverage across history (15 of 24 kinds have real usage):** terrain, region-count,
nearest-owned, reachable-nearest, best-site, settle-site, constraint-site, compare-two-attacks,
adv-assault-target, cf-vacate, triage-reinforce, fogged-assault, forward-posting, hidden-force,
surprise-strike.
**9 kinds have ZERO trace coverage:** adjacency, direction, distance, nearest, reachability,
unit-strength, city-defense, **t3-retreat**, **t3-threat**. These are covered by the structural map only.

**Per-run bias to respect:** (a) survivor's `raw-maxops` is the *11-tool* era — it could not call the 8
newer ops, so their absence there is not evidence; (b) the frontier kinds live in one 55-question
interactive-maxops slice; (c) most runs are DeepSeek-nothink, so usage reflects one model's tool
disposition (per `deepseek-gemini-tool-engagement`, agentic effort is model-specific).

---

## 2. KIND-COMPLETE structural map (the unbiased backbone)

For every registered kind (`all_kinds` L141-167), the ground-truth quantity its solver computes and the
tool(s) that MEASURE that same quantity at the solver's granularity. A tool listed here is **parity-required**:
withhold it and the model must brute-force or heuristic-shortcut the quantity (the `findings-tool-parity`
failure mode). Perception (`get_tile/scan/scan_grid`) is implicitly required on interactive surfaces for
every kind (locating entities); listed explicitly only where the *ground truth* leans on it (fog).

| kind (tier) | ground-truth quantity | parity-required tool(s) | trace usage confirms? |
|---|---|---|---|
| terrain (T0) | terrain of a tile | *(perception only)* | yes (get_tile) |
| adjacency (T0) | terrain of a neighbor | *(perception only)* | — no run |
| direction (T0) | compass dir a→b | *(perception only)* | — no run |
| distance (T1) | Chebyshev a→b | **distance** | — no run (tool used elsewhere) |
| nearest (T1) | min Chebyshev to a resource | **distance**, count_resource | — no run |
| region-count (T1) | count terrain in radius | **count_terrain** | yes (64 calls, →ceiling) |
| reachability (T1) | ≤budget 8-dir path avoiding terrain | **travel_turns** (approx) | — no run |
| unit-strength (T2) | context-free att/def compare | **att_eff, def_eff_base** | — no run |
| city-defense (T2) | best defender def_eff w/ terrain | **city_defense** | — no run |
| best-site (T1) | argmax count_terrain over candidates | **count_terrain** | yes |
| nearest-owned (T1) | min land-turns from NEAREST own city to unexploited resource | **travel_turns_from_owned** (+`list_tiles` exclusion) | yes — the parity precedent (§4) |
| settle-site (T3) | 4-axis dominance food/prod/resources/safety over work radius | **site_axes** (bundles all 4) *or* count_terrain+count_resource+**threat** | yes (site_axes 96 calls) |
| **t3-retreat (T3)** | 3-axis dominance **cover/safety/support** | **tile_cover** + **threat** + **support** | **NO run — support/tile_cover unexercised here** |
| forward-posting (T3) | pressure(Σatt_eff + Σcity_fall_prob) vs survival(1−max win_prob) | **att_eff, city_fall_prob, win_prob, reach_turns** | yes (all 4 called) |
| **t3-threat (T3)** | max city fall probability | **city_fall_prob** | **NO run** |
| reachable-nearest (T1) | nearest resource a unit reaches ≤horizon | **reach_turns** | yes (520 calls) |
| cf-vacate (T3) | remaining-defender vs threat on counterfactual board | **city_defense / garrison_defense + threat + win_prob(city_fall_prob)** | yes |
| adv-assault-target (T3) | ease(city_fall_prob) × value(size) | **city_fall_prob** + perception(size) | yes (via city_defense+threat in 11-tool era) |
| triage-reinforce (T3) | marginal flip capturable→held | **city_fall_prob + garrison_defense + win_prob** | yes (garrison_defense 47, reach_turns 66) |
| compare-two-attacks (T3) | exchange(win_prob) × threat-removed(att_eff) | **win_prob + att_eff** | yes (att_eff 144, win_prob 32) |
| constraint-site (T3) | is_defensible ∧ within_water ∧ not_enemy (conjunction + none-proof) | **is_defensible, within_water_radius, not_enemy_territory** | yes (63/58/55 once offered) |
| fogged-assault (T3) | worst-case-garrison capture prob under fog | **att_eff, city_defense, win_prob** + fog perception | yes (att_eff 23, city_defense 12) |
| surprise-strike (T3) | hidden ThreatField near own city (fog ≤5) | **threat, distance** + fog perception (scan_grid/count_terrain to read fog) | yes (distance 82, scan_grid 58, threat 3) |
| hidden-force (T3) | Σ hidden att_eff on fogged tiles per area | **att_eff** + fog perception (scan_grid/count_terrain) | partial — model reads fog via count_terrain(108)/scan_grid(45), rarely calls att_eff (§5) |

**Structural verdict:** *every one of the 25 maxops tools is parity-required by at least one kind.* There
is **no dead-weight tool** (unused AND not solver-required) in the maxops roster. The reduction levers are
therefore MERGES (redundant tools measuring co-required quantities) and one perception REDUNDANCY, not
deletions of unneeded capability.

---

## 3. Global historical usage (all surfaces, correctness of the questions each tool appeared in)

`calls` = total invocations; `q` = distinct questions it appeared in; status = outcome of those questions.

| tool | calls | q | correct/wrong/invalid/error | note |
|---|---|---|---|---|
| distance | 1692 | 198 | 128/56/12/2 | ubiquitous |
| travel_turns | 1629 | 188 | 115/58/15/0 | dispersed kinds |
| get_tile | 956 | 104 | 78/16/9/1 | perception |
| **region_summary** | 868 | 45 | 30/5/10/0 | **all on `interactive`/`interactive-ops`; 0 on any maxops surface** |
| scan_grid | 764 | 191 | 148/26/16/1 | perception workhorse |
| count_terrain | 749 | 127 | 101/23/3/0 | region-count/settle/fog |
| travel_turns_from_owned | 392 | 24 | 23/0/1 | parity precedent, near-perfect |
| count_resource | 324 | 129 | 82/34/12/1 | settle-site |
| def_eff | 265 | 98 | 85/7/5/1 | |
| att_eff | 246 | 81 | 62/14/4/1 | |
| reach_turns | 226 | 44 | 25/12/7 | |
| city_defense | 188 | 86 | 74/8/3/1 | |
| threat | 163 | 73 | 69/1/2/1 | |
| site_axes | 96 | 24 | 24/0/0 | settle-site, perfect |
| is_defensible | 95 | 24 | 18/2/3/1 | constraint-site (current era only) |
| list_units | 83 | 82 | 47/22/12/1 | |
| not_enemy_territory | 71 | 20 | 16/1/2/1 | constraint-site |
| list_cities | 70 | 70 | 47/11/11/1 | |
| list_tiles | 65 | 48 | 39/7/1/1 | enum |
| garrison_defense | 60 | 33 | 31/0/2 | cf-vacate/triage |
| within_water_radius | 58 | 16 | 15/0/1 | constraint-site |
| def_eff_base | 53 | 27 | 13/9/4/1 | |
| list_owned_cities | 48 | 48 | 39/7/1/1 | enum |
| scan | 47 | 16 | 9/5/1/1 | |
| tile_cover | 40 | 10 | 4/3/2 | forward-posting only |
| win_prob | 35 | 18 | 18/0/0 | perfect where used |
| city_fall_prob | 19 | 10 | 4/3/2 | forward-posting only |
| **list_owned_units** | **0** | 0 | — | **offered on raw-calc-enum, NEVER called** |
| **support** | **0** | 0 | — | **offered on both current maxops surfaces, NEVER called (t3-retreat, its only consumer, never ran)** |

---

## 4. What the WIDE view reveals that the narrow (7-kind) pass missed

1. **`region_summary` is dead ON THE MAXOPS SURFACE, but looks load-bearing globally (868 calls).**
   Every one of those 868 calls is on the calculator-free `interactive` (839) or `interactive-ops` (29)
   query loop. On `interactive-maxops` — the current roster — it is called **zero** times; the model uses
   `scan_grid`+`get_tile` instead (`scan_grid` 130, `get_tile` 210 there). `scan_grid` strictly subsumes
   `region_summary` (glyph grid + exact detail, no size cap vs a fixed-10×10 qualitative blurb). A
   narrow pass that saw 868 calls would call it essential; the wide, per-surface view shows it is
   redundant *specifically where the full roster lives*. **Best perception-side reduction.**

2. **`support` is not dead weight — it is UNEXERCISED.** Zero calls across all history, which the narrow
   view would read as "remove." Structurally it is the SUPPORT axis of **t3-retreat**, and t3-retreat
   appears in **no run** (and `support` only entered the roster in the current era). Removing it opens a
   parity gap the moment t3-retreat runs (identical shape to the `travel_turns_from_owned` precedent:
   the trap/safe-twin construction in `build_retreat_item` L1061 offers candidates with IDENTICAL cover
   AND support, so the model MUST measure support to separate them). Keep; it is the M3-deferred merge
   target, not a deletion.

3. **`list_owned_units` is the cleanest genuine REMOVE.** Zero calls even when offered (raw-calc-enum), and
   `findings-clean-ablation.md` (Finding 3) shows enumeration's whole accuracy story was a *pre-B1
   render-bug artifact*: once the render shows every stacked unit with its id, the model locates units
   from the board block and never needs `list_owned_units`. Enum buys efficiency, not accuracy — and this
   specific verb buys neither (the model prefers `list_units`/the render).

4. **The parity precedent generalizes to a live gap on the maxops surface.** `travel_turns_from_owned`
   (fixed nearest-owned 0.667→1.000, `findings-tool-parity.md` §Fix) lives on ENUM surfaces only. It is
   **absent from the 25-tool maxops roster.** No maxops run exercises nearest-owned, so this is latent —
   but if the maxops surface ever hosts an aggregate/multi-source kind (nearest-owned, or any "from
   nearest own city/unit" quantity), the same O(tiles×cities) blow-up recurs. Documented, not urgent.

5. **`att_eff` under-called on hidden-force is a MODEL shortfall, not a missing tool.** hidden-force needs
   Σ hidden `att_eff` on fogged tiles, but traces show the model reading fog with `count_terrain`(108)/
   `scan_grid`(45) and calling `att_eff` almost never — consistent with the frontier kinds being
   deliberately tool-RESISTANT (memory: "frontier resists maxops"). This argues AGAINST adding a
   fog-aware convenience tool (§6): the observability gap is the intended difficulty.

---

## 5. Ranked recommendations

Each: action · confidence · **evidence type** (structural / usage + which runs) · **parity risk**.

### REMOVE
| # | tool | conf | evidence | parity risk |
|---|---|---|---|---|
| R1 | **`list_owned_units`** (enum surfaces) | **High** | usage: 0 calls in all 6 `nearowned-*` runs that offered it; structural: render shows all unit ids post-B1 (`findings-clean-ablation` F3) | **Low** — no kind's ground truth needs a *unit index* the render doesn't already give |
| R2 | **`region_summary`** from maxops surfaces (keep on bare `interactive` if that surface is retained) | **Medium** | usage: 0/55 on `interactive-maxops` (nonsat), abandoned for `scan_grid`; structural: `scan_grid` strictly subsumes it | **Low** — perception only; no ground-truth quantity is unique to it. Caveat: evidence is one 55-q run |

### MERGE (already-decided M1/M2/M4 confirmed by this data; M3 re-affirmed deferred)
| # | merge | conf | evidence | parity risk |
|---|---|---|---|---|
| M1 | `def_eff_base` → **`def_eff`** (contextual w/ a "bare" flag) | High (decided) | both parity-required (unit-strength wants base, cf-vacate/compare want contextual); usage def_eff 265 / def_eff_base 53, co-used in compare-two-attacks | Low if the merged tool still exposes the context-free value; **High if the base value is dropped** (unit-strength/compare lose parity) |
| M2 | `is_defensible`+`within_water_radius`+`not_enemy_territory` → **`site_check`** | High (decided) | structural: the exact conjunction constraint-site scores; usage: always called together (63/58/55) in nonsat | Low — one kind consumes all three; a 3-field bundle preserves the none-proof. Keep fields separable in the return |
| M4 | `count_terrain`+`count_resource` → **`count`** | High (decided) | structural: settle-site/nearest/region-count use both; usage 749/324, co-used in settle-site | Low — a `kind` arg preserves both. Note enum surfaces already retire both for `list_tiles` |
| M3 | `tile_cover`+`support` → `retreat_axes` | **DEFERRED (hold)** | structural: t3-retreat's cover+support axes; usage: tile_cover 40 (forward-posting reuses cover), support 0 | **Medium** — `tile_cover` is ALSO used by forward-posting's survival axis, so a retreat-only bundle would strand that reuse. Do NOT merge until a t3-retreat + forward-posting run confirms the split of concerns |

### SPLIT / ADD
| # | action | conf | evidence | parity risk |
|---|---|---|---|---|
| S1 | **No split warranted.** `site_axes` (4-in-1) and `city_fall_prob` (threat∘odds) are composites, but each maps to exactly one kind's single decision; splitting would re-open the arithmetic the calculator exists to remove | High | structural: settle-site/t3-threat score the composite directly; usage: site_axes 24/24, win_prob 18/18 perfect | — |
| A1 | **Add `travel_turns_from_owned` to the maxops roster** IF nearest-owned (or any nearest-own-entity kind) is ever hosted there | Medium | structural: the proven parity gap (`findings-tool-parity` §Fix, 0.667→1.000); usage: 392 calls / 23-0 correct on enum surfaces | Adding is safe; the RISK is *not* adding it when the kind moves to maxops |
| A2 | **Do NOT add** a fog-aware tool (`count_fog`, `is_fogged`) for surprise-strike/hidden-force | High | structural + design: these kinds are intentionally tool-resistant (the hidden quantity is the difficulty); usage: model already proxies fog via count_terrain("Unknown") | Adding would DELETE the frontier kinds' discriminating power |

**Net effect:** remove 2 (`list_owned_units`, `region_summary`-from-maxops), merge 3 groups (7 tools → 3),
hold M3, no adds. Maxops surface 25 → ~19 distinct verbs without touching any kind's parity.

---

## 6. Safe-now (structural) vs needs-a-full-kind-run (usage-dependent)

**SAFE NOW — grounded in the kind-complete structural map, robust to the single-run usage caveat:**
- Execute **M1, M2, M4** merges (each is a lossless re-packaging of co-required, co-called tools; parity
  preserved as long as the merged tool keeps every field).
- **R1** remove `list_owned_units` (0 usage across 6 runs + render subsumes it structurally).
- **A2** keep the frontier fog gap open (design decision, not usage).
- Keep **`support`, `tile_cover`, `city_fall_prob`, `win_prob`, `threat`, `garrison_defense`** — each is
  parity-required by a kind whether or not a given run exercised it.

**NEEDS A FULL-KIND RUN before acting (usage-dependent, currently thin evidence):**
- **R2** remove `region_summary` from maxops — rests on one 55-question run; confirm with a full-kind
  interactive-maxops run (esp. the perception-heavy dispersed kinds) that `scan_grid` fully replaces it.
- **M3** tile_cover+support merge — needs a **t3-retreat** run (the only `support` consumer, never run)
  AND a forward-posting run in the same surface to confirm `tile_cover`'s dual use before bundling.
- **A1** travel_turns_from_owned on maxops — only if nearest-owned/nearest-own-entity kinds are added to
  the maxops slate; today they live on enum surfaces where the tool already exists.
- Any claim about the 9 **zero-coverage kinds** (adjacency, direction, distance, nearest, reachability,
  unit-strength, city-defense, **t3-retreat**, **t3-threat**): structural map only — no usage exists.

---

## 7. Bottom line

The wide historical view **overturns two narrow-pass reflexes and confirms the roster is already lean**:
`region_summary` looks essential (868 calls) but is dead where the full calculator lives; `support` looks
dead (0 calls) but is parity-load-bearing for an unrun kind. **No maxops tool is true dead weight** — every
one is required by some solver — so the reduction is 2 removes + 3 lossless merges (25→~19 verbs), with M3
and the region_summary remove gated on a t3-retreat / full-kind run. The one enduring risk is the reverse
of over-pruning: the `travel_turns_from_owned` parity precedent means any future aggregate kind added to
the maxops surface must bring its matching aggregate operator, or it will read as a model ceiling when it
is a tool-parity gap.
