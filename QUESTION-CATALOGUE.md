# CivSpatial — Question Catalogue & Coverage

*A reference of every question kind the eval currently asks, the exact prompt text, the answer format, and commentary — followed by a coverage/gap assessment against the game's domains. Generated 2026-08-05 from `civ-eval/src/question.rs` + `rules.rs`; run-status reconciled 2026-09-15. Placeholders like `{tile}`, `{player}`, `{list}` are filled per item; a `{tile}`/referent renders per encoding (coords, a named unit/city, etc.).*

---

## ⚑ Run status — what this catalogue lists vs. what the experiment actually scored

This catalogue lists **every kind the generator can produce**. The published experiment — the 3-arm
clean run (`analysis/findings-3arm-clean-run.md`) — scored a **deliberately leaner roster**. Read every
table below through this legend (each kind row is tagged):

- **·RUN** — scored in the published experiment. **10 kinds:** region-count, reachability,
  nearest-owned, reachable-nearest, city-defense, constraint-site, compare-two-attacks, t3-retreat,
  triage-reinforce, **forward-posting**. (forward-posting was added after this doc's 2026-08-05
  generation; its row is included in §5.) Movement kinds (nearest-owned, reachable-nearest) ran on the
  **mid board only** — rail trivializes them on late boards.
- **·EXCL** — ran but **excluded from publication**: cf-vacate (corpus-degenerate — all-"no" on both
  frozen boards; re-measured standalone in `findings-cfvacate-tooltrap.md`).
- **·FLOOR** — kept in code, saturated, used as a floor-check only, **never scored in a run**: terrain,
  adjacency, direction.
- **·DROPPED** — in code but removed from the run roster: distance, nearest (early/redundant),
  **best-site** (underdetermined in real play), **unit-strength** (near-table-lookup, redundant with
  city-defense), **settle-site / adv-assault-target / t3-threat** (saturator trims — the decision story
  is carried by the ·RUN decision kinds).

**So the experiment is 10 scored kinds (+ cf-vacate measured but withheld), not the 20 catalogued
below.** The ·DROPPED kinds are precisely the ones most open to a "that's not a decision a player would
actually make" objection (best-site's narrow proxy, unit-strength's table lookup); they are documented
here for completeness but were **not part of the result**.

---

## 0. What every prompt shares

Two blocks precede the question, held **identical across encodings** so the elicitation format is a constant, not a confound.

**Geometry preamble (always present):**
> You are reasoning about a FIXED game board laid out on a square grid. Conventions:
> - Coordinates are (x, y). x increases east; y increases south; y = 0 is the north edge.
> - The 8 compass directions are N, NE, E, SE, S, SW, W, NW. North is toward smaller y.
> - Distance = Chebyshev (a diagonal counts as one step, `max(|dx|,|dy|)`).
> - The board does NOT wrap at its edges.

**Rules block (only for T2/T3 kinds; empty for T0/T1):** a stated combat/strength model + per-kind judgment rules. Summarised in §5; this is the "…RULES above" the decision questions reference. Every answer ends with a strict `Answer: <X>` line whose format is the "Answer format" noted per kind.

---

## 1. T0 — basic perception

| Kind | Prompt | Answer |
|---|---|---|
| **terrain** ·FLOOR | "What is the terrain of {tile}?" | a single terrain name |
| **adjacency** ·FLOOR | "What is the terrain of the tile immediately to the {DIR} ({d}) of {tile}?" | a single terrain name |
| **direction** ·FLOOR | "In which single compass direction does {target} lie from {origin}?" | one compass direction |

*Commentary:* pure readout of the encoded board — the always-green floor check. Saturated on `raw` (models get these ~always right), so these are **kept in code but cut from experiment runs**.

---

## 2. T1 — spatial computation & geography

