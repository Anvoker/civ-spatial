//! Mini-board unit tests for the T2 valuation solvers (DESIGN.md §6 "verify a question type"
//! level 1: pin the ground truth on a hand-built board). Complements the end-to-end
//! Oracle-100% invariant in `oracle_invariant.rs`.

use std::collections::BTreeSet;

use civ_core::{Board, City, Player, Tile, Unit};
use civ_eval::question::{solve, Answer, Question};
use civ_eval::rules::{self, StrengthAxis};
use civ_eval::Referent;

// --- hand-built board helpers ---------------------------------------------

fn board(w: i32, h: i32, default_terrain: &str) -> Board {
    let tiles = (0..h)
        .map(|y| {
            (0..w)
                .map(|x| Tile {
                    x,
                    y,
                    terrain: default_terrain.to_string(),
                    extras: BTreeSet::new(),
                    owner: None,
                })
                .collect()
        })
        .collect();
    Board {
        width: w,
        height: h,
        tiles,
        known: std::collections::HashMap::new(),
        visibility: None,
        ruleset: None,
        turn: None,
        cities: Vec::new(),
        units: Vec::new(),
        players: vec![
            Player {
                id: 0,
                name: "P0".into(),
                nation: "A".into(),
                is_alive: true,
            },
            Player {
                id: 1,
                name: "P1".into(),
                nation: "B".into(),
                is_alive: true,
            },
        ],
        source: "t2-mini".into(),
    }
}

fn set_terrain(b: &mut Board, x: i32, y: i32, terrain: &str) {
    b.tiles[y as usize][x as usize].terrain = terrain.to_string();
}

fn add_extra(b: &mut Board, x: i32, y: i32, extra: &str) {
    b.tiles[y as usize][x as usize]
        .extras
        .insert(extra.to_string());
}

fn unit(id: i32, x: i32, y: i32, kind: &str, owner: &str, veteran: i32, hp: i32) -> Unit {
    Unit {
        x,
        y,
        id,
        kind: kind.into(),
        owner: owner.into(),
        veteran,
        hp,
    }
}

fn city(id: i32, x: i32, y: i32, name: &str, owner: &str) -> City {
    City {
        x,
        y,
        id,
        name: name.into(),
        owner: owner.into(),
        size: 1,
        improvements: BTreeSet::new(),
    }
}

fn walled_city(id: i32, x: i32, y: i32, name: &str, owner: &str) -> City {
    let mut c = city(id, x, y, name, owner);
    c.improvements.insert("City Walls".into());
    c
}

fn city_with(id: i32, x: i32, y: i32, name: &str, owner: &str, improvement: &str) -> City {
    let mut c = city(id, x, y, name, owner);
    c.improvements.insert(improvement.into());
    c
}

fn strength_q(a: i32, b: i32, axis: StrengthAxis) -> Question {
    Question::UnitStrengthCompare {
        a: Referent::Unit { id: a },
        b: Referent::Unit { id: b },
        axis,
    }
}

fn choice(value: &str, options: &[&str]) -> Answer {
    Answer::Choice {
        value: value.to_string(),
        options: options.iter().map(|s| s.to_string()).collect(),
    }
}

// --- T2a: unit strength ----------------------------------------------------

#[test]
fn warriors_vs_armor_known_ordering() {
    let mut b = board(5, 5, "Grassland");
    b.units.push(unit(1, 0, 0, "Warriors", "P0", 0, 10));
    b.units.push(unit(2, 1, 1, "Armor", "P0", 0, 30));

    // Attack: Armor 10 vs Warriors 1 -> Armor.
    let a = solve(&strength_q(1, 2, StrengthAxis::Attack), &b).unwrap();
    assert_eq!(
        a,
        choice(
            "Armor at (1, 1)",
            &["Warriors at (0, 0)", "Armor at (1, 1)"]
        )
    );

    // Defense: Armor 5 vs Warriors 1 -> Armor (order of referents flipped).
    let d = solve(&strength_q(2, 1, StrengthAxis::Defense), &b).unwrap();
    assert_eq!(
        d,
        choice(
            "Armor at (1, 1)",
            &["Armor at (1, 1)", "Warriors at (0, 0)"]
        )
    );
}

#[test]
fn veteran_level_decides() {
    // Two identical Warriors; only veteran level differs. Elite ×2.0 beats green ×1.0 on defense.
    let mut b = board(5, 5, "Grassland");
    b.units.push(unit(1, 0, 0, "Warriors", "P0", 0, 10)); // green   def_eff 1.0
    b.units.push(unit(2, 2, 2, "Warriors", "P0", 3, 10)); // elite   def_eff 2.0

    let d = solve(&strength_q(1, 2, StrengthAxis::Defense), &b).unwrap();
    assert_eq!(
        d,
        choice(
            "Warriors at (2, 2)",
            &["Warriors at (0, 0)", "Warriors at (2, 2)"]
        )
    );

    // Sanity: the rules module agrees on the underlying numbers.
    assert_eq!(rules::def_eff_base(&b.units[0]).unwrap(), 1.0);
    assert_eq!(rules::def_eff_base(&b.units[1]).unwrap(), 2.0);
}

