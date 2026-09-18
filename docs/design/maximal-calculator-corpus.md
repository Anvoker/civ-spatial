# Maximal-calculator question corpus — design catalog

**Status:** design ideation, 2026-07-30. The candidate corpus for the maximal-calculator pivot
(`interactive-compute-forks-design.md` §7, memory `project-direction-maximal-calculator`). Not built;
`settle-site`/`city-threat` already exist as the prototype. Build order = the shortlist below.

## The survival test (the operator dial, made mechanical)
Free measurement operators evaluate the **frozen board as it is**, pointed at inputs the model names
(`distance`, `travel_turns`/`ReachField`, `count_terrain`/`count_resource`, `att_eff`/`def_eff`, the
`threat` field, ownership/terrain lookups, working-radius axis sums). A kind **survives a maximal
calculator only if the answer needs something no measurement returns** — exactly one of five:

- **(S) Withheld aggregation** — ≥2 axes trade off and the *weighting* isn't a measurement → dominance
  judgment. (Single-axis argmax does NOT survive — it's a loop over one free operator.)
- **(C) Board-counterfactual** — the answer is a measurement over a board that **differs** from the
  frozen one (a founded city, a removed/placed unit, a blocked tile). The calculator can't point at a
  state that doesn't exist.
- **(P) Perspective re-pointing** — reconstruct *which* measurements matter and in *which polarity*
  from the enemy's viewpoint.
- **(I) Predicate composition + none-proof** — conjunction/search over predicates; no operator returns
  "the intersection" or proves a set empty.
- **(J) Ranking under a partial order** — order scored by dominance-inversions, not a given scalar.

**Key point: the maximal calculator is a STRICTLY HARDER filter than the basic-primitives one.**
Making `threat`/`ReachField` free *kills* kinds that survive primitives-only (see the collapse list).
That's the thesis working — the survivors are the (S)/(C)/(P)/(I)/(J) reasoning kinds.

## Catalog by reasoning type (survival tag in brackets)

**1. Multi-axis dominance / trade-off judgment (S)** — the richest vein; every axis free, the whole
reasoning is the withheld weighting; `ChoiceSet` Pareto scoring.
- `settle-site` *(built)* — sound city site among K; food/prod/resource/safety axes free, weighting isn't.
- `city-threat` *(built)* — most-threatened city; two axes (incoming↑ `threat`, `def_eff`↓) never scalarized.
- `best-attack-target` *(new)* — what to strike; survives only as 2 axes: exchange favorability × threat-removed (target's own `att_eff`). Not single-axis exchange (that collapses).
- `triage-reinforce` *(new)* — which city gets the one spare defender; axis is **marginal** benefit (does the garrison flip *capturable→held*), not current exposure. Doomed/safe cities are wrong even if most/least threatened.
- `defensive-position` *(new)* — where to park a defender to cover a city; cover × approach-coverage × own exposure. "Coverage" crisp = # distinct enemy land approaches whose shortest path passes adjacent.
- `resource-priority` *(new)* — which contested resource to secure first; value × contestedness (enemy reach ≤ yours) × your cost.

**2. Counterfactual (C)** — only mutations the calculator can't be pointed at; frame strictly **this-turn**.
- `cf-vacate` *(new — strongest)* — can you pull the defender out of a city for a turn? `def_eff` reads the *current* garrison; model recomputes with best defender removed vs. incoming `threat`. **Bool.**
- `cf-interdict` *(new)* — move a unit onto a tile → do you block the enemy's only within-reach land route to a city this turn? **Bool** (reach with a mentally-removed/added node).
- `cf-settle-compare` *(new — C×S)* — of two sites, which is the better city *after founding* (a new defended, territory-owning object) — or incomparable? `Choice{A,B,neither}` by dominance over the hypothetical.
- `struct-chokepoint` *(second-wave)* — if a tile became impassable, could A still reach B over land? **Bool**; the sole structural survivor (needs a connectivity-with-removal solver). Keep phrasing player-facing ("is that pass our lifeline?").

**3. Adversarial / perspective (P+S)** — free measurements; residue is recognizing which the enemy optimizes.
- `adv-assault-target` *(planned)* — red's most attractive assault target (low `def_eff`, high reachable enemy `att_eff`); dominance.
- `adv-exposed-unit` *(new)* — which of your units red picks off (enemy reachable `att_eff`↑, unit `def_eff`↓, value↑).
- `adv-approach`/flank *(defer)* — which direction a city is most open from; needs a **Direction** variant + a crisp "approach sector" def. High definitional risk; parked (T2-T3 doc concurs).

**4. Constraint-satisfaction (I)** — conjunction + none-proof.
- `constraint-site` *(built)* — which of K tiles meets ALL stated predicates (defensible ∧ near water ∧ not-enemy-territory) or 'none'.
- `constraint-path` *(new)* — is there a land route A→B never entering enemy territory? **Bool** (constrained reach isn't a free operator). Collapses if a constrained-reach operator is ever added — keep withheld.

**5. Comparative A-vs-B (S)** — must be multi-axis or it collapses.
- `compare-two-attacks` / `compare-two-sites` *(new)* — `Choice{A, B, incomparable}` by dominance. A *scalar* compare (closer/stronger) collapses — don't build. The survivor tests whether the model **refuses to invent a weighting** (routes genuine trade-offs to "incomparable").

**6. Ranking (J)** — new variant.
- `rank-by-exposure` *(new)* — order cities by exposure; score = # dominance-order inversions, incomparable pairs **un-scored**. Needs an `Answer::Order` variant + inversion scorer. Higher resolution than single-pick.

**7. Triage/evacuation (C+S)**
- `evacuate-vs-hold` *(new)* — is a city still savable *even with best available reinforcement* (`def_eff'` vs incoming), or write it off? **Bool.**

## Collapse list — do NOT build for the MAXIMAL calculator (several survive the BASIC one — that's the point of the first run)
- **Single-axis argmax/argmin** — nearest city/resource/unit, strongest/weakest, "closest enemy," scalar-most-threatened. A loop over one free operator. (Why `city-threat` is kept two-axis.)
- **`reach-race`** — each side's min `travel_turns` is free → compare. Survives primitives-only, collapses here.
- **`cf-settle-threat` single-count** — `threat`/`ReachField` to the hypothetical tile is a free measurement. Use `cf-settle-compare` (which mutates something unmeasurable) instead.
- **encirclement / containment / contiguity / "is city A in enemy territory"** — lookups/reachability. Collapse.
- **distance/strength "is A closer/stronger than B"** — two free calls + compare. Only the *multi-axis* comparison survives.

Rule of thumb: a structural/reach question survives the maximal calculator **only if it involves a board-counterfactual** (removed tile/unit); else `ReachField` already answers it.

## Ranked shortlist — build first (one exemplar per reasoning type, max diversity per build cost)
1. **`settle-site`** (S) — built; gold-standard anchor / calibration.
2. **`cf-vacate`** (C) — cleanest counterfactual, maximally natural, Bool, reuses `def_eff`+`threat`. **First new build.**
3. **`adv-assault-target`** (P) — theory-of-mind, `ChoiceSet`, reuses `threat`+`def_eff`.
4. **`triage-reinforce`** (S, allocation) — marginal-benefit twist discriminates where a naive threat-ranker fails.
5. **`constraint-site`** (I) — built; composition/search + none-proof, predicates fully stateable.
6. **`compare-two-attacks`** (S, comparative) — the "incomparable" option sharply tests refusal-to-invent-a-weighting.
(`struct-chokepoint` = honorable-mention 7th; needs a connectivity-with-removal solver + Bool.)

## New answer/scoring variants the corpus needs
- **`Bool`** — cf-vacate, cf-interdict, struct-chokepoint, constraint-path, evacuate-vs-hold. Cheap; add a negative self-test.
- **`Choice{A,B,neither/incomparable}`** — comparative kinds; "incomparable" is correct on genuine trade-offs.
- **`Order` / partial-order ranking (inversion scorer, incomparable pairs un-scored)** — rank-by-exposure. Highest-value new variant (raises resolution above the blunder-rate floor).
- **`Direction`** — adv-approach/flank. **Defer** (definitional risk).
- **Open assignment/matching** (units→cities) — **do NOT build**: breaks decisive-by-construction + clean scoring. Degrade to single-slot `triage-reinforce`.

## Tensions to design around (decisive + crisp + frozen)
- **Counterfactuals leak into dynamics** — frame every one strictly **this-turn** ("does blocking here stop them *this turn*"), or they become opponent-response/T4 questions.
- **"Good trade" resists crispness** — cross-unit value is a withheld function; keep exchange questions in the dominance frame (favorability × threat-removed, both measured), never "is it worth it."
- **Perspective assumes a rational adversary** — score against the enemy's *stated* axes in `rules_block`, not the actual AI (avoids the T4 AI-idiosyncrasy confound).
- **Ranking's incomparable pairs must be un-scored** — penalizing either order smuggles a weighting back in; get the inversion scorer right or the metric is corrupt.
