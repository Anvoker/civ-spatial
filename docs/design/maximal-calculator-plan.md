# Maximal-calculator implementation plan

**Status:** implementation plan + build log, 2026-07-31. The next step past the BASIC
primitives-only calculator (`raw-ops`/`interactive-ops`, `analysis/findings-calc.md`). Expose the
**full measurement operator set** so the corpus tests spatial **decisions/judgments over exact
measurements** rather than mental arithmetic — the deliberate thesis shift of
`interactive-compute-forks-design.md` §7 and `maximal-calculator-corpus.md`.

The design premise (corpus §"survival test"): **a maximal calculator is a STRICTLY HARDER filter
than the basic one.** Making every measurement free (threat, def_eff, site axes, …) collapses every
kind whose answer *is* a measurement (single-axis argmax, reach-race, best-site) and keeps only the
kinds whose answer needs something **no measurement returns** — the (S) withheld-weighting,
(C) board-counterfactual, (P) perspective, (I) predicate-composition, (J) partial-order survivors.
We control that boundary by choosing **which SELECTION operator to withhold** for each kind.

---

## (a) Full operator inventory

Every operator is a deterministic MEASUREMENT over the **frozen board**. Each reuses the *same*
`rules.rs`/`geometry` function the ground-truth solver calls, so the calculator is exactly faithful
to the scored answer (the measurement-parity gate, §(d)). Operators are addressed **by coordinate**
(consistent with the existing box operators and with how the raw board block prints every unit/city
with its `(x, y)`), never by an opaque id.

### Basic tier — already shipped (`operator_tools()` in `encoders.rs`)

| operator | signature | backing function | returns |
|---|---|---|---|
| `distance` | `(x0,y0,x1,y1)` | `geometry::chebyshev` | king-move tiles |
| `travel_turns` | `(x0,y0,x1,y1)` | `rules::ReachField` (move 1, whole-board horizon) | land turns / `unreachable` |
| `count_terrain` | `(x0,y0,x1,y1,terrain)` | `encoders::op_count_terrain` (= `geometry::tiles_within` filter) | int |
| `count_resource` | `(x0,y0,x1,y1,resource)` | `Tile::has` membership (gated by `is_resource`) | int |

### Maximal tier — new (`maximal_operator_tools()`)

| operator | signature | backing `rules.rs` fn | returns | which kind it feeds |
|---|---|---|---|---|
| `att_eff` | `(x,y)` | `rules::att_eff(unit@xy)` | attack strength (f64) | compare-two-attacks (both axes) |
| `def_eff` | `(x,y)` | `rules::unit_def_on_own_tile(unit@xy)` | contextual defense of the unit where it stands | compare-two-attacks (favorability divisor); cf-vacate (per-garrison) |
| `city_defense` | `(x,y)` | `rules::city_defense(city@xy)` | best defender's `def_eff` incl. terrain/center/walls; `0` undefended | cf-vacate, adv-assault-target, triage-reinforce (defense axis) |
| `garrison_defense` | `(ux,uy,cx,cy)` | `rules::def_eff_if_garrisoned(unit@uxy, city@cxy)` | the `def_eff` the unit@(ux,uy) *would* have garrisoning city@(cx,cy) | triage-reinforce (the after-defense) |
| `threat` | `(x,y,player)` | `rules::ThreatField::compute(board,player).at(x,y)` | scariest incoming enemy-LAND force at the tile, reach-faded | cf-vacate, adv-assault-target, triage-reinforce, settle-site (safety) |
| `site_axes` | `(x,y,player)` | `rules::site_axes(board,player,(x,y))` | the four settle axes `food / production / resources / safety` over the working radius | settle-site (all four axes) |
| `reach_turns` | `(ux,uy,tx,ty)` | `rules::ReachField` at the unit@(ux,uy)'s own `move_rate` | turns for THAT unit / `unreachable` | general (reachable-nearest parity); unit-scaled reach |

