# CivSpatial — Question-Kind Roster: a holistic analysis

*Which kinds we have, which we need, which we can cut. Generated 2026-08-07 from the authoritative
`civ-eval/src/generators.rs` (`all_kinds`, 21 built kinds) cross-read against `QUESTION-CATALOGUE.md`,
`analysis/reasoning-frontier-questions-v2.md` (v2 + v2.6, the CURRENT frontier verdicts),
`analysis/findings-{tool-parity,calc,interactive}.md`, and `data/corpus/selection-optimized.md`
(per-kind fogged board-support). This is analysis only — no code changed.*

The lens throughout is **the three theses the kinds serve**, and a kind earns its slot only if it
supplies discrimination for at least one of them that no other kind already supplies:

1. **Encoding thesis** — raw vs ascii vs hierarchical vs interactive vs calculator. Served by
   perception/spatial kinds; the signal is *cross-encoding* spread (e.g. interactive cracks region-count
   that raw miscounts — `findings-interactive.md` Stage B/D).
2. **Tool–task-fit thesis (core result)** — generic spatial ops don't help decision kinds; only the
   *matching* combat/valuation ops win (maxops 0.92 ≫ spatial-ops 0.61 ≈ no-ops 0.64 —
   `findings-tool-parity.md`). Served by the T2/T3 decision kinds; the signal is the *arm* spread
   (no-ops → maxops) on a fixed decision kind.
3. **Reasoning-frontier thesis (newest)** — kinds where NO simple deterministic tool hits 100%, built on
   three tool-resistance engines: (a) hidden-information via fog, (b) underdetermined objective, (c)
   intractable; governed by the **luck ↔ tool-resistance duality** (`reasoning-frontier-questions-v2.md`
   §v2.0). Served by the frontier kinds (P1–P7, N1); the signal is *lift over the best fog-blind/proxy
   tool at large N*.

A kind can serve more than one thesis, and the strongest kinds do. The saturation traps differ by
thesis: a kind that saturates *with a calculator* (adv-assault-target → 1.00 under maxops) is not dead —
it is precisely the tool-fit thesis's evidence, because it discriminates *across arms*. Only a kind that
discriminates on **no** thesis is truly dead weight.

---

## 1. Inventory

### 1a. Built kinds (21, the `all_kinds` list in `generators.rs`)

