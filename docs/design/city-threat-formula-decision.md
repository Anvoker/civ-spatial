# City-Threat Formula Decision — attacker-attractiveness vs. raw force

**Status:** recommendation delivered, awaiting one input (aggregation rule) before it can be
folded into `T2-T3-design.md` §4.3. No core code written.

**Context:** this resolves the open question raised in session `b9ed03ea` about how to define
"threat to a city" for the T3 "most threatened city" / "rank cities by exposure" question kinds
(continuity doc's headline T3 examples). The tension: a well-defended big city draws more *raw
force*, but a soft small city is the smarter *target* — the two definitions diverge, and one had
to be picked as canonical.

**Read first:** `T2-T3-design.md` (esp. §1 the reframing, §2 combat model, §4 the dominance
principle) and `T2-T3-valuation-continuity.md` (the four disciplines).

---

## What I researched

- Re-read `T2-T3-design.md` end to end — the combat/strength `rules` module (§2), the T2b
  city-defense comparison (§3.2), the T3 dominance principle (§4.1), the settle-site safety
  threat field (§4.2), and the "other T3 family members" sketch incl. rank-by-exposure (§4.3).
- Re-read `T2-T3-valuation-continuity.md` — the four non-negotiable disciplines, especially
  #1 (give the model the same rules), #3 (decisive-margin filter), #4 (do NOT reimplement
  Freeciv exchange resolution).
- Framed the decision against the design's own stated purpose (§1: T2/T3 exist to stress the
  encoding's *information-capacity* axis via long attribute cross-reference chains, the "error
  amplifier" that breaks the T0/T1 ceiling).

## What I found (the recommendation)

**Attacker-attractiveness is the right *concept*, but it should be encoded as two dominance
axes — incoming force (↑) and own defense (↓) — not as a single scalar. And the "which is
canonical" framing is a false dichotomy: raw-force and attacker-attractiveness are the correct
tools for two structurally different questions that already exist in the design.**

### Why attacker-attractiveness wins on concept

1. **It's what the question-name promises.** "Most threatened" = "most likely to fall," not
   "most force nearby." A walled Metropolis on a mountain with a Mech. Inf. garrison, ringed by
   Warriors, is *not* threatened. An undefended size-2 city with a Cavalry two tiles out *is*.
   Raw force ranks these backwards.
2. **It's maximally on-mission per §1.** Raw-force threat only reads enemy units + reachability
   — a short chain, ~T1 + a strength lookup. Attacker-attractiveness *additionally* forces
   locating each city's garrison, its walls bit, and its terrain/fort bonus, then combining —
   a strictly longer cross-reference chain = strictly more capacity-axis stress = the exact
   ceiling-breaker T2/T3 exist to create.
3. **It composes committed machinery.** It's the T3a incoming-force numerator over the T2b
   `def_eff` denominator. No third notion invented; reuses the T2b city-defense solver and the
   T3a threat field. One source of truth.

### The reframe that removes the hard part

The worry behind "which formula" is really the **combination rule** (ratio? difference? win-
chance?) — every choice there risks discipline #4 (exchange-resolution creep) and the "scoring
my own heuristic" trap.

**So don't combine them.** Dominance exists precisely to avoid committing to a weighting (§4.1).
Keep incoming-force and defense as **two separate axes** and score by Pareto dominance:

> City A is unambiguously more threatened than B iff A has **≥ incoming force** AND
> **≤ defense**, with at least one strict.

A city that's both more-attacked and softer dominates — no coherent attacker-model disagrees.
Genuine tradeoffs (more force *and* more defense — the big well-defended city) are exactly what
dominance withholds. No combination rule chosen, no heuristic scored, and the soft-small-city
still surfaces whenever it is genuinely dominant. Same escape hatch already blessed for
settle-site.

### The two-question split (the actual answer to "which is canonical")

Neither formula is globally canonical — the two questions have different structure:

| Question | Formula | Why |
|---|---|---|
| **City threat / exposure** (§4.3: "most threatened", "rank by exposure") | **Attacker-attractiveness** as two dominance axes (incoming↑, defense↓) | A defender exists — its garrison/walls are the whole point |
| **Settle-site safety** (§4.2) | **Raw incoming force** (`att_eff / reach_turns`) | The candidate tile is *empty* — no defender to divide by; attacker-attractiveness is degenerate here |

The settle axis already uses raw force in §4.2 and is correct there. City-threat is where
attacker-attractiveness belongs. This is the principled reading, not a compromise.

### Single-answer `Choice` form ("which ONE city is most threatened?")

Dominance gives only a partial order, so for a single winner:
- **"Most threatened" = the unique city that dominates all others** on (incoming↑, defense↓).
- **Decisive margin** on both axes (reuse the 1.25× discipline).
- **Drop instances with no dominator** — frequent on sparse boards; log the drop rate. That is
  the honest cost of a preference-free definition and keeps answers non-arguable.

### If a scalar is ever unavoidable

(e.g. a UI sort or "threat bucket") use **ratio** `att_eff / def_eff`, not difference — it's
scale-free and tracks "can it be taken." Treat `def = 0` (undefended) as **top-rank** rather
than dividing by zero (undefended = maximally threatened, which is correct). Difference
(`att − def`) systematically understates how soft a tiny undefended city is — avoid it.

---

## Where I left off

Recommendation is complete and internally consistent with the existing design. It has **not**
yet been written into `T2-T3-design.md` — §4.3 still only sketches "rank by exposure" without
committing a threat definition. Next mechanical step (once the input below is resolved): expand
§4.3 to specify the two-axis attacker-attractiveness dominance model + the single-`Choice`
"most threatened" variant with its margin/drop rules, mirroring the §4.2 write-up style.

## Input I need before finalizing

**Aggregation rule for the incoming-force axis: `max` (scariest single attacker) or `sum`
(siege by many)?** §4.2 left this as a v1-`max` / alt-`sum` choice for settle safety. It should
be **consistent** across both questions so the model learns one threat semantics, not two. My
lean is **`max` for v1** (matches "which single unit takes this city"), with `sum` documented as
the stated alternative — but it's genuinely your call, since it turns on how much multi-unit
siege should matter on the dense T677 board.

Secondary (lower stakes, can default): confirm the decisive-margin ε for the two threat axes —
proposal is to reuse the existing **1.25×** ratio discipline on each axis independently.