Notes:
- `att_eff`/`def_eff`/`city_defense`/`garrison_defense`/`reach_turns` return `error:` if no
  unit/city stands at the named coordinate (never a panic; the model can retry) — the same
  discipline as the basic operators' bounds checks.
- `threat`/`site_axes` take the `player` whose perspective is asked (its enemies are the threat);
  the raw board prints every city's `owner`, so the model has the player token.
- Floats are printed to 3 decimals; the **parity gate tests the f64 functions**, not the strings,
  so formatting can never corrupt a scored comparison.

### What is deliberately WITHHELD (the selection operators)

No operator returns a **choice, a dominance verdict, a frontier, or a counterfactual re-scoring**:
no `best_site`, `most_threatened`, `dominates`, `can_vacate`, `assault_target`, `attack_axes`,
`decisively_dominates`, `city_defense_vacated`, `capture_state`, or constrained/none-proof search.
Those are exactly the "choose-among-K / judge" steps that make a kind survive (corpus §3, §7). The
maximal calculator hands over every **input** to the judgment and withholds the **judgment**.

---

## (b) How the operators are exposed — the maximal tool-loop mode(s)

**Decision: add two surfaces, `raw-maxops` and `interactive-maxops`**, mirroring the existing
`raw-ops` / `interactive-ops` pair, rather than a flag on the basic surfaces.

Justification:
- **Continuity + faithful comparison.** The basic run established `raw-ops` (full board + compute,
  think-off) as the winning substrate (0.986) and `interactive-ops` as the over-fetch-bound control.
  Keeping the same *two* surfaces, now with the maximal operator set, preserves the
  **perception × compute** factorial (`interactive-compute-forks-design.md` §2) and lets a maximal
  run be read directly against the basic one — same board access, strictly more compute.
- **`raw-maxops` is the primary arm.** The surviving decision kinds are *global* judgments over the
  whole board (which of my cities is most exposed; is this site sound); full-board perception is the
  right substrate, exactly as the basic finding showed compute only pays off on full perception.
- **A flag would muddy the encoding label** in results/traces; a distinct `--encoding` name keeps
  every arm self-describing (the CLI already routes queryable surfaces by name).

Both surfaces share one implementation seam:
- `maximal_operator_tools()` returns basic + maximal `ToolDef`s (so `raw-maxops` exposes the whole
  calculator and no fetch verbs; `interactive-maxops` = the interactive fetch verbs + the whole
  calculator).
- `execute_maximal_operator(board, call)` tries the basic operators first (`execute_operator`), then
  the maximal ones; `None` falls through (to the "unknown tool" error for `raw-maxops`, or to the
  interactive fetch verbs for `interactive-maxops`).
- `reconstruct_via_tools` reuses `RawEncoder`'s decoder (`raw-maxops`, board is the overview) /
  `Interactive`'s tiling gate (`interactive-maxops`) — the operators carry no per-tile facts of
  their own, so completeness is inherited unchanged.
- Each surface's `system_preamble` documents the full operator menu (basic doc + maximal doc).

Registered in `queryable_surface()` and `QUERYABLE_SURFACES`; the CLI, runner (`run_interactive`),
and `rules_block` plumbing already handle *any* queryable surface generically — the runner renders
the question with `RawEncoder` and injects `rules_block` for T2/T3 items — so **no per-kind wiring is
needed beyond registering the surface.** Oracle-only paths are untouched (queryable surfaces need a
live model; `verify-oracle` and the Oracle-100% invariant run on static encoders only).

---

## (c) Per-decision-kind survival check

For each kind: the tag, the operators it may FREELY call, the SELECTION operator that stays
WITHHELD, and why the residue is genuine judgment no measurement returns.

