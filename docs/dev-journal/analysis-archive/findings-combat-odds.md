# Findings — combat odds (win-probability) vs. the stated scalar

**Date:** 2026-08-04 · branch `combat-odds` (isolated worktree, not merged). Question: does replacing the
deterministic strength scalar with a real Freeciv-`classic` **win probability** change the oracle's "correct"
answers enough to be worth merging? **Boards:** `testcontroller_T677.sav` (dense late-game, 682 units / 118 cities)
and `myagent_T50.sav` (sparse early game). **Reproducer:** `cargo run -p civ-eval --example combat_odds_divergence`.

## TL;DR
Higher fidelity is **material, not cosmetic**. On the dense T677 board, swapping the raw strength ratio for the
HP/firepower win-probability flips a large share of the oracle's "correct" answers:

| decision kind | T677 answers changed | T50 answers changed |
|---|---|---|
| compare-two-attacks (favorability verdict) | **28.6%** (5450/19044) | 9.5% (4/42) |
| cf-vacate (yes / no / dropped) | **12.3%** (10/81) | 0/0 (too sparse) |
| capture-state (triage-reinforce core) | **6.9%** (7/102) | 0/4 |

The single biggest driver: a lopsided **strength ratio is not a lopsided win chance** once hitpoints enter. A 2:1
`att_eff/def_eff` edge against a high-HP defender is often a coin-flip or worse, which the scalar model could never
see. On the sparse T50 board the effect is muted (fewer stacked, contested cities), so the fidelity mainly matters
on realistic dense maps — exactly where the eval's decision kinds live.

**Recommendation: worth merging.** The primitive is small (an O(hp^2) DP), reuses the existing modifier stack, adds
one parity-complete op, and the divergence is concentrated in the combat decision kinds the eval is built to score.

---

## What changed

**STEP 1 — the primitive (`rules.rs`).** `win_probability(a_power, att_hp, att_fp, d_power, def_hp, def_fp)`
models classic combat as an HP/firepower race: each round the attacker hits with `p = A/(A+D)` (A, D are the
existing `att_eff` / `def_eff` — veteran x health x terrain/fort/walls already folded in, *not* rebuilt), and must
land `ceil(def_hp/att_fp)` hits before the defender lands `ceil(att_hp/def_fp)`. The exact race probability is an
O(n*m) DP (n, m <= 40). Degenerate: `A<=0 -> 0.0`, `D<=0 -> 1.0`. Unit tests cover the equal-fight 50/50 identity,
overwhelming/degenerate cases, the HP-dominates-a-high-ratio case, and firepower monotonicity.

**HP / firepower source.** *No new constants were needed* — `UnitStat` already carries `hitpoints` and `firepower`
(the classic table, e.g. `Fighter`/`Battleship`/`Cannon`/`Artillery`/`Howitzer` fp = 2, all others 1), and `Unit`
carries current `hp`. The race uses current `hp` and table `firepower` directly; `att_eff`/`def_eff` supply A/D.
(On real snapshots `hp = max`, so the `health` factor already inside A/D is 1 and there is no double-count; on a
synthetic damaged unit it is a deliberate simplification, documented at the function.)

**STEP 2 — integration (`question.rs`, helpers in `rules.rs`).**
- **compare-two-attacks** favorability axis: `att_eff/def_eff` ratio -> `attack_win_prob` (the win probability).
  Threat-removed axis unchanged; the two-axis Pareto/near-tie discipline (1.25x margin) is preserved.
- **cf-vacate**: the 1.25x "incoming force vs remaining defense" ratio -> the win probability that the city's
  scariest incoming attacker beats the *remaining* (second-best) defender, judged against the decisive band
  (falls >= 0.7, holds <= 0.3, near-tie between -> dropped).
- **triage-reinforce**: the capture-state classifier (`Held`/`Capturable`/`Contested`) now reads the same
  win-probability band instead of the strength ratio; "flips capturable->held" logic is otherwise unchanged.
- `ThreatField` was extended to record the scariest attacker's combat stats per tile (`AttackerStat`), so the
  odds model can ask that attacker's win probability against a defender. Selection of *which* attacker is scariest
  is unchanged (still the turns-faded `att_eff` max).

**STEP 3 — parity op (`encoders.rs`).** Added `win_prob(attacker, defender)` = `rules::attack_win_prob` verbatim
(the compare-two-attacks favorability). Because a defender standing in its own city carries the city-center/walls
context through `unit_def_on_own_tile`, the *same* op also reproduces the cf-vacate / triage city-fall odds, so the
maxops calculator stays parity-complete with the odds solver. New parity test `op_win_prob_parity_and_matches_solver`.