| Kind | Prompt | Answer |
|---|---|---|
| **distance** ·DROPPED | "How many tiles apart are {a} and {b}, counting a diagonal step as one move?" | a whole number (Chebyshev) |
| **nearest** ·DROPPED | "How many tiles is it from {from} to the nearest tile containing {resource}, counting a diagonal step as one move?" | a whole number |
| **region-count** ·RUN | "How many {terrain} tiles are within {radius} tiles (Chebyshev) of {center}, including that tile itself?" | a whole number |
| **reachability** ·RUN | "Starting on {from}, can a unit reach {to} in at most {budget} steps (8-directional) without ever entering a {avoid} tile?" | yes / no |
| **best-site** ·DROPPED | "Which of these candidate tiles would make the best city site — the one with the most {terrain} tiles within {radius} tiles of it: {list}?" | the best tile's coordinates |

*Commentary:*
- **region-count** is the historically hard "counting wall" — it survives reasoning and is where the interactive/query loop most clearly beat front-loaded encodings.
- **best-site is deliberately player-agnostic** — the best site is defined *purely by terrain in the work radius*, so no "you are player X" appears. Its player-relative sibling is **settle-site** (T3). This is by design, not an omission.
- **reachability** is straight 8-directional flood within a step budget avoiding one terrain — distinct from the *turn-costed* movement in §3.

---

## 3. T1 "real-play" — logistics (player-relative, turn-costed movement)

| Kind | Prompt | Answer |
|---|---|---|
| **nearest-owned** ·RUN (mid-board only) | "You are player {player}. Considering only {resource} tiles NOT already within 2 tiles of one of your cities (resources you don't yet work), what is the fewest TURNS a unit needs to travel over land (never crossing ocean) from your nearest city to the closest such tile? Consider only tiles reachable within {horizon} turns; answer a whole number, or \"none\"." | a whole number of turns, or "none" |
| **reachable-nearest** ·RUN (mid-board only) | "You control {unit}. Moving over land only (never crossing ocean), what is the fewest turns it needs to reach the nearest {resource}? A unit moves up to its own move rate per turn. …within {horizon} turns; a whole number or \"none\"." | a whole number of turns, or "none" |

*Commentary:*
- These use the **classic movement model** (Dijkstra): rail = free, road/river = 1 fragment, terrain 3/6/9 (SINGLE_MOVE = 3). This replaced a uniform-land BFS and **flipped 66% of nearest-owned / ~96% of reachable-nearest answers** on T677 — the old version was wrong, not just simpler.
- **Caveat baked into the corpus plan:** free rail makes ~41% of late-board reach instances trivial (0–1 turns). Reach kinds want *earlier / less-railed* boards.

---

## 4. T2 — unit & city valuation

| Kind | Prompt | Answer |
|---|---|---|
| **unit-strength** ·DROPPED | "Which unit has the greater {axis} strength, {a} or {b}?" (axis = attack or defense) | the stronger unit, named by type + coords |
| **city-defense** ·RUN | "Which city is better defended, {a} or {b}?" | the better-defended city's name |

*Commentary:* first use of the stated combat model — `att_eff/def_eff = base × veteran × health`, defense further multiplied by terrain + fortification + City-Walls bonuses; a city's defense is its **single best defender** on the city tile. Great Wall grants walls to all of an owner's cities; Coastal Defense is sea-only. Near-ties are dropped at generation so only decisive items remain.

---

## 5. T3 — decisions & judgment (player-relative; use the rules block + combat odds)

All of these inject a **COMBAT ODDS** model: an attack with strength A vs defense D wins each round with p = A/(A+D), and must win `ceil(def_hp/att_firepower)` rounds before the defender wins `ceil(att_hp/def_firepower)` — so **a lopsided strength ratio ≠ a certain win** once hitpoints enter.