| kind | tag | freely measurable now | WITHHELD selection operator | survives? why |
|---|---|---|---|---|
| **settle-site** | (S) | `site_axes(x,y,player)` gives all four axes per candidate; `threat`, `count_terrain`, `count_resource` | `best_site` / `dominates` / any weighting | **Yes.** With the four axes free, the residue is the **withheld weighting** — the Pareto/non-dominated judgment over food×prod×resources×safety. No operator returns "sound" or a scalar site score. |
| **cf-vacate** | (C) | `city_defense(x,y)` (current garrison), `def_eff(x,y)` per garrison unit, `threat(x,y,player)` | `city_defense_vacated` / `can_vacate` | **Yes.** The calculator only reads the *frozen* board. "Defense after the best defender is removed" is a **board-counterfactual** — the model must find the best & second-best garrison `def_eff`, drop the best, and compare the remainder to `threat`. No operator points at that non-existent board. |
| **adv-assault-target** | (P) | `threat(x,y,player)` (incoming), `city_defense(x,y)` (defense) per own city | `assault_target` / frontier / `city_threat_axes`→scalar | **Yes.** Both axes are free, but the model must **re-point** them to the *attacker's* polarity (high incoming ↑, low defense ↓ = attractive) and read the **non-dominated frontier**. No operator says "attractive target" or scalarizes the two axes. |
| **triage-reinforce** | (S, allocation) | `threat`, `city_defense` (before), `garrison_defense(ux,uy,cx,cy)` (after) per city | `capture_state` / the flip verdict / which-city-to-send | **Yes.** Even with before- and after-defense free, the answer is the city where the reserve **FLIPS capturable→held** — a **marginal-benefit** judgment over a margin threshold, per city, then a select. A doomed or already-safe city is wrong however exposed; no operator returns the flip. |
| **compare-two-attacks** | (S, comparative) | `att_eff(x,y)`, `def_eff(x,y)` for attackers/targets → favorability = `att_eff(A)/def_eff(T)`, threat-removed = `att_eff(T)` | `attack_axes` / `decisively_dominates` / a scalar "better" | **Yes.** The two axes are free to *compute*, but combining them is a **withheld weighting**: the model must route a genuine trade-off (one attack more favorable, the other removes more threat) to **`incomparable`** rather than invent a weight. No operator returns the verdict. |
| **constraint-site** | (I) | *(not yet built)* terrain/water-distance/territory lookups + `count_*` | the **intersection / none-proof** operator | **Yes (by design).** Conjunction over several spatial predicates + a search that can prove the set empty ("none"). No operator returns "the tile meeting ALL predicates" or certifies none. **DEFERRED** — see §(e). |

### Kinds that COLLAPSE under the maximal set — do NOT run them under `*-maxops`

Per the corpus collapse list, and confirmed against the code:

- **`best-site`** (`BestSiteChoice`) — "which candidate has the MOST `terrain` in radius" is a
  **single-axis argmax**; `count_terrain` (already in the *basic* calculator) or `site_axes`
  solves it by a loop. **Collapses.** It is a T1 siting kind, not a survivor; keep it off the
  maximal arm (it stays useful only as a pre-calculator baseline).
- **`nearest` / `nearest-owned` / `reachable-nearest`** — argmin over `distance`/`travel_turns`/
  `reach_turns`. Free-operator loops. **Collapse** (this is the basic-calculator sweep result).
- **`region-count`** — literally `count_terrain`. **Collapses.**
- **`distance` / `unit-strength` / `city-defense`** (scalar A-vs-B) — one or two free calls + a
  compare. **Collapse.** (Only the *multi-axis* comparison, `compare-two-attacks`, survives.)

### One survival-test concern to flag for review

