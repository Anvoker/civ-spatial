# Question-Design Review — lock the kind roster before the 3-arm run

*Created 2026-08-14. Purpose: give the owner enough to answer the pre-run blocker —
**"will I wish the question KINDS (or their forms) had been different?"** — at **zero cloud
cost**, before freezing the roster for the re-scoped 3-arm run (`raw` vs `raw-maxops` vs
`roster-maxops`). Every example below is the real, deterministic offline output
(`gen-questions … --model oracle`), so what you read here is exactly what the LLM will be asked.
Board: `corpus_medium_a6_s1337-T0221-Y01600-final.sav`, `--fog 1`, `--difficulty hard`, seed 0.*

> **How to use this.** For each kind decide one of: **KEEP** / **TRIM** (drop for
> redundancy) / **REDESIGN** (form is wrong) / **DEMOTE** (illustrative, not scored). The
> roster is only *right* if every kind earns its place against a **claim** — under path (a) the
> claims changed (see next section), so a kind that was justified pre-audit may no longer be.

---

## What the roster is FOR now (post-ceiling-theorem, path (a))

The fog audit retired "reasoning-frontier hidden-info" as a claim. Under path (a) **within-maxops
accuracy saturating is EXPECTED** (the ceiling theorem: any frozen-board function is
tool-computable). So a kind no longer earns its place by "resisting tools." It earns it by serving
one of these **four** live claims:

1. **Accuracy ablation** — `raw` (eyeball) ≪ `raw-maxops`/`roster-maxops` (tooled). Needs kinds an
   LLM *cannot reliably* do by eyeballing but *can* with the right tool. (The decision kinds.)
2. **Cost frontier** — `raw-maxops` vs `roster-maxops` at equal accuracy: front-loading the whole
   board is strictly dominated. Needs enough kinds/N to bound the accuracy difference.
3. **Won't-self-select** — the model keeps a wasteful tool habit even when a cheaper matching tool
   is on the menu. (Cross-kind, from traces.)
4. **The two withheld-tool survivors** — constraint-site (0.79) + compare-two-attacks (0.88): the
   *live demo* that "difficulty on a frozen board = the tool we didn't bundle."

Plus a **perception control** (region-count / reachability) so the ablation reads as a *compute*
lever, not a perception crutch (raw is fine on perception, weak on decisions).

**Consequence:** the roster no longer needs *many* saturators. It needs (a) a clean perception
control, (b) the two survivors, (c) *enough* decision kinds to make the arm-spread + cost claims
robust — but likely **fewer than the current 8 decision saturators**. That is the main trim
question below.

---

## Roster at a glance (16 in-run kinds)

| Kind | Tier | Serves | Answer shape | Status | KEEP? — concern |
|---|---|---|---|---|---|
| region-count | T1 | perception control | unique number | encoding discriminator | **KEEP** — the counting wall; the cleanest "raw is OK here" control. |
| reachability | T1 | perception control | yes/no | cheap spatial | **KEEP** — distinct skill (flood + ZOC). |
| best-site | T1 | encoding + decision-lite | unique coords | ⚠ flagged | **DECIDE** — owner flagged "best site is underdetermined in real play." Its *proxy* ("most Plains in radius") IS well-defined but narrow. Demote to perception-only, or drop? |
| nearest-owned | T1-rp | tool-fit (parity-gap win) | number/"none" | ⚠ board-dependent | **KEEP but RESTRICT** — trivial/"none" on late boards (example returned "none"). Must sample early/mid only (decision E). |
| reachable-nearest | T1-rp | tool-fit | number/"none" | ⚠ board-dependent | **KEEP but RESTRICT** — same; also "none" here. |
| unit-strength | T2 | tool-fit floor | unique (which unit) | saturates | **DECIDE** — very trivial ("which has greater DEFENSE"). Redundant with city-defense? Keep as a floor or drop? |
| city-defense | T2 | tool-fit | unique (which city) | saturates | **KEEP** — best-defender + walls; the meaningful T2. |
| constraint-site | T3 | ★ survivor | coords/"none" | **0.79** | **KEEP + FEATURE** — headline survivor. |
| compare-two-attacks | T3 | ★ survivor | coords/"incomparable" | **0.88** | **KEEP + FEATURE** — headline survivor. |
| settle-site | T3 | saturator (arm spread) | any sound coords | ° 1.00, thin support | **TRIM CANDIDATE** — "sound/non-dominated" cluster; thinnest support (10/15). |
| adv-assault-target | T3 | saturator | any sound city | ° | **TRIM CANDIDATE** — "sound" cluster. |
| cf-vacate | T3 | saturator | yes/no | ° 0.96 | **KEEP-ish** — the only yes/no decision; cheap variety. |
| triage-reinforce | T3 | saturator | unique (best city) | ° 0.92 | **TRIM CANDIDATE** — resource-allocation; overlaps t3-threat's "which city." |
| t3-retreat | T3 | tool-fit | any sound coords | 3-axis Pareto | **KEEP-ish** — "sound" cluster but the richest (3-axis). |
| t3-threat | T3 | tool-fit (defense mirror) | unique (which city) | ▽ weakest | **DECIDE** — single-axis, tool-trivial; already K=4. Keep light or drop? |
| forward-posting | T3-frontier | was frontier(b) | any sound coords | tool-reducible | **RE-FRAME** — no longer a "frontier" kind post-ceiling-theorem (2-ply is tool-computable on a frozen board). Keep as an adversarial tool-fit kind, or move to path (c) as a 2-ply rollout later? |

