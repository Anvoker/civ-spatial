# CivSpatial — T2/T3 Valuation Design (combat model + question kinds)

**Status:** design complete, feasibility-validated, ready for implementation behind the stable
contract. **No core code written yet.** This is the deliverable the valuation track owes per
`T2-T3-valuation-continuity.md`.

**Read first:** `DESIGN.md` (durable design, esp. §6–§9) and `T2-T3-valuation-continuity.md`
(the remit + the four non-negotiable disciplines). This doc works *within* that contract; the
one place it needs to **extend** the contract is called out explicitly in §5 (a flag to the
core owner, per the continuity doc's instruction to flag rather than work around).

---

## 0. TL;DR of decisions

1. **The tiers are a *composition-depth* ladder, not a *cognition-type* ladder.** Any question
   we can score deterministically decomposes into T0/T1 primitives — that's what "computed
   ground truth" means. So we stop calling T2/T3 "valuation/judgment" as if it were a new
   *kind* of reasoning, and justify them by what they do for the **encoding comparison** (§1).
2. **Combat model = the `classic` ruleset's own stat tables + a simple stated scalar.** We do
   **not** invent constants (that looked gratuitously wrong) and we do **not** reimplement
   Freeciv's combat *resolution* (rounds/firepower/ZoC/win-chance — the discipline-#4 trap). We
   take the ruleset's *data* (per-unit attack/defense/hp, terrain defense bonuses, veteran
   multipliers) and feed it into a simple product formula, stated to the model (§2).
3. **T2 = attribute-integrated point computation, exact-match** (unit-strength & city-defense
   comparisons). Clean, `Choice`/`Bool`, reuses existing `AnswerSpec` (§3).
