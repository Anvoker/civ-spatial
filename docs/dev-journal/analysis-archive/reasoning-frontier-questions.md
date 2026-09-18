# Reasoning-Frontier Question Kinds — Proposal

> **Revised — see [`reasoning-frontier-questions-v2.md`](reasoning-frontier-questions-v2.md).** After owner feedback the verdicts changed: P2 & P6 DROPPED, P1/P3/P4 CHAMPIONED, P5/P7 MODIFIED, new **N1** added, and P1/P3/N1/P7-fogged unified into one Hidden-Force family. The proposals below are the original **v1**, kept as the reasoning trail.

*Design/ideation only. No implementation. Proposes new question kinds that test HIGHER strategic
reasoning while satisfying the hard filter: **no simple deterministic tool solves them at 100%.**
Written 2026-08-05 against `question.rs`, `rules.rs`, `board.rs` (three-state fog), `T2-T3-design.md`,
and `QUESTION-CATALOGUE.md`. Goes past the catalogue's flagged gaps — deliberately AVOIDS the
"global aggregates" gap it ranked #1, because a counter tool nails aggregates exactly (see §Rejected).*

---

## 0. The filter, sharpened (so every proposal below is honest about it)

The project's own T3 kinds already "have a deterministic oracle," yet they qualify as tool-resistant.
Reconciling that with "no tool solves it at 100%" gives three — and only three — legitimate engines of
tool-resistance. Every proposal names which one(s) it rests on:

- **(a) Hidden information.** The answer is **not a function of the board the model is shown.** Under
  three-state fog the model sees terrain/owner/last-known cities on fogged tiles but *not* live enemy
  occupants. So a tool run on the model's inputs is **structurally unable** to compute the answer — the
  determining state is absent. The generator holds the *unmasked* board, so it can still score. This is
  the strongest engine and the newest, least-explored vein.
