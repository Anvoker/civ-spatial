//! Divergence reproducer for the combat-odds model (`analysis/findings-combat-odds.md`).
//!
//! For each combat-based decision kind, it recomputes the "correct" answer under BOTH the old
//! scalar model (favorability = `att_eff / def_eff`; capture by the 1.25× strength-ratio margin) and
//! the new win-probability model (`rules::win_probability` / `rules::attack_win_prob`; capture by the
//! 0.7 / 0.3 win-prob band), over a deterministic enumeration of instances, and reports how many
//! "correct" answers CHANGE. This is the payload that tells us whether the extra fidelity matters.
//!
//! Run: `cargo run -p civ-eval --example combat_odds_divergence`.

use std::collections::HashMap;

use civ_core::{parse_save, Board, City, Unit};
use civ_eval::rules::{
    self, att_eff, city_defense, city_defense_vacated, city_fall_prob, city_fall_prob_undefended,
    defenders_ranked, unit_def_on_own_tile, AttackAxes, ThreatField, WIN_PROB_HIGH, WIN_PROB_LOW,
};

/// The T2 decisive-margin ratio the old scalar solvers used (mirrors `question.rs::T2_MARGIN`).
const MARGIN: f64 = 1.25;
const EPS: f64 = 1e-9;

fn main() {
    for board_name in ["testcontroller_T677.sav", "myagent_T50.sav"] {
        let path = format!("data/saves/{board_name}");
        let Ok(board) = parse_save(&path) else {
            eprintln!("skip {board_name}: parse failed");
            continue;
        };
        println!("\n================ {board_name} ================");
        compare_two_attacks_divergence(&board);
        cf_vacate_divergence(&board);
        capture_state_divergence(&board);
    }
}

// --------------------------------------------------------------------------
// compare-two-attacks: old favorability (att/def ratio) vs new (win probability)
// --------------------------------------------------------------------------

/// The three-way verdict of comparing two attacks under a given axis pair.
fn attack_verdict(ax_a: &AttackAxes, ax_b: &AttackAxes) -> &'static str {
    if ax_a.decisively_dominates(ax_b, MARGIN) {
        "A"
    } else if ax_b.decisively_dominates(ax_a, MARGIN) {
        "B"
    } else if ax_a.genuine_tradeoff(ax_b, MARGIN) {
        "incomparable"
    } else {
        "dropped"
    }
}

/// The OLD axes: favorability = raw `att_eff / def_eff` ratio, threat-removed = target `att_eff`.
fn old_axes(board: &Board, attacker: &Unit, target: &Unit) -> Option<AttackAxes> {
    let att = att_eff(attacker)?;
    let def = unit_def_on_own_tile(board, target)?;
    if def <= 0.0 {
        return None;
    }
    Some(AttackAxes {
        favorability: att / def,
        threat_removed: att_eff(target)?,
    })
}

fn compare_two_attacks_divergence(board: &Board) {
    // Enumerate the "which target" comparison: one attacker, two distinct enemy targets. A clean,
    // deterministic subset of the full sampling generator.
    let mut by_owner: HashMap<&str, Vec<&Unit>> = HashMap::new();
    for u in &board.units {
        let land = rules::unit_stat(&u.kind)
            .map(|s| s.class == rules::UnitClass::Land)
            .unwrap_or(false);
        if land && att_eff(u).map(|a| a > 0.0).unwrap_or(false) {
            by_owner.entry(u.owner.as_str()).or_default().push(u);
        }
    }
    // Bound the enumeration on dense boards: a fixed attacker cap per owner and a fixed target
    // window keeps the reproducer fast while sampling a representative, deterministic slice.
    const ATTACKERS_PER_OWNER: usize = 12;
    const TARGET_WINDOW: usize = 24;
    let mut total = 0usize;
    let mut changed = 0usize;
    let mut examples: Vec<String> = Vec::new();
    let mut owners: Vec<&str> = by_owner.keys().copied().collect();
    owners.sort();
    for owner in owners {
        let attackers = &by_owner[owner];
        let targets: Vec<&Unit> = board
            .units
            .iter()
            .filter(|u| u.owner.as_str() != owner)
            .filter(|u| {
                unit_def_on_own_tile(board, u)
                    .map(|d| d > 0.0)
                    .unwrap_or(false)
            })
            .take(TARGET_WINDOW)
            .collect();
        for a in attackers.iter().take(ATTACKERS_PER_OWNER) {
            for i in 0..targets.len() {
                for j in (i + 1)..targets.len() {
                    let (ta, tb) = (targets[i], targets[j]);
                    let (Some(oa), Some(ob)) = (old_axes(board, a, ta), old_axes(board, a, tb))
                    else {
                        continue;
                    };
                    let (Some(na), Some(nb)) = (
                        rules::attack_axes(board, a, ta),
                        rules::attack_axes(board, a, tb),
                    ) else {
                        continue;
                    };
                    total += 1;
                    let old = attack_verdict(&oa, &ob);
                    let new = attack_verdict(&na, &nb);
                    if old != new {
                        changed += 1;
                        if examples.len() < 3 && old != "dropped" && new != "dropped" {
                            examples.push(format!(
                                "    {} strikes [{} @({},{}) def_eff={:.1} hp={}] vs [{} @({},{}) \
                                 def_eff={:.1} hp={}]: OLD ratio-fav {:.2}/{:.2} -> {}, NEW win-prob \
                                 {:.2}/{:.2} -> {}",
                                a.kind,
                                ta.kind, ta.x, ta.y, unit_def_on_own_tile(board, ta).unwrap_or(0.0),
                                ta.hp, tb.kind, tb.x, tb.y,
                                unit_def_on_own_tile(board, tb).unwrap_or(0.0), tb.hp,
                                oa.favorability, ob.favorability, old, na.favorability,
                                nb.favorability, new,
                            ));
                        }
                    }
                }
            }
        }
    }
    println!(
        "\ncompare-two-attacks: {changed}/{total} verdicts changed ({:.1}%)",
        pct(changed, total)
    );
    for e in &examples {
        println!("{e}");
    }
}