4. **T3 = judgment under a *withheld* value function, scored by Pareto dominance** ("is this an
   obviously-bad choice?"). This is the escape from the decomposition trap: we legitimately
   withhold the subjective *weighting* while keeping deterministic scoring, because **dominance
   is preference-free** (§4). Worked example: best-settle-site.
5. **T3 needs one contract extension** — set-valued acceptable answers + scorer sees the board.
   Flagged to the core owner (§5).
6. **Feasibility validated by direct measurement** on real boards: hundreds of decisive
   judgment-loaded instances are constructible per board (§6).
7. **Boards:** validate on **T50** (cheap fixture) + **T677** (dense, contested — the headline
   measurement). Combat constants are hard-coded from the `classic` ruleset (values verified
   against it); machine-extraction from the ruleset files is a §8 TODO, not yet done.

---

## 1. Why T2/T3 exist — the honest reframing

The original framing called T2/T3 "valuation" and "judgment," implying a new cognitive tier
above T0 (perception) and T1 (geometry). That framing is **wrong**, and it matters:

> **Any question with deterministic ground truth decomposes into T0/T1 primitives.**
> "Deterministic ground truth" means "there is an algorithm that computes the answer," and an
> algorithm *is* a decomposition into lookups + geometry + arithmetic. The moment we put the
> combat model in the prompt (discipline #1 — give the model the rules), "which city is most
> threatened" stops being judgment and becomes a mechanical pipeline: look up units (T0),
> compute reachability (T1), apply the stated formula. There is **no** deterministically-scored
> question that resists this — the ones that do (genuine judgment) are exactly the ones we
> *can't* exact-match score.

So the tiers are a **composition-depth** ladder. What T2/T3 buy for the eval is not new
cognition but new **stress on the encoding**, which is the actual dependent variable:

- **T0/T1 only stress spatial fidelity** (terrain + coordinates). They never touch the
  *information-capacity* axis (`DESIGN.md` §4), and on a small board they **ceiling** (see
  `RESUME.md`).
- **T2/T3 force the encoding to carry per-object *attributes*** — unit type, veteran, HP, which
  unit garrisons which city — **simultaneously with** spatial layout, and to cross-reference
  them. An ASCII-glyph encoding must shove all that into a legend and the model must walk
  legend ↔ grid ↔ arithmetic. That cross-referencing under a long dependency chain is the thing
  T0/T1 *structurally cannot* test, and it is an **error amplifier**: a 10-step chain turns a
  97%-vs-99% per-primitive gap into a much wider end-to-end gap — precisely the ceiling-breaker
  we need.

**Two distinct tier characters** (this is the design's shape):

- **T2 — attribute-integrated point computation.** Exact-match. Tests *retrieval of unit/city
  attributes under the encoding* + arithmetic over stated rules. Local, few definitional
  choices. Built first.
- **T3 — judgment under a withheld value function.** Dominance-scored. Tests whether the model
  can weigh a genuinely-subjective tradeoff without committing an *objectively* bad choice.

---

## 2. The combat / strength `rules` module

### 2.1 Philosophy (resolves "aren't we just being wrong?")

Earlier drafts invented round non-Freeciv constants to prevent "memorizing Freeciv." That
rationale is weak: **the real protection against memorization is that we hand the model the
rules** (discipline #1) — and the model still has to read *this board* (which units, what HP,
what terrain) to apply them; memorizing constants doesn't reveal board state. So there is no
reason to be gratuitously wrong.

Instead: **adopt the `classic` ruleset's own *data tables*** (the save is `rulesetdir="classic"`)
and plug them into a **simple stated scalar**. This is the defensible middle of discipline #4:

- We take the ruleset's **stat tables** (unit attack/defense/hp, terrain defense bonuses,
  veteran multipliers) as *stated inputs*. Authoritative, complete, and they scale to any board
  or era for free (T50's 6 types → T677's ~20 modern types) with zero hand-tuning.
- We do **not** take the ruleset's combat **algorithm** — no HP/firepower rounds, no ZoC, no
  `unit_win_chance`. Our "strength" is a single **product**, not a simulated fight. Its
  simplicity is a feature, and the model is told it too.

All constants live in a single module (`rules.rs`), so `rules_block` and the solver share one
source and cannot drift. They are currently **hard-coded** from `classic` (values verified
against it); machine-extraction from the ruleset files (see §8) is a TODO, not yet done — which
means a save on a *different* ruleset would silently mismatch, a hazard for multi-board runs.

### 2.2 Constants (from `classic`, representative subset)

**Unit stat table** — `attack / defense / hitpoints / firepower / move_rate / class`
(full 52-type table extracted from `units.ruleset`; `rules_block` emits only board-present
types). Illustrative rows:

| type | att | def | hp | fp | move | class |
|---|---|---|---|---|---|---|
| Warriors | 1 | 1 | 10 | 1 | 1 | Land |
| Phalanx | 1 | 2 | 10 | 1 | 1 | Land |
| Legion | 4 | 2 | 10 | 1 | 1 | Land |
| Chariot | 3 | 1 | 10 | 1 | 2 | Land |
| Musketeers | 3 | 3 | 20 | 1 | 1 | Land |
| Cannon | 8 | 1 | 20 | 1 | 1 | Land |
| Riflemen | 5 | 4 | 20 | 1 | 1 | Land |
| Cavalry | 8 | 3 | 20 | 1 | 2 | Land |
| Alpine Troops | 5 | 5 | 20 | 1 | 1 | Land |
| Armor | 10 | 5 | 30 | 1 | 3 | Land |
| Mech. Inf. | 6 | 6 | 30 | 1 | 3 | Land |
| Fighter | 4 | 3 | 20 | 2 | 10 | Air |
| Battleship | 12 | 12 | 40 | 2 | 4 | Sea |

**Veteran multiplier** (`power_fact`, applied to both attack and defense): green **×1.0**,
veteran **×1.5**, hardened **×1.75**, elite **×2.0**. *(Confirmed from classic `veteran_system`:
`power_fact = 100/150/175/200`, `move_bonus = 0,0,0,0`, names green/veteran/hardened/elite.
Still read it from the shipped ruleset at integration rather than hard-coding, and fail loudly if
absent.)* Live on dense boards (T677 has all four levels).

**Terrain defense bonus** (additive %, from `terrain.ruleset`): Forest/Jungle/Swamp **+50%**,
Hills **+100%**, Mountains **+200%**, River (extra) **+50%**, all others 0.

**Fortification** (additive %): Fortress extra **+100%** (from `terrain.ruleset`); **city center
+50%** (base, for any city). **City Walls +100% vs land** — walls *are* recoverable: each city
carries an `improvements` bitvector, decodable against `improvement_vector` (which lists "City
Walls", "Great Wall", "Coastal Defense") exactly like the tile-extra bit-planes. So decode it
and surface a per-city `has_walls` fact into the encoding **and** `rules_block`. Two wrinkles to
state: **Great Wall** is a wonder granting walls to *all* its owner's cities (a per-player flag,
not per-city); **Coastal Defense** is +100% vs *sea* only. This is a small parser extension —
coordinate with the core owner. *(Adding `has_walls` to the encoding is a feature, not a cost: it
is one more per-object attribute the model must locate and integrate — exactly the capacity-axis
stress T2/T3 exist to create.)*

### 2.3 Formulas

```
health(u)   = hp(u) / hitpoints[type(u)]              # ~1.0 in practice (see below)
vet(u)      = power_fact[veteran(u)]                   # 1.0 / 1.5 / 1.75 / 2.0

att_eff(u)          = attack[type]  × vet(u) × health(u)                       # context-free
def_eff_base(u)     = defense[type] × vet(u) × health(u)                       # context-free
def_eff(u, tile)    = def_eff_base(u) × (1 + terrain_bonus(tile) + fort_bonus(tile))
```

- **Two named axes** (attack, defense) — comparisons always specify which. No invented composite
  "power."
- **T2a** uses the **context-free** forms (clean table-lookup + veteran/health arithmetic).
  **T2b / T3** use the **contextual** `def_eff` (integrates terrain + fortification).
- **Air/Sea classes** are excluded from land-threat reachability in T3 (a Battleship can't
  besiege an inland city). They still appear in T2 comparisons.

### 2.4 HP is in the formula but inert in practice — and that's fine

`health(u)` is in every formula (your call: HP-in). Empirically, **battle damage is vanishingly
rare in any snapshot** — 0–3 wounded units across *every* corpus board, including "battle" and
"almost-defeated" saves, because damage heals between turns. So `health ≈ 1.0` almost always.
We keep it for correctness and exercise it with a **synthetic damaged-unit mini-board** in the
unit tests (§7); we never rely on it for live signal. Veteran, by contrast, *does* vary on dense
boards, so `vet(u)` is a live differentiator.

### 2.5 `rules_block` content

Filled from this module (`None` for T0/T1). Contains: the strength formulas (§2.3, plain
prose), the unit-stat rows for **board-present types only**, the veteran/terrain/fort tables,
and the air/sea-can't-besiege note. It is part of the cacheable prefix (`DESIGN.md` §9), so its
tokens are paid once per (board × encoding).

---

## 3. T2 question kinds — attribute-integrated, exact-match

### 3.1 T2a — unit strength comparison

- **Question:** *"Which unit has the greater {ATTACK|DEFENSE} strength, {unit A} or {unit B}?"*
  (axis is a question parameter; context-free strength per §2.3).
- **Answer:** `Choice{ value, options=[A_ref, B_ref] }`. Reuses the existing `Choice`
  `AnswerSpec` unchanged. (`Choice` over the two units also lets the scorer flag a reply naming a
  third unit as *invalid* rather than *wrong*.)
- **Referents:** both units via `Referent::Unit{id}` so the question survives non-coordinate
  encodings.
- **Admissibility / decisive margin:** both units exist; **winner's strength ≥ 1.25 × loser's**
  on the chosen axis. Same-type + same-vet + same-hp pairs tie → auto-dropped. Log drop count.
- **What it tests:** locating two units' attributes in the encoding + veteran/health arithmetic.
  Stresses the *capacity* axis (find unit facts in raw list vs ASCII legend vs adjacency nodes).

### 3.2 T2b — city defense comparison

- **Question:** *"Which city is better defended, {city X} or {city Y}?"* — compares
  `def_eff(best defender, city tile)` (§2.3) including terrain + Fortress/city bonus. Undefended
  city → `def_eff = 0`.
- **Answer:** `Choice{ value, options=[X_ref, Y_ref] }`, `Referent::City{id}`.
- **Admissibility / margin:** both cities exist; **winner ≥ 1.25 ×** loser's defense; drop ties
  and both-undefended. Log drops.
- **What it tests:** find each city's garrison (retrieval) + apply terrain/fort multipliers
  (integration). Local, two named cities → genuinely T2.

*(A single-city "defense strength bucket" `Int` variant was considered and rejected: exact-int
answers are brittle and not spatial. Comparisons are robust and margin-filterable.)*

---

## 4. T3 question kinds — judgment under a withheld value function

### 4.1 The dominance principle

Some questions have **no single correct answer** because "best" is a *tradeoff across axes* with
no canonical weighting (a settle site trades growth vs. production vs. resources vs. safety).
Stating a weighting would (a) be arbitrary and (b) collapse the question back to mechanical T2.
So we **withhold the weighting** and score only the objective floor:

> **Pareto dominance.** Option A *dominates* B iff A is ≥ B on **every** axis and strictly
> better on at least one. A dominated choice is wrong under **every** preference — no coherent
> strategy justifies it. We score **only** dominated picks as wrong; any non-dominated pick is
> **not-wrong** (we do not adjudicate among the frontier).

This is the escape from the decomposition trap: computing *whether a pick is dominated* is
mechanical, but the model is **not told how to weigh the axes**, so it cannot reduce the task to
arithmetic — it must exercise judgment, and we penalize only judgment no weighting could reach.
We state the **axes** to the model (they are stated mini-models, like combat) but **not the
aggregation**. "Obviously bad" = "dominated," made precise.

**Scoring semantics (metric):** this measures **blunder-avoidance**, a *floor*, not answer
*quality* — a mediocre-but-valid pick scores the same as a brilliant one. For the encoding
comparison that is still real signal ("encoding A induces more blunders than encoding B"); the
reported quantity is a **blunder rate**, compared across encodings. (User has accepted this
tradeoff for now.)

### 4.2 T3a — best settle site (the worked example)

- **Question:** *"You are founding a new city for {player}. Which of these candidate tiles is a
  sound site? {K candidates}."* Multiple-choice (curated K, typically 4–5).
- **Answer:** `ChoiceSet { acceptable, options }` (§5) where **acceptable = the non-dominated
  candidates** (precomputed at generation). Options = the K candidate tiles (as
  `Referent::Tile`).
- **Axes** (stated to the model; computed over the working radius, Chebyshev ≤ 2):
  - **Food/growth** — Grassland/Plains + food resources (Wheat/Oasis/Fish/Game/Fruit/…) +
    River/Irrigation.
  - **Production** — Hills/Forest/Mountains + Mine + production resources (Iron/Coal/Resources).
  - **Resources** — count of special/trade resources in radius.
  - **Safety** — a *distance-decayed* threat field (graded, not binary — a binary in-reach/out
    model gave a flat, near-useless safety axis on sparse boards). Reuse the T1 BFS to get, per
    enemy land unit, `reach_turns(u, T) = ceil(BFS steps from u to a tile adjacent to T /
    move[u])`, terrain-aware (can't cross ocean), `∞` past a horizon cap `H` (≈6 turns). Then:
    ```
    threat(T) = max over enemy land units u of   att_eff(u) / reach_turns(u, T)
    safety(T) = − threat(T)
    ```
    A this-turn attacker contributes its full `att_eff`; one 3 turns away contributes a third;
    distant/blocked units fade to 0. Air/sea excluded (can't besiege inland). `max` = "scariest
    single incoming attacker" (v1); `sum` ("surrounded by many") is a stated alternative.
- **Dominance predicate:** A dominates B iff A ≥ B on all four axes, one strict.
- **Generator recipe (targeted — NOT random sampling):**
  1. Enumerate settleable tiles (land; not on/adjacent to an existing city).
  2. Find a **threatened, good-terrain trap** T and a **safe twin** T′ with ≥ terrain on all
     three terrain axes (ideally *identical* terrain, so only safety separates them) → T′
     dominates T.
  3. Add ≥2 **distractors** that do not dominate T (so T is non-dominated on terrain within the
     set, i.e. a naive terrain-only reasoner would pick it).
  4. Present {T, T′, distractors}. The only way to avoid the blunder T is to weigh safety.
- **Admissibility / decisive margin:** the set must contain ≥1 **decisively** dominated trap
  (safety gap large, e.g. threatened vs. safe) **and** ≥2 non-dominated options. Prefer traps
  where the *dominance depends on the safety axis* ("judgment-loaded") — otherwise it's a
  terrain-counting question, not a valuation one. Log the split.
- **Why multiple-choice, not open:** open form ("name any tile") is secretly "find a near-optimal
  tile" — 98% of all tiles are globally dominated, so it collapses to single-best. Within-set
  dominance is the correct, well-behaved framing.

### 4.3 T3b — city threat / exposure (committed)

The continuity doc's headline T3 example — *"which of your cities is under the greatest
threat?"* / *"rank your cities by exposure"* — is now committed to a concrete definition. The
tension it raised: a well-defended big city draws more *raw force*, but a soft small city is the
smarter *target*, and the two orderings diverge. The resolution, mirroring §4.2, is **not to
pick a scalar** but to keep threat as **two Pareto dominance axes**, so we never commit an
(arbitrary) combination rule — the same §4.1 escape hatch, applied to the defended-target case.

- **Question:** *"Which of {player}'s cities is under the greatest threat?"* (single-`Choice`
  variant), or *"Rank these cities by exposure"* (partial-order variant). Cities referenced via
  `Referent::City`.
- **Answer:** `Choice{ value, options=[city_refs] }` for the single-winner form; for the ranking
  form, score **inversions of the dominance order** (see below), ignoring the subjective total
  order — exactly the T3a treatment.
- **Axes** (stated to the model; two axes, **never** collapsed to a scalar — this is the point):
  - **Incoming force (↑, higher = more threatened).** Over the reachable enemy land attackers of
    the city's tile, the **same threat field as §4.2**: per attacker `att_eff(u) / reach_turns(u,
    T)` (terrain-aware BFS, air/sea excluded, horizon cap `H`), **aggregated by `max`** (the
    scariest single incoming attacker) for **v1**, with **`sum`** (multi-unit siege — "surrounded
    by many") documented as the stated alternative. This aggregation is held **identical to the
    settle-site safety axis (§4.2)** on purpose, so the model learns *one* threat semantics, not
    two.
    ```
    incoming(C) = max over enemy land units u of   att_eff(u) / reach_turns(u, C.tile)
    ```
  - **Own defense (↓, lower = more threatened).** The city's **T2b `def_eff`** (§3.2): best
    defender's `defense × vet × health × (1 + terrain_bonus + fort_bonus)`, including the City
    Walls bit and city-center/Fortress bonuses. **Reuse the committed T2b city-defense solver**
    verbatim — no new notion of defense is invented. Undefended city → `def_eff = 0` (maximally
    soft).
- **Dominance predicate:** city **A is unambiguously more threatened than B** iff
  `incoming(A) ≥ incoming(B)` **AND** `defense(A) ≤ defense(B)`, with at least one strict. A city
  that is both more-attacked *and* softer dominates — no coherent attacker model disagrees.
  Genuine trade-offs (more incoming force *and* more defense — the big well-defended city) are
  **correctly withheld**, never adjudicated.
- **Single-`Choice` "most threatened" variant:** dominance is only a partial order, so the single
  winner is the **unique city that dominates *all* others** on (incoming↑, defense↓). Decisive
  margin (as implemented in `t3-threat`): **on each axis, the more-threatened city must either tie
  or lead by ≥1.25×** (reuse the existing 1.25× discipline), **with at least one axis a genuine
  decisive lead** — a sub-margin lead on an axis is a *near-tie* that makes the whole dominance
  non-decisive (dropped). This is the faithful reading of the dominance predicate above, which
  permits **equality on one axis**; a literal "≥1.25× on *both* axes" would contradict that predicate
  and, on e.g. a 5-city board of tied-defense + undefended cities, admit **nothing**. **Drop
  instances with no unique dominator** — frequent on sparse boards where the frontier has several
  incomparable cities — and **log the drop rate**. That drop is the honest price of a preference-free,
  non-arguable definition, and it keeps the single answer un-litigable.
- **The two-question split (stated explicitly, not a compromise):** city-threat/exposure uses
  this **two-axis attacker-attractiveness** model *because a defender exists* — its garrison and
  walls are the whole point of "which city falls." Settle-site safety (§4.2) correctly stays
  **raw incoming force**, because the candidate tile is *empty*: there is no defender to divide
  by, so attacker-attractiveness is degenerate there. Two structurally different questions, two
  correct tools — principled, not a patch.
- **If a scalar is ever unavoidable** (a UI sort, a coarse "threat bucket"): use the **ratio**
  `incoming / def_eff` (equivalently `att_eff / def_eff`), **not** the difference — it is
  scale-free and tracks "can it be taken." Treat `def_eff = 0` (undefended) as **top-rank**, not
  a divide-by-zero (undefended = maximally threatened, which is correct). The difference
  (`att − def`) systematically **understates** how soft a tiny undefended city is — avoid it.
  This scalar is a *rendering* convenience only; scoring stays on the two dominance axes.
- **Discipline anchors:** the two threat axes and their `max` aggregation are stated to the model
  in `rules_block` (**discipline #1** — same rules scored and shown), and this is a **static
  exposure metric**, *not* a Freeciv exchange resolution (**discipline #4** — no rounds,
  firepower, or win-chance; `att_eff / reach_turns` and `def_eff` are the whole model).

### 4.4 Other T3 family members (same pattern, sketched)

Any remaining select/rank task with an objective partial order fits — score **violations of the
dominance order**, ignore the subjective total order:
- **Safest retreat tile** for a wounded unit (mobility × cover × distance-from-threat).
- **Which enemy can your {unit} attack** without being dominated on the exchange.

Build **T3a first** (well-behaved), then **T3b** (city threat/exposure, §4.3 — reuses the T3a
threat field and the T2b defense solver, so it adds no new machinery); treat the rest as
follow-ons. The `Direction`-valued "most vulnerable flank" from the continuity doc stays
**deferred** (highest definitional risk).

---

## 5. Contract extension required — FLAG TO CORE OWNER

Everything in T2 fits the existing contract unchanged. **T3 dominance scoring does not** — but
the extension is minimal and *additive*, and crucially **the scorer does not need the board**.

**Decided shape — `ChoiceSet` with the acceptable set precomputed at generation:**

```
Answer::ChoiceSet { acceptable: Vec<String>, options: Vec<String> }
    correct  ⇔  reply ∈ acceptable
    invalid  ⇔  reply ∉ options                 (named something not offered)
    wrong    ⇔  reply ∈ options \ acceptable     (a dominated pick — a blunder)
```

The generator already holds the board, so it computes the non-dominated set **once, at
generation**, and stores it as `acceptable`. The scorer then stays a **pure set-membership
comparator** — it never sees the board, never re-runs a solver — fully preserving the
`DESIGN.md` §6 separation. (This supersedes an earlier draft that had the scorer take board
context; precomputing is cleaner.)

- **Oracle-100% still holds:** the Oracle returns any member of `acceptable` → correct. Add a
  **negative** self-test to the invariant: a deliberately-dominated option must score *wrong*
  (guards the blunder-detection path, not just the happy path).
- **Analysis hook:** log *which* option was picked and its dominated-by margin (blunder severity)
  in the result row, so the analysis layer can report blunder rate *and* severity per encoding.

**APPROVED (user, 2026-07-27).** The shape is blessed; implementation is handed to the core
owner (`question.rs`/`scoring.rs`) — no code is written in this design track. The only detail
left to the implementer is the exact variant name + where `acceptable` is stored on the
`EvalItem`.

---

## 6. Feasibility evidence (measured, not asserted)

The make-or-break question was: can we *construct* decisive judgment-loaded instances? Measured
directly on real boards (scratchpad prototypes over the actual saves):

| metric | T677 (dense) | T50 (sparse) |
|---|---|---|
| settleable tiles | 1596 | 1964 |
| threatened settleable tiles | 571 (36%) | 162 (8%) |
| **STRICT trap supply** (safe twin, *identical* terrain → only safety separates) | **248** | **115** |
| **LOOSE trap supply** (safer tile with ≥ terrain) | **514** | **162** |

**Verdict: feasible on both boards.** Hundreds of decisive instances are constructible.

**Important methodological note (a correction):** an earlier pass reported "~0.1% judgment-loaded"
and "T50 has 0–2 traps." Both were artifacts, not real limits:
- The **0.1%** was the rate at which *random* K-tile draws happen to form a trap — irrelevant,
  because the generator **constructs** targeted sets (§4.2), it does not sample randomly.
- The **0–2** used a too-strict definition (trap must be Pareto-optimal against the *entire
  board* — only ~33 tiles ever are). The correct **within-the-presented-set** definition yields
  115 on T50.

So feasibility was never board-limited. **T677 is still preferred** for the headline run — its
threat field is multi-level (many independent enemies, varied magnitudes) vs. T50's near-binary
one-pocket field, and it exercises the full unit-type table — but it is a *quality* preference,
not a gate.

---

## 7. Verification plan (both levels, per every question kind)

1. **Mini-board unit tests** pinning each solver on a hand-built board:
   - T2a: a Warriors-vs-Armor board with a known strength ordering; a **veteran** case; a
     **synthetic damaged-unit** case (e.g. Armor at 9/30 hp) to exercise `health(u)` that real
     boards never trigger.
   - T2b: two cities, one on Hills-with-Fortress, one on open ground, known defense ordering.
   - T3a: a hand board with one planted judgment-loaded trap (good terrain, threatened) + safe
     twin + distractors; assert the dominated option is the trap and the frontier is the rest.
2. **Oracle-100% end-to-end** on all kinds (generation → prompt → extraction → scoring
   self-consistency). For T3, add the **negative** test: a deliberately-dominated answer must
   score *wrong* (guards the dominance scorer, not just the happy path).

---

## 8. Boards & corpus (provenance)

- **Corpus source:** the CivRealm repo's `tests/game_save/` tree (github.com/bigai-ai/civrealm,
  branch `dev`) — not a downloadable dataset; states are Freeciv `.sav` snapshots, all
  `rulesetdir="classic"`, Freeciv 3.2.90-dev. Spans T1→T677 with battle/conquer/pillage
  scenarios. Compressed as `.zst`/`.xz` (decompress to plaintext first — `DESIGN.md` §3).
- **T50** (`myagent_T50`, already in `data/saves/`): 78×52, 2 players, 8 units, 6 cities. Cheap
  **fixture / smoke test**. Threat field near-binary; veteran/HP inert here.
- **T677** (`testcontroller_T677`): 84×56, **10 players, 682 units, 118 cities**. The dense,
  contested **headline-measurement** board (the exact "turn-677" `DESIGN.md` §10 anticipated).
  Alternates: T257 (7p/309u/148c), T401 ("almost defeated", 6p/120u/37c).
- **Combat constants** are **hard-coded** from `classic` (values verified against it):
  `units.ruleset` (attack/defense/hitpoints/firepower/move_rate/class × 52 types),
  `terrain.ruleset` (defense_bonus, Fortress/River extras). TODO (§8): read these from the
  ruleset shipped with the save's generator instead of hard-coding, so the model's `rules_block`
  and the solver stay in lockstep *and* a differently-ruled save can't silently mismatch.

---

## 9. Open decisions / TODO

- [x] **Veteran `power_fact`** — RESOLVED: classic = 1.0/1.5/1.75/2.0, move_bonus 0 (confirmed
  from `veteran_system`). Still read from the shipped ruleset at integration, don't hard-code.
- [x] **T3 scoring contract extension** — APPROVED (user). `ChoiceSet` with `acceptable`
  precomputed at generation; scorer stays pure (§5). **Hand to core owner to implement** in
  `question.rs`/`scoring.rs` — not written here.
- [x] **City Walls / `improvements` decode** — APPROVED (user). Decode the per-city
  `improvements` bitstring (fixed-length, indexed against `improvement_vector`; City Walls=7,
  Coastal Defense=8, Palace=21, Great Wall=47) into a `City.improvements` set in `civ-core`.
  City Walls +100% vs land, Great Wall = all owner's cities, Coastal Defense +100% vs sea; also
  yields the true capital (Palace). **Hand to core owner** — not written here.
- [ ] **Rich-rendering seam** (coupled to the walls decode) — encoders must surface rich
  attributes (walls, veteran, HP) **only for T2/T3**, leaving T0/T1 board blocks byte-identical
  so in-flight T0/T1 runs are undisturbed (`DESIGN.md` §3 "rich model lights up at T2/T3"). Core
  owner / other agent owns this; flag before they wire walls into the encoders.
- [x] **Working radius** — RESOLVED: **Chebyshev ≤ 2** (one geometry; `DESIGN.md` §2 forbids a
  second one — radius_sq is an *area shape*, not a distance metric, so it could only ever be an
  *extra* concept, never a replacement). `city_radius_sq` is in the save if we ever want true
  catchment for existing cities, but the settle axes stay Chebyshev.
- [x] **Threat horizon** — RESOLVED: *graded* distance-decayed threat, `att_eff / reach_turns`,
  `max` over enemies, terrain-aware BFS, cap `H≈6` (§4.2). Replaces the binary 1-turn model.
- [ ] **Decisive-margin ε** — tuning task: 1.25× ratio for T2 comparisons; a "decisively
  dominated" gap for T3. Instrument drop-rate logs from day one and tune to keep ~70%.
- [ ] Then: implement `rules` module + T2a/T2b/T3a `QuestionKind`s behind the contract; unit
  tests + Oracle-100% (incl. the T3 negative test).
```