| # | Kind (name) | Tier | Skill probed | Primary thesis | Answer shape |
|---|---|---|---|---|---|
| 1 | **terrain** | T0 | Point terrain readout | Encoding (floor) | terrain name |
| 2 | **adjacency** | T0 | Neighbour readout in a direction | Encoding (floor) | terrain name |
| 3 | **direction** | T0 | Compass bearing origin→target | Encoding (floor) | compass dir |
| 4 | **distance** | T1 | Chebyshev distance | Encoding | int |
| 5 | **nearest** | T1 | Unbounded global nearest-resource search | Encoding | int |
| 6 | **region-count** | T1 | Bounded local aggregation (count-by-exclusion of default) | Encoding (the residual hard perception skill) | int |
| 7 | **reachability** | T1 | Step-budget flood avoiding one terrain | Encoding | yes/no |
| 8 | **unit-strength** | T2 | Stated combat model: att/def_eff compare | Tool-fit | stronger unit |
| 9 | **city-defense** | T2 | Best-defender def_eff + walls/terrain compare | Tool-fit | better city |
| 10 | **best-site** | T1 | Best-of-K local terrain aggregation (player-agnostic) | Encoding (interactive's showcase win) | tile |
| 11 | **nearest-owned** | T1 real-play | Empire-relative multi-source (all cities) nearest-unexploited resource, turn-costed, horizon 6 | Encoding + **tool-parity** (the multi-source-BFS parity gap) | int / none |
| 12 | **reachable-nearest** | T1 real-play | Single-unit turn-costed nearest resource (Dijkstra, rail/road/river) | Encoding | int / none |
| 13 | **settle-site** | T3 | 4-axis Pareto siting (food/prod/resource/**safety**); safe-twin blunder trap | Tool-fit + Frontier (b) | ChoiceSet |
| 14 | **t3-retreat** | T3 | 3-axis Pareto retreat (cover/safety/support); safe-twin trap | Tool-fit + Frontier (b) | ChoiceSet |
| 15 | **t3-threat** | T3 | Single-axis argmax: which own city most likely to FALL | Tool-fit | city name |
| 16 | **cf-vacate** | T3 | Counterfactual: can the city spare its best defender | Tool-fit | yes/no |
| 17 | **adv-assault-target** | T3 | 2-axis Pareto (capture-prob × size), enemy POV | Tool-fit | ChoiceSet |
| 18 | **triage-reinforce** | T3 | Marginal allocation: reserve that FLIPS capturable→held | Tool-fit | city |
| 19 | **compare-two-attacks** | T3 | 2-axis Pareto (exchange × threat-removed) + genuine "incomparable" | Tool-fit (**survives tooling**) | better / incomparable |
| 20 | **constraint-site** | T3 | Hard 3-predicate conjunction + none-proof (near-miss decoys) | Tool-fit (**hardest under tooling**) | ChoiceSet / none |
| 21 | **forward-posting** | T3 | Adversarial 1-ply: pressure × survival-after-enemy-reply Pareto | Frontier (b) + adversarial [**P4, BUILT**] | ChoiceSet |

### 1b. Designed-but-unbuilt frontier kinds (`reasoning-frontier-questions-v2.md`)

| Kind | Engine | Verdict (v2.6) | Skill probed | Buildable now? |
|---|---|---|---|---|
| **P1** surprise-strike exposure | (a) hidden-info | CHAMPION, 2 fixes | Which own city about to be hit by an unseen attacker (hidden **attacker** near a city) | Yes — needs unmasked-board pipeline (§v2.3) |
| **P3** hidden-force localization | (a) hidden-info | CHAMPION, history-hungry | Which fogged **region** hides the massed force (hidden force **magnitude**) | Yes on coarse margin — shares P1's pipeline |
| **P7f** fogged-garrison assault | (a) hidden-info | MODIFIED (fogged only) | Can I take their city whose **garrison strength** is fog-hidden | Yes — pipeline + fallen-defender sequencing rule; **cleanest snapshot-honest** hidden-force kind |
| **N1** exposed-maneuver | (a) hidden-info | **DEFERRED — history-gated** | Is my moving unit walking into an ambush by an unseen striker | **No** — blocked until a turn-history corpus ships |
| **P5** contested land-grab | (b) underdetermined | MODIFIED (denial axis) | Expansion-**denial** value of a frontier site, gated on claim-feasibility | Yes but **at-risk** of collapsing into settle-site's safety axis |
| **P2** concealment "are you seen?" | — | **DROPPED** | (visibility from visible enemies = tool-trivial; hard core folded into N1) | — |
| **P6** scouting priority | — | **DROPPED** | (duality-forbidden: every version is either luck or tool-trivial) | — |

---

## 2. Coverage map by thesis

### 2a. Encoding thesis — ADEQUATE, one high-leverage gap

Served by kinds 1–7, 10, 11, 12. The gradient is deliberate: T0 (1–3) is the saturated floor; the
residual hard perception skill is **local aggregation** — `region-count` (6) and `best-site` (10) — which
is exactly where the encodings separate (`findings-interactive.md`: interactive 6/6 vs raw 5/6,
hierarchical 3/6 on region-count; interactive 8/8 vs raw 4/8 on best-site). `nearest` (5) is the
documented **interactive-pathology** demonstrator (unbounded global search → cap-out;
`findings-interactive.md` Stage A). The calculator collapses all of these to ceiling on a *small* board
(`findings-calc.md`: region-count → 24/24 with a `count` op) but the collapse is itself an encoding-thesis
result, and at *scale* a second "locate the entities" ceiling (~0.75) re-emerges — so these kinds still
carry live signal across the perception × compute grid.

- **GAP (highest leverage, backs a claim with no kind): global / whole-board aggregation.** The catalogue
  (§6 "Global/aggregative reasoning: **None**", §7 gap #1) and RESUME both predict that *whole-board
  aggregates* — "which quadrant has the most X", "which player controls the most territory", "how many X
  on the whole board" — are the query shape where hierarchical/interactive should most separate from flat
  `raw`, and there is **zero** coverage. Every current kind is point / bounded-radius / small-candidate.
  This is the one place the encoding thesis makes a strong prediction with no kind to test it. It is a
  question-design change, not a bigger board. **BUILD.**
- **REDUNDANCY:** the three "nearest resource" kinds (5 `nearest`, 11 `nearest-owned`, 12
  `reachable-nearest`) overlap on the surface skill "find the nearest resource," but probe three
  *distinct* movement models — global Chebyshev (5), multi-source empire BFS turn-costed (11), single-unit
  Dijkstra turn-costed (12) — and `nearest-owned` additionally carries the entire tool-parity Result 2
  (`findings-tool-parity.md`: the missing multi-source primitive). Not true redundancy. The weakest of the
  three is **`nearest` (5)**: it collapses under a calculator, breaks interactive, and its movement model
  (naive Chebyshev, ocean ignored) is the least game-real. It survives only as the interactive-failure
  case — treat it like the T0 floor kinds (keep in code, cut from most runs).

### 2b. Tool–task-fit thesis — STRONG, well-covered

Served by the T2 pair (8, 9) and the T3 decision kinds (13–20). The core result rests on 6 of them
(`findings-tool-parity.md` Result 1): adv-assault-target, settle-site, cf-vacate, triage-reinforce,
compare-two-attacks, constraint-site — showing no-ops 0.64 ≈ spatial-ops 0.61 ≪ maxops 0.92, and that the
*wrong* tools even hurt (triage-reinforce 0.58 → 0.33 under spatial-ops). Coverage of the decision-kind
space is broad: comparison (unit-strength, city-defense, compare-two-attacks), argmax (t3-threat),
Pareto-frontier (settle-site, t3-retreat, adv-assault-target), counterfactual (cf-vacate), marginal
allocation (triage-reinforce), predicate-composition (constraint-site).

- The **discrimination survivors under full tooling** are the valuable ones: `constraint-site` (0.79) and
  `compare-two-attacks` (0.88) still separate *with* maxops — the "still-hard-even-with-tools" headliners.
  `adv-assault-target` and `settle-site` **saturate to 1.00** under maxops; that is not a defect (it is the
  tool-fit evidence — they discriminate across *arms*), but they add little *within* the maxops arm.
- **REDUNDANCY (mild): `t3-threat` (15) vs `adv-assault-target` (17).** Both rank cities by
  `city_capture_prob`. t3-threat is the single-axis argmax (your cities, defense POV); adv-assault-target
  adds the size axis (enemy POV). t3-threat's single axis is the most tool-trivial decision kind — a
  matching capture-prob op nails it — so it carries the *least* frontier value; keep it only as the
  defense-POV mirror and the cheapest exposure-ranking probe.
- **REDUNDANCY (structural, not behavioural): the safe-twin dominance triplet** `settle-site` (13),
  `t3-retreat` (14), `forward-posting` (21) share an identical *construction* (trap + identical-non-safety-
  axes safe twin + non-dominating distractors + ChoiceSet scorer). They are **not** redundant as probes —
  siting vs retreat vs advance are different decision domains with different axes — but the shared scaffold
  means a bug or a scorer artefact in one likely afflicts all three; treat them as one family for
  validation.
- **GAP:** economic valuation beyond city *size* (assault value = size only; no production/yield/gold
  city-vs-city), and multi-unit / stack combat. Both are documented fidelity simplifications (catalogue §6,
  §7 #3), blocked partly on decode. Not urgent for the first writeup.

### 2c. Reasoning-frontier thesis — PARTIAL; one built, the payoff pair pending a pipeline

Served today by exactly one built kind, **`forward-posting` (21, P4)** — engine (b) + the novel 1-ply
"let the enemy move before you evaluate" mechanic. The rest of the family (P1, P3, P7f) is designed and
buildable but unbuilt; N1 is deferred; P2/P6 dropped.

- **GAP:** the whole engine-(a) **hidden-information** payoff — the thing "nothing in the field has"
  (`reasoning-frontier-questions-v2.md` §v2.5) — has **no built kind**. The fogged 15-board corpus now
  exists (`selection-optimized.md`), so the corpus precondition is met; what is missing is the one-time
  **unmasked-board pipeline** (§v2.3: solve ground truth on the true board, render the masked board, plus
  a leak-invariant test). Building it unlocks P1 + P3 + P7f together.
- **No redundancy inside the frontier family** — the v2 table (§v2.2) deliberately factored P1/P3/N1/P7f
  into one engine with four *distinct* decision-framings (defense / intel / maneuver / offense), and
  dropped P2 precisely because its core was redundant with N1.

---

## 3. Tool-resistance audit (frontier kinds)

Applying the duality (`§v2.0`): a kind is engine-(a) tool-resistant **iff** its answer depends on hidden
fog contents — which is exactly what makes it a *prediction* with residual luck. Honesty comes from (1)
reasoning-rejectable "provably-safe" decoys as a luck-free floor, (2) a curated decisive+grounded true
answer, (3) reporting *lift over the best fog-blind proxy at large N*. Engine (b)/(c) kinds resist
differently (underdetermined objective / intractability).

| Candidate | Genuinely tool-resistant? | Does a simple counter/aggregate/solver nail it? | Verdict |
|---|---|---|---|
| **P4 forward-posting** (BUILT) | **Yes, engine (b)** — Pareto non-dominance has no canonical pressure↔survival weight (same resistance as settle-site). The 1-ply enemy-reposition search is what the *static* `ThreatField` structurally under-reads. | A *matching* posting tool could compute it (like maxops solves the others) — but that is tool-*fit*, not a 100% deterministic shortcut; the underdetermined weight is genuinely resistant. | KEEP (built). |
| **P1 surprise-strike** | **Yes, engine (a)** — depends on the masked-out attacker. Duality-honest via the **provably-safe decoy** (a candidate city with no fogged tile in strike range cannot be the answer, provable without seeing fog → luck-free floor). | Fog-blind `ThreatField` ≈ 0; nearest-visible-enemy and any-adjacent-fog proxies are beatable by curation. No proxy nails it. Latent speed-dependence handled by disclosing the public roster + K=6 horizon (§v2.6.4). | BUILD (needs pipeline). |
| **P3 hidden-force localization** | **Yes, engine (a)** but **no luck-free floor** — standing units defeat reach-grounding (a masked stack could have sat there pre-fog). Survives on a *coarse, decisive-margin* summed-`att_eff` quantity that tolerates grounding noise. | "Biggest region" and "closest-to-visible-enemy" proxies are the baselines to beat; curation makes them wrong on a fraction. History would raise honesty most of any surviving kind. | BUILD with P1 (shares pipeline); flag history-hungry. |
| **P7f fogged-garrison assault** | **Yes, engine (a)**, and the **cleanest snapshot-honest** one: the hidden quantity is a *static* garrison whose **location is known (the city)** — so no speed-leak (critique 2 n/a) and no history-localization (critique 3 n/a); only strength is hidden. | The fog-blind "0 visible defenders → trivially takeable" tool is *systematically wrong* on every defended city — that gap is the whole point. Decisive-band curation (best-case and greedy orderings agree) tames critique 1. | BUILD (needs pipeline + fallen-defender sequencing rule). |
| **N1 exposed-maneuver** | Engine (a) **only if** it depends on realized hidden contents — and grounding a *single-tile, single-turn* strike honestly (above a coin flip) **requires turn history** (§v2.6.1 critique 3). Any history-free redesign either becomes a reachability **tool** (duality trap) or **collapses into P4**. | On the snapshot the predictive candidate is under-grounded → score leans almost entirely on the provably-clear-destination floor with a near-coin-flip among the rest → a thin kind. | **DEFER** (turn-history-gated); reframe binary → argmax selection when built. |
| **P5 contested land-grab** | Engine (b) — denial value trades against economy/exposure with no canonical weight. | **At-risk:** if the denial axis produces no non-dominated instances *distinct from* what settle-site's safety axis already captures, it collapses **into** settle-site and is redundant. | CONDITIONAL: run a generation probe first; build only if denial is distinct. |
| **P2 / P6** | No. P2's hard part is tool-trivial visibility bookkeeping (folded into N1). P6 is duality-forbidden (every version is luck or trivial). | Yes — a `vision_radius_sq` disk (P2) / a reachability computation (P6) nails the tractable core. | **DROP** (correctly already dropped). |

**What to build next given the fogged corpus now exists.** The corpus precondition (fogged 15-board
support) is satisfied for the snapshot-honest kinds. The gate is the *unmasked-board pipeline* (paid once,
amortized across the family — §v2.3), not the corpus:

- **Buildable now (snapshot-honest, need the shared pipeline):** **P1**, **P3**, **P7f**. P7f is the
  least information-leaky and least history-dependent (§v2.6.4) and can proceed without waiting on
  anything else; P1 + P3 should be built together to pay the pipeline seam once.
- **Blocked until a turn-history corpus ships:** **N1** (fatally history-dependent) and, as a
  *strengthener not a blocker*, P3's honesty (it is buildable now on the coarse margin, materially better
  later). Turn-history is now a first-class corpus gate (§v2.6.3: future saves retain ~3 preceding turns);
  it also unlocks the broader multi-turn/sequential-planning gap from the catalogue.

---

## 4. Recommendation — KEEP / DROP / MERGE / BUILD

Prioritised, and split into **first-writeup** (the three theses as currently framed) vs **later**.

### KEEP (earning their slot now)

- **The tool-fit survivors:** `constraint-site`, `compare-two-attacks` — the only decision kinds that
  *still discriminate under full tooling* (0.79, 0.88). Highest-value keeps; feature them.
- **The tool-fit saturators:** `adv-assault-target`, `settle-site`, `cf-vacate`, `triage-reinforce` — keep
  for the *arm* spread (no-ops → maxops) that IS the core result, even though they saturate within maxops.
- **The T2 pair** `unit-strength`, `city-defense` — cheap, decisive, the stated-combat-model floor.
- **The encoding residual-hard pair** `region-count`, `best-site` — the interactive/hierarchical
  showcase; keep as the encoding thesis's live discriminators.
- **The movement pair** `nearest-owned` (also carries tool-parity Result 2), `reachable-nearest`.
- **`forward-posting` (P4)** — the only built frontier kind; keep.
- **`distance`, `reachability`** — cheap encoding-grid coverage; low cost, keep.

### DROP (from experiment runs; keep in code as a floor/demonstrator)

- **T0 `terrain`, `adjacency`, `direction`** — saturated on raw; already "kept in code but cut from
  experiment runs" (catalogue §1). Confirm they stay cut.
- **`nearest`** — the weakest of the three nearest-variants: collapses under calculator, breaks
  interactive, least game-real movement model. Demote to the same status as T0 (keep in code as the
  interactive-failure demonstrator; cut from routine runs). Its distinct signal is fully covered by
  `nearest-owned` + `reachable-nearest`.
- **P2, P6** — stay dropped (duality-forbidden / tool-trivial). No action.

### MERGE / de-duplicate (validation, not deletion)

- **`t3-threat`** overlaps `adv-assault-target` on the capture-prob axis and is the most tool-trivial
  decision kind (single-axis argmax). Do **not** delete — keep it as the cheap defense-POV mirror — but do
  not count it as independent frontier evidence; it is the weakest discriminator of the decision set.
- Treat the **safe-twin triplet** (`settle-site`, `t3-retreat`, `forward-posting`) as one construction
  family for scorer validation (shared scaffold → shared failure modes).

### BUILD

**For the first writeup (in priority order):**

1. **Global / whole-board aggregation kind** *(encoding thesis — the one gap that backs a stated claim
   with no kind)*. E.g. "which quadrant holds the most {terrain/cities}", "which player controls the most
   tiles", "how many {X} on the whole board". Zero current coverage; RESUME predicts it is where
   hierarchical/interactive most separate from raw. Cheapest high-leverage build (question-design only, no
   pipeline, no corpus change).
2. **P1 + P3 together** *(frontier thesis — the engine-(a) hidden-information payoff)*. Pay the
   unmasked-board pipeline once (§v2.3); get the defense-framed (P1, with its luck-free provably-safe-decoy
   floor) and intel-framed (P3, coarse decisive margin) hidden-force kinds. This is "the thing nothing in
   the field has." The fogged corpus is ready; the pipeline is the only lift.
3. **P7f (fogged-garrison assault)** *(frontier thesis, snapshot-honest)*. Cleanest of the hidden-force
   family — static garrison, no speed-leak, no history need. Build alongside P1/P3 (reuses the pipeline;
   adds a fallen-defender sequencing rule).

**Later (deferred or conditional):**

4. **N1 (exposed-maneuver)** — **blocked** on a turn-history corpus (§v2.6.3). Build after history ships,
   as an argmax *selection* (not a per-move binary, not a probability emission). It is the only kind that
   tests "is my *maneuver* walking into an ambush" — a distinct tempo/exposure dimension — so defer, don't
   kill.
5. **P5 (contested land-grab, denial axis)** — **conditional**: run a generation probe first; ship only if
   the denial axis yields non-dominated instances *distinct from* settle-site's safety axis. Otherwise drop
   as redundant.
6. **Economic-valuation-beyond-size** and **multi-turn/sequential-planning** kinds — real catalogue gaps
   (§7 #3, #4) but blocked on decode / turn-history; naval/amphibious (§7 #5) is lower priority. All
   later.

### Near-saturated / low-discrimination flags (cost without signal)

- Within the **maxops** arm: `adv-assault-target`, `settle-site` (→1.00), `cf-vacate` (0.96),
  `triage-reinforce` (0.92) add little *within-arm* discrimination — they earn their slot only via the
  cross-arm spread. If run budget is tight, sample them lightly in the maxops arm and spend the budget on
  `constraint-site` / `compare-two-attacks` (the survivors) and the frontier kinds.
- `t3-threat` — single-axis, most tool-trivial decision kind; lowest discrimination of the T3 set.
- Early-board corpus support is thin for the decision kinds (`selection-optimized.md`: boards 12–14 yield
  only 1–2/10 discriminating kinds; settle-site support = 10, the corpus's weakest). Any new decision or
  frontier kind should be budgeted against *per-(kind × board)* support, since a board can legitimately
  yield **zero** of a kind (§v2.6.5) — target enough *supporting* boards, not just enough boards.

---

## 5. One-paragraph synthesis

The 21 built kinds cover the encoding and tool-fit theses well and the frontier thesis barely (one built
kind, P4). The two concrete gaps that back a *stated claim with no kind* are (1) **global/whole-board
aggregation** for the encoding thesis — cheap to build, no pipeline — and (2) the **engine-(a)
hidden-information** payoff (P1/P3/P7f) for the frontier thesis, whose corpus precondition is now met and
which is gated only on a one-time unmasked-board pipeline. Redundancy is minor and mostly benign
(`nearest` is the one demotion; `t3-threat` the one weak-discriminator to keep-but-discount; the safe-twin
triplet shares a scaffold to validate together). N1 is correctly deferred to a turn-history corpus, and
P5 is a conditional probe. Nothing needs deleting outright beyond confirming the saturated T0 kinds and
`nearest` stay cut from routine runs.
