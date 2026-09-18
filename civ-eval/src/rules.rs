//! The combat / strength `rules` module — the T2/T3 valuation model (DESIGN.md §7, and
//! `T2-T3-design.md` §2).
//!
//! This module is the **single source** of the combat constants and formulas. Both the
//! ground-truth solvers (`question.rs`) and the prose `rules_block` that goes into the prompt
//! (`prompt.rs`) read from here, so the rules the model is scored against and the rules the
//! model is *told* can never drift (the discipline-#1 requirement).
//!
//! We adopt the `classic` ruleset's own *data tables* (per-unit attack/defense/hitpoints,
//! terrain defense bonuses, veteran multipliers) and feed them into a **simple stated scalar**
//! — a single product, not a simulated fight. We deliberately do NOT reimplement Freeciv's
//! combat *resolution* (no HP/firepower rounds, no ZoC, no win-chance).
//!
//! NOTE (integration TODO): the constants below are hard-coded from the `classic` ruleset for
//! now. Per `T2-T3-design.md` §8, a later pass should machine-extract them from the ruleset
//! shipped with the save's generator (`units.ruleset` / `terrain.ruleset` / `veteran_system`)
//! so `rules_block` and the solver share one extracted source. The values here are the ones the
//! design lists as verified from `classic`.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use civ_core::{Board, City, Unit};

/// The movement/combat class of a unit. Air/Sea are excluded from land-threat reachability in
/// T3 (a Battleship can't besiege an inland city); they still appear in T2 comparisons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitClass {
    Land,
    Sea,
    Air,
}

impl UnitClass {
    pub fn as_str(self) -> &'static str {
        match self {
            UnitClass::Land => "Land",
            UnitClass::Sea => "Sea",
            UnitClass::Air => "Air",
        }
    }
}

/// A unit's stat row (from `classic`'s `units.ruleset`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnitStat {
    pub attack: f64,
    pub defense: f64,
    pub hitpoints: f64,
    pub firepower: f64,
    pub move_rate: i32,
    pub class: UnitClass,
}

/// The named axis a strength comparison is over. Comparisons always specify which; there is no
/// invented composite "power".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrengthAxis {
    Attack,
    Defense,
}

impl StrengthAxis {
    /// Upper-case token used in question text ("ATTACK" / "DEFENSE").
    pub fn as_str(self) -> &'static str {
        match self {
            StrengthAxis::Attack => "ATTACK",
            StrengthAxis::Defense => "DEFENSE",
        }
    }

    /// Lower-case word, for ids.
    pub fn key(self) -> &'static str {
        match self {
            StrengthAxis::Attack => "attack",
            StrengthAxis::Defense => "defense",
        }
    }
}

/// `power_fact` for the four `classic` veteran levels: green ×1.0, veteran ×1.5, hardened
/// ×1.75, elite ×2.0 (applied to both attack and defense). Levels outside 0..=3 clamp to the
/// nearest end (defensive; real saves only carry 0..=3).
pub const VETERAN_POWER_FACT: [f64; 4] = [1.0, 1.5, 1.75, 2.0];

/// Human-facing names for the veteran levels (index = level).
pub const VETERAN_NAMES: [&str; 4] = ["green", "veteran", "hardened", "elite"];

pub fn power_fact(veteran: i32) -> f64 {
    VETERAN_POWER_FACT[veteran.clamp(0, 3) as usize]
}

/// Look up the stat row for a unit type by its `classic` name (Freeciv `type_by_name`).
/// Returns `None` for a type not in the table — the caller then skips it (so an unmodeled
/// modern type never produces a bogus comparison rather than a wrong one).
///
/// The 13 rows the design lists as *verified from `classic`* are marked; the remainder are the
/// well-known classic values, included so denser boards (T677) get broader coverage. All are
/// pending the machine-extraction TODO above.
pub fn unit_stat(kind: &str) -> Option<UnitStat> {
    use UnitClass::{Air, Land, Sea};
    // (attack, defense, hitpoints, firepower, move_rate, class)
    let s = |attack, defense, hitpoints, firepower, move_rate, class| UnitStat {
        attack,
        defense,
        hitpoints,
        firepower,
        move_rate,
        class,
    };
    let row = match kind {
        // --- verified-from-classic rows (T2-T3-design.md §2.2) ---
        "Warriors" => s(1.0, 1.0, 10.0, 1.0, 1, Land),
        "Phalanx" => s(1.0, 2.0, 10.0, 1.0, 1, Land),
        "Legion" => s(4.0, 2.0, 10.0, 1.0, 1, Land),
        "Chariot" => s(3.0, 1.0, 10.0, 1.0, 2, Land),
        "Musketeers" => s(3.0, 3.0, 20.0, 1.0, 1, Land),
        "Cannon" => s(8.0, 1.0, 20.0, 1.0, 1, Land),
        "Riflemen" => s(5.0, 4.0, 20.0, 1.0, 1, Land),
        "Cavalry" => s(8.0, 3.0, 20.0, 1.0, 2, Land),
        "Alpine Troops" => s(5.0, 5.0, 20.0, 1.0, 1, Land),
        "Armor" => s(10.0, 5.0, 30.0, 1.0, 3, Land),
        "Mech. Inf." => s(6.0, 6.0, 30.0, 1.0, 3, Land),
        "Fighter" => s(4.0, 3.0, 20.0, 2.0, 10, Air),
        "Battleship" => s(12.0, 12.0, 40.0, 2.0, 4, Sea),
        // --- additional common classic types (hard-coded; pending extraction) ---
        "Horsemen" => s(2.0, 1.0, 10.0, 1.0, 2, Land),
        "Archers" => s(3.0, 2.0, 10.0, 1.0, 1, Land),
        "Pikemen" => s(1.0, 2.0, 10.0, 1.0, 1, Land),
        "Catapult" => s(6.0, 1.0, 10.0, 1.0, 1, Land),
        "Knights" => s(4.0, 2.0, 10.0, 1.0, 2, Land),
        "Dragoons" => s(5.0, 2.0, 20.0, 1.0, 2, Land),
        "Artillery" => s(10.0, 1.0, 20.0, 2.0, 1, Land),
        "Howitzer" => s(12.0, 2.0, 30.0, 2.0, 2, Land),
        "Partisan" => s(4.0, 4.0, 20.0, 1.0, 1, Land),
        "Marines" => s(8.0, 5.0, 20.0, 1.0, 1, Land),
        "Paratroopers" => s(6.0, 4.0, 20.0, 1.0, 1, Land),
        "Settlers" => s(0.0, 1.0, 20.0, 1.0, 1, Land),
        "Migrants" => s(0.0, 1.0, 10.0, 1.0, 1, Land),
        "Engineers" => s(0.0, 2.0, 20.0, 1.0, 2, Land),
        "Workers" => s(0.0, 1.0, 10.0, 1.0, 1, Land),
        "Explorer" => s(0.0, 1.0, 10.0, 1.0, 1, Land),
        "Bomber" => s(12.0, 1.0, 20.0, 2.0, 8, Air),
        "Helicopter" => s(10.0, 3.0, 20.0, 2.0, 6, Air),
        "Frigate" => s(4.0, 2.0, 20.0, 1.0, 4, Sea),
        "Ironclad" => s(4.0, 4.0, 30.0, 1.0, 4, Sea),
        "Destroyer" => s(4.0, 4.0, 30.0, 1.0, 6, Sea),
        "Cruiser" => s(6.0, 6.0, 30.0, 2.0, 5, Sea),
        "AEGIS Cruiser" => s(8.0, 8.0, 30.0, 2.0, 5, Sea),
        "Submarine" => s(15.0, 2.0, 30.0, 2.0, 3, Sea),
        "Carrier" => s(1.0, 9.0, 40.0, 2.0, 5, Sea),
        "Transport" => s(0.0, 3.0, 30.0, 1.0, 5, Sea),
        _ => return None,
    };
    Some(row)
}

// --------------------------------------------------------------------------
// context-free strength (T2a)
// --------------------------------------------------------------------------

/// `health(u) = hp(u) / hitpoints[type(u)]` — ~1.0 in practice (damage heals between turns), so
/// this axis is inert on real snapshots but kept for correctness and exercised by a synthetic
/// damaged-unit mini-board test.
pub fn health(u: &Unit, stat: &UnitStat) -> f64 {
    if stat.hitpoints <= 0.0 {
        1.0
    } else {
        u.hp as f64 / stat.hitpoints
    }
}

/// `att_eff(u) = attack[type] × vet(u) × health(u)` — context-free. `None` if the type is
/// unmodeled.
pub fn att_eff(u: &Unit) -> Option<f64> {
    let s = unit_stat(&u.kind)?;
    Some(s.attack * power_fact(u.veteran) * health(u, &s))
}

/// `def_eff_base(u) = defense[type] × vet(u) × health(u)` — context-free (no terrain/fort).
pub fn def_eff_base(u: &Unit) -> Option<f64> {
    let s = unit_stat(&u.kind)?;
    Some(s.defense * power_fact(u.veteran) * health(u, &s))
}

/// The strength of `u` on `axis`, context-free (T2a). `None` if the type is unmodeled.
pub fn strength(u: &Unit, axis: StrengthAxis) -> Option<f64> {
    match axis {
        StrengthAxis::Attack => att_eff(u),
        StrengthAxis::Defense => def_eff_base(u),
    }
}

// --------------------------------------------------------------------------
// contextual defense (T2b / T3): terrain + fortification multipliers
// --------------------------------------------------------------------------

/// Additive terrain defense bonus (fraction): Forest/Jungle/Swamp +0.5, Hills +1.0,
/// Mountains +2.0, everything else 0.
pub fn terrain_defense_bonus(terrain: &str) -> f64 {
    match terrain {
        "Forest" | "Jungle" | "Swamp" => 0.5,
        "Hills" => 1.0,
        "Mountains" => 2.0,
        _ => 0.0,
    }
}

/// Additive bonus from tile extras: River +0.5, Fortress +1.0.
pub fn extras_defense_bonus(extras: &BTreeSet<String>) -> f64 {
    let mut b = 0.0;
    if extras.contains("River") {
        b += 0.5;
    }
    if extras.contains("Fortress") {
        b += 1.0;
    }
    b
}

/// City-center base fortification (+0.5 for any city tile).
pub const CITY_CENTER_BONUS: f64 = 0.5;

/// City Walls fortification bonus **vs land** (+1.0). Granted either by a city's own City Walls
/// improvement or, for all of an owner's cities at once, by the Great Wall wonder.
pub const CITY_WALLS_VS_LAND_BONUS: f64 = 1.0;

/// Coastal Defense bonus **vs sea** (+1.0). Decoded and exposed for completeness, but it does
/// **not** apply to the land-threat defense our T2b/T3 model scores, so it is intentionally never
/// added into [`defense_multiplier`] / [`city_defense`] (which are land-threat quantities).
pub const COASTAL_DEFENSE_VS_SEA_BONUS: f64 = 1.0;

/// The full contextual defense multiplier at a tile **vs a land threat**:
/// `1 + terrain + extras + (city-center) + (walls)`. `has_walls` folds in City Walls / Great
/// Wall (+1.0 vs land). Coastal Defense is *not* here — it is a vs-sea bonus only (see
/// [`COASTAL_DEFENSE_VS_SEA_BONUS`]).
pub fn defense_multiplier(
    terrain: &str,
    extras: &BTreeSet<String>,
    is_city_center: bool,
    has_walls: bool,
) -> f64 {
    let mut m = 1.0 + terrain_defense_bonus(terrain) + extras_defense_bonus(extras);
    if is_city_center {
        m += CITY_CENTER_BONUS;
    }
    if has_walls {
        m += CITY_WALLS_VS_LAND_BONUS;
    }
    m
}

/// `def_eff(u, tile) = def_eff_base(u) × defense_multiplier(tile)` for the tile `u` stands on.
/// `has_walls` applies the City Walls / Great Wall bonus (vs land). `None` if the type is
/// unmodeled.
pub fn def_eff_on_tile(
    u: &Unit,
    board: &Board,
    is_city_center: bool,
    has_walls: bool,
) -> Option<f64> {
    if !board.in_bounds(u.x, u.y) {
        return None;
    }
    let base = def_eff_base(u)?;
    let tile = board.tile(u.x, u.y);
    Some(base * defense_multiplier(&tile.terrain, &tile.extras, is_city_center, has_walls))
}

/// Whether `owner` has built the Great Wall — a wonder that grants City Walls to **every** one of
/// that player's cities (a per-player effect, not a per-city one).
pub fn owner_has_great_wall(board: &Board, owner: &str) -> bool {
    board
        .cities
        .iter()
        .any(|c| c.owner == owner && c.has("Great Wall"))
}

/// Whether `city` is walled **vs land**: it has its own City Walls, or its owner holds the Great
/// Wall (which walls all their cities).
pub fn city_is_walled(board: &Board, city: &City) -> bool {
    city.has_walls() || owner_has_great_wall(board, &city.owner)
}

/// The defensive strength of a city = `def_eff` of its **best defender** (a unit standing on the
/// city tile, owned by the city's owner), including terrain + city-center bonus **and City Walls
/// (+100% vs land)** when the city is walled (own walls, or the owner's Great Wall). An undefended
/// city (no eligible defender with a modeled type) scores `0.0`.
pub fn city_defense(board: &Board, city: &City) -> f64 {
    let walls = city_is_walled(board, city);
    board
        .units
        .iter()
        .filter(|u| u.x == city.x && u.y == city.y && u.owner == city.owner)
        .filter_map(|u| def_eff_on_tile(u, board, true, walls))
        .fold(0.0, f64::max)
}

/// The city's defense **with** its best defender (the current [`city_defense`]) and **without** it —
/// the counterfactual `cf-vacate` reads both. `without_best` is the `def_eff` of the best REMAINING
/// defender after the single strongest is pulled off the city tile for a turn (0.0 if none remains,
/// i.e. the city had only one garrison). Both use the same terrain + city-center + City Walls
/// context as [`city_defense`], so the "before" value equals [`city_defense`] exactly.
pub fn city_defense_vacated(board: &Board, city: &City) -> (f64, f64) {
    let walls = city_is_walled(board, city);
    let mut defs: Vec<f64> = board
        .units
        .iter()
        .filter(|u| u.x == city.x && u.y == city.y && u.owner == city.owner)
        .filter_map(|u| def_eff_on_tile(u, board, true, walls))
        .collect();
    // Descending: [0] = best defender, [1] = best remaining once the strongest is vacated.
    defs.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let with_best = defs.first().copied().unwrap_or(0.0);
    let without_best = defs.get(1).copied().unwrap_or(0.0);
    (with_best, without_best)
}

/// The def_eff a `defender` unit would have if it garrisoned `city` (best-defender candidate for
/// the `triage-reinforce` allocation): its context-free `def_eff_base` scaled by the CITY tile's
/// defense multiplier (terrain + city-center + City Walls). Independent of where the defender
/// currently stands — the reinforcement is an abstract allocation, not a modeled move. `None` if
/// the unit type is unmodeled.
pub fn def_eff_if_garrisoned(defender: &Unit, board: &Board, city: &City) -> Option<f64> {
    let base = def_eff_base(defender)?;
    let tile = board.tile(city.x, city.y);
    let walls = city_is_walled(board, city);
    Some(base * defense_multiplier(&tile.terrain, &tile.extras, true, walls))
}

/// The contextual `def_eff` of a unit on the tile it currently stands on — terrain + extras, plus
/// the city-center and City Walls bonuses when it sits in one of its OWN cities. Used to value an
/// attack's target (how hard it is to kill where it is). `None` if the type is unmodeled.
pub fn unit_def_on_own_tile(board: &Board, u: &Unit) -> Option<f64> {
    let own_city = board
        .cities
        .iter()
        .find(|c| c.owner == u.owner && c.x == u.x && c.y == u.y);
    let is_city_center = own_city.is_some();
    let has_walls = own_city.map(|c| city_is_walled(board, c)).unwrap_or(false);
    def_eff_on_tile(u, board, is_city_center, has_walls)
}

// --------------------------------------------------------------------------
// combat odds — Freeciv-classic win probability (the HP/firepower race)
// --------------------------------------------------------------------------