- **(b) Underdetermined objective.** The optimum is **not well-defined** (no principled weighting across
  axes, or the *opponent's* objective is unknown), so no algorithm can output "the answer." We score
  only a **necessary condition** (Pareto non-dominance / robustness), exactly as settle-site does.
- **(c) Intractable combinatorics + partial verification.** The exact optimum is super-polynomial
  (expectimax over sequences × probabilistic combat) AND we verify only a **feasibility / bound**, never
  the optimum. Weak on its own (small game trees are tool-solvable), so used only with (a) or (b).

A necessary discipline for (a)/(c): several of these are **prediction tasks whose ceiling is below
100% for everyone, including the model.** That is a feature — the metric is the *gap* between a
fog-blind matching tool and a model that reasons about the information state, not an absolute score.
Where that is the case it is stated plainly.

---

## 1. Ranked proposals

Ranked by (tool-resistance × strategic value × verifiability).

---

### P1 — Surprise-strike exposure  *(fog / hidden-info)*  ★ top pick

**Concept.** Of your cities, which is about to be hit by an enemy you currently **cannot see**?

**Draft phrasing.**
> You are player {P}. Some of the map is fogged: on fogged tiles you remember the terrain but cannot
> see who stands there now. Considering everything visible to you AND what could be hidden in the fog,
> which ONE of your cities is most likely to come under attack THIS turn from an enemy unit you cannot
> currently see: {list of your cities}? Answer with that city's name.

**Strategic relevance.** The defining anxiety of fog play: an undefended border city looks safe on your
screen precisely because the stack that will take it is one tile inside the fog. Good players garrison
against *what the fog could hide near an enemy source*, not against what is visibly adjacent.

**Tool-resistance — engine (a), the cleanest in the set.** Curate each instance so a real enemy land
unit sits on a **fogged** tile within striking range of exactly one candidate city. On the board the
model is shown, that unit **does not exist** — so `ThreatField` (the exact matching tool) run on the
model's input returns **0 threat for every candidate**. The determining fact is absent from the model's
world; no tool over the model's inputs can be even non-trivially correct. The one over-approximating
tool a model might reach for — "which candidate has enemy-reachable fog adjacent" (a BFS over the masked
board from visible enemy sources) — flags *every* border city with fog near it, including the decoys
whose fog is empty, so it **cannot be exact either** (systematic false positives). The only route to the
answer is graded inference: this fogged pocket is fed by a visible enemy city/stack within move range
and channels toward *my* city, that one does not.

**Partial verification / scoring.** Ground truth = the threatened city, computed on the **unmasked**
board via the existing `ThreatField` (an attacker on the now-visible tile clears a fall-probability
band). `Answer::Choice` over the player's own city names — the scorer stays board-free. **Curation
conditions** (generator, which holds both boards):
1. Exactly one candidate has a fog-hidden land attacker in range on the true board (fall-prob ≥
   `WIN_PROB_HIGH` band), and it is **grounded**: that attacker is itself reachable within its move
   rate from a source *visible* to P (an enemy city or a visible enemy unit), so the inference has a cue
   and is not a blind coin flip.
2. Decoy cities have fog nearby but **no** actual hidden attacker (empty fog), so the fog-reachability
   over-approx tool is provably wrong, isolating information reasoning from bookkeeping.

Report as **accuracy vs the fog-blind `ThreatField` baseline** (which scores ~0 on these by
construction) — the headline is the *lift* a reasoning model gets over the matching tool. Honest ceiling
is below 100% (it is a prediction), which is the point.

**Data feasibility.** Needs three-state fog (built) + `ThreatField` (built). No decode gap. Generation
must run the solver on the unmasked board while the model sees the masked one — the generator already
has both (fog is applied downstream of parse). Aligns with the fog-perspective coherence rule
(`fog-three-state-design.md` §9): P = fog perspective = question's player.

**Cost / novelty.** Medium cost, **very high novelty** (nothing in the field scores spatial
*information* reasoning). Sketch: new `Question::SurpriseStrikeChoice{player, candidates}`; generator
computes the unmasked `ThreatField`, keeps instances meeting the two curation conditions, stores the
true city as `Answer::Choice`.

---

### P2 — Concealment: "are you seen?"  *(fog / reverse hidden-info)*  ★ novel

**Concept.** Which of *your* units can currently reposition **unseen** — i.e. lies outside every
enemy's sight?

**Draft phrasing.**
> You are player {P}. An enemy can only react to units it can see; a unit hidden from all enemies can
> move or mass in secret. Of your units {list}, which ONE is currently HIDDEN from every enemy — not
> within sight of any enemy unit or city? Answer with that unit (type + coordinates).

**Strategic relevance.** Concealment is the basis of every flank march, ambush, and secret buildup.
Knowing which of your pieces the enemy has *not* spotted tells you where you can maneuver without
inviting a response — a genuine, constantly-made judgment.

**Tool-resistance — engine (a).** To know whether *your* unit is seen you must compute the **enemy's**
vision, whose sources are enemy units — many of which are themselves **fogged from you.** So on the
model's inputs the enemy sight-map is only partially known; a tool over P's fogged board **cannot**
compute enemy visibility exactly. The model must infer: "these enemy units I *can* see project sight
disks (`vision_radius_sq`) that cover my unit → seen; this unit of mine sits deep in my territory with
no visible enemy within sight range → probably unseen, though a hidden enemy scout is always a residual
risk." Ground truth *is* computable — but only from the **unmasked** board the generator holds, not from
anything the model or a model-side tool can see.

**Partial verification / scoring.** Ground truth: on the unmasked board, is the unit inside any enemy
source's `vision_radius_sq` disk (using the shipped classic radii already in `board.rs`)? `Answer::Choice`
over P's units; the acceptable answer is a unit that is **truly unseen**. Curate so exactly one candidate
is unseen and the rest are seen **by a P-visible enemy source** (so "seen" is inferable) — this keeps the
distractors honestly rejectable and concentrates the difficulty on the unseen one, whose safety the model
must argue from *absence* of nearby visible enemies. Report accuracy; ceiling < 100% (residual hidden
scouts), which correctly models real concealment uncertainty.

**Data feasibility.** `visible_grid`/`unit_vision_radius_sq` already compute per-source sight; the
generator computes enemy sight on the unmasked board. **Decode caveat:** the Fortress (+8) and Mountains
vision bonuses are the documented un-modelled approximation (`fog-three-state-design.md` AS-BUILT #1) —
curate candidates away from enemy fortresses/mountain-top spotters so the un-modelled bonus can't make a
"seen" unit read "unseen." No new decode needed.

**Cost / novelty.** Medium cost, **very high novelty** (reasoning about the opponent's information, not
just your own). Sketch: `Question::ConcealmentChoice{player, candidates}`; generator computes enemy
`vision_radius_sq` coverage on the unmasked board.

---

### P3 — Hidden-force localization  *(fog / hidden-info)*

**Concept.** The fog hides enemy strength somewhere near your front. Which fogged region most likely
harbors the enemy's massed force?

**Draft phrasing.**
> You are player {P}. Enemy units are massing somewhere in the fog along your frontier. Of these fogged
> regions, each centered on {A}, {B}, {C}, {D}, which most likely conceals the enemy's largest attacking
> force? Answer with that region's label.

**Strategic relevance.** Reading the enemy's main effort from indirect cues — border shape, which of
their cities is productive/forward, terrain that channels an advance — is the core of defensive
positioning and of committing your own reserve before you can see the blow.

**Tool-resistance — engine (a).** The quantity asked (enemy attack-power actually present in a fogged
region) is **not present** in the model's board — the units are masked out. A tool on the model's inputs
can at best measure *proxies* (region size, distance to visible enemy sources, terrain), which do not
determine the true occupancy → cannot be exact. The generator scores against the **real** hidden
occupancy on the unmasked board.

**Partial verification / scoring.** Ground truth = the region whose fogged tiles contain the greatest
summed enemy `att_eff` on the unmasked board. `Answer::Choice` over region labels. Admit an instance
only when the true leader beats the runner-up region by a decisive margin (reuse the margin discipline)
**and** the leader is inferable — it is the region nearest a visible enemy source and/or its own
territory, so a well-reasoned prior can find it. This is a **prediction-match** kind; report accuracy and
the lift over a naive "biggest region" or "closest-to-visible-enemy" tool baseline (both of which the
curation can make wrong on a fraction of instances).

**Data feasibility.** Fog + unit stats (built). Regions = fixed-radius windows on fogged tiles.
No decode gap. **Honest caveat:** verifiability is softer than P1/P2 — occupancy is genuinely partly
random, so keep only instances where visible cues make one region a defensible favorite; otherwise this
degrades toward a guess and should be dropped at generation.

**Cost / novelty.** Medium cost, high novelty. Sketch: `Question::HiddenForceChoice{player, regions}`;
generator sums masked-out enemy `att_eff` per region on the unmasked board.

---

### P4 — Robust forward posting under enemy response  *(adversarial + underdetermined)*

**Concept.** You must push one unit forward to pressure the enemy. Which advance is **sound** once the
enemy repositions to punish it?

**Draft phrasing.**
> You are player {P}. To pressure the enemy you will advance {unit} to one of {candidate tiles}. Assume
> the enemy then moves to punish you. Following the POSTING RULES above, which advance is SOUND — one
> that is NOT clearly worse than another when you weigh the pressure it creates against the risk of being
> destroyed on the enemy's reply: {list}? Answer with the coordinates of any sound advance.

**Strategic relevance.** Every aggressive maneuver trades tempo/pressure against exposure. Overextending
a lone unit into a pocket where the enemy caves it in next turn is the textbook blunder this catches.

**Tool-resistance — engines (b) + adversarial.** Two axes, weighting **withheld** (engine b, exactly the
settle-site escape): **pressure created** (enemy attack-power / soft cities the posting now threatens) vs
**worst-case survival** (1 − max over the enemy's *reachable repositions* of their kill-probability
against your unit at the new tile). The survival axis is the novelty: it requires a **1-ply opponent
search where the enemy moves TO exploit your posting** — the *static* `ThreatField` (which threatens
tiles from where enemies stand *now*) systematically **under-reads** a posting the enemy would step over
to hit. Because the trade-off between aggression and survival has no principled weighting, no algorithm
outputs "the best" posting; we score only the non-dominated frontier.

**Partial verification / scoring.** Precompute per candidate: `pressure` (sum of `att_eff` of enemy
units, or `city_capture_prob` of enemy cities, the posting brings into your reach) and `survival`
(1 − worst-case enemy kill-prob, computing enemy reach with the same Dijkstra + `win_probability` the
project already has, but from the enemy's post-move positions). Pareto non-dominated set →
`Answer::ChoiceSet`. A candidate another decisively dominates on **both** axes is the scored blunder.
Necessary-condition scoring identical in shape to t3-retreat; the only new machinery is a "reachable
enemy repositions" pass, which is a bounded reuse of existing Dijkstra.

**Data feasibility.** All built (Dijkstra reach, `win_probability`, `att_eff`, `city_capture_prob`).
No decode gap. Works omniscient or fogged (fogged makes the enemy-reposition search itself uncertain — a
natural harder variant that also gains engine (a)).

**Cost / novelty.** Medium-high cost (the opponent-reposition pass), high novelty (first kind that lets
the enemy *move* before evaluating). Sketch: `Question::ForwardPostingChoice{player, unit, candidates}`
+ a `best_response_kill_prob(board, my_unit_at, enemy_owner)` helper.

---

### P5 — Contested-frontier land grab  *(underdetermined, opponent-aware siting)*

**Concept.** Which unclaimed site should you settle **now**, weighing its own worth against the risk the
enemy takes it (or the border) first?

**Draft phrasing.**
> You are player {P} racing an enemy to expand. Following the LAND-GRAB RULES above, which of these
> unclaimed sites is a SOUND one to found on NOW — weighing the site's own value against how CONTESTED
> it is (how close an enemy settler or city is to claiming it or its borders): {list}? Answer with the
> coordinates of any sound site.

**Strategic relevance.** Early-game expansion is a race: a rich site the enemy will grab first (or wall
you off from) is worth less than a modest site you can secure. This is the *timing/denial* dimension the
catalogue flags as missing from siting (§6 "no founding-timing").

**Tool-resistance — engine (b).** Extends settle-site with a **contestedness** axis (proximity of enemy
settlers/cities/borders that could claim the site or its working radius first) alongside the existing
food/production/resources/safety. The weighting of "grab the rich contested tile before they do" vs
"take the safe uncontested one" is a genuine strategic preference with no canonical value — so the
optimum is undefined; only non-dominance is scored. Note this is **more novelty of axis than of engine**:
the resistance is the same withheld-weighting (b) as existing kinds, honestly.

**Partial verification / scoring.** Add a `contested` axis to a settle-variant `SiteAxes` (e.g. negative
of "min turns for an enemy settler/city to reach and claim the tile"), then the same Pareto
`Answer::ChoiceSet`. Precomputed at generation, board-free scorer.

**Data feasibility.** Owner/territory decoded; settler units decoded; Dijkstra reach built. **Minor
decode note:** identifying enemy *settlers* specifically uses the unit `kind` (available). No gap.

**Cost / novelty.** Low-medium cost (one more axis on existing settle machinery), medium novelty. Sketch:
`Question::ContestedSiteChoice{player, candidates}` reusing `site_axes_with` plus a contestedness term.

---

### P6 — Scouting priority under uncertainty  *(fog / info-value, underdetermined)*

**Concept.** With one scouting action, which fogged tile should you reveal to best resolve your threat
uncertainty?

**Draft phrasing.**
> You are player {P}. You can send a scout to reveal ONE fogged area this turn. Following the SCOUT RULES
> above, which of these fogged tiles is a SOUND one to reveal — one whose contents most bear on whether
> your cities are safe: {list}? Answer with the coordinates of any sound choice.

**Strategic relevance.** Directing limited reconnaissance is a real, recurring decision; you scout where
the *answer would change your plan*, not where you are already confident.

**Tool-resistance — engines (a) + (b).** The *value* of revealing a tile depends on what is actually
there (hidden, engine a) AND on a subjective weighting of "how much this reduces danger to my assets"
(engine b). No tool on the model's inputs can compute the realized information gain (contents unknown),
and the value function over "uncertainty reduced" is underdetermined.

**Partial verification / scoring.** This is the **hardest to verify** in the set — be candid. Two viable
scorers: (i) **realized** — score against the tile that, on the unmasked board, actually harbors the
threat most relevant to P's cities, banded (partial credit for revealing an *adjacent* pocket); or (ii)
**soundness** — a choice is a blunder only if it reveals a tile that is provably irrelevant (deep in
already-safe, enemy-free space) while another candidate sits on the approach to a genuinely at-risk
city. Prefer (ii): it is a clean necessary condition (`Answer::ChoiceSet`, board-free) and avoids scoring
luck. If neither scorer yields decisive instances at generation, **drop the kind** — flagged honestly.

**Data feasibility.** Fog built. No decode gap. Verifiability is the risk, not data.

**Cost / novelty.** Medium cost, high novelty; **medium confidence it clears the verifiability bar** —
included because the vein is important, with the explicit fallback of dropping it if soundness instances
prove too rare.

---

### P7 — Multi-unit assault feasibility  *(combinatorial + partial verification)*

**Concept.** Given several attackers that can all reach an enemy city/stack, is it takeable this turn —
and which commitment is part of a plan that takes it?

**Draft phrasing.**
> You are player {P}. Your units {list} can all reach the enemy {target} this turn. Under the odds model
> above — where a defender that falls exposes the next behind it — is there a way to commit your units
> that captures {target} this turn with good odds? If so, name a first attacker that belongs to a
> capturing plan; if not, answer "no".

**Strategic relevance.** Deciding whether to launch an all-in assault, and in what order to feed
attackers so early kills soften the stack for later ones, is core siege reasoning (the catalogue's §6
"no multi-unit / stack combat", "no multi-turn siege" gap).

**Tool-resistance — engine (c), used carefully.** Ordering k attackers against a multi-unit defense with
**path-dependent probabilistic outcomes** (each attack may fail; a kill changes who defends next) is an
**expectimax over orderings × outcomes** — super-polynomial in k and stack depth. Be candid: for small k
a tool can brute-force it, so we do **not** ask for the optimal order (that would be tool-solvable). We
ask a **feasibility** question and verify only a **necessary/bounded** condition.

**Partial verification / scoring.** Score the **yes/no capturability** with decisive bounds computed at
generation: `no` iff even the **best-case** ordering's success probability is below a low band (an upper
bound: greedily always attack the current weakest defender, take the max), `yes` iff a **greedy** plan
already clears a high band (a lower bound). Keep only instances where the two bounds agree (decisively
`yes` or decisively `no`) so the answer is un-litigable without solving the full expectimax; drop the
ambiguous middle. `Answer::Bool` for the core; an optional `Choice` of a valid first attacker can be
graded against membership in any bound-certified plan. The model, lacking the enumerator, must reason
about the exchange sequence — which is the skill.

**Data feasibility.** `win_probability`, `defenders_ranked`, reach all built. **Modeling note:** requires
a stated rule for how one attack's damage/kill carries to the next attacker's problem (the "fallen
defender exposes the next" rule) — a small, honestly-stated extension of the existing single-attack odds
model, not a full battle sim.

**Cost / novelty.** Higher cost (bound computation + a stated sequencing rule), high novelty. Lowest-
ranked because its tool-resistance leans on (c), the weakest engine, and it needs the most new modeling.

---

## 2. Ranking summary

| # | Kind | Engine(s) | Verifiability | Novelty | Overall |
|---|---|---|---|---|---|
| P1 | Surprise-strike exposure | (a) hidden-info | exact vs hidden truth, curated-inferable | very high | **★ strongest** |
| P2 | Concealment "are you seen?" | (a) hidden-info | exact vs true enemy vision | very high | **★ strong** |
| P3 | Hidden-force localization | (a) hidden-info | prediction-match, banded | high | strong |
| P4 | Robust forward posting | (b) + adversarial | Pareto non-dominance | high | strong |
| P5 | Contested-frontier land grab | (b) underdetermined | Pareto non-dominance | medium | solid |
| P6 | Scouting priority | (a)+(b) | soundness (fallback: drop) | high | promising, at-risk |
| P7 | Multi-unit assault feasibility | (c) + bounds | bounded yes/no | high | worthwhile, costly |

---

## 3. Rejected as tool-trivial (the filter, made visible)

- **Global territory / quadrant aggregates** ("who controls the most territory", "which quadrant has the
  most X"). The catalogue ranks this its #1 gap, but a **counter** computes it exactly with a unique
  answer — pure bookkeeping, zero reasoning residue. Disqualified by the filter's own headline example.
- **Optimal defender assignment** ("assign your k spare defenders to k cities to minimize total fall
  probability"). This is a **linear assignment / min-cost matching → Hungarian in polynomial time,
  exact.** A matching tool nails it. Rejected (the task names "polynomial assignment" explicitly).
- **Shortest scouting route** ("fewest turns to reveal all of {tiles}"). Ocean-aware multi-target routing
  is TSP-flavored, but the *pairwise* costs are exact Dijkstra and small instances are solved exactly; and
  the single-target form is just **shortest-path — tool-trivial** by the project's own precedent
  (nearest-owned/reachable-nearest). Rejected as exact pathing.
- **Optimal attack order for small k** ("in what order do 3 units attack this stack to maximize capture
  odds"). A bounded **expectimax the tool brute-forces exactly.** Any small, fully-observed game tree is a
  formula. Rejected — which is exactly why P7 asks only a *bounded feasibility* question, not the order.
- **"Which city has the most fogged tiles adjacent"** (a tempting fog question). This is a **count +
  argmax over the masked board** — the answer *is* a function of the visible state, so a tool computes it
  exactly. It tests fog *bookkeeping*, not fog *inference*. Rejected — contrast P1, whose answer depends
  on the hidden occupant, not the count of fog.
- **Exact most-threatened city on an omniscient board via a `city_fall_prob` tool.** Already essentially
  covered by t3-threat, and with the odds primitive exposed it is a direct argmax — a matching tool nails
  it. Only the **fogged** variant (P1) restores reasoning, by hiding the attacker.

---

## 4. Decode gaps that bound the best ideas

- **No turn history / no previous-frame state.** Every board is a single snapshot, so hidden-info kinds
  (P1–P3, P6) must be inferable from the *current* fogged view alone — we cannot ask "where did the unit
  that damaged your city last turn come from." All fog proposals above are designed within this limit.
- **Un-modelled vision bonuses (Fortress +8, Mountains).** The documented approximation
  (`fog-three-state-design.md` AS-BUILT #1) means P2's "true enemy vision" is slightly under-computed near
  enemy fortresses/mountain spotters — curate candidates away from those tiles (a generation filter, not a
  blocker).
- **Naval / amphibious not modeled** (ocean = hard barrier). Blocks any hidden-force-at-sea, sea-lane, or
  transport-based fog inference — a whole rich category is currently off the table (would need the naval
  movement lift the catalogue ranks lowest).
- **Economy / production not decoded** (value = city `size` only). Blocks economic prediction and makes
  "which city the enemy most wants" (P1/assault value) size-biased — Palace *is* decoded (true capital) if
  we want a better prize signal, but production is not.
- **No zones-of-control.** P4/P7's opponent-reposition and assault-sequencing models ignore ZoC, so they
  are honest simplifications of real maneuver, not faithful ones — state it in the rules block.

**Category honesty:** P6 (scouting value) is the one proposal I am not confident clears the verifiability
bar — realized info-gain is partly luck and the soundness fallback may not yield decisive instances often.
It is included because the vein matters, with an explicit "drop at generation if soundness instances are
too rare" escape. Everything P1–P5 has a concrete, board-free scorer of the same shape the project already
ships.