**Prompt (`rules_block`).** Added a COMBAT ODDS paragraph (p = A/(A+D), the HP/firepower race, "a big ratio can
still be a coin-flip"), added firepower to the printed unit-stats table, and rewrote the VACATE / TRIAGE /
ATTACK-COMPARISON rule blocks to state the win-probability model — so the model is told exactly the rules it is
scored against (discipline #1).

---

## Concrete flips (from the reproducer, T677)

**compare-two-attacks — a fake favorability trade-off collapses.**
`Cannon strikes [Mech.Inf. def_eff=22.5 hp30] vs [Battleship def_eff=45 hp40]`: OLD ratio-fav 0.36 vs 0.18 ->
**incomparable** (A leads favorability, B leads threat-removed). NEW win-prob 0.00 vs 0.00 -> **B dominates**: the
Cannon has ~0 chance to kill *either* tough target, so favorability ties at the floor and B simply removes more
threat (Battleship att 12 > Mech.Inf att 6). The scalar invented a trade-off out of two hopeless attacks.

**compare-two-attacks — a real edge appears.**
`Cannon strikes [Riflemen def_eff=12 hp20] vs [Helicopter def_eff=9 hp20]`: OLD 0.67 vs 0.89 -> **B** (Helicopter
the softer div). NEW 0.10 vs 0.01 -> **incomparable**: both are long odds, and once the near-zero win chances tie
at the floor the threat-removed axis genuinely trades off — the raw ratio had over-committed to B.

**cf-vacate — the scalar was too lenient.**
`Dikrech: def with/without best 18.8/7.5, incoming(scalar)=4.50`: OLD said **yes** (7.5 >= 1.25x4.5, remaining
garrison "holds"), NEW says **no** — the scariest attacker's win probability against the def-7.5 second defender is
decisive. The scalar's 1.25x margin treated a >70%-loss position as safe.

**capture-state — high def_eff != safe.**
`Chambery: incoming(scalar)=7.5, defense=18.0`: OLD **held** (18 >= 1.25x7.5), NEW **capturable** — the scariest
attacker actually wins the HP race >=70% of the time despite the 18 vs 7.5 headline.

---

## Caveats / honest edges

- **Timing gates reachability, not odds.** The odds are computed at the scariest attacker's *full* effective
  power; the turns-faded `ThreatField` value still selects *which* attacker and still zeroes out anything past the
  ~6-turn horizon, but a 3-turns-away attacker now contributes its full win chance rather than a faded scalar.
  This is a deliberate, documented choice (a probability does not divide by turns cleanly) and it accounts for
  some of the more aggressive `capturable` / cf-vacate `no` verdicts above. A stricter "can it strike *this* turn"
  gate is a possible refinement, not done here.
- **Band vs. ratio.** The pass/fail kinds use a 0.7 / 0.3 win-prob band; the two-axis Pareto kinds keep the 1.25x
  ratio margin (now applied to a probability). Both preserve the near-tie-drop design. Generation stays healthy on
  T677 (cf-vacate keeps 12/13, triage 12, compare 12); on sparse T50 cf-vacate yields 0 (as it nearly did before —
  T50 has too few contested multi-garrison cities), which is a board-sparsity effect, not a gate failure.

## STOPPED (left for a follow-up, by design)

**t3-threat and adv-assault-target were NOT converted to odds.** Their incoming axis was to become
"win-prob the attacker takes the city's best defender". But that win probability is computed *against that city's
own defender* — so it becomes (the inverse of) the second axis (`def_eff`), collapsing the two-axis
withheld-weighting Pareto design into one axis and destroying the "no invented weighting" property those kinds
exist to test. Making it faithful needs a design decision (e.g. keep raw reachable force as the incoming axis and
add win-probability as a *third* axis, or redefine the frontier), not a mechanical swap. Per the task's guidance to
integrate the cleanest subset and stop rather than force instability, these two kinds keep the scalar
`ThreatField` incoming axis for now. Everything else (primitive, op, compare-two-attacks, cf-vacate,
triage-reinforce) is integrated and green.

## Gate status
`cargo build -p civ-cli --features remote` PASS · `cargo test --workspace` PASS (all green) ·
`cargo clippy --workspace --all-targets -- -D warnings` PASS (no `#[allow]` silencing) ·
`verify-oracle --board data/saves/myagent_T50.sav` PASS (618 trials, all correct).