/// Win-probability band for the "decisive" capture judgments (`cf-vacate`, `triage-reinforce`): an
/// attack the attacker wins with probability ≥ [`WIN_PROB_HIGH`] is a decisive win (the city FALLS);
/// ≤ [`WIN_PROB_LOW`] a decisive hold (the city HOLDS); the band between is a near-tie, dropped
/// exactly as the T2 ratio-margin discipline drops near strength ratios. Tuned (like the 1.25×
/// margin) so instances still generate on the real boards.
pub const WIN_PROB_HIGH: f64 = 0.7;
pub const WIN_PROB_LOW: f64 = 0.3;

/// The probability the ATTACKER wins a Freeciv-`classic` combat, modelled as the HP/firepower race
/// (`T2-T3-design.md` §2, upgrading the stated scalar to true odds). Each round the attacker "hits"
/// (the defender loses the round) with probability `p = A / (A + D)`, where `A` / `D` are the
/// EFFECTIVE attack / defense powers taken from the existing modifier stack ([`att_eff`] /
/// [`def_eff_on_tile`] — veteran × health × terrain / fortification / walls already folded in). The
/// attacker must land `ceil(def_hp / att_fp)` hits to destroy the defender before the defender lands
/// `ceil(att_hp / def_fp)` hits to destroy the attacker; the exact probability of winning that race
/// is an O(n·m) DP over the two hit counts (n, m ≤ max_hitpoints ≈ 40, so this is negligible).
///
/// `att_hp` / `def_hp` are the units' CURRENT hitpoints and `att_fp` / `def_fp` their firepower —
/// both already in the `classic` [`UnitStat`] table, so no new constants are introduced. Degenerate
/// inputs: `A ≤ 0` → `0.0` (a unit that cannot hurt the defender never wins); `D ≤ 0` → `1.0` (an
/// undefended target is destroyed for certain, provided the attacker can land a hit).
///
/// NOTE on health double-count: `A` / `D` already fold in the `health = hp/max` factor, and this
/// race ALSO consumes current `hp`. On real snapshots `hp = max` (damage heals between turns) so
/// `health = 1` and there is no double-count; on a synthetic damaged unit it is a deliberate
/// modelling simplification, consistent with the existing stated scalar reusing the same stack.
pub fn win_probability(
    a_power: f64,
    att_hp: f64,
    att_fp: f64,
    d_power: f64,
    def_hp: f64,
    def_fp: f64,
) -> f64 {
    if a_power <= 0.0 {
        return 0.0;
    }
    if d_power <= 0.0 {
        return 1.0;
    }
    let p = a_power / (a_power + d_power);
    let q = 1.0 - p;
    // Hits each side must land to kill the other (≥1; firepower guarded to a positive divisor).
    let hits_to_kill = |hp: f64, fp: f64| -> usize {
        let fp = if fp > 0.0 { fp } else { 1.0 };
        (hp / fp).ceil().max(1.0) as usize
    };
    let n = hits_to_kill(def_hp, att_fp); // attacker hits needed to destroy the defender
    let m = hits_to_kill(att_hp, def_fp); // defender hits needed to destroy the attacker
                                          // DP over the race. `row[i]` = P(attacker wins | it needs `i` more hits, defender needs `j`
                                          // more), for the current `j`. Boundary j=0: attacker wins iff it needs 0 more hits.
                                          // Recurrence W(i,j) = p·W(i-1,j) + q·W(i,j-1); W(0,·) = 1, W(i>0, 0) = 0.
    let mut last = vec![0.0f64; n + 1];
    last[0] = 1.0;
    let mut cur = vec![0.0f64; n + 1];
    for _ in 1..=m {
        cur[0] = 1.0;
        for i in 1..=n {
            cur[i] = p * cur[i - 1] + q * last[i];
        }
        std::mem::swap(&mut last, &mut cur);
    }
    last[n]
}

/// The attacker's win probability striking `target` where the target currently stands — its own
/// tile's terrain plus the own-city center / City Walls bonuses ([`unit_def_on_own_tile`]). The odds
/// form of the exchange-favorability axis (`compare-two-attacks`). `None` if either type is
/// unmodeled or the target has no contextual defense (a degenerate 0-defense target).
pub fn attack_win_prob(board: &Board, attacker: &Unit, target: &Unit) -> Option<f64> {
    let a_power = att_eff(attacker)?;
    let astat = unit_stat(&attacker.kind)?;
    let d_power = unit_def_on_own_tile(board, target)?;
    if d_power <= 0.0 {
        return None;
    }
    let dstat = unit_stat(&target.kind)?;
    Some(win_probability(
        a_power,
        attacker.hp as f64,
        astat.firepower,
        d_power,
        target.hp as f64,
        dstat.firepower,
    ))
}

/// The best defender UNIT on a city's tile (the unit whose contextual `def_eff` IS [`city_defense`]),
/// paired with that `def_eff`. `None` if the city has no eligible (modeled) defender. Unlike the
/// scalar [`city_defense`] this keeps the unit itself, so its current HP + firepower feed the odds
/// model (which the scalar does not need).
pub fn best_defender<'a>(board: &'a Board, city: &City) -> Option<(&'a Unit, f64)> {
    defenders_ranked(board, city).into_iter().next()
}

/// All eligible defenders of `city` on its tile, each with its contextual `def_eff`, sorted best
/// first. `[0]` is the [`city_defense`] defender; `[1]` is the one that remains after the strongest
/// is vacated (the `cf-vacate` counterfactual garrison).
pub fn defenders_ranked<'a>(board: &'a Board, city: &City) -> Vec<(&'a Unit, f64)> {
    let walls = city_is_walled(board, city);
    let mut defs: Vec<(&Unit, f64)> = board
        .units
        .iter()
        .filter(|u| u.x == city.x && u.y == city.y && u.owner == city.owner)
        .filter_map(|u| def_eff_on_tile(u, board, true, walls).map(|d| (u, d)))
        .collect();
    defs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    defs
}

/// The probability the tile's scariest incoming attacker (as selected by [`ThreatField`]) BEATS
/// `defender`, i.e. the odds the city FALLS this turn. `defender` supplies its current HP + firepower
/// and `def_power` its contextual `def_eff`. `0.0` if no attacker reaches the tile.
pub fn city_fall_prob(field: &ThreatField, x: i32, y: i32, defender: &Unit, def_power: f64) -> f64 {
    let Some(dstat) = unit_stat(&defender.kind) else {
        return 0.0;
    };
    field.win_prob_at(x, y, def_power, defender.hp as f64, dstat.firepower)
}

/// The probability the tile's scariest incoming attacker takes an UNDEFENDED city (`def_power = 0`,
/// so the attacker wins for certain) — `1.0` when an attacker reaches the tile, else `0.0`.
pub fn city_fall_prob_undefended(field: &ThreatField, x: i32, y: i32) -> f64 {
    field.win_prob_at(x, y, 0.0, 0.0, 1.0)
}

/// THE probability `city` FALLS to its scariest incoming attacker this turn — the single "ease of
/// capture" scalar shared by every combat-capture surface: the `t3-threat` most-likely-to-fall axis,
/// the `adv-assault-target` capture axis, and the `city_fall_prob(x, y, player)` calculator op. It
/// composes [`best_defender`] (whose HP/firepower/contextual `def_eff` set the defence side) with the
/// precomputed [`ThreatField`] (whose scariest reachable attacker sets the attack side) via
/// [`city_fall_prob`]; an undefended city defers to [`city_fall_prob_undefended`] (certain fall if any
/// attacker reaches it). `field` MUST be built for `city.owner` (enemies of the owner are the
/// attackers). Because all three surfaces call THIS one function, the tool value is the scored value
/// verbatim (the parity gate).
pub fn city_capture_prob(board: &Board, city: &City, field: &ThreatField) -> f64 {
    match best_defender(board, city) {
        Some((u, def_power)) => city_fall_prob(field, city.x, city.y, u, def_power),
        None => city_fall_prob_undefended(field, city.x, city.y),
    }
}

/// The probability an ordered attacking `stack` captures `city`, resolving the `classic` assault as a
/// sequence of single combats: each attacker strikes the city's strongest REMAINING defender; a win
/// destroys that defender (the "fallen defender exposes the next" rule) and spends the attacker, a
/// loss destroys the attacker. The city is captured when every defender is gone before the stack is
/// exhausted (`reasoning-frontier-questions-v2.md` §v2.1). Defenders come from [`defenders_ranked`]
/// on the board passed. `stack` is pre-ordered by the caller; ordering changes the odds (which
/// attacker faces which defender). Thin wrapper over the [`assault_capture_prob_vs`] DP core.
///
/// NOTE: P7-fogged no longer scores against this REAL garrison (that made the yes/no depend on the
/// unobservable hidden defenders); it scores against a synthetic [`worst_case_garrison`] via
/// [`assault_capture_prob_vs`]. O(k·m) DP over (attacker index, defender depth); k, m are small.
pub fn assault_capture_prob(board: &Board, stack: &[&Unit], city: &City) -> f64 {
    assault_capture_prob_vs(stack, &defenders_ranked(board, city))
}

/// The sequential-storm DP core of [`assault_capture_prob`], taking an EXPLICIT defender list
/// (`(unit, contextual def power)` pairs, strongest first) rather than reading the city's real
/// garrison via [`defenders_ranked`]. Lets a caller score the assault against a SYNTHETIC garrison
/// — e.g. the worst-case single defender the P7-fogged solver assumes for a fog-hidden city (see
/// [`worst_case_garrison`]) — instead of the true, unobservable occupants. An empty `defenders`
/// list is an undefended city (certain capture); an empty `stack` never captures. With a SINGLE
/// defender the result is attacker-order-independent. O(k·m) DP over (attacker index, depth).
pub fn assault_capture_prob_vs(stack: &[&Unit], defenders: &[(&Unit, f64)]) -> f64 {
    let defs = defenders;
    let m = defs.len();
    if m == 0 {
        return 1.0; // undefended city: the assault walks in
    }
    let k = stack.len();
    if k == 0 {
        return 0.0; // no attackers
    }
    let win = |ai: usize, dj: usize| -> f64 {
        let a = stack[ai];
        let (d, d_power) = defs[dj];
        let (Some(astat), Some(dstat)) = (unit_stat(&a.kind), unit_stat(&d.kind)) else {
            return 0.0;
        };
        let Some(a_power) = att_eff(a) else {
            return 0.0;
        };
        win_probability(
            a_power,
            a.hp as f64,
            astat.firepower,
            d_power,
            d.hp as f64,
            dstat.firepower,
        )
    };
    // dp[i][j] = P(eventual capture | attacker i is about to strike defender j). A win advances both
    // (attacker spent, defender dead → dp[i+1][j+1]); a loss advances only the attacker (dp[i+1][j]).
    // Base: dp[k][j] = (j==m); dp[i][m] = 1. Rolled into one row, filled from the last attacker up.
    let mut next = vec![0.0f64; m + 1];
    next[m] = 1.0;
    for i in (0..k).rev() {
        let mut cur = vec![0.0f64; m + 1];
        cur[m] = 1.0;
        for j in 0..m {
            let w = win(i, j);
            cur[j] = w * next[j + 1] + (1.0 - w) * next[j];
        }
        next = cur;
    }
    next[0]
}

/// The WORST-CASE garrison a rational fog-reasoner must assume for `city`: a single fresh (green,
/// veteran 0, full-HP) unit of the city owner's STRONGEST fielded defender TYPE, standing on the
/// city centre. "Strongest fielded defender type" = the modeled COMBAT type (`attack > 0`, so
/// civilians like Settlers/Workers/Engineers/Explorer are excluded) that maximises [`def_eff_base`]
/// over every unit the owner has ANYWHERE on `board`. Returns the synthetic defender and its
/// contextual defence power at the city (terrain + city-center + walls). `None` when the owner
/// fields no modeled combat unit at all → the city is worst-case UNDEFENDED (a certain capture).
///
/// OBSERVED-REPERTOIRE CONTRACT: this is a function of OBSERVABLES — the owner's *visible* repertoire
/// — **only when `board` is the perspective's MASKED board** (enemy units on fogged tiles are masked
/// out, so the scan sees exactly what the player can). The P7-fogged solver MUST call it on the masked
/// board (`analysis/surprise-strike-redesign.md`, extended to fogged-assault): passing the unmasked
/// board would let the worst case be a unit TYPE that exists only on fogged tiles — a type the model
/// has no evidence exists — reintroducing the aleatoric channel this design removes. All the tile
/// context it reads (city terrain/extras, walls via [`city_is_walled`]) is last-known-retained on the
/// masked board, so the fogged city's defensive multiplier is the one the model can see.
pub fn worst_case_garrison(board: &Board, city: &City) -> Option<(Unit, f64)> {
    let best_kind = board
        .units
        .iter()
        .filter(|u| u.owner == city.owner)
        .filter(|u| unit_stat(&u.kind).is_some_and(|s| s.attack > 0.0))
        .filter_map(|u| def_eff_base(u).map(|d| (d, u.kind.clone())))
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(_, kind)| kind)?;
    let stat = unit_stat(&best_kind)?;
    let synthetic = Unit {
        x: city.x,
        y: city.y,
        id: -1,
        kind: best_kind,
        owner: city.owner.clone(),
        veteran: 0,
        hp: stat.hitpoints as i32,
    };
    let walls = city_is_walled(board, city);
    let tile = board.tile(city.x, city.y);
    let mult = defense_multiplier(&tile.terrain, &tile.extras, true, walls);
    let def_power = def_eff_base(&synthetic)? * mult;
    Some((synthetic, def_power))
}

// --------------------------------------------------------------------------
// T3 constraint-site: spatial-predicate lookups (the (I) predicate-composition kind)
// --------------------------------------------------------------------------

/// Chebyshev radius within which a candidate site must find open water to satisfy the "near water"
/// predicate — coastal siting (`maximal-calculator-plan.md` §(c)/§(e)).
pub const NEAR_WATER_RADIUS: i32 = 2;

/// Whether a terrain name is open water (Ocean / Deep Ocean / Lake). `"Unknown"` (fog) is NOT water
/// — you cannot certify water you have never seen — so it is excluded from BOTH this and
/// [`crate::question::is_land`], keeping the fog boundary honest.
pub fn is_water(terrain: &str) -> bool {
    matches!(terrain, "Ocean" | "Deep Ocean" | "Lake")
}

/// Predicate — the tile at `p` is DEFENSIBLE: its terrain grants a positive terrain defense bonus
/// (Forest / Jungle / Swamp / Hills / Mountains), the same [`terrain_defense_bonus`] signal the
/// T2b/T3 defense model already uses. Open, flat terrain (Grassland / Plains / Desert / Tundra) is
/// not defensible. Reuses the existing defensive-terrain definition — no new convention.
pub fn is_defensible(board: &Board, p: (i32, i32)) -> bool {
    board.in_bounds(p.0, p.1) && terrain_defense_bonus(&board.tile(p.0, p.1).terrain) > 0.0
}

/// Predicate — some tile within [`NEAR_WATER_RADIUS`] (Chebyshev) of `p`, including `p` itself, is
/// open water (the coastal-access predicate).
pub fn is_within_water_radius(board: &Board, p: (i32, i32)) -> bool {
    civ_core::geometry::tiles_within(board, p, NEAR_WATER_RADIUS)
        .into_iter()
        .any(|(x, y)| is_water(&board.tile(x, y).terrain))
}

/// Predicate — the tile at `p` is NOT in enemy territory for `player`: it is unclaimed (`owner`
/// `None`) or claimed by `player` themselves. A tile owned by any OTHER player is enemy territory.
pub fn not_enemy_territory(board: &Board, player: &str, p: (i32, i32)) -> bool {
    match &board.tile(p.0, p.1).owner {
        None => true,
        Some(o) => o == player,
    }
}