#[test]
fn damaged_unit_health_scales_strength() {
    // Armor at 9/30 hp -> health 0.3 -> att_eff 10 * 0.3 = 3.0. A full Warriors is 1.0, so the
    // damaged Armor still wins decisively (3.0 >= 1.25 * 1.0).
    let mut b = board(5, 5, "Grassland");
    b.units.push(unit(1, 0, 0, "Armor", "P0", 0, 9)); // damaged
    b.units.push(unit(2, 1, 1, "Warriors", "P0", 0, 10)); // full

    assert!((rules::att_eff(&b.units[0]).unwrap() - 3.0).abs() < 1e-9);

    let a = solve(&strength_q(1, 2, StrengthAxis::Attack), &b).unwrap();
    assert_eq!(
        a,
        choice(
            "Armor at (0, 0)",
            &["Armor at (0, 0)", "Warriors at (1, 1)"]
        )
    );

    // Against a full Chariot (att 3.0), the damaged Armor (3.0) ties -> dropped as not decisive.
    b.units.push(unit(3, 2, 2, "Chariot", "P0", 0, 10));
    assert!(solve(&strength_q(1, 3, StrengthAxis::Attack), &b).is_none());
}

#[test]
fn near_tie_is_dropped() {
    // Mech. Inf. def 6 vs Armor def 5 -> ratio 1.2 < 1.25 -> not decisive.
    let mut b = board(5, 5, "Grassland");
    b.units.push(unit(1, 0, 0, "Mech. Inf.", "P0", 0, 30));
    b.units.push(unit(2, 1, 1, "Armor", "P0", 0, 30));
    assert!(solve(&strength_q(1, 2, StrengthAxis::Defense), &b).is_none());
}

// --- T2b: city defense -----------------------------------------------------

#[test]
fn city_on_hills_beats_open_ground() {
    // Both cities garrisoned by a green Warriors (def 1). Hills city: 1*(1+1.0+0.5)=2.5;
    // open city: 1*(1+0.5)=1.5. 2.5/1.5 = 1.667 -> decisive, Hills wins.
    let mut b = board(6, 6, "Grassland");
    set_terrain(&mut b, 1, 1, "Hills");
    b.cities.push(city(10, 1, 1, "Highton", "P0"));
    b.cities.push(city(11, 3, 3, "Flatton", "P0"));
    b.units.push(unit(1, 1, 1, "Warriors", "P0", 0, 10));
    b.units.push(unit(2, 3, 3, "Warriors", "P0", 0, 10));

    let q = Question::CityDefenseCompare {
        a: Referent::City { id: 10 },
        b: Referent::City { id: 11 },
    };
    assert_eq!(
        solve(&q, &b).unwrap(),
        choice("Highton", &["Highton", "Flatton"])
    );
    assert_eq!(rules::city_defense(&b, &b.cities[0]), 2.5);
    assert_eq!(rules::city_defense(&b, &b.cities[1]), 1.5);
}

#[test]
fn defended_beats_undefended() {
    let mut b = board(6, 6, "Grassland");
    b.cities.push(city(10, 1, 1, "Garrison", "P0"));
    b.cities.push(city(11, 4, 4, "Empty", "P0"));
    b.units.push(unit(1, 1, 1, "Warriors", "P0", 0, 10)); // defends Garrison
                                                          // Empty has no defender -> defense 0.
    assert_eq!(rules::city_defense(&b, &b.cities[1]), 0.0);

    let q = Question::CityDefenseCompare {
        a: Referent::City { id: 11 },
        b: Referent::City { id: 10 },
    };
    assert_eq!(
        solve(&q, &b).unwrap(),
        choice("Garrison", &["Empty", "Garrison"])
    );
}

#[test]
fn both_undefended_is_dropped() {
    let mut b = board(6, 6, "Grassland");
    b.cities.push(city(10, 1, 1, "A", "P0"));
    b.cities.push(city(11, 4, 4, "B", "P0"));
    let q = Question::CityDefenseCompare {
        a: Referent::City { id: 10 },
        b: Referent::City { id: 11 },
    };
    assert!(solve(&q, &b).is_none());
}

