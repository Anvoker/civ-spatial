# Continuity — Designing T2/T3 (Valuation) Questions for CivSpatial

> **STATUS (2026-07-27): design DELIVERED.** This remit was fulfilled — see **`T2-T3-design.md`**
> for the completed, feasibility-validated design (combat model, T2a/T2b/T3a kinds, the
> `ChoiceSet` dominance-scoring contract extension, all user-approved). What remains is
> *implementation* on the core track (see `RESUME.md` NEXT ACTIONS #5). This file is kept as the
> original remit/constraints record.

**For:** a separate Claude session tasked with designing the **valuation** question tiers.
**Read first:** `DESIGN.md` (the durable design) and `spatial-encoding-eval-continuity.md`
(original brief). This file tells you your remit, your hard constraints, and the open design
work. It was written by the session that is building T0/T1 and owns the core contract.

---

## Your remit in one sentence
Design the **T2 (pointwise valuation)** and **T3 (spatial × valuation)** question kinds and
the **combat/strength `rules` module** they depend on — *within* the stable contract below,
which you must not redesign.

## Why these tiers exist (the point you are serving)
Pure-geometry questions (T0/T1) could run on a random grid — the realistic Freeciv board
would be decoration. **T2/T3 are what justify using a real 4X board:** they ask about
threat, defensibility, relative force — things only a domain-structured board has. Example
targets the user explicitly wants:
- **T2:** "Is unit A stronger than unit B?"; "What is city X's defensive strength?"
- **T3:** "Which of your cities is under the greatest threat?"; "Which side of your empire
  (as a cardinal direction) is most vulnerable to attack?"

The T3 examples are the *reason the project is interesting*. They are also the most
definitionally fraught — see the discipline section.

---

## The stable contract you design WITHIN (do not change these)
These are owned by the core build. Your work adds implementations behind them; it does not
alter their shapes. (Names are Rust; see `DESIGN.md` §6, §8, §9.)

- **`QuestionKind`** — the family/generator+solver trait. You add new impls.
- **`Question`** — abstract question: referents/params, **no answer**, stable id.
- **`Answer`** — typed enum: `Int | Direction | Terrain | Bool | Choice{value,options} | Coord`.
  **Your answers must resolve to these discrete variants** (a city → `Choice`; a flank →
  `Direction`; a comparison → `Bool`/`Choice`). Do **not** introduce free-form or scalar-
  with-tolerance answers without coordinating — exact-match scoring depends on discreteness.
- **`AnswerSpec`** — how the scorer extracts/compares. Reuse existing specs where possible.
- **`Referent`** — handle to a board thing (`Tile|City|Unit|Landmark`); encoders render it.
  Your questions reference board things through `Referent`, never baked-in strings.
- **`EvalItem`** — question + expected `Answer` + `AnswerSpec` + provenance. Frozen row.
- **`Scorer`** — separate, type-directed by `AnswerSpec`. You should not need to touch it if
  your answers use existing variants.
- **`Prompt { board_block, rules_block: Option<String>, question_block }`** — the
  `rules_block` seam is reserved for you (`None` in T0/T1). You fill it from your `rules`
  module. **Same source of truth feeds both the solver and the model's prompt.**
- **Grid semantics** (`geometry`): 8-neighbor Moore, north = y0, Chebyshev distance,
  non-wrapping. Use these; do not invent a second geometry.

If you find the contract genuinely can't express something, **flag it to the core owner**
rather than working around it — the whole eval's validity rests on these being stable.

---

## Non-negotiable disciplines (this is where T2/T3 lives or dies)
Valuation questions have a subtle failure mode: their ground truth is only correct *relative
to a model of value you define*. Left sloppy, the eval silently shifts from "can the model
assess threat" to "can the model reproduce MY heuristic," and a smarter model can be marked
wrong. These four disciplines defuse that. **All four are mandatory.**

1. **Give the model the same rules you score against.** Put a small, explicit
   combat/strength model in `rules_block`, generated from the **same `rules` module** your
   solver uses. Then the eval measures *spatial reasoning over stated semantics*, not
   memorized Freeciv constants — and the encoding stays the only varying thing. The model's
   simplicity is a *feature* because the model is told it too.
2. **Discrete answers only** (see `Answer` above) → exact-match scoring survives.
3. **Decisive-margin filter.** At generation, compute the top-2 candidates; **discard** the
   instance if within epsilon. Keep only questions any reasonable value model agrees on.
   This is what makes "arguable" answers non-arguable. Log how many instances you drop.
4. **Do NOT reimplement Freeciv's real combat model.** A simple, principled, *stated* model
   is the goal. Reimplementing hitpoints/firepower/ZoC/veteran-tables/terrain-multipliers
   chasing "true" ground truth is the scope-creep trap — it's a lot of work, a bug source,
   and ruleset-specific. Define *a* defensible model, state it, move on.

**Verification you must satisfy (same as every question kind):**
- A **unit test on a hand-built mini-board** pinning each solver's answer.
- The **Oracle model scores 100%** end-to-end on your questions (generation → prompt →
  extraction → scoring self-consistency). If not, your `AnswerSpec`/`Answer` disagree.

---

## The data you have to work with
The neutral `Board` will carry the **rich model** by the time you need it (see `DESIGN.md`
§3): units with `veteran` level and `hp`; tiles with the full `extras` set (Road, River,
Railroad, resources, Fortress, etc.) and territory `owner`; cities with `size`. **All of
this is already present in the save** (unit table has `veteran`/`hp` columns; extras fully
decoded) — if a field you need isn't yet surfaced on the struct, it's a small parser
extension, not a missing data source. Coordinate with the core owner to light up fields.

Note the current lean structs may not yet expose `veteran`/`hp` on `Unit` — confirm before
designing solvers that depend on them.

---

## The design work to actually do
1. **Specify the combat/strength `rules` model.** Minimal but principled. Likely needs:
   per-unit-type attack/defense base values (a small table); a terrain defense modifier; a
   fortification/city-walls modifier; a "can this unit threaten this tile/city" reachability
   predicate (movement + `geometry`). State every constant. This module must be serializable
   into `rules_block` *and* callable by solvers.
2. **T2 question kinds** (pointwise): unit-vs-unit strength comparison (`Bool`/`Choice`);
   single-city defensive strength bucket or comparison. Start here — local, easiest ground
   truth, fewest definitional choices.
3. **T3 question kinds** (spatial × valuation): "most threatened city" (`Choice` over the
   player's cities) — well-behaved, do this first. "Most vulnerable flank/direction"
   (`Direction`) — **highest definitional risk**: it needs an empire centroid, a partition
   of threats into bearings, and per-sector aggregation, and *the definition does most of
   the work*. Hold it to the strictest margin filter; expect to iterate its definition; it
   is the canary for over-reaching. Consider shelving it until the safer T3 kinds work.
4. **Referent rendering implications.** Make sure your questions phrase board things through
   `Referent` so they survive non-coordinate encodings (egocentric/adjacency).
5. **Admissibility + margins per kind.** Define, per question kind, what "unambiguous" and
   "decisive margin" mean numerically.

## Explicitly out of your scope
- Changing the core contract (§ above), the geometry, the scorer, or the elicitation format.
- Reimplementing Freeciv combat faithfully.
- The interactive/queryable encoding, observability, analysis layer — separate tracks.
- Building T0/T1 (already being built) — but read those solvers for the pattern.

## Deliverable
A design doc (and, if you go further, `QuestionKind` impls + a `rules` module) that: defines
the stated combat model with all constants; lists each T2/T3 question kind with its `Answer`
variant, `AnswerSpec`, admissibility/margin rules, and prompt template; and demonstrates the
two-level verification (mini-board unit tests + Oracle-100%). Hand back to the core owner for
integration behind the stable contract.