/// The constraint-site conjunction: a candidate site is ACCEPTABLE for `player` iff it satisfies
/// ALL THREE predicates — defensible ∧ within-2-of-water ∧ not-enemy-territory. This intersection
/// (and the certificate that no candidate meets it, i.e. the "none" answer) is the withheld
/// judgment of the (I) kind: each individual predicate is a cheap lookup, but no single measurement
/// returns "the tile meeting every predicate."
pub fn constraint_site_ok(board: &Board, player: &str, p: (i32, i32)) -> bool {
    is_defensible(board, p)
        && is_within_water_radius(board, p)
        && not_enemy_territory(board, player, p)
}

/// Minimum Chebyshev spacing a NEW city site must keep from EVERY existing city — the Freeciv
/// `citymindist` rule (classic default 3). Deliberately the ONE constraint-site requirement NOT
/// surfaced by the `site_check` tool: to enforce it the model must read the city coordinates off
/// the board and compute the distance itself, so a single `site_check` call cannot settle a
/// candidate (the tool computes 3/4 of the truth, and the missing quarter is this geometry).
pub const CITY_MIN_SPACING: i32 = 3;

/// Predicate — the tile at `p` is at least `dist` tiles (Chebyshev) from EVERY existing city, i.e.
/// it respects the minimum city-spacing rule. A tile on, or within `dist - 1` of, any city fails.
/// A pure function of `board.cities` — Oracle-trivial and deterministic.
pub fn is_spaced_from_cities(board: &Board, p: (i32, i32), dist: i32) -> bool {
    board
        .cities
        .iter()
        .all(|c| civ_core::geometry::chebyshev((c.x, c.y), p) >= dist)
}

/// The full constraint-site acceptability test for the region-search redesign: the three cheap,
/// tool-visible predicates ([`constraint_site_ok`]) AND the non-bundled spacing constraint
/// ([`is_spaced_from_cities`]). This is the withheld conjunction — no single `site_check` call
/// returns it, because `site_check` never reports spacing.
pub fn constraint_site_ok_spaced(board: &Board, player: &str, p: (i32, i32), dist: i32) -> bool {
    constraint_site_ok(board, player, p) && is_spaced_from_cities(board, p, dist)
}

// --------------------------------------------------------------------------
// T3 settle-site valuation: graded threat field + working-radius desirability axes
// --------------------------------------------------------------------------

/// City working radius (Chebyshev), matching `T2-T3-design.md` §4.2.
pub const WORK_RADIUS: i32 = 2;

/// Threat horizon in turns: an enemy that cannot bring an attack to bear within this many turns
/// contributes no threat (the graded field fades to 0 past it). `T2-T3-design.md` §4.2 (H ≈ 6).
pub const THREAT_HORIZON: i32 = 6;

/// Numerical slack when comparing the real-valued safety axis in the dominance test.
const SAFETY_EPS: f64 = 1e-6;

/// Terrain feeding the FOOD / PRODUCTION axes.
const FOOD_TERRAIN: &[&str] = &["Grassland", "Plains"];
const PROD_TERRAIN: &[&str] = &["Hills", "Forest", "Mountains"];
/// Resource extras feeding FOOD / PRODUCTION; the RESOURCES axis counts any TRADE_RESOURCE.
const FOOD_RESOURCES: &[&str] = &[
    "Wheat", "Oasis", "Fish", "Game", "Fruit", "Buffalo", "Pheasant", "Whales",
];
const PROD_RESOURCES: &[&str] = &["Iron", "Coal", "Resources"];
const TRADE_RESOURCES: &[&str] = &[
    "Gold",
    "Iron",
    "Game",
    "Furs",
    "Coal",
    "Fish",
    "Fruit",
    "Gems",
    "Buffalo",
    "Wheat",
    "Oasis",
    "Peat",
    "Pheasant",
    "Resources",
    "Ivory",
    "Silk",
    "Spice",
    "Whales",
    "Wine",
    "Oil",
];

/// The four desirability axes of a settle site over its working radius. Food/production/resources
/// are integer tallies; safety is the negative of the graded threat field. The *weighting* across
/// axes is deliberately withheld — only Pareto dominance is scored (`T2-T3-design.md` §4.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SiteAxes {
    pub food: i64,
    pub production: i64,
    pub resources: i64,
    pub safety: f64,
}

impl SiteAxes {
    fn terrain_ge(&self, o: &SiteAxes) -> bool {
        self.food >= o.food && self.production >= o.production && self.resources >= o.resources
    }
    fn terrain_gt_some(&self, o: &SiteAxes) -> bool {
        self.food > o.food || self.production > o.production || self.resources > o.resources
    }
    /// Pareto dominance over the TERRAIN-only sub-space — used to detect a *judgment-loaded* trap
    /// (a site sunk only once safety is weighed, not a mere terrain-counting mistake).
    pub fn dominates_terrain(&self, o: &SiteAxes) -> bool {
        self.terrain_ge(o) && self.terrain_gt_some(o)
    }
    /// Full 4-axis Pareto dominance: `>=` on every axis, strictly `>` on at least one.
    pub fn dominates(&self, o: &SiteAxes) -> bool {
        let ge = self.terrain_ge(o) && self.safety >= o.safety - SAFETY_EPS;
        let gt = self.terrain_gt_some(o) || self.safety > o.safety + SAFETY_EPS;
        ge && gt
    }
}

/// A precomputed graded land-threat field for one player: `at(x, y)` is the strength of the
/// scariest single enemy LAND unit able to attack `(x, y)`, faded by how many turns it needs
/// (`att_eff / reach_turns`, ceil, min 1 turn), and 0 past [`THREAT_HORIZON`]. Built once per
/// (board, player): each enemy unit is BFS-expanded once over land, bounded by its reach.
pub struct ThreatField {
    w: i32,
    h: i32,
    grid: Vec<f64>,
    /// Per tile, the combat stats of the attacker that SET the scariest value in `grid` (the unit
    /// maximising `att_eff / reach_turns`). Kept so the odds model can ask that attacker's win
    /// probability against a defender; the scalar `grid` alone cannot. `None` where no attacker
    /// reaches the tile.
    attacker: Vec<Option<AttackerStat>>,
}

/// The combat-relevant stats of the scariest incoming attacker at a tile (its EFFECTIVE attack
/// power [`att_eff`], current HP, and firepower) — the attacker side of [`win_probability`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttackerStat {
    pub power: f64,
    pub hp: f64,
    pub firepower: f64,
}

impl ThreatField {
    // ZOC DECISION (2026-08-08). This field models the movement of INCOMING ENEMY units toward a
    // tile, so a ZOC-aware variant would have `player`'s OWN units/cities exert control that slows
    // the attacker. That is deliberately NOT modeled here, for three reasons: (1) it is ill-defined —
    // the field is a per-enemy BFS max-reduction faded by turns, not the [`ReachField`] Dijkstra, so
    // a defender's-ZOC block on an attacker changes reach non-monotonically and destabilises the
    // margin-based decision kinds; (2) the same "ZOC / staging occupancy NOT modeled" simplification
    // is already made by forward-posting's enemy-reply reach (see [`PostingField::compute`]), so the
    // two incoming-enemy models stay consistent; (3) leaving `compute`/`compute_where` unchanged
    // keeps EVERY consumer — the `threat` tool op, settle-site safety, t3-retreat safety,
    // surprise-strike, adv-assault-target, `city_fall_prob`/`city_capture_prob`, and t3-threat — in
    // strict MUTUAL PARITY by construction: they all read this one field, so none can diverge from
    // the tool. Owner-attributed OUTGOING movement (reach_turns / travel_turns* / retreat / the
    // nearest-owned & reachable-nearest solvers) IS ZOC-aware via [`ReachField::from_origins_tactical`].
    pub fn compute(board: &Board, player: &str) -> ThreatField {
        Self::compute_where(board, player, |_| true)
    }

    /// Like [`compute`](Self::compute) but only enemy units for which `keep(u)` is true contribute
    /// threat. The hidden-force family (P1/P3) builds the field on the UNMASKED board with
    /// `keep = |u| masked.is_fogged(u.x, u.y)`, so the field reflects only attackers the perspective
    /// CANNOT currently see — a surprise-strike / massed-force quantity that depends on masked-out
    /// occupants (`reasoning-frontier-questions-v2.md` §v2.1).
    pub fn compute_where(
        board: &Board,
        player: &str,
        mut keep: impl FnMut(&Unit) -> bool,
    ) -> ThreatField {
        let w = board.width;
        let h = board.height;
        let mut grid = vec![0.0f64; (w * h) as usize];
        let mut attacker: Vec<Option<AttackerStat>> = vec![None; (w * h) as usize];
        let idx = |x: i32, y: i32| (y * w + x) as usize;
        for u in &board.units {
            if u.owner == player {
                continue;
            }
            if !keep(u) {
                continue;
            }
            let Some(stat) = unit_stat(&u.kind) else {
                continue;
            };
            if stat.class != UnitClass::Land {
                continue;
            }
            let Some(att) = att_eff(u) else { continue };
            if att <= 0.0 || !board.in_bounds(u.x, u.y) {
                continue;
            }
            let ustat = AttackerStat {
                power: att,
                hp: u.hp as f64,
                firepower: stat.firepower,
            };
            let mv = stat.move_rate.max(1);
            let max_steps = THREAT_HORIZON * mv;
            // Bounded BFS over land from the unit; a staging tile reached in `d` steps lets the
            // unit attack any adjacent site in ceil(d / mv) turns (min 1).
            let mut dist = vec![-1i32; (w * h) as usize];
            let mut q = std::collections::VecDeque::new();
            dist[idx(u.x, u.y)] = 0;
            q.push_back((u.x, u.y));
            while let Some((x, y)) = q.pop_front() {
                let d = dist[idx(x, y)];
                let turns = ((d as f64) / (mv as f64)).ceil().max(1.0);
                if turns <= THREAT_HORIZON as f64 {
                    let v = att / turns;
                    for (_, (nx, ny)) in civ_core::geometry::neighbors(board, x, y) {
                        let gi = idx(nx, ny);
                        if v > grid[gi] {
                            grid[gi] = v;
                            attacker[gi] = Some(ustat);
                        }
                    }
                }
                if d >= max_steps {
                    continue;
                }
                for (_, (nx, ny)) in civ_core::geometry::neighbors(board, x, y) {
                    if !crate::question::is_land(&board.tile(nx, ny).terrain) {
                        continue;
                    }
                    let ni = idx(nx, ny);
                    if dist[ni] == -1 {
                        dist[ni] = d + 1;
                        q.push_back((nx, ny));
                    }
                }
            }
        }
        ThreatField {
            w,
            h,
            grid,
            attacker,
        }
    }

    /// Threat at `(x, y)` (0 if out of bounds).
    pub fn at(&self, x: i32, y: i32) -> f64 {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            0.0
        } else {
            self.grid[(y * self.w + x) as usize]
        }
    }

    /// The combat stats of the scariest incoming attacker at `(x, y)`, or `None` if none reaches it
    /// (or it is out of bounds).
    pub fn attacker_at(&self, x: i32, y: i32) -> Option<AttackerStat> {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            None
        } else {
            self.attacker[(y * self.w + x) as usize]
        }
    }

    /// The probability that the scariest incoming attacker at `(x, y)` wins against a defender with
    /// contextual defense power `d_power`, current HP `d_hp`, and firepower `d_fp` — the odds the
    /// tile FALLS. `0.0` if no attacker reaches the tile. Selection of WHICH attacker is unchanged
    /// (the turns-faded [`att_eff`] max the field already tracks); the odds are then computed at that
    /// attacker's full effective power (timing gates reachability via the horizon, not the odds).
    pub fn win_prob_at(&self, x: i32, y: i32, d_power: f64, d_hp: f64, d_fp: f64) -> f64 {
        match self.attacker_at(x, y) {
            None => 0.0,
            Some(a) => win_probability(a.power, a.hp, a.firepower, d_power, d_hp, d_fp),
        }
    }
}

/// Chebyshev radius within which a fogged tile could plausibly hide an enemy able to launch a
/// surprise strike (P1). A candidate with NO fogged tile inside this radius is a **provably-safe
/// decoy**: it cannot be the surprise-strike answer, and a reader can prove that WITHOUT seeing the
/// fog (the luck-free credit floor, `reasoning-frontier-questions-v2.md` §v2.1 P1). Stated in the
/// SURPRISE-STRIKE RULES so the reasoning is grounded, not guessed.
pub const SURPRISE_REACH: i32 = 5;

/// Whether any tile within Chebyshev `radius` of `center` is FOGGED on the masked board — i.e. could
/// hide an unseen enemy. `false` certifies a P1 provably-safe decoy (no hidden striker can be in
/// range). Reads the masked board's visibility, so it asks exactly what the model can see.
pub fn any_fogged_within(masked: &Board, center: (i32, i32), radius: i32) -> bool {
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let (x, y) = (center.0 + dx, center.1 + dy);
            if masked.in_bounds(x, y) && masked.is_fogged(x, y) {
                return true;
            }
        }
    }
    false
}

/// The DEDUCIBLE surprise-strike exposure of `center` (one of `player`'s cities) — a WORST-CASE,
/// structural upper bound on the force the FOG could admit against it, computed as a **pure function of
/// the MASKED board** (what `player` can see), NOT of the true hidden enemy placement. This is the P1
/// redesign (`analysis/surprise-strike-redesign.md`): the old quantity read the enemy unit actually
/// standing in the fog on the *unmasked* board, which made the answer depend on information the model
/// is denied (luck). Here nothing hidden is read — moving a masked-out striker to any other fogged tile
/// cannot change the result.
///
/// Construction: build the graded land-[`ThreatField`] from the VISIBLE enemies on `masked`
/// ([`ThreatField::compute`] selects the scariest reachable enemy LAND [`att_eff`], faded by reach
/// turns and horizoned at [`THREAT_HORIZON`]; fogged land tiles keep their real terrain on a masked
/// board, so they are pathable). Then take the max over in-range FOGGED tiles of the field value there,
/// weighted by closeness to the city — see [`surprise_exposure_with`]. Convenience wrapper that builds
/// the field for one call; prefer [`surprise_exposure_with`] with a shared field when scoring several
/// cities.
pub fn surprise_exposure(masked: &Board, player: &str, center: (i32, i32), radius: i32) -> f64 {
    let field = ThreatField::compute(masked, player);
    surprise_exposure_with(masked, &field, center, radius)
}

/// [`surprise_exposure`] against a prebuilt masked-board [`ThreatField`] (built for `player`):
///
/// ```text
/// exposure(center) = max over tiles T with is_fogged(T) and chebyshev(T, center) <= radius
///                        of   field.at(T) * closeness(T, center)
/// closeness(T, center) = 1 / max(chebyshev(T, center), 1)   (a nearer hiding spot is scarier)
/// ```
///
/// A fogged tile contributes only when a VISIBLE enemy could operate at it (`field.at(T) > 0`) — i.e.
/// some enemy unit the player can SEE could move into that fog and bring an attack to bear within the
/// horizon. Both ingredients ("which fog is enemy-reachable" and "how strong is the scariest enemy that
/// reaches it") are read from observables, so a perfect reasoner with only the masked board can compute
/// this. `0.0` when no in-range fogged tile is enemy-reachable — in particular the provably-safe decoy
/// (no in-range fogged tile at all, [`any_fogged_within`] false) scores 0, keeping the luck-free floor
/// SOUND by construction.
pub fn surprise_exposure_with(
    masked: &Board,
    field: &ThreatField,
    center: (i32, i32),
    radius: i32,
) -> f64 {
    let mut best = 0.0f64;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let (x, y) = (center.0 + dx, center.1 + dy);
            if !masked.in_bounds(x, y) || !masked.is_fogged(x, y) {
                continue;
            }
            let threat = field.at(x, y);
            if threat <= 0.0 {
                continue;
            }
            let cheb = civ_core::geometry::chebyshev((x, y), center).max(1);
            let weighted = threat / cheb as f64;
            if weighted > best {
                best = weighted;
            }
        }
    }
    best
}