| Kind | Prompt (abridged) | Answer | Scoring model |
|---|---|---|---|
| **settle-site** ·DROPPED | "You are founding a new city for player {player}. Following the SETTLE-SITE RULES, which candidate is a SOUND (non-dominated) site: {list}?" | coords of any sound candidate | **4-axis Pareto** (food / production / resources / safety); any non-dominated candidate is correct |
| **constraint-site** ·RUN | "You are choosing a fortified city site for player {player}. Which candidate satisfies ALL: defensible AND within 2 of water AND not enemy territory: {list}?" | coords of any qualifying tile, or "none" | **hard conjunction** of 3 predicates; genuine "none" cases included |
| **t3-retreat** ·RUN | "One of your units, {unit}, is under enemy threat and must retreat. Which of these is a SOUND (non-dominated) tile to retreat to: {list}?" | coords of any sound candidate | **3-axis Pareto** (cover / safety / support) |
| **t3-threat** ·DROPPED | "You are player {player}. Which single one of your cities is under the GREATEST threat — MOST LIKELY TO FALL this turn: {list}?" | the most-threatened city's name | **single-axis argmax** of fall-probability (combat odds) |
| **cf-vacate** ·EXCL | "Can {city} spare its single best defender for THIS turn — if you pulled it out now, would the city still hold?" | yes / no | **counterfactual**: attacker's win-prob vs the *remaining* (2nd-best) defender |
| **adv-assault-target** ·DROPPED | "You are player {player}. Reasoning from the enemy's POV, which of your cities is a MOST ATTRACTIVE assault target: {list}?" | a sound target city's name | **2-axis Pareto** (capture-win-prob × city size) |
| **triage-reinforce** ·RUN | "You are player {player}. You have one spare defender ({reserve}); assume you can deploy it to ANY of these cities. Which does it best protect?" | the city to reinforce | **marginal** effect: the city the defender FLIPS from capturable→held |
| **compare-two-attacks** ·RUN | "Compare two attacks purely on VALUE (assume either could be made). Attack A: {atk_a} strikes {tgt_a}. Attack B: {atk_b} strikes {tgt_b}. Which is better, or incomparable?" | the better target's coords, or "incomparable" | **2-axis Pareto** (exchange favorability × threat removed); genuine incomparables exist |
| **forward-posting** ·RUN | "You are player {player}. Which of these forward positions is a SOUND advance — not left clearly worse off after the enemy's best reply next turn: {list}?" *(abridged; added after 2026-08-05)* | coords of any sound advance | **adversarial 1-ply**: any advance not dominated after the enemy's best single-ply response |

*Commentary / fidelity notes worth knowing:*
- **"Magic placement" is stated honestly.** compare-two-attacks and triage-reinforce say "assume either attack could be made / ignore travel distance" — they test the *value* judgment, not reachability. t3-threat/cf-vacate DO fold in turns-to-arrive.
- **Pareto kinds have multiple correct answers** (any non-dominated pick). The scorer uses a precomputed acceptable-set; near-ties are dropped at generation so items stay decisive.
- **Value = city SIZE only** for adv-assault-target. Production / Palace / wonders are deliberately **not** in the value proxy yet (not reliably decoded) — a known simplification.
- **Player-relative kinds name the player** ("You are player {name}…"). Under the new three-state fog, the fogged perspective and this "you" must be the *same* live player — that coherence enforcement is being built now.

---

## 6. Coverage by game area

Rating: **Strong** = decision-grade coverage; **Partial** = touched but shallow/proxied; **None** = not asked.

> **Coverage ≠ scored.** This table rates what the generator *can* ask, so it credits ·DROPPED and
> ·FLOOR kinds too (e.g. best-site, settle-site, unit-strength). For what the published experiment
> actually scored, see the ⚑ Run status legend at the top — the decision-grade coverage that is
> *load-bearing in the result* rests on the ·RUN kinds (city-defense, constraint-site,
> compare-two-attacks, t3-retreat, triage-reinforce, forward-posting).

