# Hidden-force (P3) audit — information-bound (luck), not a reasoning frontier

*Status: ANALYSIS (worktree branch, not merged). Verdict below. Production behaviour of the kind is*
*unchanged; the only code added is two committed audit tests. Companion to*
*`analysis/surprise-strike-redesign.md`, whose diagnostic and fix pattern this mirrors.*

## Verdict

**LUCK (information-bound).** The ground-truth answer is a function of the **unmasked** (hidden)
enemy placement, not of anything on the masked board the model can observe. A 2× smarter model does
**not** cross the plateau, because the winning area is decided by where invisible units actually
sit. This is the *same defect* the old `surprise-strike` had — and in a sharper form: the candidate
"hot" area is literally centred on a tile the model cannot see, occupied by a unit it cannot see.

One-line recommendation: **redesign** to an observed-repertoire, masked-board worst-case (P3 is in
fact *more* salvageable than surprise-strike — see §5); demote to illustrative only if the redesign
fails the support-width bar.

## 1. Code evidence — which board it reads

The magnitude the answer keys on is computed by `rules::hidden_force_in_region`
(`civ-eval/src/rules.rs:994`). It reads `unmasked.units` and sums the effective attack power of the
enemy units **actually standing** on fogged tiles in the region:

```rust
// civ-eval/src/rules.rs:1001
unmasked
    .units
    .iter()
    .filter(|u| u.owner != player)
    .filter(|u| civ_core::geometry::chebyshev((u.x, u.y), center) <= radius)
    .filter(|u| masked.is_fogged(u.x, u.y))   // masked used ONLY as an is-hidden mask
    .filter(|u| /* land */)
    .filter_map(att_eff)
    .sum()
```

The doc-comment states it plainly (`rules.rs:990-993`): *"…on the UNMASKED board — the 'massed
hidden force' … Depends entirely on masked-out occupants."* The `masked` argument is used **only**
for the `is_fogged` predicate; it never contributes a force value.

The generator seals this. In `HiddenForceKind::generate` (`civ-eval/src/generators.rs:2385`) the
**hot** candidate centres are the tiles of enemy land units the perspective *cannot see*:

```rust
// generators.rs:2385
let hot: Vec<(i32,i32)> = unmasked.units.iter()
    .filter(|u| u.owner != me)
    .filter(|u| board.is_fogged(u.x, u.y))   // hidden from the model
    .filter(|u| /* land */)
    .map(|u| (u.x, u.y)).collect();
```

The correct answer is, by construction, the centre sitting on a hidden unit; the **cold** decoys are
provably-empty regions (no fogged tile at all, `generators.rs:2397`). `hidden_force_answer`
(`generators.rs:2318`) then ranks the candidates purely by these unmasked force sums and returns the
top one. Single-board `solve` returns `None` for this kind (`question.rs:1508`), deferring the whole
ground truth to the unmasked-reading generator — exactly the old surprise-strike shape.

Contrast the already-redesigned sibling `rules::surprise_exposure` (`rules.rs:943`), whose signature
`(masked, player, center, radius)` has **no** `unmasked` parameter at all — the structural guarantee
P3 lacks.

## 2. Empirical perturbation result

Test: `civ-eval/src/generators.rs`, module `hidden_force_audit`
(`answer_flips_when_hidden_stack_moves_masked_board_unchanged`). Both tests pass
(`cargo test -p civ-eval --lib hidden_force_audit` → 2 passed).

Setup: a 12-wide grassland row; tiles `x >= 6` are fogged. Candidate centres `(9,0)` and `(6,0)`
(both entirely inside the fog band) plus a provably-empty decoy `(2,0)`. The **masked board the model
sees is byte-for-byte identical** in both worlds — there is no visible enemy at all, because every
enemy unit stands on a fogged tile.

| World | Hidden 3×Legion stack at | Ground-truth answer |
|---|---|---|
| A | `(9, 0)` | `(9, 0)` |
| B | `(6, 0)` | `(6, 0)` |

Moving the invisible stack from region A to region B **flips the answer**, with zero change to any
observable input. The companion test `region_force_is_read_from_unmasked_units` shows the region's
score is `> 0` with the hidden stack present and exactly `0` when it is removed — again with an
identical masked view. This is the operational definition of information-bound: **the answer is a
function of hidden state on the unmasked board.**

## 3. Observable-proxy assessment

The `HIDDEN-FORCE RULES` prose (`rules.rs:2026`) tells the model to reason from cues — *"fogged area
near the enemy's own territory or a visible enemy unit is a likelier muster point; an area with NO
fogged tiles is provably empty."* Assess honestly whether the ground truth is a **function** of those
cues:

- **Provably-empty decoy** — this cue is real and sound: a region with no fogged tile scores 0 and
  can be *excluded* from the masked board. But it only rules candidates **out**; it never selects the
  winner among the fogged regions.
- **Proximity to enemy territory / visible enemy units** — the ground truth does **not** read
  territory ownership or visible-enemy distance at all (`hidden_force_in_region` filters on
  `owner != player`, `chebyshev <= radius`, `is_fogged`, land — nothing about muster proximity). The
  hot centre is placed wherever a hidden unit happens to be. On the corpus these units are AI-placed;
  their fogged position correlates only weakly, and non-causally, with the suggested cues. In the
  perturbation test the "muster proximity" cue is *constant* across both worlds, yet the answer moves.

So a weak proxy exists (the decoy-exclusion cue), but **the ground truth is not a function of the
proxy** — it is a function of the hidden placement. Per the diagnostic framework, a kind is
information-bound *even if* a weak proxy exists, when the label isn't determined by that proxy. That
is exactly this case: among the fogged candidates, which one wins is pure hidden entropy.