/// Sum of the EFFECTIVE attack power ([`att_eff`]) of every enemy (non-`player`) LAND unit standing
/// on a FOGGED tile within Chebyshev `radius` of `center`, on the UNMASKED board — the "massed hidden
/// force" in a region (P3, `reasoning-frontier-questions-v2.md` §v2.1 P3). Depends entirely on
/// masked-out occupants: a region the perspective can fully see contributes 0.
pub fn hidden_force_in_region(
    unmasked: &Board,
    masked: &Board,
    player: &str,
    center: (i32, i32),
    radius: i32,
) -> f64 {
    unmasked
        .units
        .iter()
        .filter(|u| u.owner != player)
        .filter(|u| civ_core::geometry::chebyshev((u.x, u.y), center) <= radius)
        .filter(|u| masked.is_fogged(u.x, u.y))
        .filter(|u| {
            unit_stat(&u.kind)
                .map(|s| s.class == UnitClass::Land)
                .unwrap_or(false)
        })
        .filter_map(att_eff)
        .sum()
}

/// Convenience: threat at a single tile (builds a one-off field — prefer [`ThreatField::compute`]
/// when scoring several tiles for the same player).
pub fn threat_at(board: &Board, player: &str, target: (i32, i32)) -> f64 {
    ThreatField::compute(board, player).at(target.0, target.1)
}

/// The four settle-site axes of `target` for `player`, tallied over the Chebyshev-[`WORK_RADIUS`]
/// working radius, using a precomputed [`ThreatField`] for safety.
pub fn site_axes_with(board: &Board, target: (i32, i32), field: &ThreatField) -> SiteAxes {
    let mut food = 0i64;
    let mut production = 0i64;
    let mut resources = 0i64;
    for (x, y) in civ_core::geometry::tiles_within(board, target, WORK_RADIUS) {
        let t = board.tile(x, y);
        if FOOD_TERRAIN.contains(&t.terrain.as_str()) {
            food += 1;
        }
        if PROD_TERRAIN.contains(&t.terrain.as_str()) {
            production += 1;
        }
        for e in &t.extras {
            let e = e.as_str();
            if FOOD_RESOURCES.contains(&e) || e == "River" || e == "Irrigation" {
                food += 1;
            }
            if PROD_RESOURCES.contains(&e) || e == "Mine" {
                production += 1;
            }
            if TRADE_RESOURCES.contains(&e) {
                resources += 1;
            }
        }
    }
    SiteAxes {
        food,
        production,
        resources,
        safety: -field.at(target.0, target.1),
    }
}

/// Convenience: the settle-site axes of `target` (builds a one-off threat field).
pub fn site_axes(board: &Board, player: &str, target: (i32, i32)) -> SiteAxes {
    let field = ThreatField::compute(board, player);
    site_axes_with(board, target, &field)
}

// --------------------------------------------------------------------------
// T3 retreat valuation: cover + safety + support of a candidate retreat tile
// --------------------------------------------------------------------------

/// Radius (Chebyshev) over which friendly backup is tallied for the SUPPORT axis of a retreat
/// tile — nearby own cities and other own units that could reinforce or counterattack.
pub const SUPPORT_RADIUS: i32 = 2;

/// The three desirability axes of a *retreat destination* for a threatened unit
/// (`T2-T3-design.md` §4.3: "mobility × cover × distance-from-threat"). All axes are
/// higher-is-better:
/// - **cover** — the tile's defensive multiplier (terrain + fortification + city-center + walls);
///   the same contextual bonus a defender there would enjoy.
/// - **safety** — the negative of the graded threat field (distance-from-threat): a destination an
///   enemy can reach sooner, or with a stronger unit, is less safe.
/// - **support** — friendly backup within [`SUPPORT_RADIUS`] (own cities + other own units), an
///   integer tally.
///
/// As with [`SiteAxes`], the *weighting* across axes is withheld — only Pareto dominance is scored
/// (`T2-T3-design.md` §4.1). Reachability of the destination is handled as an admissibility filter
/// at generation (only tiles the unit can actually retreat to are offered), not as a scored axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RetreatAxes {
    pub cover: f64,
    pub safety: f64,
    pub support: i64,
}

impl RetreatAxes {
    fn static_ge(&self, o: &RetreatAxes) -> bool {
        self.cover >= o.cover - SAFETY_EPS && self.support >= o.support
    }
    fn static_gt_some(&self, o: &RetreatAxes) -> bool {
        self.cover > o.cover + SAFETY_EPS || self.support > o.support
    }
    /// Pareto dominance over the NON-SAFETY (cover + support) sub-space — used to detect a
    /// *judgment-loaded* trap (a retreat sunk only once safety is weighed, not a cover/support
    /// counting mistake), mirroring [`SiteAxes::dominates_terrain`].
    pub fn dominates_static(&self, o: &RetreatAxes) -> bool {
        self.static_ge(o) && self.static_gt_some(o)
    }
    /// Full 3-axis Pareto dominance: `>=` on every axis, strictly `>` on at least one.
    pub fn dominates(&self, o: &RetreatAxes) -> bool {
        let ge = self.static_ge(o) && self.safety >= o.safety - SAFETY_EPS;
        let gt = self.static_gt_some(o) || self.safety > o.safety + SAFETY_EPS;
        ge && gt
    }
}

/// The three retreat axes of destination `dest` for `owner`'s threatened unit (`exclude_unit` — the
/// retreating unit itself, so it never counts as its own backup), using a precomputed
/// [`ThreatField`] for safety. Cover folds in the own-city center/walls bonus when `dest` holds one
/// of `owner`'s cities.
pub fn retreat_axes_with(
    board: &Board,
    dest: (i32, i32),
    owner: &str,
    exclude_unit: i32,
    field: &ThreatField,
) -> RetreatAxes {
    let (x, y) = dest;
    RetreatAxes {
        cover: retreat_cover(board, owner, dest),
        safety: -field.at(x, y),
        support: retreat_support(board, owner, dest, Some(exclude_unit)),
    }
}

/// The COVER axis of retreat destination `dest` for `owner`: the tile's [`defense_multiplier`]
/// (terrain + extras, plus the own-city center/walls bonus when `dest` holds one of `owner`'s
/// cities). Factored out of [`retreat_axes_with`] so the `t3-retreat` solver and the `tile_cover`
/// calculator op share ONE implementation.
pub fn retreat_cover(board: &Board, owner: &str, dest: (i32, i32)) -> f64 {
    let (x, y) = dest;
    let tile = board.tile(x, y);
    let own_city = board
        .cities
        .iter()
        .find(|c| c.owner == owner && c.x == x && c.y == y);
    let is_city_center = own_city.is_some();
    let has_walls = own_city.map(|c| city_is_walled(board, c)).unwrap_or(false);
    defense_multiplier(&tile.terrain, &tile.extras, is_city_center, has_walls)
}

/// The SUPPORT axis of retreat destination `dest` for `owner`: the count of `owner`'s cities and
/// units within [`SUPPORT_RADIUS`] (Chebyshev) of `dest`. `exclude_unit` drops one unit id from the
/// tally — the retreating unit, so it never counts as its own backup; pass `None` to count all of
/// `owner`'s units. Factored out of [`retreat_axes_with`] so the `t3-retreat` solver and the
/// `support` calculator op share ONE implementation.
pub fn retreat_support(
    board: &Board,
    owner: &str,
    dest: (i32, i32),
    exclude_unit: Option<i32>,
) -> i64 {
    let mut support = 0i64;
    for (sx, sy) in civ_core::geometry::tiles_within(board, dest, SUPPORT_RADIUS) {
        support += board
            .cities
            .iter()
            .filter(|c| c.owner == owner && c.x == sx && c.y == sy)
            .count() as i64;
        support += board
            .units
            .iter()
            .filter(|u| u.owner == owner && Some(u.id) != exclude_unit && u.x == sx && u.y == sy)
            .count() as i64;
    }
    support
}

// --------------------------------------------------------------------------
// T3 forward-posting valuation (P4): pressure created vs. survival under the enemy's 1-ply
// reposition-to-punish reply (`reasoning-frontier-questions-v2.md` §v2.1 P4).
// --------------------------------------------------------------------------

/// How far (turns) an enemy may REPOSITION before attacking your posted unit — the 1-ply "enemy moves
/// TO exploit the posting" reply. Exactly one turn: the enemy's immediate response. This is the
/// novelty the static [`ThreatField`] (threat from where enemies stand NOW) structurally under-reads —
/// an enemy that would STEP toward your posting to hit it.
pub const POST_ENEMY_REPLY_TURNS: i32 = 1;

/// Chebyshev reach of an attack FROM the posted tile: a unit attacks an adjacent tile, so an enemy
/// unit or city within this many tiles of the posting is "in your attack reach" from it.
pub const POST_ATTACK_REACH: i32 = 1;

/// The two withheld-weighting axes of a forward POSTING (advancing your unit to `dest`), both
/// higher-is-better. The weighting between aggression and survival is deliberately withheld — only
/// Pareto dominance is scored (as with [`SiteAxes`] / [`RetreatAxes`] / [`AssaultAxes`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PostingAxes {
    /// PRESSURE created (↑ = a more aggressive, more valuable post): the sum of the ATTACK strength
    /// ([`att_eff`]) of every enemy unit and the ease-of-capture ([`city_capture_prob`]) of every
    /// enemy city that the posting brings within an attack step (Chebyshev [`POST_ATTACK_REACH`]) of
    /// `dest`.
    pub pressure: f64,
    /// SURVIVAL of the posted unit (↑ = safer): `1 − max` over the enemy's REACHABLE
    /// [`POST_ENEMY_REPLY_TURNS`]-turn repositions of their [`win_probability`] attacking your unit at
    /// `dest`. `1.0` when no enemy can reposition to bring an attack to bear on the posting.
    pub survival: f64,
}

impl PostingAxes {
    /// `self` **decisively dominates** `o` as a posting: Pareto-`≥` on BOTH axes, clearing a decisive
    /// margin on at least one and at worst tying the other. PRESSURE (a continuous strength-like sum)
    /// uses the MULTIPLICATIVE `pressure_margin` (like [`AttackAxes`]); SURVIVAL (a probability) uses
    /// the ADDITIVE `survival_margin` (like the `t3-threat` / [`AssaultAxes`] probability axis). A
    /// sub-margin lead on either axis is a near-tie, so the whole dominance is non-decisive (keeping
    /// the answer un-litigable, matching the project's decisive-margin discipline). A posting is a
    /// blunder iff SOME other candidate decisively dominates it this way.
    pub fn decisively_dominates(
        &self,
        o: &PostingAxes,
        pressure_margin: f64,
        survival_margin: f64,
    ) -> bool {
        // Pareto-≥ on both axes first.
        if self.pressure < o.pressure - SAFETY_EPS || self.survival < o.survival - SAFETY_EPS {
            return false;
        }
        // pressure ↑ (continuous): tie within eps, else a lead must clear the multiplicative margin.
        let p_tie = (self.pressure - o.pressure).abs() <= SAFETY_EPS;
        let p_lead = !p_tie && self.pressure >= pressure_margin * o.pressure;
        // survival ↑ (probability): tie within eps, else a lead must clear the additive margin.
        let s_tie = (self.survival - o.survival).abs() <= SAFETY_EPS;
        let s_lead = !s_tie && self.survival >= o.survival + survival_margin;
        (p_tie || p_lead) && (s_tie || s_lead) && (p_lead || s_lead)
    }
}

/// One enemy LAND unit's reposition reply: its attack stats plus its
/// [`POST_ENEMY_REPLY_TURNS`]-turn reposition reach — the per-enemy state the survival axis searches.
struct EnemyReply {
    stat: AttackerStat,
    reach: ReachField,
}

/// Precomputed forward-posting state for one player `me`: every enemy LAND unit's attack power +
/// 1-turn reposition reach (the SURVIVAL side) and every enemy unit's/city's pressure value (the
/// PRESSURE side). Built once per (board, player); [`PostingField::axes`] then scores any posting tile
/// — the SINGLE shared computation the solver AND the generator both call, so scored truth and the
/// generator's admissibility can never drift.
pub struct PostingField {
    /// Enemy LAND units as `(x, y, att_eff)` — the pressure value of each scalp brought into reach.
    enemy_units: Vec<(i32, i32, f64)>,
    /// Enemy cities as `(x, y, city_capture_prob)` — the pressure value of each soft city in reach.
    enemy_cities: Vec<(i32, i32, f64)>,
    /// Enemy LAND units' reposition replies (the survival side).
    replies: Vec<EnemyReply>,
}

impl PostingField {
    pub fn compute(board: &Board, me: &str) -> PostingField {
        let mut enemy_units = Vec::new();
        let mut replies = Vec::new();
        for u in &board.units {
            if u.owner == me {
                continue;
            }
            let Some(stat) = unit_stat(&u.kind) else {
                continue;
            };
            if stat.class != UnitClass::Land {
                continue; // air/sea cannot besiege / punish a land posting (as in ThreatField)
            }
            let Some(att) = att_eff(u) else { continue };
            if att <= 0.0 || !board.in_bounds(u.x, u.y) {
                continue;
            }
            enemy_units.push((u.x, u.y, att));
            let reach = ReachField::from_origins(
                board,
                &[(u.x, u.y)],
                stat.move_rate,
                POST_ENEMY_REPLY_TURNS,
            );
            replies.push(EnemyReply {
                stat: AttackerStat {
                    power: att,
                    hp: u.hp as f64,
                    firepower: stat.firepower,
                },
                reach,
            });
        }
        // Enemy cities' capture probability (one ThreatField per enemy owner, cached).
        let mut enemy_cities = Vec::new();
        let mut field_by_owner: std::collections::HashMap<String, ThreatField> =
            std::collections::HashMap::new();
        for c in &board.cities {
            if c.owner == me {
                continue;
            }
            let field = field_by_owner
                .entry(c.owner.clone())
                .or_insert_with(|| ThreatField::compute(board, &c.owner));
            enemy_cities.push((c.x, c.y, city_capture_prob(board, c, field)));
        }
        PostingField {
            enemy_units,
            enemy_cities,
            replies,
        }
    }

    /// The two posting axes of advancing `unit` to `dest`. `None` if the unit's type is unmodeled.
    pub fn axes(&self, board: &Board, unit: &Unit, dest: (i32, i32)) -> Option<PostingAxes> {
        let stat = unit_stat(&unit.kind)?;
        let def_base = def_eff_base(unit)?;
        // The posted unit's contextual defense at `dest` — its terrain/extras cover, plus the own-city
        // center/walls bonus when `dest` is one of the mover's own cities (the same multiplier a
        // retreat destination uses).
        let def_power = def_base * retreat_cover(board, &unit.owner, dest);
        // PRESSURE: enemy scalps + soft cities within one attack step (Chebyshev POST_ATTACK_REACH).
        let mut pressure = 0.0;
        for &(ex, ey, att) in &self.enemy_units {
            if civ_core::geometry::chebyshev(dest, (ex, ey)) <= POST_ATTACK_REACH {
                pressure += att;
            }
        }
        for &(cx, cy, cap) in &self.enemy_cities {
            if civ_core::geometry::chebyshev(dest, (cx, cy)) <= POST_ATTACK_REACH {
                pressure += cap;
            }
        }
        // SURVIVAL: 1 − the worst enemy reply. An enemy can strike `dest` if it can reposition (within
        // POST_ENEMY_REPLY_TURNS) onto ANY tile adjacent to `dest` and attack from there (Zones of
        // Control and staging-tile occupancy are NOT modeled — the same simplification the ThreatField
        // staging model makes).
        let mut worst = 0.0f64;
        for r in &self.replies {
            if self.can_reach_adjacent(board, r, dest) {
                let p = win_probability(
                    r.stat.power,
                    r.stat.hp,
                    r.stat.firepower,
                    def_power,
                    unit.hp as f64,
                    stat.firepower,
                );
                worst = worst.max(p);
            }
        }
        Some(PostingAxes {
            pressure,
            survival: 1.0 - worst,
        })
    }

    /// Whether enemy `r` can reposition (within [`POST_ENEMY_REPLY_TURNS`]) onto some tile ADJACENT to
    /// `dest` — the staging tile from which it attacks the posting. Its own current tile counts (a
    /// 0-turn "reposition") when already adjacent, since a ReachField labels its origin 0 turns.
    fn can_reach_adjacent(&self, board: &Board, r: &EnemyReply, dest: (i32, i32)) -> bool {
        civ_core::geometry::neighbors(board, dest.0, dest.1)
            .into_iter()
            .any(|(_, (nx, ny))| r.reach.turns_at(nx, ny).is_some())
    }
}