| Game area | Covered by | Rating | Notes / gaps |
|---|---|---|---|
| Terrain & adjacency perception | terrain, adjacency, direction | **Strong** | Saturated; kept as floor-check only. |
| Spatial measurement | distance, nearest, region-count, reachability | **Strong** | region-count is the residual hard skill. |
| Movement & logistics | nearest-owned, reachable-nearest | **Strong** (land) | Faithful road/rail/river costs. **Gaps:** no naval transport / amphibious (ocean is only a barrier), no zones-of-control, no air range/fuel, no stack movement. |
| City siting & expansion | best-site, settle-site, constraint-site | **Strong** | Terrain, 4-axis economy+safety, and hard-constraint variants all present. **Gap:** no penalty for overlapping an existing city's radius; no founding-*timing*. |
| Unit combat | unit-strength, compare-two-attacks | **Strong** (single attack) | **Gaps:** no multi-unit / stack combat, no bombardment/ranged, no combined arms. |
| City defense & threat | city-defense, t3-threat, cf-vacate, triage-reinforce, adv-assault-target | **Strong** | Walls/Great Wall/Coastal, best-defender, counterfactual and marginal reasoning. **Gap:** no multi-turn siege planning. |
| Economy / production / yields | settle-site (food/prod/resource axes) | **Partial** | Economy enters only as siting axes. **Gap:** no direct questions on city output, growth, trade/gold/science, or economic city-vs-city comparison; assault value = size only. |
| Territory / borders / control | constraint-site ("not enemy territory") | **Partial** | Owner is decoded but only used as a predicate. **Gap:** no "who controls the most territory", border/contested-zone, or territory-aggregate questions. |
| Global / aggregative reasoning | — | **None** | Everything is local/point/bounded-radius or small-candidate decisions. **Gap:** no whole-board aggregates ("which quadrant has the most X", "which player is largest"). Flagged in RESUME as the regime where hierarchical/interactive encoders should shine. |
| Information state / fog | — (fog masks the board, but nothing *asks* about it) | **None** | Fog is now faithfully modeled (three-state). **Gap/opportunity:** no questions testing what's visible vs hidden ("is enemy X currently in your sight?", "which city is blind on which flank?"). |
| Multi-turn / sequential planning | — (turns-to-arrive is a scalar input only) | **None** | Every question is single-decision / this-turn. **Gap:** no action ordering, lookahead, or plan-a-route-over-N-turns. |
| Naval / coastal / islands | Coastal Defense; constraint-site water proximity | **Partial** | **Gap:** no naval movement, transport, island-hopping, or sea-lane reasoning. |
| Diplomacy / player relations | enemy-vs-own is binary | **None** | Reasonably out of scope for a *spatial* eval; note barbarians/pirates/animals are special and now excluded as fog perspectives. |
| Infrastructure / special tiles | roads/rail/rivers/mines/fortress/resources feed movement, defense, siting | **Partial** | Used as inputs but rarely the *subject* of a question (e.g. no "is A road-connected to B?", "is this tile mined?"). |

---

## 7. Gaps worth closing (prioritised)

1. **Global / aggregative queries** *(highest leverage)* — whole-board counts, quadrant aggregates, "who controls the most territory". Currently zero coverage, and it's exactly the query shape RESUME predicts will separate the hierarchical/interactive encoders from flat `raw`. A question-design change, not a bigger board.
2. **Information-state / fog questions** *(novel, leverages new work)* — now that fog is faithful, ask the model what it can and cannot see. Directly exercises the three-state fog + perspective coherence just built; nothing else in the field tests spatial *information* reasoning this way.
3. **Economic valuation beyond size** — production/yield/gold city comparisons, and folding production/Palace into assault value. Blocked partly on decode (Palace is decoded; production is not).
4. **Multi-turn / sequential planning** — the hardest to score cleanly, but the most "agentic"; even a bounded 2–3 turn route/order question would open a new axis.
5. **Naval / transport / amphibious** — turn ocean from a hard barrier into a modeled traversal (ferry a unit, island-hop). Larger modeling lift; lower priority.

*Note on fidelity, not coverage:* the combat model is a **stated simplification** (not a full battle sim), value proxies to **size**, and decision kinds use **honest magic placement** where noted. These are documented design choices, not bugs — but they bound how "real-game" a strong score can claim to be.
