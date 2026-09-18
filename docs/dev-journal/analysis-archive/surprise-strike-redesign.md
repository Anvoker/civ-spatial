# Surprise-strike (P1) redesign — a deducible, worst-case exposure

*Status: PROPOSED (worktree branch, not merged). Owner sign-off required — this changes what the*
*kind measures. Companion to `reasoning-frontier-questions-v2.md` §v2.1 P1 and*
*`kind-roster-preregistration.md` (surprise-strike row).*

## 1. The problem, restated crisply

`surprise-strike` (P1) asks the model: *"which ONE of your own cities is most likely to come under a
surprise attack within the next few turns from an enemy unit you cannot currently see (hidden in the
fog nearby)?"*

The **old** ground truth was

```
exposure(city) = hidden_strike_threat(unmasked, masked, player, city, SURPRISE_REACH)
               = max att_eff over enemy LAND units ACTUALLY STANDING on a fogged tile
                 within Chebyshev SURPRISE_REACH (=5) of the city, read from the UNMASKED board.
```

The defect: the winning city is decided by **where the enemy units really are** — information the
model is denied. The generator's luck-free floor (a *provably-safe decoy*: a candidate with no fogged
tile in strike range at all) lets a reasoner rule out one city without seeing the fog. But on the
3-candidate instances that also carry a *fog-adjacent-but-empty filler* city, once the decoy is ruled
out the model is left choosing between two observationally-identical fog-adjacent cities, and which
one is "exposed" is set by an **invisible unit**. That fraction of the score is therefore **luck**, and
a low score confounds the thesis ("reasoning-frontier resistance") with aleatoric noise ("the answer
was not knowable from what the model saw").

We want a version where a perfect reasoner can, **in principle, always get it right from the masked
board** — the ground truth must be a deducible / worst-case-robust function of *observable* state.

## 2. The new definition

**Ground truth is now a pure function of the MASKED board.** The unmasked board is not read for this
kind at all.

Build the graded land-threat field from the **visible** enemies on the masked board:

```
field = ThreatField::compute(masked, player)
```

`ThreatField` already does exactly the worst-case-strength-under-reach reduction we need: for every
enemy LAND unit it can see, it BFS-expands over land (fogged land tiles keep their real terrain on the
masked board, so they are pathable), and records at each tile the **scariest** incoming `att_eff`,
faded by the number of turns to bring it to bear (`att_eff / reach_turns`) and zeroed past
`THREAT_HORIZON` (=6). Because it is built on the **masked** board it sees **only the enemy units the
player can see** — the observable repertoire.

Then, for a candidate city `C`:

```
exposure(C) = max over tiles T with is_fogged(T) and chebyshev(T, C) <= SURPRISE_REACH
                  of   field.at(T) * closeness(T, C),

  where closeness(T, C) = 1 / max(chebyshev(T, C), 1)   (a nearer hiding spot is scarier).
```

A fogged tile `T` contributes only when **a visible enemy could actually operate at it**
(`field.at(T) > 0`) — i.e. some enemy unit the player *can see* could move into that fog and bring an
attack to bear within the horizon. `exposure(C) = 0` when no in-range fogged tile is enemy-reachable
(in particular, the provably-safe decoy — no in-range fogged tile at all — scores 0).

Two observable ingredients are combined, and **both** require real spatial reasoning:

1. **Which fog is enemy-reachable** — the model must trace, over land, whether a *visible* enemy unit
   could get into each fogged tile near `C` within `THREAT_HORIZON` turns. A city ringed by fog that
   no visible enemy can reach (blocked by ocean, or simply too far) scores 0, even with lots of fog.
2. **How strong is the scariest reachable enemy** — the worst-case striker is the strongest *seen*
   enemy attacker that can reach that fog, faded by how many turns it needs and weighted by how close
   the hiding tile is to the city.

The reasoning the kind asks: *"how much force could the fog **structurally admit** against this city,
given what you can observe"* — a worst-case upper bound, **not** the realized truth.

### Preserved scaffolding

* **Provably-safe decoy floor** — unchanged and still the simplest, unarguable certificate: a
  candidate with `!any_fogged_within(masked, C, SURPRISE_REACH)` has no fogged tile in strike range,
  so `exposure(C) = 0` and the model can prove it *without seeing the fog*. Still required in the
  candidate set (`has_safe_decoy`).
* **Decisive-margin gate** — unchanged: the top city must beat the runner-up by the multiplicative
  `POSTING_PRESSURE_MARGIN` (=1.25×) ratio, else the instance is a near-tie and is dropped.
* **Admissibility** — unchanged: need ≥1 exposed (`exposure > 0`) and ≥1 provably-safe candidate, all
  distinctly-named cities of the perspective player.

### Deducibility

`exposure` reads only `masked` (its fog grid + its visible enemy units + terrain). Hidden units never
appear on the masked board, so **moving a hidden enemy unit to any other fogged tile cannot change the
answer** — the function's very signature (`surprise_exposure(masked, player, center, radius)`) has no
access to the unmasked board. This is the property the old design lacked, and it is asserted directly
by a test.

## 3. Alternatives considered

**Alt A — pure fog geometry ∩ enemy-reachable, unweighted count.**
`exposure(C) = |{ enemy-reachable fogged tiles within reach of C }|`. *Rejected:* (i) drifts toward a
counting tool; (ii) ignores strength and closeness, so it is a poor worst-case — a city ringed by
distant reachable fog outscores a city with a single *adjacent* reachable fog tile, which is backwards
for a *surprise strike* (proximity is the whole point); (iii) loses the "scariest known unit type"
ingredient the design calls for.

**Alt B — global worst-case strength × nearest admissible fog.**
`exposure(C) = (max att_eff over ALL visible enemy attackers) × max closeness(T, C)`. *Rejected:*
strength becomes a single global constant, so the ranking collapses to "which city has the nearest
enemy-reachable fog" and the strength ingredient is inert for ranking. It also ignores turn-fade and
*which* enemy can reach *which* city. The chosen per-tile `field.at(T)` strictly subsumes Alt B: it is
the strongest *seen* enemy that can *actually reach that specific tile*, already turn-faded.

**Alt C — retain an unmasked worst-case (strongest enemy TYPE anywhere, placed adversarially in the
in-range fog).** *Rejected:* reading the unmasked roster leaks the existence and strength of unit types
the player has **never seen**. That is precisely the unknowable/aleatoric channel we are removing. The
worst case must be taken over the **observed** repertoire only (consistent with
`rules::worst_case_garrison`, which the fogged-assault kind already uses for the same reason).

**Chosen:** per-tile masked `ThreatField` × closeness, max over in-range fogged tiles (§2). It is the
formulation that (a) is a pure function of the masked board, (b) combines reachability + worst-case
strength + proximity, (c) does not collapse to a single tool op, and (d) reuses the project's existing,
already-tested threat machinery rather than inventing a parallel one.

## 4. Worked example

24×5 all-Grassland board; fog covers the band `x >= 19`. Perspective player "Me" owns two cities:

* **Haven** at `(2, 2)` — the nearest fogged tile is at `x = 19`, Chebyshev 17 away (> 5). No fogged
  tile in strike range ⇒ **provably safe**, `exposure = 0`. A reader proves this from fog geometry
  alone, without seeing into the fog.
* **Marches** at `(18, 2)` — fogged tiles at `x = 19..23` lie within Chebyshev 5.

A **visible** enemy Legion (att 4) stands at `(16, 2)` (on a visible tile). Over land it reaches `(18,
2)` in 2 steps and can bring an attack to bear on the fogged tile `(19, 2)`, so `field.at(19, 2) =
4 / ceil(2/1) = 2.0`. `(19, 2)` is Chebyshev 1 from Marches, so `closeness = 1`, giving
`exposure(Marches) >= 2.0`.

A hidden enemy striker actually sits at `(22, 2)` in the fog — but it is **never read**. Whether it is
at `(22, 2)` or `(23, 3)` or absent, `exposure(Marches)` is unchanged, because it is computed from the
visible Legion's reach into the fog, not from the hidden unit.

Result: `exposure(Marches) = 2.0` vs `exposure(Haven) = 0` — decisive (`2.0 >= 1.25 × 0`), a safe
decoy is present, so the answer is **Marches**. A fog-blind reader who just counts nearby fog, or who
picks the biggest city, can be made to miss; the model must reason that a *visible* enemy can *reach*
the fog *close to* Marches.

## 5. What the kind measures now vs. before

| | Before | After |
|---|---|---|
| Ground-truth input | UNMASKED board (true hidden units) | MASKED board only |
| Quantity | realized: `att_eff` of the unit *actually* in the fog | worst-case: strongest *seen* enemy reach into in-range fog × proximity |
| Decidable from what the model sees? | **No** (needs hidden placement) | **Yes** |
| Question answered | *"predict the actual striker"* | *"assess structural exposure to a possible striker"* |

**This is a genuine semantic shift and it is the single thing the owner must approve.** The kind stops
being a *prediction of a hidden fact* and becomes an *assessment of observable structural risk*. The
upside is that it is now luck-free by construction: every instance has a from-the-masked-board-correct
answer, so a below-ceiling score is attributable to reasoning difficulty (tracing visible-enemy reach
into fog and weighing proximity), which is exactly the reasoning-frontier signal the thesis wants — not
to aleatoric noise about where an unseen unit happened to be.

## 6. Residual validity caveats

* **Worst-case is bounded by the observed repertoire.** If the true hidden striker is a *type the
  player has never seen* (faster or stronger than anything visible), the worst-case under-counts it.
  This is deliberate and consistent with `worst_case_garrison` (fogged-assault): you cannot be asked to
  account for a capability you have no evidence exists. It is the honest price of deducibility.
* **`field.at(T)` is "can attack T", used as a proxy for "can hide a striker at/near T".** The
  `ThreatField` marks a tile threatened when an enemy can reach a tile *adjacent* to it; a striker that
  *stands in* the fog to hit `C` needs to reach the fogged tile itself. Within the generous `K = 6`
  horizon these differ by at most one step and are immaterial to the coarse worst-case ranking; the
  choice reuses one already-parity-tested field rather than adding a second reach model.
* **Connectivity band still matters (unchanged from v2.1).** On fully-railed boards every fog pocket is
  reachable from every visible enemy and the reachability ingredient flattens; on rail-free boards it
  dominates. The kind's signal lives in the middle band, and the decisive-margin gate plus the
  provably-safe decoy keep only instances with real separation. Tag instances with the connectivity
  regime in post-analysis, as already planned.
* **Support width.** The kind still needs ≥1 exposed and ≥1 provably-safe own city; additionally it now
  needs the player to have **seen at least one enemy land attacker** that can reach the exposing fog
  (else `exposure = 0` everywhere and nothing generates). On scenarios where the only enemy is fully
  hidden, the kind now *correctly declines to generate* — the honest outcome, since with no visible
  enemy there is no deducible basis to rank the cities. Expect support to remain narrow (it was 3/15
  boards before); this does not widen it and may slightly narrow it, in exchange for luck-free scoring.

## 7. Companion fix — `fogged-assault` (P7f) to the same observed-repertoire rule

The owner asked that `fogged-assault` be brought to the identical philosophy so both hidden-info kinds
are fully deducible from what the model sees. P7f already scored against a *worst-case synthetic
garrison* (`rules::worst_case_garrison`) rather than the true hidden garrison — good — but it solved
ground truth on the **unmasked** board, so `worst_case_garrison` scanned `unmasked.units`. If the
enemy's strongest defender type appeared **only on fogged tiles**, the label used a unit type the model
has no evidence exists — the same aleatoric channel removed from surprise-strike.

**Fix (minimal, verified sound):** solve `FoggedAssault` ground truth on the **masked** board
(`ctx.board`) instead of the unmasked one. Everything the solve arm needs is present and last-known on
the masked board — the enemy city (kept last-known under fog with its walls/terrain context), my own
visible attacker stack, and the fogged tile's retained terrain — and `worst_case_garrison` then scans
only the **visible** enemy units, i.e. the observed repertoire. The kind, like surprise-strike, now
reads the unmasked board **not at all**. The `None` → certain-capture behaviour is preserved: when the
enemy has no *visible* combat unit, the worst case is an undefended city → "yes" (the correct
observed-repertoire answer — no evidence of any defender).

**Consequence to note:** for a defended-but-fully-fogged enemy city with no visible enemy units, P7f
now labels the assault a certain **capture** ("yes"), because the model can see no defender and no
repertoire evidence of one. Previously the hidden (or fog-only) garrison could force a "no". This is
the intended honesty trade — the label is now exactly what a rational fog-reasoner concludes from
observables — but it does shift some fully-fogged instances from "no" toward "yes", and it may reduce
yield where the only defender evidence was itself fogged. `worst_case_garrison`'s contract doc now
states it must be called on the masked board for the observable guarantee to hold.