// --------------------------------------------------------------------------
// T3 assault-target valuation: ease-of-capture vs. city value (two orthogonal axes)
// --------------------------------------------------------------------------

/// The two attractiveness axes of one of a player's OWN cities, read from the ATTACKER's point of view
/// (`maximal-calculator-corpus.md` §3 `adv-assault-target`). Both are higher-is-better and genuinely
/// orthogonal: `capture` is how EASILY the city falls, `value` is how much the prize is worth. The
/// *weighting* between "easy" and "valuable" is withheld — only Pareto dominance is scored — so a
/// small soft city and a big tough city are a real trade-off (both on the frontier), not one obviously
/// better target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AssaultAxes {
    /// CAPTURE-WIN PROBABILITY (↑ = a softer, more attractive target): the probability the city falls
    /// to its scariest incoming attacker — [`city_capture_prob`] verbatim (the SAME odds the
    /// `t3-threat` solver and the `city_fall_prob` op use).
    pub capture: f64,
    /// CITY VALUE (↑ = a richer prize): the city's `size`. Size ONLY — it is the single value signal
    /// both carried on [`City`] and printed to the model — no Palace / improvements / production /
    /// coastal terms enter here.
    pub value: f64,
}

impl AssaultAxes {
    /// `self` **decisively dominates** `o` as an assault target: Pareto-`≥` on BOTH axes, clearing the
    /// decisive margin on at least one and at worst tying the other. The capture axis (a probability)
    /// uses the ADDITIVE `capture_margin` (a lead must exceed the tie by ≥ margin), matching the
    /// probability-space discipline of the `t3-threat` winner; the value axis (an exact integer size)
    /// uses a strict comparison with no margin, exactly as the integer terrain axes of [`SiteAxes`] do.
    /// A sub-margin capture lead, or an equal size, counts as a tie on that axis; at least one axis
    /// must be a genuine decisive lead (else the two are equal, neither dominates). A city is a blunder
    /// pick iff SOME other candidate decisively dominates it this way.
    pub fn decisively_dominates(&self, o: &AssaultAxes, capture_margin: f64) -> bool {
        // Pareto-≥ on both axes first.
        if self.capture < o.capture - SAFETY_EPS || self.value < o.value - SAFETY_EPS {
            return false;
        }
        // capture ↑ (probability): tie within eps, else a lead must clear the additive margin.
        let cap_tie = (self.capture - o.capture).abs() <= SAFETY_EPS;
        let cap_lead = !cap_tie && self.capture >= o.capture + capture_margin;
        let cap_ok = cap_tie || cap_lead;
        // value ↑ (exact integer size): tie iff equal, else any strict lead is decisive (no margin).
        let val_tie = (self.value - o.value).abs() <= SAFETY_EPS;
        let val_lead = !val_tie && self.value > o.value + SAFETY_EPS;
        let val_ok = val_tie || val_lead;
        cap_ok && val_ok && (cap_lead || val_lead)
    }
}

/// The two assault-target axes of `city` (owned by `player`), using a precomputed [`ThreatField`] for
/// the capture axis. `capture` is [`city_capture_prob`] (the odds the city falls to its scariest
/// incoming attacker); `value` is the city's `size`.
pub fn assault_axes(board: &Board, city: &City, field: &ThreatField) -> AssaultAxes {
    AssaultAxes {
        capture: city_capture_prob(board, city, field),
        value: city.size as f64,
    }
}

// --------------------------------------------------------------------------
// comparative attack valuation: exchange favorability vs. threat removed (two axes)
// --------------------------------------------------------------------------

/// The two axes of a *candidate attack* (a friendly attacker striking an enemy target), both
/// higher-is-better (`maximal-calculator-corpus.md` §1 "best-attack-target survives only as 2 axes:
/// exchange favorability × threat-removed"). A *scalar* compare (just favorability, or just target
/// strength) would collapse to a free measurement; keeping two withheld-weighting axes is what
/// makes the comparison a genuine judgment — and routes a real trade-off to "incomparable".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttackAxes {
    /// EXCHANGE FAVORABILITY (↑): the attacker's WIN PROBABILITY striking the target where it stands
    /// ([`attack_win_prob`] — the HP/firepower race odds, not a raw strength ratio). Higher = a more
    /// certain kill. A high strength ratio that is actually a coin-flip because the defender has far
    /// more HP now reads as ≈0.5 here, which the raw `att_eff/def_eff` ratio hid.
    pub favorability: f64,
    /// THREAT REMOVED (↑): the target's own `att_eff` — how much enemy attacking power the kill
    /// deletes from the board. Higher = a more valuable scalp.
    pub threat_removed: f64,
}

impl AttackAxes {
    /// `self` **decisively dominates** `o` as an attack: Pareto-`≥` on BOTH axes (favorability and
    /// threat-removed), clearing the 1.25× margin on at least one axis and at worst tying the other
    /// (a sub-margin lead on an axis is a near-tie, so the whole dominance is non-decisive). Mirrors
    /// [`AssaultAxes::decisively_dominates`] on two higher-is-better axes (both use a multiplicative
    /// margin here, where both axes are continuous strengths).
    pub fn decisively_dominates(&self, o: &AttackAxes, margin: f64) -> bool {
        if self.favorability < o.favorability - SAFETY_EPS
            || self.threat_removed < o.threat_removed - SAFETY_EPS
        {
            return false; // not Pareto-≥ on both axes
        }
        let fav_tie = (self.favorability - o.favorability).abs() <= SAFETY_EPS;
        let fav_lead = !fav_tie && self.favorability >= margin * o.favorability;
        let thr_tie = (self.threat_removed - o.threat_removed).abs() <= SAFETY_EPS;
        let thr_lead = !thr_tie && self.threat_removed >= margin * o.threat_removed;
        (fav_tie || fav_lead) && (thr_tie || thr_lead) && (fav_lead || thr_lead)
    }

    /// The two attacks form a **genuine trade-off** ("incomparable"): each decisively leads the
    /// other on a DIFFERENT axis (`self` wins favorability by ≥ margin while `o` wins threat-removed
    /// by ≥ margin, or vice-versa). This is the answer that tests refusal to invent a weighting —
    /// distinct from a mere near-tie (both axes within margin), which is dropped as non-decisive.
    pub fn genuine_tradeoff(&self, o: &AttackAxes, margin: f64) -> bool {
        let a_fav = self.favorability >= margin * o.favorability;
        let b_fav = o.favorability >= margin * self.favorability;
        let a_thr = self.threat_removed >= margin * o.threat_removed;
        let b_thr = o.threat_removed >= margin * self.threat_removed;
        (a_fav && b_thr) || (b_fav && a_thr)
    }
}

/// The two attack axes of striking `target` with `attacker`, using each unit's committed strength
/// model. Favorability is the attacker's WIN PROBABILITY against the target on its own tile
/// ([`attack_win_prob`] — the HP/firepower odds); threat-removed is the target's own `att_eff`.
/// `None` if either type is unmodeled or the target has no defense (a degenerate 0-defense target).
pub fn attack_axes(board: &Board, attacker: &Unit, target: &Unit) -> Option<AttackAxes> {
    let favorability = attack_win_prob(board, attacker, target)?;
    let threat_removed = att_eff(target)?;
    Some(AttackAxes {
        favorability,
        threat_removed,
    })
}

// --------------------------------------------------------------------------
// reachable space — classic-faithful land travel cost (the real-play scope bound, DESIGN.md §6)
// --------------------------------------------------------------------------

/// A full move in `classic` is `SINGLE_MOVE` movement *fragments*; per-terrain and road/rail costs
/// are all expressed in these fragments so the model can charge a road a *fraction* of a move.
/// (`classic`'s `game.ruleset`: `SINGLE_MOVE = 3`.)
pub const SINGLE_MOVE: i32 = 3;

/// Cost (fragments) of moving between two ROAD tiles — `classic` `terrain.ruleset road_move_cost`
/// = 1 fragment, i.e. 1/3 of a full move. A River acts as a road for the along-river case.
pub const ROAD_MOVE_COST: i32 = 1;

/// Per-terrain base move cost in **full moves** from `classic` `terrain.ruleset movement_cost`
/// (multiplied by [`SINGLE_MOVE`] to get fragments). Ocean/impassable are handled by the land
/// filter, not here; any unrecognised land terrain defaults to 1 (a plain).
fn terrain_move_moves(terrain: &str) -> i32 {
    match terrain {
        // 1 full move: open ground.
        "Grassland" | "Plains" | "Desert" | "Tundra" => 1,
        // 2 full moves: rough/vegetated ground (classic Glacier is also 2).
        "Forest" | "Hills" | "Jungle" | "Swamp" | "Glacier" => 2,
        // 3 full moves: mountains.
        "Mountains" => 3,
        _ => 1,
    }
}

/// The `classic` move cost (in fragments) of stepping from tile `a` to land tile `b`, honouring
/// infrastructure on BOTH endpoints:
/// - **Railroad↔Railroad** = 0 (free; a rail-connected component costs ~no movement).
/// - **Road↔Road**, or **River↔River** (moving *along* a river counts as a road) = [`ROAD_MOVE_COST`].
/// - otherwise the destination terrain's base cost (`terrain_move_moves(b) × SINGLE_MOVE`).
///
/// The cheapest applicable rule wins (rail < road < terrain), matching classic pathing.
fn edge_move_cost(a: &civ_core::Tile, b: &civ_core::Tile) -> i32 {
    if a.has("Railroad") && b.has("Railroad") {
        0
    } else if (a.has("Road") && b.has("Road")) || (a.has("River") && b.has("River")) {
        ROAD_MOVE_COST
    } else {
        terrain_move_moves(&b.terrain) * SINGLE_MOVE
    }
}

/// Classic-faithful land travel cost (in turns) from a set of origin tiles, via a multi-source
/// **Dijkstra** over `classic` move costs (ocean impassable for land units), for a unit with
/// `move_rate` full moves/turn, bounded at `horizon` turns. Per-terrain base costs, road/river
/// discounts and free railroads are all charged in movement *fragments* ([`edge_move_cost`]).
///
/// TURN ACCOUNTING. A unit refreshes `move_rate × SINGLE_MOVE` fragments each turn. The label of a
/// tile is the *clock time* `E` (in fragments) at which it is first reached; `turns_at = ⌈E / B⌉`
/// where `B` is the per-turn budget (origins are reached at `E = 0` → 0 turns). Crossing a tile
/// whose cost exceeds the fragments left this turn is still allowed — the classic rule that a unit
/// with *any* moves left can always advance one tile — but it consumes the rest of the turn (the
/// excess is wasted and the clock rounds up to the next turn boundary). Because that transition is
/// monotone in `E`, minimising `E` (Dijkstra; rail's 0-cost edges are non-negative) minimises turns.
/// `turns_at` is the fewest turns to reach a tile, or `None` if unreachable within the horizon.
pub struct ReachField {
    w: i32,
    h: i32,
    turns: Vec<i32>, // fewest turns to reach each tile; -1 = unreachable within the horizon
}

impl ReachField {
    pub fn from_origins(
        board: &Board,
        origins: &[(i32, i32)],
        move_rate: i32,
        horizon: i32,
    ) -> ReachField {
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;

        let w = board.width;
        let h = board.height;
        let mv = move_rate.max(1);
        let budget = (mv * SINGLE_MOVE) as i64; // fragments refreshed each turn
        let max_clock = horizon.max(0) as i64 * budget; // clock cap for `horizon` turns
        let idx = |x: i32, y: i32| (y * w + x) as usize;

        let mut best = vec![i64::MAX; (w * h) as usize]; // min clock-time `E` to each tile
        let mut turns = vec![-1i32; (w * h) as usize];
        let mut heap: BinaryHeap<Reverse<(i64, i32, i32)>> = BinaryHeap::new();
        for &(ox, oy) in origins {
            if board.in_bounds(ox, oy) && best[idx(ox, oy)] != 0 {
                best[idx(ox, oy)] = 0;
                heap.push(Reverse((0, ox, oy)));
            }
        }
        while let Some(Reverse((e, x, y))) = heap.pop() {
            let i = idx(x, y);
            if e > best[i] {
                continue; // stale heap entry
            }
            if e > max_clock {
                continue; // beyond the horizon; do not expand
            }
            // First (smallest-`E`) settle of this tile: record its turn count.
            if turns[i] < 0 {
                turns[i] = ((e + budget - 1) / budget) as i32; // ⌈E / budget⌉
            }
            let here = board.tile(x, y);
            for (_, (nx, ny)) in civ_core::geometry::neighbors(board, x, y) {
                let nb = board.tile(nx, ny);
                if !crate::question::is_land(&nb.terrain) {
                    continue; // ocean impassable for land units
                }
                let c = edge_move_cost(here, nb) as i64;
                // Fragments left in the current turn (always > 0, so a move is always possible).
                let remaining = budget - (e % budget);
                // Enough this turn → charge `c`; else the always-advance rule burns the rest of
                // the turn (excess wasted, clock rounds to the next turn boundary).
                let ne = if c <= remaining { e + c } else { e + remaining };
                if ne > max_clock {
                    continue;
                }
                let j = idx(nx, ny);
                if ne < best[j] {
                    best[j] = ne;
                    heap.push(Reverse((ne, nx, ny)));
                }
            }
        }
        ReachField { w, h, turns }
    }