**`t3-threat`** (`CityThreatChoice`, "which ONE of my cities is most threatened") is *not* in the
build-6, but it is generated by `all_kinds` and would run under a maximal arm. With `threat` and
`city_defense` both free, it reduces to: read the two axes per city, find the **unique decisive
2-axis dominator**. That is the *same* free-axes situation as `adv-assault-target` and `settle-site`,
and it still requires the withheld 2-axis dominance judgment (no operator returns "most
threatened"), so by the corpus's own logic it **survives** — but it is the *thinnest* survivor of
the group, because demanding a **unique** dominator makes the judgment nearly mechanical once both
axes are in hand (find the row that is ≥ on incoming and ≤ on defense vs. all others). If a maximal
run shows `t3-threat` saturating while `adv-assault-target` (frontier, not unique) still
discriminates, that is the signal to **retire the unique-dominator framing and keep only the
frontier `ChoiceSet` forms** (adv-assault-target, settle-site). `adv-assault-target` is the robust
version of the same reasoning; `t3-threat` is the one to watch.

---

## (d) Measurement-parity gate (tests)

For every operator, an offline test asserts the operator returns **exactly** what the solver
computes internally — the invariant that keeps the calculator faithful to the scored answers:

1. **Per-operator == its `rules.rs` fn** on a fixture board:
   `op_att_eff == rules::att_eff`, `op_def_eff == rules::unit_def_on_own_tile`,
   `op_city_defense == rules::city_defense`, `op_garrison_defense == rules::def_eff_if_garrisoned`,
   `op_threat == ThreatField::at`, `op_site_axes == rules::site_axes`,
   `op_reach_turns == ReachField(unit move_rate)`.
2. **Operator inputs == a decision kind's solver inputs.** e.g. for `compare-two-attacks`,
   `favorability = op_att_eff(A)/op_def_eff(T)` and `threat_removed = op_att_eff(T)` reproduce
   `rules::attack_axes`; for `cf-vacate`, `op_city_defense` = the solver's "with garrison" value and
   `op_threat` = the solver's incoming force.
3. **Surface shape** — `raw-maxops` exposes exactly the basic + maximal verbs and no fetch verbs;
   `interactive-maxops` exposes fetch verbs + the whole calculator; both `reconstruct_via_tools`
   pass; a withheld verb name (e.g. `most_threatened`) is an `error:`.

`just check` = `cargo test` (incl. the parity gate + the Oracle-100% invariant on both boards/tiers,
untouched) + `cargo clippy -D warnings` + `verify-oracle`.

---

## (f) Enumeration / index operators — breaking the LOCATING wall

**Motivation (`analysis/findings-scale-ceiling.md`).** On a large well-explored board both ops arms
plateau at **~0.75**, and the ceiling is a **LOCATING** wall, not a computing one: ~92% of failures
are pointing operators at the wrong tile/city or over-fetching to find them; only ~5% are arithmetic.
Every operator in §(a) returns a **SCALAR for a position the model must already have**; none returns
a **position**. The predicted lever is an operator group that returns **predicate-filtered
POSITIONS**, pushing the LOCATING step into the tool while leaving the JUDGMENT to the model.

### Operator inventory (returns POSITIONS, never a scalar or a choice)

| operator | signature | backing fn(s) | returns |
|---|---|---|---|
| `list_tiles` | `(x0,y0,x1,y1, terrain?, resource?)` | same clipped iteration + `terrain==` / `Tile::has` predicates as `op_count_terrain`/`op_count_resource` | the COORDINATES of every in-bounds tile in the inclusive box matching the terrain and/or resource predicate (ANDed when both given; ≥1 required), row-major; leading count |
| `list_owned_cities` | `(player)` | `City::owner == player` (analogue of the interactive `list_cities` verb) | that player's city COORDINATES + count |
| `list_owned_units` | `(player)` | `Unit::owner == player` (analogue of `list_units`) | that player's unit COORDINATES + count |

- **`list_tiles` is the workhorse** and directly fixes both dispersed failure modes: it enumerates
  the WHERE of resources/terrain (raw-ops' "eyeball the board" tax) in one call, and it does so
  without the interactive over-fetch round-trips. Its box may span the whole board (clipped like
  `count_terrain`); results are capped at **`LIST_MAX = 1024`** (bounds the tool output — a scalar
  count never had that cost; region-count windows ≤49 and typical resource sweeps are far under it,
  and "narrow the box / add a predicate" is the right response past it). `resource` is gated by the
  same `is_resource` allow-list as `count_resource`; a reversed box, an out-of-range/typo resource,
  or no predicate returns `error:` (never a panic).
- `list_owned_cities`/`list_owned_units` are the **raw-side analogues** of the interactive fetch
  verbs, filtered to one owner — they solve the ownership-locating step (findings: raw-ops "can't
  keep its own cities straight") that `list_cities`/`list_units` solved for interactive-ops.

**Purity — these are ENUMERATORS, not selectors.** Each returns the FULL matching set (or a
player's whole raw roster), never an argmin, a nearest, a dominance verdict, or the answer to a
survivor kind. This keeps the withheld-selection thesis (§(a) "what is WITHHELD") intact: the tool
LOCATES, the model still JUDGES/composes.

### Operator judged too task-shaped — `nearest_owned_city` — EXCLUDED

The task floated `nearest_owned_city(x,y,player) → nearest city + distance` as a deliberate INDEX
helper for the dispersed *collapse* kinds. **Excluded**, because:
1. It bakes in an **argmin selection** — exactly the "choose-among-K" step the maximal calculator
   withholds. Adding a selection operator to the very surface built to withhold selection is in
   direct tension with the thesis (it would collapse `nearest-owned` by *answering* it, not by
   *locating* for it).
2. `list_owned_cities` + the existing `distance`/`travel_turns` already give the model everything to
   compute nearest-owned itself — the locating half in the tool, the argmin composition left to the
   model. That is the intended split.
3. The findings' nearest-owned failure was **locating ownership** ("can't keep its own cities
   straight"), which `list_owned_cities` fixes directly; the argmin was never the bottleneck (~5%
   computing). `nearest_owned_city` would add selection the model can already do and blur a clean
   "enumerate, don't select" boundary.

Excluding it also keeps the raw enum surface lean (12 tools, mid-range of the 11–13 target) and is
trivially reversible if a live run shows the argmin composition itself is a bottleneck.

### Redundancy retired (the user's explicit ask)

`list_tiles` **strictly subsumes** the scalar `count_terrain`/`count_resource`: over any box,
`op_list_tiles(box, pred).len() == op_count_*(box, pred)` (same clipped iteration, same predicate —
proven by `op_list_tiles_parity_and_subsumes_counts`). So on the enumeration surfaces **`count_terrain`
and `count_resource` are RETIRED** — the model takes `list_tiles`' length instead. Exposing both
would be needless menu bloat and defeats the point of consolidating. On the enum surfaces a
`count_terrain`/`count_resource` call now returns `error:` (retired), tested in the surface-shape
gates. No other clean redundancy remains: `distance` (Chebyshev) and `travel_turns` (BFS) are not
subsumed by anything; `list_owned_cities`/`list_owned_units` are kept as **two** verbs (mirroring the
existing `list_cities`/`list_units` pair) rather than merged, since cities and units are distinct
entity types the model addresses separately.

The **shipped basic surfaces** (`raw-ops`, `interactive-ops`) and the **measurement-only maxops
surfaces** (`raw-maxops`, `interactive-maxops`) are **UNCHANGED** — they keep `count_*` so
region-count still works there and they remain the ablation controls.

### Ablation arms (measurement-only vs measurement+enumeration on the SAME substrate)

Two new surfaces, registered in `queryable_surface()` + `QUERYABLE_SURFACES` (the CLI/runner route
queryable surfaces generically — no per-kind wiring):

- **`raw-maxops-enum` (PRIMARY).** Full raw board + the maximal calculator with `count_*` swapped for
  `list_tiles` + the enumeration group. The clean toggle against `raw-maxops`: identical perception
  and identical measurement operators, the *only* difference being positional enumeration — so it
  isolates "does positional enumeration break the LOCATING ceiling?" The scale finding says the wall
  is global locating over a huge board, which full perception + cheap enumeration is meant to break.
- **`interactive-maxops-enum` (symmetry; expected lower-value / possibly counterproductive).** The
  interactive fetch surface + the *identical* enum calculator. Flagged in the finding's own logic:
  interactive's measured failure mode is **over-fetch/dither**, and a bigger menu widens that
  surface — here the fetch verbs' `list_cities`/`list_units` sit alongside the enumerators'
  player-filtered `list_owned_cities`/`list_owned_units` (deliberate overlap, to keep the enum group
  byte-identical across substrates so perception is the *only* between-arm difference). It is added
  for factorial completeness, not because it is expected to win.

The enumeration operators are **identical** across the two substrates (shared
`maximal_enum_operator_tools()` / `execute_maximal_enum_operator`).

### (g) Per-surface operator-count table

| surface | fetch verbs | primitives | maximal measure | enumeration | total tools |
|---|---|---|---|---|---|
| `raw-ops` (control) | — | distance, travel_turns, count_terrain, count_resource | — | — | **4** |
| `interactive-ops` (control) | 6 | 4 (as above) | — | — | **10** |
| `raw-maxops` (control) | — | 4 (as above) | att_eff, def_eff, city_defense, garrison_defense, threat, site_axes, reach_turns | — | **11** |
| `interactive-maxops` (control) | 6 | 4 | 7 | — | **17** |
| **`raw-maxops-enum`** (NEW, primary) | — | distance, travel_turns *(count_* retired)* | 7 (as above) | list_tiles, list_owned_cities, list_owned_units | **12** |
| **`interactive-maxops-enum`** (NEW) | 6 | 2 *(count_* retired)* | 7 | 3 | **18** |

Fetch verbs = `region_summary, scan, scan_grid, get_tile, list_cities, list_units`. The retirement of
`count_terrain`/`count_resource` on the enum surfaces is what nets `raw-maxops-enum` to 12 (not 14):
+3 enumerators, −2 retired counts vs the 11-tool `raw-maxops`.

### Gate evidence

- **Parity + subsumption** — `op_list_tiles_parity_and_subsumes_counts` (length == `count_*` over
  several boxes, both predicates ANDed, OOB clipped like `count_terrain`),
  `op_owned_rosters_parity_with_board_membership` (each roster == the board's owner-filtered coords).
- **Surface shape** — `raw_maxops_enum_surface_shape` (exactly the 12 verbs; `count_*` and all fetch
  verbs absent and now `error:`; a withheld selection op is `error:`; `reconstruct_via_tools` passes
  — inherited raw decoder, operators carry no per-tile facts), `list_tiles_error_paths`,
  `interactive_maxops_enum_has_fetch_and_enum_calculator`.
- `just check` (workspace `cargo test` incl. the untouched Oracle-100% invariant + these tests,
  `cargo clippy -D warnings` on default AND `--features remote`) and `verify-oracle` (582 trials) all
  green; `civ-core` untouched.

---

## (h) Deferred / remaining work

- **`constraint-site` (I)** — not implemented in `question.rs`/`generators.rs` (the corpus lists it
  as "built" but the enum has no `ConstraintSiteChoice`). Building it is a self-contained follow-up:
  a new `Question` variant + solver (conjunction of `defensible ∧ within-2-of-water ∧
  not-enemy-territory` over K offered tiles, `ChoiceSet` with a real `"none"` option), a generator
  that guarantees a decisive some/none split, and Oracle-100% coverage. Its survival design is fixed
  (§(c)): expose terrain/water-distance/territory **lookups**, withhold the intersection/none-proof.
  Deferred here to keep this change focused on the calculator; flagged for the next build.
- **Cross-model / frontier runs** — none performed (no cloud/LLM runs in this worktree per the task
  constraint); the maximal arm is offline-verified only (Oracle parity), ready for a live run.