## 4. Verdict and reasoning about a smarter model

**Information-bound (luck).** A perfect reasoner given only the masked board cannot distinguish
World A from World B above — they are the same board — so it cannot exceed chance among the fogged
candidates beyond what the decoy-exclusion cue buys. A 2× smarter model climbs **only** on the
decoy-exclusion fraction (ruling out no-fog regions), which is a trivial masked-board check, not a
frontier; on the discriminating step it is guessing where invisible units are. The plateau is
environmental entropy, not model capability. It does **not** meet the reasoning-frontier bar.

## 5. Proposed fix — observed-repertoire redesign (P3 is MORE salvageable than surprise-strike)

Follow the surprise-strike precedent: make "force an area could hide" a **worst-case quantity
computed from the masked board**, so moving hidden units cannot change the answer. Concretely, define
the region's deducible massed-force potential as the enemy attacking strength that *visible* enemies
could bring into the area's fogged tiles, reusing the existing masked-board `ThreatField`
(`rules.rs:789`) exactly as `surprise_exposure_with` (`rules.rs:963`) already does:

```text
force(area C) = sum over tiles T with is_fogged(T) and chebyshev(T, C) <= radius
                    of  field.at(T)          // ThreatField::compute(masked, player)
```

(A **sum** over the area's fogged tiles, not a max — "massed force" is about aggregate strength an
area can admit, which is the natural P3 analogue of surprise-strike's per-tile max.) `field.at(T)` is
the scariest *seen* enemy `att_eff` that can actually reach `T` over land within the horizon, faded by
reach turns — a pure function of the masked board. The generator's hot centre would then be the fogged
region with the greatest *reachable-visible-enemy* potential, not the one hiding an unseen unit.

The kind then reads the unmasked board **not at all**, and the perturbation test would assert the
answer *cannot* change when hidden units move (mirroring the surprise-strike deducibility test).

**Why P3 is more salvageable than surprise-strike.** Surprise-strike's owner debated demotion because
its per-city max collapsed toward "which city has the nearest reachable fog" once strength was faded.
P3's framing is inherently kinder to the redesign:

1. **"Massed force" is intrinsically spatial and observable.** Massing real strength requires enough
   fogged tiles *and* visible-enemy reach into them — both masked-board quantities. A big massed force
   needs observable *space* (a large fog pocket) and observable *access* (visible enemies able to
   funnel in). The redesigned quantity therefore has genuine spread across candidates rather than
   collapsing to a single nearest-fog tiebreak.
2. **Relative "which area is biggest" is robust to worst-case under-counting.** Surprise-strike's
   validity caveat — the true striker may be a type never seen, so the worst-case under-reads — hurts
   an *absolute* "will this city be hit" question more than a *relative* "which area is largest" one.
   For P3 the under-count applies roughly uniformly across candidate areas (same observed repertoire),
   so the **ranking** it produces stays sound even when absolute magnitudes are conservative.
3. **The sum-over-area aggregation restores a strength gradient.** Because it sums reachable strength
   over multiple fogged tiles, area size and reach-density both matter, so the ingredients don't
   degenerate to a single constant the way surprise-strike's Alt B did.

### Preserved scaffolding (unchanged from current P3)

- **Provably-empty decoy** (`any_fogged_within(masked, C, radius) == false` ⇒ force 0) — already
  masked-board-sound; keep as the luck-free floor.
- **Decisive-margin gate** (`POSTING_PRESSURE_MARGIN` 1.25×, `generators.rs:2352`) — keep.
- **Admissibility** — need ≥1 area with `force > 0` and ≥1 provably-empty decoy.

### Same caveats surprise-strike hit

- **Worst-case bounded by observed repertoire** — if the true massing type was never seen, the label
  under-counts it; deliberate and consistent with `worst_case_garrison` / `surprise_exposure`.
- **Support width** — the redesign now also requires the player to have *seen at least one enemy land
  attacker able to reach the fog*; on boards where the only enemy is fully hidden the kind *correctly
  declines to generate* rather than scoring on luck. Expect narrow support (P3's yield was already
  narrow), traded for luck-free scoring.
- **`field.at(T)` is "can attack T" used as a proxy for "can mass at T"** — within the `THREAT_HORIZON`
  these differ by at most one step; immaterial to a coarse ranking, and reuses one already-parity-
  tested field.
- **Connectivity band** — on fully-railed boards every fog pocket is reachable and the reach
  ingredient flattens; the kind's signal lives in the middle band, kept by the margin gate + decoy.

## 6. Recommendation

**Redesign P3 to the observed-repertoire, masked-board worst-case above.** It is a genuine semantic
shift — the kind stops *predicting a hidden fact* and starts *assessing observable structural risk*,
exactly the shift the owner approved for surprise-strike/fogged-assault — and it makes P3 luck-free by
construction, so a below-ceiling score becomes attributable to reasoning (tracing visible-enemy reach
into fog and aggregating strength) rather than to where unseen units happened to be. Given §5, P3 is a
*better* redesign candidate than surprise-strike was; demote to illustrative only if the redesign
can't clear the support-width bar on the corpus.

## Appendix — audit artifacts (committed on this branch)

- `civ-eval/src/generators.rs` → `mod hidden_force_audit`:
  - `answer_flips_when_hidden_stack_moves_masked_board_unchanged` — the decisive perturbation.
  - `region_force_is_read_from_unmasked_units` — force is read off unmasked units.
- No production behaviour was changed; `hidden_force_answer` / `hidden_force_in_region` are untouched.