    /// Enemy-aware, classic-faithful TACTICAL reachability for `mover_owner`: the [`from_origins`]
    /// movement model (fragments / road / rail discounts, ocean impassable) PLUS the rules that make a
    /// tile a LEGAL advance rather than merely geometrically close. On top of ocean it BLOCKS ENTERING
    /// (a) a tile occupied by an enemy unit and (b) an enemy-owned city, and it applies classic ZONES
    /// OF CONTROL: enemy land MILITARY units (Land class with `attack > 0` — civilians like Settlers /
    /// Workers / Engineers / Explorer / Caravan / Diplomat exert NO ZOC) project ZOC onto their 8
    /// neighbouring tiles, and a step from `A` to `B` is forbidden when BOTH `A` and `B` lie in some
    /// enemy ZOC, UNLESS `B` is a friendly (`mover_owner`) city or holds a friendly unit. This is
    /// the DEFAULT for every owner-attributed movement path — forward-posting, `reach_turns`,
    /// `travel_turns_from_owned`, `travel_turns` (with a player), t3-retreat, and the nearest-owned /
    /// reachable-nearest solvers — so every offered advance is one the unit could actually make.
    /// [`from_origins`] stays enemy-blind only for genuinely ownerless point-to-point geography (the
    /// `travel_turns` fallback) and for INCOMING-enemy models (`ThreatField`, forward-posting's enemy
    /// reply) where a defender's own ZOC on the attacker is deliberately not modeled.
    pub fn from_origins_tactical(
        board: &Board,
        origins: &[(i32, i32)],
        move_rate: i32,
        horizon: i32,
        mover_owner: &str,
    ) -> ReachField {
        use std::cmp::Reverse;
        use std::collections::{BinaryHeap, HashSet};

        let w = board.width;
        let h = board.height;
        let mv = move_rate.max(1);
        let budget = (mv * SINGLE_MOVE) as i64;
        let max_clock = horizon.max(0) as i64 * budget;
        let idx = |x: i32, y: i32| (y * w + x) as usize;

        // Precompute the enemy-occupancy, friendly-unit and enemy-ZOC tile sets in one pass over units.
        let mut enemy_occ: HashSet<(i32, i32)> = HashSet::new();
        let mut friendly_unit: HashSet<(i32, i32)> = HashSet::new();
        let mut zoc: HashSet<(i32, i32)> = HashSet::new();
        for u in &board.units {
            if u.owner == mover_owner {
                friendly_unit.insert((u.x, u.y));
                continue;
            }
            enemy_occ.insert((u.x, u.y));
            // Only enemy LAND MILITARY units (attack > 0) exert a zone of control.
            if unit_stat(&u.kind).is_some_and(|s| s.class == UnitClass::Land && s.attack > 0.0) {
                for (_, (nx, ny)) in civ_core::geometry::neighbors(board, u.x, u.y) {
                    zoc.insert((nx, ny));
                }
            }
        }
        // Enemy cities block entry; friendly cities are a ZOC exception (may always be entered).
        let mut enemy_city: HashSet<(i32, i32)> = HashSet::new();
        let mut friendly_city: HashSet<(i32, i32)> = HashSet::new();
        for c in &board.cities {
            if c.owner == mover_owner {
                friendly_city.insert((c.x, c.y));
            } else {
                enemy_city.insert((c.x, c.y));
            }
        }

        let mut best = vec![i64::MAX; (w * h) as usize];
        let mut turns = vec![-1i32; (w * h) as usize];
        let mut heap: BinaryHeap<Reverse<(i64, i32, i32)>> = BinaryHeap::new();
        for &(ox, oy) in origins {
            if board.in_bounds(ox, oy) && best[idx(ox, oy)] != 0 {
                best[idx(ox, oy)] = 0;
                heap.push(Reverse((0, ox, oy)));
            }
        }
        while let Some(Reverse((e, x, y))) = heap.pop() {
            let i = idx(x, y);
            if e > best[i] {
                continue; // stale heap entry
            }
            if e > max_clock {
                continue; // beyond the horizon; do not expand
            }
            if turns[i] < 0 {
                turns[i] = ((e + budget - 1) / budget) as i32; // ⌈E / budget⌉
            }
            let here = board.tile(x, y);
            let a_in_zoc = zoc.contains(&(x, y));
            for (_, (nx, ny)) in civ_core::geometry::neighbors(board, x, y) {
                let nb = board.tile(nx, ny);
                if !crate::question::is_land(&nb.terrain) {
                    continue; // ocean impassable for land units
                }
                // Entry blocks: you cannot advance ONTO an enemy unit or an enemy city (that is an
                // assault / siege, not a posting).
                if enemy_occ.contains(&(nx, ny)) || enemy_city.contains(&(nx, ny)) {
                    continue;
                }
                // Classic ZOC: a step between two enemy-controlled tiles is forbidden, unless the
                // destination is a friendly city or already holds a friendly unit.
                if a_in_zoc
                    && zoc.contains(&(nx, ny))
                    && !friendly_city.contains(&(nx, ny))
                    && !friendly_unit.contains(&(nx, ny))
                {
                    continue;
                }
                let c = edge_move_cost(here, nb) as i64;
                let remaining = budget - (e % budget);
                let ne = if c <= remaining { e + c } else { e + remaining };
                if ne > max_clock {
                    continue;
                }
                let j = idx(nx, ny);
                if ne < best[j] {
                    best[j] = ne;
                    heap.push(Reverse((ne, nx, ny)));
                }
            }
        }
        ReachField { w, h, turns }
    }

    /// Fewest turns to reach `(x, y)`, or `None` if unreachable within the horizon.
    pub fn turns_at(&self, x: i32, y: i32) -> Option<i32> {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return None;
        }
        let t = self.turns[(y * self.w + x) as usize];
        if t >= 0 {
            Some(t)
        } else {
            None
        }
    }
}

// --------------------------------------------------------------------------
// rules_block prose (fed into the prompt for T2/T3; None for T0/T1)
// --------------------------------------------------------------------------

/// The distinct, modeled unit types present on the board, sorted — the only rows emitted into
/// `rules_block` (paying tokens only for types the reader can actually encounter).
fn present_unit_types(board: &Board) -> Vec<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for u in &board.units {
        if unit_stat(&u.kind).is_some() {
            set.insert(u.kind.clone());
        }
    }
    set.into_iter().collect()
}