// --------------------------------------------------------------------------
// cf-vacate: old (scalar incoming vs remaining def, 1.25×) vs new (win-prob band)
// --------------------------------------------------------------------------

/// OLD cf-vacate verdict: "yes" / "no" / "dropped", using the faded scalar incoming force and the
/// 1.25× ratio against the remaining (second-best) defender.
fn old_vacate(board: &Board, city: &City, field: &ThreatField) -> &'static str {
    let (with_best, without_best) = city_defense_vacated(board, city);
    if with_best <= EPS {
        return "dropped";
    }
    let incoming = field.at(city.x, city.y);
    if incoming <= EPS {
        return "dropped";
    }
    if with_best < MARGIN * incoming {
        return "dropped";
    }
    if without_best >= MARGIN * incoming {
        "yes"
    } else if incoming >= MARGIN * without_best {
        "no"
    } else {
        "dropped"
    }
}

/// NEW cf-vacate verdict (the win-probability solver, verbatim from `question.rs`).
fn new_vacate(board: &Board, city: &City, field: &ThreatField) -> &'static str {
    let ranked = defenders_ranked(board, city);
    let Some(&(best_u, best_def)) = ranked.first() else {
        return "dropped";
    };
    if field.attacker_at(city.x, city.y).is_none() {
        return "dropped";
    }
    if city_fall_prob(field, city.x, city.y, best_u, best_def) > WIN_PROB_LOW {
        return "dropped";
    }
    let fall_without = match ranked.get(1) {
        Some(&(u2, d2)) => city_fall_prob(field, city.x, city.y, u2, d2),
        None => city_fall_prob_undefended(field, city.x, city.y),
    };
    if fall_without <= WIN_PROB_LOW {
        "yes"
    } else if fall_without >= WIN_PROB_HIGH {
        "no"
    } else {
        "dropped"
    }
}

fn cf_vacate_divergence(board: &Board) {
    let mut fields: HashMap<&str, ThreatField> = HashMap::new();
    let mut total = 0usize;
    let mut changed = 0usize;
    let mut examples: Vec<String> = Vec::new();
    for city in &board.cities {
        let field = fields
            .entry(city.owner.as_str())
            .or_insert_with(|| ThreatField::compute(board, &city.owner));
        let old = old_vacate(board, city, field);
        let new = new_vacate(board, city, field);
        // Only count cities that at least one model would keep (else it is not a cf-vacate instance).
        if old == "dropped" && new == "dropped" {
            continue;
        }
        total += 1;
        if old != new {
            changed += 1;
            if examples.len() < 3 {
                let (wb, wob) = city_defense_vacated(board, city);
                examples.push(format!(
                    "    {} @({},{}): def with/without best {:.1}/{:.1}, incoming(scalar)={:.2} -> \
                     OLD {}, NEW {}",
                    city.name, city.x, city.y, wb, wob, field.at(city.x, city.y), old, new
                ));
            }
        }
    }
    println!(
        "\ncf-vacate: {changed}/{total} verdicts changed ({:.1}%)",
        pct(changed, total)
    );
    for e in &examples {
        println!("{e}");
    }
}

// --------------------------------------------------------------------------
// capture-state (the core of triage-reinforce): old ratio vs new win-prob band
// --------------------------------------------------------------------------

fn old_capture(incoming: f64, defense: f64) -> &'static str {
    if incoming <= EPS {
        return "held";
    }
    if defense >= MARGIN * incoming {
        "held"
    } else if incoming >= MARGIN * defense {
        "capturable"
    } else {
        "contested"
    }
}

fn new_capture(fall_prob: f64) -> &'static str {
    if fall_prob >= WIN_PROB_HIGH {
        "capturable"
    } else if fall_prob <= WIN_PROB_LOW {
        "held"
    } else {
        "contested"
    }
}

fn capture_state_divergence(board: &Board) {
    let mut fields: HashMap<&str, ThreatField> = HashMap::new();
    let mut total = 0usize;
    let mut changed = 0usize;
    let mut examples: Vec<String> = Vec::new();
    for city in &board.cities {
        let field = fields
            .entry(city.owner.as_str())
            .or_insert_with(|| ThreatField::compute(board, &city.owner));
        // Only cities under a live threat exercise the capture judgment.
        let Some(_) = field.attacker_at(city.x, city.y) else {
            continue;
        };
        let incoming = field.at(city.x, city.y);
        let def = city_defense(board, city);
        let old = old_capture(incoming, def);
        let new = match rules::best_defender(board, city) {
            Some((u, d)) => new_capture(city_fall_prob(field, city.x, city.y, u, d)),
            None => new_capture(city_fall_prob_undefended(field, city.x, city.y)),
        };
        total += 1;
        if old != new {
            changed += 1;
            if examples.len() < 3 {
                examples.push(format!(
                    "    {} @({},{}): incoming(scalar)={:.2}, defense(def_eff)={:.1} -> OLD {}, \
                     NEW {}",
                    city.name, city.x, city.y, incoming, def, old, new
                ));
            }
        }
    }
    println!(
        "\ncapture-state (triage core): {changed}/{total} classifications changed ({:.1}%)",
        pct(changed, total)
    );
    for e in &examples {
        println!("{e}");
    }
}

fn pct(a: usize, b: usize) -> f64 {
    if b == 0 {
        0.0
    } else {
        100.0 * a as f64 / b as f64
    }
}