#[test]
fn fortress_lifts_open_city_to_a_tie_and_drops() {
    // Flatton gains a Fortress: 1*(1 + 0.5 + 1.0) = 2.5, tying the Hills city -> dropped.
    let mut b = board(6, 6, "Grassland");
    set_terrain(&mut b, 1, 1, "Hills");
    add_extra(&mut b, 3, 3, "Fortress");
    b.cities.push(city(10, 1, 1, "Highton", "P0"));
    b.cities.push(city(11, 3, 3, "Flatton", "P0"));
    b.units.push(unit(1, 1, 1, "Warriors", "P0", 0, 10));
    b.units.push(unit(2, 3, 3, "Warriors", "P0", 0, 10));

    assert_eq!(rules::city_defense(&b, &b.cities[1]), 2.5);
    let q = Question::CityDefenseCompare {
        a: Referent::City { id: 10 },
        b: Referent::City { id: 11 },
    };
    assert!(solve(&q, &b).is_none());
}

#[test]
fn walled_city_beats_identical_unwalled() {
    // Two identical cities on identical open ground, each with a green Warriors (def 1).
    // Walled: 1*(1 + 0.5 + 1.0) = 2.5; unwalled: 1*(1 + 0.5) = 1.5. 2.5/1.5 = 1.667 -> decisive.
    let mut b = board(6, 6, "Grassland");
    b.cities.push(walled_city(10, 1, 1, "Bastion", "P0"));
    b.cities.push(city(11, 4, 4, "Hamlet", "P0"));
    b.units.push(unit(1, 1, 1, "Warriors", "P0", 0, 10));
    b.units.push(unit(2, 4, 4, "Warriors", "P0", 0, 10));

    assert_eq!(rules::city_defense(&b, &b.cities[0]), 2.5); // walled
    assert_eq!(rules::city_defense(&b, &b.cities[1]), 1.5); // unwalled

    let q = Question::CityDefenseCompare {
        a: Referent::City { id: 11 },
        b: Referent::City { id: 10 },
    };
    assert_eq!(
        solve(&q, &b).unwrap(),
        choice("Bastion", &["Hamlet", "Bastion"])
    );
}

#[test]
fn great_wall_walls_all_owner_cities() {
    // P0 owns the Great Wall (built in city 20). Its *other* city (id 10) carries no City Walls
    // improvement of its own, yet counts as walled -> 2.5 vs an enemy unwalled city's 1.5.
    let mut b = board(8, 8, "Grassland");
    b.cities
        .push(city_with(20, 6, 6, "WonderCap", "P0", "Great Wall"));
    b.cities.push(city(10, 1, 1, "Outpost", "P0")); // no walls improvement of its own
    b.cities.push(city(11, 4, 4, "EnemyTown", "P1")); // enemy, unwalled
    b.units.push(unit(1, 1, 1, "Warriors", "P0", 0, 10));
    b.units.push(unit(2, 4, 4, "Warriors", "P1", 0, 10));

    // Outpost is walled by the Great Wall despite lacking its own City Walls.
    assert!(rules::city_is_walled(&b, &b.cities[1]));
    assert_eq!(rules::city_defense(&b, &b.cities[1]), 2.5); // Outpost, walled by Great Wall
                                                            // EnemyTown's owner has no Great Wall -> unwalled.
    assert!(!rules::city_is_walled(&b, &b.cities[2]));
    assert_eq!(rules::city_defense(&b, &b.cities[2]), 1.5);

    let q = Question::CityDefenseCompare {
        a: Referent::City { id: 10 },
        b: Referent::City { id: 11 },
    };
    assert_eq!(
        solve(&q, &b).unwrap(),
        choice("Outpost", &["Outpost", "EnemyTown"])
    );
}

#[test]
fn enemy_unit_does_not_defend_city() {
    // A stacked enemy unit is not a defender; the city stays undefended.
    let mut b = board(6, 6, "Grassland");
    b.cities.push(city(10, 1, 1, "Mine", "P0"));
    b.units.push(unit(1, 1, 1, "Armor", "P1", 0, 30)); // enemy-owned
    assert_eq!(rules::city_defense(&b, &b.cities[0]), 0.0);
}

// --- rules_block prose -----------------------------------------------------

#[test]
fn rules_block_lists_present_types_and_tables() {
    let mut b = board(4, 4, "Grassland");
    b.units.push(unit(1, 0, 0, "Warriors", "P0", 0, 10));
    b.units.push(unit(2, 1, 1, "Armor", "P0", 0, 30));
    let rb = rules::rules_block(&b);
    assert!(rb.contains("att_eff"));
    assert!(rb.contains("def_eff"));
    assert!(rb.contains("Warriors"));
    assert!(rb.contains("Armor"));
    assert!(rb.contains("elite")); // veteran table
    assert!(rb.contains("Hills")); // terrain table
                                   // Only board-present types are emitted.
    assert!(!rb.contains("Battleship"));
}