**Two answer-shape families** (a validity axis worth a deliberate choice):
- **Unique-answer** (one correct): region-count, reachability, best-site, unit-strength,
  city-defense, compare-two-attacks, triage-reinforce, t3-threat, nearest-owned, reachable-nearest.
- **Acceptable-set** ("name ANY sound / non-dominated one"): constraint-site, settle-site,
  adv-assault-target, cf-vacate (yes/no), t3-retreat, forward-posting. This framing is the
  deliberate fix for *underdetermination* (no unique best → accept any non-dominated). If you
  distrust "sound/non-dominated" as fuzzy, that distrust hits this whole family at once.

---

## Per-kind cards (real examples)

### Perception control

**region-count** — *what it tests:* local aggregation / the counting wall.
```
How many Forest tiles are within 3 tiles (Chebyshev distance) of tile (31, 31), including that tile itself?
-> 1
```
*Answer type:* whole number. *Decisive filter:* excludes the board's default terrain (confound fix).
*Concern:* none — this is the perception anchor. The example's answer (1) is on the low end; the
generator should spread true counts (check the K=8 sample isn't all 0–2).

**reachability** — *what it tests:* 8-dir flood + enemy ZOC.
```
Starting on (5, 44), can a unit belonging to Atawallpa reach (10, 39) in at most 3 steps … respecting enemy zones of control …?
-> no
```
*Answer type:* yes/no. *Concern:* the prompt is long (full ZOC rules inline). Faithful, but is the
yes/no too coarse to discriminate encoders? It's a control, so probably fine.

### Movement (restrict to early/mid boards — decision E)

**nearest-owned** —
```
… tiles bearing Spice that are NOT already within 2 tiles of one of your cities … fewest TURNS over land from your nearest city … within 6 turns; answer a whole number or "none".
-> none
```
**reachable-nearest** —
```
You control the Alpine Troops at (61, 33). Moving over land only … fewest turns to the nearest Game tile … within 6 turns; number or "none".
-> none
```
*Concern (both):* on this LATE board both answer **"none"** — degenerate. These only carry signal on
early/mid boards (rail + dense cities trivialize them late). **If the run includes late boards, these
two must be board-restricted or they add noise, not signal.**

### T2 combat floor

**unit-strength** —
```
Which unit has the greater DEFENSE strength, the Riflemen at (29, 19) (unit #1541) or the Cruiser at (30, 28) (unit #1454)?
-> Cruiser at (30, 28)
```
*Concern:* this is nearly a table lookup (unit → base defense × modifiers). Genuine floor, but does
it add anything city-defense doesn't? Candidate to drop if trimming.

**city-defense** —
```
Which city is better defended, the city "Mosul" at (11, 42) or the city "Romi" at (29, 19)?
-> Romi
```
*Concern:* good — composite (best defender × walls × terrain). Keep.

### ★ Survivors (feature these)

**constraint-site** —
```
… search the square REGION within 5 tiles of (75, 5) … name any LAND tile that meets ALL of: defensible AND within 2 tiles of water AND not in enemy territory AND ≥3 tiles from every existing city … or "none".
-> (70, 1)
```
*Why it survives:* the conjunctive spacing constraint is not bundled by any single tool — the model
must compose site_check + distance. Confirmed live (Variant C). **This is the exhibit; keep exactly.**

**compare-two-attacks** —
```
… compare two attacks purely on VALUE … Attack A … strikes the Alpine Troops at (33, 20). Attack B … strikes the Transport at (31, 21) … which is better, or incomparable? Answer target coords or "incomparable".
-> (33, 20)
```
*Why it survives:* two-outcome value trade-off with an "incomparable" escape hatch. Keep.

### Saturators / "sound" cluster (the trim question lives here)

**settle-site** — `which of these candidates is a SOUND (non-dominated) city site … -> (26, 23)`
**adv-assault-target** — `which of your cities is a MOST ATTRACTIVE assault target (a sound answer) … -> Yo'k'ib'`
**t3-retreat** — `which tile is a SOUND (non-dominated) tile to retreat to … -> (59, 37)`
**forward-posting** — `which advance is SOUND (not clearly worse …) … -> (29, 25)`
**triage-reinforce** — `which city does the spare defender best protect … -> Masuul`
**cf-vacate** — `can the city spare its single best defender … Answer yes or no. -> yes`
**t3-threat** — `which single city is under the GREATEST threat … -> Masuul`

*Shared concern:* these five "sound/non-dominated" + two "which-city" kinds all **saturate under
maxops** and their only job now is the **arm-spread + cost** claims. You almost certainly do **not
need all seven** for that — 2–3 representative ones (one "sound-set", one "which-best", one yes/no)
would make the same point at lower N-spread and a cleaner story. **Biggest single roster decision.**

---

## Roster-level questions for the owner

1. **Trim the saturator cluster?** Keep all 7 (settle-site, adv-assault-target, t3-retreat,
   forward-posting, triage-reinforce, cf-vacate, t3-threat) or cut to a representative 2–3? They add
   little *within* maxops; full sampling only matters in the no-ops/spatial-ops arms for arm-spread.
2. **best-site** — you flagged "best site is underdetermined." Its proxy is well-defined but narrow.
   Demote to a perception-only kind, or drop (region-count already anchors perception)?
3. **unit-strength** — keep as a T2 floor, or drop as near-lookup redundant with city-defense?
4. **forward-posting** — re-label as an adversarial tool-fit kind (it's tool-reducible on a frozen
   board), or pull it out of the frozen run entirely and save it for the path-(c) 2-ply rollout?
5. **Movement kinds** — confirm the early/mid board restriction (decision E) is wired for the actual
   run boards; otherwise drop them if the run is late-board only.
6. **Answer-shape trust** — are you comfortable scoring "name ANY sound / non-dominated" via the
   precomputed acceptable-set (`Answer::ChoiceSet`)? If not, that's a redesign touching 6 kinds.
7. **Deliberate gaps** (confirm they stay out): global/whole-board aggregation (declined — a counter
   nails it), multi-turn/temporal (path c, deferred), economic-value-beyond-size (decode-blocked),
   the fog trio (audited out).

---

## The retired fog trio (for completeness)

hidden-force / fogged-assault / surprise-strike — **audited out** as information-bound (luck), kept
in code as the ceiling-theorem exhibit. See the amendment in `kind-roster-preregistration.md` and
`reasoning-frontier-vs-luck.md`. Not scored in the run; nothing to decide here.

---

## After you decide

Whatever you choose becomes a small edit to the run's `--kinds` list (and, if trimming, a
second amendment note in the preregistration so the change is logged, not silent). Then the
`--per-kind 1` pre-flight, then the run. Nothing here has cost cloud budget.
