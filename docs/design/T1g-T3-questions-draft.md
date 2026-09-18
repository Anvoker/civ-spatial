# DRAFT — Global-aggregation tier (T1g) + T3 question ideas

**Status:** brainstorming draft for review (2026-07-28). Not approved, not built. Companion to
`T2-T3-design.md` (approved T3 design) and `findings-t677-crossover.md` (why we want T1g: to give
`hierarchical` a fair test — our current questions are point/local, flat-list territory).

---

## 0. The design principle (what makes a question stress aggregation)

A question stresses **aggregation** iff answering it forces a reduction over *many* tiles/entities
(count / argmax / plurality / centroid) that a **flat coordinate list (raw) must compute by scanning the
whole region**, while a **pre-summarized encoding (hierarchical) can read off its region stats**. Our
T0/T1 kinds are the opposite — point lookups and *local* windows where flat lists win. T1g is the regime
where hierarchy *should* pay off, if it ever does.

**Two coupled moves — both required for a fair test:**
1. **Add aggregation questions** (this doc).
2. **Enrich `hierarchical` to actually carry the aggregates the questions ask about** — per-region terrain
   histograms, owner tallies, entity counts. *If the encoder doesn't pre-aggregate the quantity asked, even
   an aggregation question won't help it* — we'd be testing a hierarchy that doesn't summarize the right
   thing. This stays reconstruction-gated (the region stats are derivable, so they don't break round-trip).
   **This is the real experiment: does a hierarchy that summarizes X beat raw on questions about X?**

**Regions are stated as explicit coordinate boxes in the question text** (e.g. "the NE quadrant: x ≥ 42,
y < 28"), so the test is **encoder-agnostic and fair** — every encoder gets the same bound; hierarchy's
summaries help only if its region boundaries align. No new `Referent` needed; no new `Answer` variant
needed (Int + Choice cover all of T1g). Partition menu: **whole board**, **quadrants** (NW/NE/SW/SE),
**halves** (N/S, E/W), **thirds** (bands). Keep boundaries on the board's own midlines.

---

## 1. T1g — global-aggregation question kinds

All are Tier::T1 in *character* (computed truth, geometry+arithmetic) but "global" in *scope*. Suggested
category strings prefixed `g-` so they pivot/filter as a family. **Decisive-margin discipline** (like
`T2_MARGIN`): argmax/plurality kinds drop near-ties (winner must beat runner-up by a margin) and log drops.

| # | category | example phrasing | answer | what it stresses | build |
|---|---|---|---|---|---|
| G1 | `g-terrain-count` | "How many **Mountains** tiles are on the entire board?" | `Int` | whole-board scan + tally; the purest aggregation test | now |
| G1b | `g-terrain-count` | "How many **Forest** tiles are in the **NE quadrant** (x ≥ 42, y < 28)?" | `Int` | region-bound + tally; matches a per-region histogram | now |
| G2 | `g-terrain-plurality` | "What is the **most common terrain** in the **northern half** (y < 28)?" | `Choice`⟨terrain⟩ | histogram + argmax over terrains | now |
| G3 | `g-territory-control` | "Which player **controls the most tiles** in the **SE quadrant**?" | `Choice`⟨player⟩ | aggregate the per-tile `owner` attribute + argmax; capacity-axis stress (owner must be carried) | now |
| G4 | `g-entity-count` | "How many of **{player}'s units** are in the **southern third** (y ≥ 37)?" | `Int` | filter a long entity list (682 units on T677) by region + count | now |
| G5 | `g-argmax-region` | "Which quadrant (NW/NE/SW/SE) contains the **most Hills**?" | `Choice`⟨region⟩ | per-region totals **compared across regions** — strongest hierarchy test (4 summaries vs 4 scans) | now |
| G6 | `g-centroid-region` | "In which quadrant is the **center of mass of {player}'s cities**?" | `Choice`⟨region⟩ | spatial aggregation (mean position) reduced to a label — no brittle coord match | now |
| G7 | `g-border-length` | "How many tiles owned by **{A}** are **adjacent to** a tile owned by **{B}**?" | `Int` | *relational* global aggregation (contested border) — stresses adjacency encoders too | later |

**Notes / rationale**
- **G1/G5 are the headline aggregation tests.** G1 = whole-board tally (raw must scan ~4700 tiles). G5 =
  argmax across regions (raw does four regional scans; a per-region-histogram hierarchy does four reads).
  If hierarchy ever beats raw, it's here.
- **G3 (territory control)** is the most 4X-meaningful ("who's the territorial leader") and doubles as a
  capacity-axis test: it aggregates `owner`, the attribute an ASCII legend once dropped (the reconstruction
  gate caught it). Great cross-stress.
- **G1 as the balanced count sweep.** Sampling G1/G1b across a *range of true counts* (0, 3, 8, 15, 25…)
  turns the backlog "balanced count sweep" into a measurement of the aggregation curve per encoding — the
  multiplicity cliff, mapped. One kind, two uses.
- **G6** avoids brittle `Coord` scoring by bucketing the centroid to a quadrant (`Choice`). If we ever want
  true centroids, that needs tolerance scoring — out of scope; the quadrant form is exact-match-clean.
- **Admissibility:** for count kinds, pick the terrain/player so the true count sits in a discriminating
  band (not 0, not the omitted default). For argmax/plurality kinds, require winner ≥ 1.25× (or ≥ N tiles
  above) the runner-up, else drop → no coin-flip ties scored as wrong.
- **No contract change.** Every T1g kind is `Int` or `Choice`, both already in `Answer`. Solvers are simple
  reductions over `board.iter_tiles()` / `board.units` / `board.cities` with a region predicate. Oracle-100%
  falls out for free.

---

## 2. T3 — judgment under a withheld value function

T3a (best-settle-site, `ChoiceSet` dominance) is **designed + user-approved** in `T2-T3-design.md` §4 but
**not built**. It needs the one approved contract extension: `Answer::ChoiceSet { acceptable, options }`
(scorer = pure set-membership; `acceptable` precomputed at generation) + a negative Oracle test. Everything
below hangs off that same variant.

### 2.1 Build-first (already specced)
- **T3a — best settle site.** `ChoiceSet` over K candidate tiles; axes = food / production / resources /
  safety (graded threat field); dominated pick = blunder. Recipe + feasibility in `T2-T3-design.md` §4.2/§6.

### 2.2 T3 family — same dominance pattern, fleshed out (new here)

| category | phrasing | options | axes (stated; weighting withheld) | dominated pick = |
|---|---|---|---|---|
| `t3-retreat` | "Your wounded **{unit}** must move. Which tile is a sound retreat?" | K adjacent/near tiles it can legally reach | **cover** (terrain def bonus), **safety** (distance-decayed threat), **egress** (onward mobility) | a tile worse on all three than another option |
| `t3-attack-target` | "Which of these enemies is a sound target for your **{unit}** this turn?" | K reachable enemy units | **damage** (my att_eff ÷ their def_eff), **risk** (their counter att_eff on me), **position** (do I end exposed) | a target worse on every axis than another |
| `t3-defend-city` | "You have one spare defender. Which city should get it?" | K of your cities | **threat** (incoming, §4.2 field), **value** (size/improvements), **current weakness** (def_eff gap) | a city dominated (less threatened, more valuable-protected, better-defended) than another |

All reuse `ChoiceSet`; all are **blunder-rate** metrics (score only dominated picks wrong; don't adjudicate
the frontier). Build order: T3a → T3b(`t3-retreat`, cleanest geometry) → the rest.

### 2.3 The synthesis you asked for — **aggregation × judgment** (T3 over regions)

These fuse T1g and T3: judgment whose *axes are themselves aggregates*, so they exercise **both**
hierarchy's region summaries **and** withheld-value dominance. Most 4X-real of the lot.

| category | phrasing | options | axes (each an aggregate over the region) | note |
|---|---|---|---|---|
| `t3-expand-region` | "Which region is the best direction to expand next?" | the 4 quadrants (or N/S/E/W) | **open land** (settleable tiles in region), **resources** (specials in region), **safety** (enemy strength in region), **reach** (distance from your core) | dominance over quadrants; a quadrant worse on all axes = blunder |
| `t3-contest` | "Which border region is most worth reinforcing?" | K contested regions | **exposure** (enemy tiles adjacent), **holdings** (your cities/tiles there), **terrain** (defensibility) | pairs G7 border-length with judgment |

`t3-expand-region` is the strongest single experiment for the whole hierarchy thesis: its four axes are
exactly the per-region aggregates a good hierarchical summary would carry, and the answer is a region label
(clean `ChoiceSet` over {NW,NE,SW,SE}). If hierarchy helps anywhere, it helps here.

### 2.4 Deferred (definitional risk)
- **`t3-city-rank`** (rank all cities by exposure, score *inversions*) — needs a rank/permutation answer +
  inversion scorer, a bigger contract move than `ChoiceSet`. Keep the *single*-pick "which city is most
  threatened" as a deterministic **T2-style `Choice`** with a decisive threat margin instead (no new
  contract) — that already gives most of the signal.
- **`Direction`-valued "most vulnerable flank"** — highest definitional risk; stays deferred (per §4.3).

---

## 3. Recommended build order (cheapest signal first)

1. **T1g core: G1, G1b, G5** (`Int` + `Choice`, no contract change) **+ enrich `hierarchical` with
   per-region terrain histograms.** Run raw/ascii/hierarchical on T677. **This directly answers "does
   hierarchy ever win?"** — the point of the whole exercise. Cheap; buildable this session.
2. **T1g breadth: G3 (territory-control), G4 (entity-count), G2, G6.** Adds the owner/entity aggregates
   (and the balanced-count sweep via G1). Enrich hierarchical with owner/entity region tallies alongside.
3. **T3 contract + T3a** (`ChoiceSet` + negative Oracle test), then `t3-retreat`.
4. **`t3-expand-region`** — the aggregation×judgment capstone (needs #1's region summaries + #3's ChoiceSet).
5. **G7 / `t3-contest` / `t3-city-rank`** — advanced, only if the earlier tiers show the effect.

**Coupling to flag:** steps 1–2 require touching the `hierarchical` encoder (add region-stat summaries)
while keeping it reconstruction-complete and leaving T0/T1 board blocks byte-identical (same rich-rendering
seam discipline as `T2-T3-design.md` §9). That encoder change *is* the experiment — do it deliberately.

---

## 4. Rooted-in-real-play + reachability-bounded scope (2026-07-29, agreed)

New governing principle for the WHOLE corpus, recorded durably in **`DESIGN.md` §6** ("Question corpus
principle"): questions are defined by **what a regular player does**, and spatial scope is bounded by the
player's **reachable space** (terrain-aware travel-cost horizon from a controlled entity), not the whole
map. This is the fairness fix for the "you tested the questions interactive wins" critique — it
externalizes corpus fairness to a citable source (FreeCiv autoplayer decision points / CivRealm tasks /
strategy guides) and, done honestly, forces the interactive-hostile tasks back in as *bounded* versions.

Concrete question-design consequences (build items):
- **`reachable-nearest-resource` (scout task) — NEW kind.** "From this unit, what is the nearest
  {resource} you can reach within N turns?" Origin = a *unit* (not a city); distance = terrain-aware
  `reach_turns` (reuse the T3 threat-field BFS), not Chebyshev; answer = an `OptionalInt` (turns) or
  `none` past the horizon. This is the real-play, bounded form of the old global `nearest`.
- **Refine `nearest-owned`.** It currently bounds by raw **Chebyshev** distance from cities; the faithful
  bound is **travel cost** (terrain-aware `reach_turns`) — mountains/ocean/rivers change what is "quickly
  reachable." Chebyshev is a cheap approximation; note the divergence and consider switching the headline
  version to travel-cost. Both `nearest-owned` (reach from cities) and `reachable-nearest-resource` (reach
  from a unit) are the same **egocentric-reachability** idea.
- **Keep the unbounded global `nearest` as a DIAGNOSTIC**, not a headline activity — it keeps the
  interactive failure mode visible instead of defining it away (§0 discipline, mirrored from the T0/T1
  primitives).
- **Egocentric + interactive encoder — hypothesis to test.** If every real spatial task is egocentric and
  reachability-bounded (origin = a controlled entity, region = its reachable neighborhood), an interactive
  surface whose overview + coordinates are **origin-relative** may be the strongest access strategy. Build
  it only after the bounded tasks exist; **measure, don't assume** — a fast unit's reachable set can be a
  large fraction of the map, so "bounded" is not automatically an interactive win.