/// Render the combat/strength rules block for `board` (DESIGN.md §9 `Prompt.rules_block`). This
/// is the same model the solver computes against — formulas, the board-present unit-stat rows,
/// and the veteran/terrain/fortification tables. Part of the cacheable prefix, so its tokens are
/// paid once per (board × encoding).
pub fn rules_block(board: &Board) -> String {
    let mut s = String::new();
    s.push_str(
        "COMBAT / STRENGTH RULES (a simple stated model — NOT a full battle simulation).\n\
         Strength is a single product, computed as follows:\n\
         - health(u)  = current_hp / max_hitpoints[type]     (a fresh unit is 1.0)\n\
         - vet(u)     = veteran multiplier (see table)\n\
         - ATTACK strength  att_eff(u)  = attack[type]  x vet(u) x health(u)\n\
         - DEFENSE strength def_eff(u)  = defense[type] x vet(u) x health(u)\n\
         On a tile, defense is further multiplied by (1 + terrain_bonus + fortification_bonus):\n\
         - def_eff(u, tile) = def_eff(u) x (1 + terrain_bonus + fortification_bonus)\n\
         A city's defense is the def_eff of its single best defender on the city tile (including\n\
         that tile's terrain, the city-center bonus, and City Walls if the city has them); an\n\
         undefended city has defense 0.\n\
         City Walls add +100% to defense vs a land attacker. The Great Wall wonder grants City\n\
         Walls to EVERY city its owner controls. Coastal Defense adds +100% vs a SEA attacker\n\
         only and does not affect defense against land threats.\n\
         Air and Sea units cannot besiege an inland city (excluded from land threat).\n\n",
    );

    s.push_str("Veteran multiplier (applies to both attack and defense):\n");
    for (i, name) in VETERAN_NAMES.iter().enumerate() {
        let _ = writeln!(s, "  {name:<9} x{:.2}", VETERAN_POWER_FACT[i]);
    }
    s.push('\n');

    s.push_str(
        "Terrain defense bonus (additive): Forest/Jungle/Swamp +50%, Hills +100%, \
         Mountains +200%, all others +0%.\n\
         Fortification bonus (additive): Fortress +100%, city center +50%, City Walls +100% \
         (vs land).\n\n",
    );

    s.push_str(
        "COMBAT ODDS (win probability of an attack). When a unit with ATTACK strength A strikes a \
         defender with DEFENSE strength D (A and D are the att_eff / def_eff above, with all \
         veteran/terrain/fortification/wall bonuses folded in), the fight is a series of rounds. \
         Each round the attacker wins with probability p = A / (A + D). The attacker must win \
         ceil(defender_hp / attacker_firepower) rounds to destroy the defender BEFORE the defender \
         wins ceil(attacker_hp / defender_firepower) rounds to destroy the attacker (hitpoints and \
         firepower are in the unit table below; a fresh unit is at full hitpoints). The ATTACKER'S \
         WIN PROBABILITY is the chance it wins that race. A lopsided strength ratio is NOT the same \
         as a certain win: a defender with far more hitpoints can make a 2:1 strength edge only a \
         coin-flip. Equal A and D with equal hitpoints/firepower is a 50/50 fight.\n\n",
    );

    let types = present_unit_types(board);
    if types.is_empty() {
        s.push_str("Unit stats: (no modeled units on this board).\n");
    } else {
        s.push_str(
            "Unit stats (attack / defense / hitpoints / firepower / class) for units on this \
             board:\n",
        );
        for t in &types {
            // Safe: present_unit_types only keeps modeled types.
            let st = unit_stat(t).unwrap();
            let _ = writeln!(
                s,
                "  {:<14} {:>2} / {:>2} / {:>2} / {:>2}  ({})",
                t,
                st.attack as i64,
                st.defense as i64,
                st.hitpoints as i64,
                st.firepower as i64,
                st.class.as_str()
            );
        }
    }

    s.push_str(
        "\nSETTLE-SITE RULES (for judging a city site). A candidate site is evaluated over its \
         WORKING RADIUS — every tile within 2 tiles (Chebyshev) of the site — on four axes:\n\
         - FOOD: Grassland/Plains tiles, food resources (Wheat/Oasis/Fish/Game/Fruit/...), and rivers.\n\
         - PRODUCTION: Hills/Forest/Mountains tiles, mines, production resources (Iron/Coal/Resources).\n\
         - RESOURCES: how many special/trade resources lie in the radius.\n\
         - SAFETY: freedom from nearby enemy military threat. An enemy LAND unit that can bring an \
           attack to bear sooner, or that is stronger, makes a site less safe; units that cannot \
           reach within a few turns — and all air/sea units (which cannot besiege an inland site) — \
           do not count.\n\
         A site is an UNSOUND choice ONLY IF another candidate is at least as good on ALL FOUR axes \
         and strictly better on at least one (it is 'dominated'). Every candidate not dominated by \
         another is a SOUND choice. There is NO fixed weighting between the axes — do not assume any \
         one axis outweighs another.\n",
    );

    s.push_str(
        "\nRETREAT RULES (for judging where a threatened unit should retreat to). Every offered \
         candidate tile is already reachable by the unit; judge each destination on three axes:\n\
         - COVER: how defensible the tile is — its defense multiplier (1 + terrain_bonus + \
           fortification_bonus, plus the city-center and City Walls bonuses if it is one of your \
           own cities). Higher is better.\n\
         - SAFETY: freedom from enemy military threat AT THE DESTINATION, measured the same way as \
           for a settle site — an enemy LAND unit that can bring an attack to bear on the tile \
           sooner, or that is stronger, makes it less safe; air/sea units do not count.\n\
         - SUPPORT: friendly backup nearby — how many of your own cities and other own units lie \
           within 2 tiles (Chebyshev) of the tile. Higher is better.\n\
         A retreat tile is an UNSOUND choice ONLY IF another candidate is at least as good on ALL \
         THREE axes and strictly better on at least one (it is 'dominated'). Every candidate not \
         dominated by another is a SOUND retreat. There is NO fixed weighting between the axes.\n",
    );

    s.push_str(
        "\nPOSTING RULES (for judging a forward ADVANCE that pressures the enemy). You will move one \
         of your units forward to a candidate tile; ASSUME the enemy then REPOSITIONS to punish it. \
         Every offered tile is a legal advance. Judge each posting on TWO axes, both \
         higher-is-better, with NO fixed weighting between them:\n\
         - PRESSURE created: how much you threaten from the posted tile — the sum of the ATTACK \
           strength (att_eff) of every enemy unit AND the ease of capture (fall probability, the \
           COMBAT ODDS above) of every enemy city that lies within one tile (an attack step) of the \
           posting. Higher = a more aggressive, more valuable post.\n\
         - SURVIVAL: your odds of living through the enemy's reply. Consider every enemy LAND unit \
           that can REPOSITION within one turn onto a tile next to your posting and attack it there; \
           SURVIVAL is 1 minus the highest such attacker's WIN PROBABILITY against your unit on the \
           posted tile (its terrain / fortification cover applies). If no enemy can reach and strike \
           the posting, survival is 1. This counts the enemy MOVING to exploit your posting — an \
           enemy that must step toward you to hit it still counts, not only enemies already adjacent.\n\
         A posting is an UNSOUND choice ONLY IF another candidate is at least as good on BOTH axes \
         and clearly better on at least one (it 'dominates' it — creating at least as much pressure \
         AND at least as survivable, with a real lead on one). Every candidate not dominated this way \
         is a SOUND advance. Zones of control are NOT modeled: an enemy's repositioning is not \
         blocked by adjacency to your units.\n",
    );

    s.push_str(
        "\nCITY-THREAT RULES (for judging which of your OWN cities is MOST LIKELY TO FALL this turn). \
         Each candidate city is scored on a SINGLE quantity — the PROBABILITY it falls to its \
         scariest incoming attacker, using the COMBAT ODDS above:\n\
         - The scariest incoming attacker is the enemy LAND unit with the largest ATTACK strength \
           divided by the turns it needs to arrive (a unit attacking this turn counts its full \
           att_eff; one three turns away counts a third; units that cannot arrive within a few \
           turns — and all air/sea units, which cannot besiege an inland city — do not count).\n\
         - The defender is the city's best defender on the city tile (its def_eff including terrain, \
           the city-center bonus, and City Walls); an undefended city falls for certain if any \
           attacker reaches it.\n\
         - The city's FALL PROBABILITY is that attacker's WIN PROBABILITY against that defender.\n\
         The MOST threatened city is the single one with the HIGHEST fall probability. A big, \
         well-defended city that draws a lot of force is not automatically the most likely to fall — \
         weigh the actual odds, not raw exposure.\n",
    );

    s.push_str(
        "\nVACATE RULES (for judging whether a city can spare its defender for one turn). The \
         question is strictly about THIS TURN: if you pulled the city's single best defender out \
         now, would the city still hold against the enemy force that can reach it, or would it fall? \
         Use the COMBAT ODDS above:\n\
         - The scariest incoming enemy LAND attacker is the single one with the largest att_eff \
           divided by the turns it needs to arrive (air/sea excluded; none past the ~6-turn \
           horizon).\n\
         - REMAINING DEFENDER — the city's best defender AFTER the strongest one is removed (its \
           second-best defender, or NONE if the city had only one). Terrain, the city-center bonus, \
           and City Walls still apply to it.\n\
         Compute that attacker's WIN PROBABILITY against the remaining defender. You CANNOT vacate \
         (answer no) when the attacker's win probability is high (the vacated city would fall). You \
         CAN vacate (answer yes) when the attacker's win probability is low (the remaining garrison \
         still holds). Judge the counterfactual board (best defender removed), NOT the city's \
         current, still-garrisoned defense.\n",
    );

    s.push_str(
        "\nASSAULT-TARGET RULES (for judging which of YOUR cities an enemy would most want to \
         assault — reason from the ATTACKER's point of view about your own cities). Each of your \
         candidate cities is judged on TWO orthogonal axes, both higher-is-better for the attacker:\n\
         - EASE OF CAPTURE: the probability the city falls to its scariest incoming attacker (the \
           COMBAT ODDS above — that attacker's WIN PROBABILITY against the city's best defender; an \
           undefended city in reach falls for certain). Higher = a softer, more attractive target.\n\
         - VALUE: the city's SIZE. Higher = a richer prize.\n\
         One city is an UNAMBIGUOUSLY better assault target than another when it is AT LEAST AS EASY \
         to capture AND AT LEAST AS VALUABLE, and strictly better on at least one axis. A city that \
         is a clearly worse target this way (another candidate is both easier to capture AND at \
         least as big) is an UNSOUND pick. Every candidate that is NOT a clearly worse target than \
         some other is a SOUND answer. There is NO fixed weighting between the two axes — a big, \
         well-defended city and a small, soft one are a genuine trade-off, both sound.\n",
    );

    s.push_str(
        "\nTRIAGE-REINFORCE RULES (for allocating your ONE spare defender among cities under \
         threat). Send the defender where it does the most good — where it actually changes the \
         outcome. Using the COMBAT ODDS above, a city is CAPTURABLE this turn when its scariest \
         incoming attacker's WIN PROBABILITY against its best defender is high, and HELD when that \
         win probability is low. Placing the spare defender in a city makes its defender whichever \
         is stronger, its current best defender or the spare defender garrisoned there (terrain, \
         city-center, and City Walls apply) — which can lower the attacker's win probability.\n\
         The RIGHT city is one that is CAPTURABLE now but would be HELD once the spare defender is \
         added — the reinforcement FLIPS it from lost to saved. Reinforcing a city that is already \
         HELD wastes the defender (no change), and reinforcing a DOOMED city that stays capturable \
         even with the defender also wastes it — BOTH are UNSOUND picks, however much or little \
         force each draws. Judge the MARGINAL effect of the defender, not raw exposure.\n",
    );

    s.push_str(
        "\nATTACK-COMPARISON RULES (for judging which of two proposed attacks is better, or whether \
         they are incomparable). Each attack — one of your units striking one enemy unit — is judged \
         on TWO axes, both higher-is-better:\n\
         - EXCHANGE FAVORABILITY: the attacker's WIN PROBABILITY (the COMBAT ODDS above) striking \
           the target where it stands (its terrain, and city-center/City Walls if it is in one of \
           its cities). A higher win probability is a more certain kill — and a big strength ratio \
           can still be only a coin-flip when the defender has far more hitpoints.\n\
         - THREAT REMOVED: the target's OWN att_eff — how much enemy attacking power the kill \
           deletes.\n\
         One attack is UNAMBIGUOUSLY better when it is AT LEAST AS GOOD on BOTH axes and strictly \
         better on at least one. If instead each attack is better on a DIFFERENT axis (one is the \
         more favorable exchange, the other removes the bigger threat), the two are INCOMPARABLE — \
         there is NO fixed weighting between the axes, so do not invent one to break the tie; answer \
         that they are incomparable.\n",
    );

    let _ = write!(
        s,
        "\nCONSTRAINT-SITE RULES (for choosing a fortified city site that meets hard requirements). \
         You are given a rectangular REGION of the map, not a shortlist — you must SEARCH it. A \
         tile is a VALID site only if it is LAND and satisfies ALL FOUR of these requirements at \
         once (a conjunction — failing any one disqualifies it):\n\
         - DEFENSIBLE: the tile's own terrain gives a defense bonus — Forest, Jungle, Swamp, Hills, \
           or Mountains. Flat, open terrain (Grassland, Plains, Desert, Tundra) is NOT defensible.\n\
         - WITHIN 2 OF WATER: at least one tile within 2 tiles (Chebyshev distance) of the \
           candidate — counting the candidate itself — is open water (Ocean, Deep Ocean, or Lake).\n\
         - NOT IN ENEMY TERRITORY: the candidate tile is unclaimed or claimed by you; a tile inside \
           another player's borders is disqualified.\n\
         - SPACED: the tile is at least {sp} tiles (Chebyshev distance) from EVERY existing city on \
           the map — yours and every other player's alike. A tile on, or within {near} tiles of, any \
           city is too close and is disqualified. (This is the minimum city-distance rule; you must \
           read the city positions and compute this distance yourself — no per-tile lookup reports \
           it.)\n\
         Answer with the coordinates of ANY tile in the region that meets all four requirements. If \
         NO tile in the region meets them all, the correct answer is the word \"none\".\n",
        sp = crate::rules::CITY_MIN_SPACING,
        near = crate::rules::CITY_MIN_SPACING - 1,
    );

    s.push_str(
        "\nCITY-ASSAULT RULES (for judging whether your attacking units can CAPTURE an enemy city \
         THIS turn — when FOG hides its garrison). You cannot see the city's defenders; you must \
         judge the odds from what you CAN see — the city's size, its terrain and whether it looks \
         walled, its owner's strength, and any enemy relief nearby. Model the storm with the COMBAT \
         ODDS above: your units attack the city's strongest defender one at a time; each attack, by \
         its win probability, either destroys that defender (and the NEXT strongest defender is then \
         exposed) or loses your attacking unit. The city is CAPTURED only if your stack destroys \
         EVERY defender before it runs out of attackers; an undefended city is taken for certain. A \
         city with no VISIBLE defenders is NOT necessarily empty — a garrison may be hidden. Answer \
         yes only if you judge the capture likely, no if the city probably holds.\n",
    );

    let _ = write!(
        s,
        "\nSURPRISE-STRIKE RULES (for judging which of your OWN cities is most exposed to an attack by \
         an enemy you CANNOT currently see). Fogged tiles may hide an enemy land unit. Judge exposure \
         as a WORST CASE from what you CAN observe — do NOT try to guess where an unseen unit actually \
         is. A fogged tile within {r} tiles (Chebyshev) of a city threatens that city ONLY IF an enemy \
         unit you CAN currently see could move into that fog within a few turns (trace its path over \
         land, at its own move rate, up to {h} turns) — fog that no visible enemy can reach is not a \
         credible hiding place. Among the fog a visible enemy CAN reach, a city is more exposed the \
         STRONGER (by attack strength, att_eff) the scariest such visible enemy is and the CLOSER that \
         reachable fog lies to the city. A city with NO fogged tile within {r} tiles CANNOT be \
         surprised — there is nowhere near it for an unseen enemy to be, so it is not the answer no \
         matter how large it is; and neither is a city whose only nearby fog no visible enemy could \
         reach. Pick the one city whose observable surroundings admit the greatest surprise-strike \
         force.\n",
        r = SURPRISE_REACH,
        h = THREAT_HORIZON,
    );

    s.push_str(
        "\nHIDDEN-FORCE RULES (for judging which fogged AREA hides the largest massed enemy force you \
         cannot see). Each offered area is the square of tiles within a stated radius of its center. \
         An area can only hide force where you have no sight — on its FOGGED tiles. Judge each area by \
         how much enemy attacking strength (att_eff) could be massed on the fogged tiles inside it, \
         reasoning from the cues you can see: fogged area near the enemy's own territory or a visible \
         enemy unit is a likelier muster point; an area with NO fogged tiles is provably empty of \
         anything hidden. Units stack without limit on any land tile — terrain does not limit how \
         much force can be massed on a tile; judge likely muster points from territory/proximity \
         cues, not from how much \"room\" the terrain seems to have. Pick the area whose fog most \
         likely conceals the biggest force.\n",
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(kind: &str, veteran: i32, hp: i32) -> Unit {
        Unit {
            x: 0,
            y: 0,
            id: 1,
            kind: kind.to_string(),
            owner: "p".to_string(),
            veteran,
            hp,
        }
    }

    #[test]
    fn context_free_strengths() {
        // Warriors 1/1, Armor 10/5, full green.
        let w = unit("Warriors", 0, 10);
        let a = unit("Armor", 0, 30);
        assert_eq!(att_eff(&w).unwrap(), 1.0);
        assert_eq!(att_eff(&a).unwrap(), 10.0);
        assert_eq!(def_eff_base(&a).unwrap(), 5.0);
    }

    #[test]
    fn veteran_multiplier_applies() {
        let green = unit("Warriors", 0, 10);
        let elite = unit("Warriors", 3, 10);
        assert_eq!(def_eff_base(&green).unwrap(), 1.0);
        assert_eq!(def_eff_base(&elite).unwrap(), 2.0); // ×2.0 elite
    }

    #[test]
    fn health_scales_strength() {
        // Armor at 9/30 hp -> health 0.3 -> att_eff 10 * 0.3 = 3.0.
        let damaged = unit("Armor", 0, 9);
        assert!((att_eff(&damaged).unwrap() - 3.0).abs() < 1e-9);
    }

    #[test]
    fn unknown_type_is_none() {
        let u = unit("Death Star", 0, 10);
        assert!(att_eff(&u).is_none());
    }

    #[test]
    fn terrain_and_fort_multipliers() {
        assert_eq!(
            defense_multiplier("Grassland", &BTreeSet::new(), false, false),
            1.0
        );
        assert_eq!(
            defense_multiplier("Grassland", &BTreeSet::new(), true, false),
            1.5
        );
        assert_eq!(
            defense_multiplier("Hills", &BTreeSet::new(), true, false),
            2.5
        );
        let mut fort: BTreeSet<String> = BTreeSet::new();
        fort.insert("Fortress".to_string());
        assert_eq!(defense_multiplier("Hills", &fort, true, false), 3.5);
    }

    #[test]
    fn win_prob_equal_fight_is_half() {
        // Equal power, equal hp/firepower → a 50/50 fight (exactly, by symmetry).
        let p = win_probability(2.0, 10.0, 1.0, 2.0, 10.0, 1.0);
        assert!((p - 0.5).abs() < 1e-9, "p={p}");
        // Symmetry across many equal-hp sizes.
        for hp in [10.0, 20.0, 30.0, 40.0] {
            let p = win_probability(5.0, hp, 1.0, 5.0, hp, 1.0);
            assert!((p - 0.5).abs() < 1e-9, "hp={hp} p={p}");
        }
    }

    #[test]
    fn win_prob_overwhelming_and_degenerate() {
        // Overwhelming attacker (100 vs 1) → near-certain win.
        let p = win_probability(100.0, 10.0, 1.0, 1.0, 10.0, 1.0);
        assert!(p > 0.999, "p={p}");
        // Zero attack power → cannot win.
        assert_eq!(win_probability(0.0, 10.0, 1.0, 3.0, 10.0, 1.0), 0.0);
        // Zero defense power → certain win (undefended target).
        assert_eq!(win_probability(3.0, 10.0, 1.0, 0.0, 10.0, 1.0), 1.0);
        // A probability, always in [0, 1].
        let p = win_probability(7.0, 13.0, 1.0, 4.0, 27.0, 1.0);
        assert!((0.0..=1.0).contains(&p), "p={p}");
    }

    #[test]
    fn win_prob_hp_dominates_a_high_ratio() {
        // Equal power (p=0.5 per round) but the defender has 3x the hitpoints → the attacker must
        // land far more hits and is a clear underdog, even though the strength RATIO is 1:1.
        let underdog = win_probability(5.0, 10.0, 1.0, 5.0, 30.0, 1.0);
        assert!(underdog < 0.1, "underdog={underdog}");
        // Flip the hitpoints: attacker with 3x hp is a strong favorite at the same 1:1 ratio.
        let favorite = win_probability(5.0, 30.0, 1.0, 5.0, 10.0, 1.0);
        assert!(favorite > 0.9, "favorite={favorite}");
        // The two are complementary (a 1:1-power race is a fair fight from each side).
        assert!((underdog + favorite - 1.0).abs() < 1e-9);
        // A 2:1 STRENGTH ratio is NOT a certain win when the defender out-HPs the attacker: here
        // A=8, D=4 (per-round p=0.667) but the defender has 30 hp to the attacker's 10, so the
        // attacker must land 30 hits before taking 10 — it is actually an UNDERDOG, not a sure kill.
        let hp_underdog = win_probability(8.0, 10.0, 1.0, 4.0, 30.0, 1.0);
        assert!(
            hp_underdog < 0.3,
            "a 2:1 ratio is far from a certain win when out-HP'd: {hp_underdog}"
        );
        // Give the attacker equal hitpoints at that same 2:1 ratio → now a strong favorite. The raw
        // ratio was identical; only the HP race (which the scalar model ignored) changed the answer.
        let hp_even = win_probability(8.0, 30.0, 1.0, 4.0, 30.0, 1.0);
        assert!(hp_even > 0.8, "2:1 ratio with equal hp: {hp_even}");
    }

    #[test]
    fn win_prob_firepower_speeds_the_kill() {
        // Higher attacker firepower (2 vs 1) needs fewer hits to kill → strictly better odds, all
        // else equal.
        let fp1 = win_probability(5.0, 20.0, 1.0, 5.0, 20.0, 1.0);
        let fp2 = win_probability(5.0, 20.0, 2.0, 5.0, 20.0, 1.0);
        assert!(fp2 > fp1, "fp2={fp2} fp1={fp1}");
    }

    #[test]
    fn walls_add_land_defense() {
        // City center on open ground: unwalled 1.5, walled 2.5 (+1.0 for City Walls).
        assert_eq!(
            defense_multiplier("Grassland", &BTreeSet::new(), true, false),
            1.5
        );
        assert_eq!(
            defense_multiplier("Grassland", &BTreeSet::new(), true, true),
            2.5
        );
    }

    /// A `width`x1 all-Grassland strip: player "A" city at `city_x`, an enemy "B" Legion at
    /// `legion_x`. Used to exercise the threat field and settle axes deterministically.
    fn strip(width: i32, city_x: i32, legion_x: i32) -> Board {
        use civ_core::{Player, Tile};
        let mut row = Vec::new();
        for x in 0..width {
            row.push(Tile {
                x,
                y: 0,
                terrain: "Grassland".to_string(),
                extras: BTreeSet::new(),
                owner: None,
            });
        }
        Board {
            width,
            height: 1,
            tiles: vec![row],
            cities: vec![City {
                x: city_x,
                y: 0,
                id: 1,
                name: "A".to_string(),
                owner: "A".to_string(),
                size: 1,
                improvements: BTreeSet::new(),
            }],
            units: vec![Unit {
                x: legion_x,
                y: 0,
                id: 9,
                kind: "Legion".to_string(),
                owner: "B".to_string(),
                veteran: 0,
                hp: 10,
            }],
            players: vec![
                Player {
                    id: 0,
                    name: "A".to_string(),
                    nation: String::new(),
                    is_alive: true,
                },
                Player {
                    id: 1,
                    name: "B".to_string(),
                    nation: String::new(),
                    is_alive: true,
                },
            ],
            known: std::collections::HashMap::new(),
            visibility: None,
            ruleset: None,
            turn: None,
            source: "strip".to_string(),
        }
    }

    #[test]
    fn worst_case_garrison_picks_strongest_fielded_combat_type() {
        use civ_core::{Player, Tile};
        // 3x1 grassland; enemy "B" owns a city at (1,0) and fields, elsewhere, a Warriors (def 1),
        // a Phalanx (def 2) and a Settlers (civilian, attack 0 — not a defender). The worst-case
        // garrison is a GREEN Phalanx (strongest COMBAT type by def_eff_base) on the city centre:
        // def_eff 2 x (1 + 0.5 center) = 3.0.
        let row: Vec<Tile> = (0..3)
            .map(|x| Tile {
                x,
                y: 0,
                terrain: "Grassland".to_string(),
                extras: BTreeSet::new(),
                owner: None,
            })
            .collect();
        let mk = |id, x, kind: &str| Unit {
            x,
            y: 0,
            id,
            kind: kind.to_string(),
            owner: "B".to_string(),
            veteran: 0,
            hp: 10,
        };
        let board = Board {
            width: 3,
            height: 1,
            tiles: vec![row],
            cities: vec![City {
                x: 1,
                y: 0,
                id: 1,
                name: "B".to_string(),
                owner: "B".to_string(),
                size: 1,
                improvements: BTreeSet::new(),
            }],
            units: vec![
                mk(10, 0, "Warriors"),
                mk(11, 2, "Phalanx"),
                mk(12, 0, "Settlers"),
            ],
            players: vec![Player {
                id: 0,
                name: "B".to_string(),
                nation: String::new(),
                is_alive: true,
            }],
            known: std::collections::HashMap::new(),
            visibility: None,
            ruleset: None,
            turn: None,
            source: "wcg".to_string(),
        };
        let (synth, def_power) =
            worst_case_garrison(&board, &board.cities[0]).expect("a combat defender is fielded");
        assert_eq!(synth.kind, "Phalanx");
        assert_eq!(synth.veteran, 0);
        assert_eq!(synth.hp, 10); // green, full HP
        assert_eq!((synth.x, synth.y), (1, 0)); // stands on the city centre
        assert!((def_power - 3.0).abs() < 1e-9);
    }

    #[test]
    fn worst_case_garrison_none_when_owner_fields_no_combat_unit() {
        use civ_core::{Player, Tile};
        let row: Vec<Tile> = (0..3)
            .map(|x| Tile {
                x,
                y: 0,
                terrain: "Grassland".to_string(),
                extras: BTreeSet::new(),
                owner: None,
            })
            .collect();
        let mk = |id, x, kind: &str| Unit {
            x,
            y: 0,
            id,
            kind: kind.to_string(),
            owner: "B".to_string(),
            veteran: 0,
            hp: 10,
        };
        let board = Board {
            width: 3,
            height: 1,
            tiles: vec![row],
            cities: vec![City {
                x: 1,
                y: 0,
                id: 1,
                name: "B".to_string(),
                owner: "B".to_string(),
                size: 1,
                improvements: BTreeSet::new(),
            }],
            units: vec![mk(10, 0, "Settlers"), mk(11, 2, "Workers")], // civilians only
            players: vec![Player {
                id: 0,
                name: "B".to_string(),
                nation: String::new(),
                is_alive: true,
            }],
            known: std::collections::HashMap::new(),
            visibility: None,
            ruleset: None,
            turn: None,
            source: "wcg-none".to_string(),
        };
        assert!(worst_case_garrison(&board, &board.cities[0]).is_none());
    }

    #[test]
    fn tactical_reach_blocks_entry_and_zoc_squeeze() {
        use civ_core::{Player, Tile};
        // 5x3 all-Grassland. Enemy "B" Legion (military → exerts ZOC) at (2,1); enemy "B" city at
        // (4,0). My mover "Me" starts at (0,1), OUTSIDE the Legion's ZOC.
        let tiles: Vec<Vec<Tile>> = (0..3)
            .map(|y| {
                (0..5)
                    .map(|x| Tile {
                        x,
                        y,
                        terrain: "Grassland".to_string(),
                        extras: BTreeSet::new(),
                        owner: None,
                    })
                    .collect()
            })
            .collect();
        let board = Board {
            width: 5,
            height: 3,
            tiles,
            cities: vec![City {
                x: 4,
                y: 0,
                id: 1,
                name: "B".to_string(),
                owner: "B".to_string(),
                size: 1,
                improvements: BTreeSet::new(),
            }],
            units: vec![
                Unit {
                    x: 2,
                    y: 1,
                    id: 9,
                    kind: "Legion".to_string(),
                    owner: "B".to_string(),
                    veteran: 0,
                    hp: 10,
                },
                Unit {
                    x: 0,
                    y: 1,
                    id: 7,
                    kind: "Warriors".to_string(),
                    owner: "Me".to_string(),
                    veteran: 0,
                    hp: 10,
                },
            ],
            players: vec![
                Player {
                    id: 0,
                    name: "B".to_string(),
                    nation: String::new(),
                    is_alive: true,
                },
                Player {
                    id: 1,
                    name: "Me".to_string(),
                    nation: String::new(),
                    is_alive: true,
                },
            ],
            known: std::collections::HashMap::new(),
            visibility: None,
            ruleset: None,
            turn: None,
            source: "tac".to_string(),
        };
        let plain = ReachField::from_origins(&board, &[(0, 1)], 5, 3);
        let tac = ReachField::from_origins_tactical(&board, &[(0, 1)], 5, 3, "Me");

        // A non-ZOC origin may still step INTO a ZOC tile: (1,1) stays reachable.
        assert!(
            tac.turns_at(1, 1).is_some(),
            "entering a ZOC tile from outside the zone is legal"
        );
        // Entry blocks: neither the enemy unit's tile nor the enemy city is an advance destination,
        // though the enemy-blind field reaches both.
        assert!(
            plain.turns_at(2, 1).is_some() && tac.turns_at(2, 1).is_none(),
            "cannot post onto an enemy unit"
        );
        assert!(
            plain.turns_at(4, 0).is_some() && tac.turns_at(4, 0).is_none(),
            "cannot post onto an enemy city"
        );
        // ZOC squeeze: a step between two enemy-controlled tiles is forbidden, so the far side of the
        // Legion is unreachable tactically even though it is geometrically close.
        assert!(
            plain.turns_at(3, 1).is_some() && tac.turns_at(3, 1).is_none(),
            "ZOC squeeze blocks crossing the enemy's zone of control"
        );
    }

    #[test]
    fn site_dominance() {
        let base = SiteAxes {
            food: 3,
            production: 2,
            resources: 1,
            safety: -4.0,
        };
        // Identical terrain, strictly safer → dominates purely on safety (judgment-loaded).
        let safer = SiteAxes {
            safety: 0.0,
            ..base
        };
        assert!(safer.dominates(&base));
        assert!(!base.dominates(&safer));
        assert!(!safer.dominates_terrain(&base)); // terrain-equal → no terrain dominance
                                                  // Equal on everything → neither dominates.
        assert!(!base.dominates(&base));
        // Strictly better food only → terrain dominance (and full dominance).
        let richer = SiteAxes { food: 4, ..base };
        assert!(richer.dominates_terrain(&base));
        assert!(richer.dominates(&base));
        // A tradeoff (more food, less production) → no dominance either way.
        let tradeoff = SiteAxes {
            food: 5,
            production: 0,
            ..base
        };
        assert!(!tradeoff.dominates(&base));
        assert!(!base.dominates(&tradeoff));
    }

    #[test]
    fn retreat_dominance() {
        // cover / safety / support, higher-is-better on all three.
        let base = RetreatAxes {
            cover: 2.0,
            safety: -3.0,
            support: 1,
        };
        // Identical cover+support, strictly safer -> dominates purely on safety (judgment-loaded).
        let safer = RetreatAxes {
            safety: 0.0,
            ..base
        };
        assert!(safer.dominates(&base));
        assert!(!base.dominates(&safer));
        assert!(!safer.dominates_static(&base)); // cover+support equal -> no static dominance
                                                 // Equal on everything -> neither dominates.
        assert!(!base.dominates(&base));
        // Strictly more cover only -> static dominance (and full dominance).
        let tougher = RetreatAxes { cover: 3.0, ..base };
        assert!(tougher.dominates_static(&base));
        assert!(tougher.dominates(&base));
        // A tradeoff (more cover, less support) -> no dominance either way.
        let tradeoff = RetreatAxes {
            cover: 3.0,
            support: 0,
            ..base
        };
        assert!(!tradeoff.dominates(&base));
        assert!(!base.dominates(&tradeoff));
    }

    #[test]
    fn assault_axes_dominance() {
        // capture ↑ (softer), value ↑ (bigger prize) = a more attractive target. Additive 0.15 margin
        // on the capture axis; strict (no-margin) comparison on the exact integer size axis.
        let soft_small = AssaultAxes {
            capture: 1.0,
            value: 2.0,
        }; // easy but cheap
        let tough_big = AssaultAxes {
            capture: 0.2,
            value: 8.0,
        }; // hard but valuable
        let soft_big = AssaultAxes {
            capture: 1.0,
            value: 8.0,
        }; // easy AND valuable
        let tough_small = AssaultAxes {
            capture: 0.2,
            value: 2.0,
        }; // hard AND cheap
           // soft_big decisively dominates all three others (≥ on both axes, a real lead on ≥1).
        assert!(soft_big.decisively_dominates(&soft_small, 0.15)); // value lead, capture tie
        assert!(soft_big.decisively_dominates(&tough_big, 0.15)); // capture lead, value tie
        assert!(soft_big.decisively_dominates(&tough_small, 0.15)); // both axes lead
                                                                    // soft_small vs tough_big is a genuine trade-off → neither dominates (the withheld weighting).
        assert!(!soft_small.decisively_dominates(&tough_big, 0.15));
        assert!(!tough_big.decisively_dominates(&soft_small, 0.15));
        // tough_small is decisively dominated by BOTH soft_small (capture lead) and tough_big (size).
        assert!(soft_small.decisively_dominates(&tough_small, 0.15));
        assert!(tough_big.decisively_dominates(&tough_small, 0.15));
        // A sub-margin capture lead (0.30 vs 0.20 = +0.10 < 0.15) at equal size is a near-tie → no
        // dominance either way (both stay on the frontier).
        let near = AssaultAxes {
            capture: 0.30,
            value: 2.0,
        };
        assert!(!near.decisively_dominates(&tough_small, 0.15));
        assert!(!tough_small.decisively_dominates(&near, 0.15));
    }

    #[test]
    fn threat_field_grades_by_distance() {
        // Legion (attack 4, move 1) at x=4 on a 20-wide strip.
        let b = strip(20, 0, 4);
        let f = ThreatField::compute(&b, "A");
        // Tiles adjacent to the Legion are attackable this turn → full att_eff 4.
        assert!((f.at(5, 0) - 4.0).abs() < 1e-9, "at(5,0)={}", f.at(5, 0));
        assert!((f.at(3, 0) - 4.0).abs() < 1e-9);
        // (0,0): nearest staging tile (1,0) is 3 steps from the Legion → 3 turns → 4/3.
        assert!(
            (f.at(0, 0) - 4.0 / 3.0).abs() < 1e-9,
            "at(0,0)={}",
            f.at(0, 0)
        );
        // Far beyond the 6-turn horizon → no threat.
        assert_eq!(f.at(15, 0), 0.0);
    }

    #[test]
    fn city_capture_prob_paths() {
        // Undefended A-city adjacent to the enemy Legion (attack 4) → certain fall (undefended
        // branch: any reachable attacker takes an empty city).
        let b = strip(10, 5, 4);
        let field = ThreatField::compute(&b, "A");
        assert_eq!(city_capture_prob(&b, &b.cities[0], &field), 1.0);
        // Garrison the same city with a Phalanx (def 2, + city-center bonus): the best_defender
        // branch now yields a real fall probability strictly inside (0, 1).
        let mut g = strip(10, 5, 4);
        g.units.push(Unit {
            x: 5,
            y: 0,
            id: 7,
            kind: "Phalanx".to_string(),
            owner: "A".to_string(),
            veteran: 0,
            hp: 10,
        });
        let gf = ThreatField::compute(&g, "A");
        let p = city_capture_prob(&g, &g.cities[0], &gf);
        assert!(p > 0.0 && p < 1.0, "garrisoned fall prob {p} not in (0, 1)");
        // A garrison strictly lowers the fall probability vs. leaving the city empty.
        assert!(p < 1.0);
        // Legion out of reach (city at x=1, Legion at x=19, 6-turn horizon) → no attacker → 0.
        let far = strip(20, 1, 19);
        let ff = ThreatField::compute(&far, "A");
        assert_eq!(city_capture_prob(&far, &far.cities[0], &ff), 0.0);
    }

    #[test]
    fn site_axes_tally_and_safety() {
        let b = strip(20, 0, 4);
        let a = site_axes(&b, "A", (0, 0));
        assert_eq!(a.food, 3); // 3 in-bounds Grassland tiles in radius 2 of (0,0)
        assert_eq!(a.production, 0);
        assert_eq!(a.resources, 0);
        assert!((a.safety + 4.0 / 3.0).abs() < 1e-9, "safety={}", a.safety);
    }

    #[test]
    fn reach_field_land_bfs_and_horizon() {
        let b = strip(20, 0, 4); // 20x1 all-Grassland (all land)
        let r = ReachField::from_origins(&b, &[(0, 0)], 1, 6);
        assert_eq!(r.turns_at(0, 0), Some(0));
        assert_eq!(r.turns_at(4, 0), Some(4)); // move 1 → 4 turns
        assert_eq!(r.turns_at(6, 0), Some(6)); // exactly at the horizon
        assert_eq!(r.turns_at(7, 0), None); // 7 turns > horizon 6
                                            // A faster unit reaches farther for the same turn budget (ceil division).
        let r2 = ReachField::from_origins(&b, &[(0, 0)], 2, 6);
        assert_eq!(r2.turns_at(4, 0), Some(2));
        assert_eq!(r2.turns_at(11, 0), Some(6)); // ceil(11 / 2) = 6
    }

    // Extras presets — typed as `&[&str]` so `[cell; N]` array-repeat literals type-check.
    const NO_EX: &[&str] = &[];
    const ROAD_EX: &[&str] = &["Road"];
    const RAIL_EX: &[&str] = &["Railroad", "Road"];
    const RIVER_EX: &[&str] = &["River"];

    /// A 1×N board from `(terrain, &[extras])` cells — the minimum a [`ReachField`] reads.
    fn terrain_row(cells: &[(&str, &[&str])]) -> Board {
        let mut row = Vec::new();
        for (x, (terrain, extras)) in cells.iter().enumerate() {
            row.push(civ_core::Tile {
                x: x as i32,
                y: 0,
                terrain: (*terrain).to_string(),
                extras: extras.iter().map(|s| s.to_string()).collect(),
                owner: None,
            });
        }
        Board {
            width: cells.len() as i32,
            height: 1,
            tiles: vec![row],
            cities: vec![],
            units: vec![],
            players: vec![],
            known: std::collections::HashMap::new(),
            visibility: None,
            ruleset: None,
            turn: None,
            source: "terrain_row".to_string(),
        }
    }

    #[test]
    fn reach_field_charges_classic_terrain_costs() {
        // Armor (move_rate 3 → budget = 9 fragments). Grass=3, Forest=6, Mountains=9 fragments.
        //   idx:  0 G | 1 F | 2 F | 3 G | 4 M | 5 G | 6 G
        let b = terrain_row(&[
            ("Grassland", NO_EX),
            ("Forest", NO_EX),
            ("Forest", NO_EX),
            ("Grassland", NO_EX),
            ("Mountains", NO_EX),
            ("Grassland", NO_EX),
            ("Grassland", NO_EX),
        ]);
        let r = ReachField::from_origins(&b, &[(0, 0)], 3, 6);
        assert_eq!(r.turns_at(0, 0), Some(0));
        assert_eq!(r.turns_at(1, 0), Some(1)); // E=6
        assert_eq!(r.turns_at(2, 0), Some(1)); // 2nd Forest overflows the turn → E=9
        assert_eq!(r.turns_at(3, 0), Some(2)); // fresh turn, +3 → E=12
        assert_eq!(r.turns_at(4, 0), Some(2)); // Mountain (9) > 6 left → burns turn, E=18
        assert_eq!(r.turns_at(5, 0), Some(3)); // fresh turn, +3 → E=21
        assert_eq!(r.turns_at(6, 0), Some(3)); // +3 → E=24
    }

    #[test]
    fn reach_field_road_beats_mountains() {
        // A row of Mountains, a 1-move unit (budget 3). Bare mountains cost a full turn each
        // (9 > 3, always-advance burns the turn); a Road along them costs 1 fragment/step, so the
        // unit covers 3 road-mountain tiles per turn instead of 1.
        let bare = terrain_row(&[("Mountains", NO_EX); 8]);
        let r = ReachField::from_origins(&bare, &[(0, 0)], 1, 6);
        assert_eq!(r.turns_at(1, 0), Some(1));
        assert_eq!(r.turns_at(6, 0), Some(6));
        assert_eq!(r.turns_at(7, 0), None); // 7 turns > horizon 6

        let roaded = terrain_row(&[("Mountains", ROAD_EX); 8]);
        let rr = ReachField::from_origins(&roaded, &[(0, 0)], 1, 6);
        assert_eq!(rr.turns_at(1, 0), Some(1)); // E=1
        assert_eq!(rr.turns_at(3, 0), Some(1)); // E=3 (3 road steps in one turn)
        assert_eq!(rr.turns_at(4, 0), Some(2)); // E=4
        assert_eq!(rr.turns_at(7, 0), Some(3)); // E=7 → reachable now (was unreachable bare)
    }

    #[test]
    fn reach_field_railroad_is_free() {
        // A rail-connected strip is reachable at ~no turn cost: every railroad tile is 0 turns.
        let rail = terrain_row(&[("Mountains", RAIL_EX); 8]);
        let r = ReachField::from_origins(&rail, &[(0, 0)], 1, 6);
        for x in 0..8 {
            assert_eq!(r.turns_at(x, 0), Some(0), "rail tile {x} should be 0 turns");
        }
        // A single non-rail Mountains tile breaks the free chain: a rail bonus needs BOTH
        // endpoints railed, so entering the gap (tile2) AND re-entering rail from the gap (tile3)
        // each cost a step; only once back on rail (tile3→tile4) is movement free again.
        let broken = terrain_row(&[
            ("Grassland", RAIL_EX),
            ("Grassland", RAIL_EX),
            ("Mountains", NO_EX), // gap: no rail
            ("Grassland", RAIL_EX),
            ("Grassland", RAIL_EX),
        ]);
        let rb = ReachField::from_origins(&broken, &[(0, 0)], 1, 6);
        assert_eq!(rb.turns_at(0, 0), Some(0));
        assert_eq!(rb.turns_at(1, 0), Some(0)); // free rail
        assert_eq!(rb.turns_at(2, 0), Some(1)); // enter the gap from rail → burns a turn (9 > 3)
        assert_eq!(rb.turns_at(3, 0), Some(2)); // gap→rail edge isn't rail-to-rail → +1 turn
        assert_eq!(rb.turns_at(4, 0), Some(2)); // rail↔rail again → free, same turn
    }

    #[test]
    fn reach_field_river_moves_like_road() {
        // Moving ALONG a river (both endpoints River) costs a road fragment, not the terrain cost.
        // Forest normally costs 6 (2 turns for tile2 at budget 3); a river makes it 1/step.
        let river = terrain_row(&[("Forest", RIVER_EX); 6]);
        let r = ReachField::from_origins(&river, &[(0, 0)], 1, 6);
        assert_eq!(r.turns_at(3, 0), Some(1)); // 3 river steps in one turn (budget 3)
                                               // Without the river these Forests would cost 6 each → tile3 = 3 turns.
        let dry = terrain_row(&[("Forest", NO_EX); 6]);
        let d = ReachField::from_origins(&dry, &[(0, 0)], 1, 6);
        assert_eq!(d.turns_at(3, 0), Some(3));
    }
}
