//! Solver verification on a hand-built mini-board — the first of the two verification levels
//! in DESIGN.md §6 (this pins the ground truth itself; the Oracle-100% test pins the plumbing).

use std::collections::BTreeSet;

use civ_core::board::{Board, Tile};
use civ_core::Dir8;
use civ_eval::encoders::Referent;
use civ_eval::question::{solve, Answer, Question};

/// 3x3 board:
///   (0,0)Grassland+River  (1,0)Forest         (2,0)Grassland
///   (0,1)Grassland        (1,1)Grassland       (2,1)Grassland
///   (0,2)Grassland        (1,2)Grassland       (2,2)Mountains
fn mini() -> Board {
    let mut tiles = Vec::new();
    for y in 0..3 {
        let mut row = Vec::new();
        for x in 0..3 {
            let terrain = match (x, y) {
                (1, 0) => "Forest",
                (2, 2) => "Mountains",
                _ => "Grassland",
            };
            let mut extras = BTreeSet::new();
            if (x, y) == (0, 0) {
                extras.insert("River".to_string());
            }
            row.push(Tile {
                x,
                y,
                terrain: terrain.to_string(),
                extras,
                owner: None,
            });
        }
        tiles.push(row);
    }
    Board {
        width: 3,
        height: 3,
        tiles,
        cities: vec![],
        units: vec![],
        players: vec![],
        known: std::collections::HashMap::new(),
        visibility: None,
        ruleset: None,
        turn: None,
        source: "mini".to_string(),
    }
}

fn tile(x: i32, y: i32) -> Referent {
    Referent::Tile { x, y }
}

#[test]
fn terrain_at() {
    let b = mini();
    match solve(&Question::TerrainAt { tile: tile(1, 0) }, &b).unwrap() {
        Answer::Choice { value, .. } => assert_eq!(value, "Forest"),
        other => panic!("expected Choice, got {other:?}"),
    }
}

#[test]
fn adjacent_terrain() {
    let b = mini();
    let q = Question::AdjacentTerrain {
        origin: tile(1, 1),
        dir: Dir8::N,
    };
    match solve(&q, &b).unwrap() {
        Answer::Choice { value, .. } => assert_eq!(value, "Forest"),
        other => panic!("got {other:?}"),
    }
    // Off the north edge -> ill-posed.
    let oob = Question::AdjacentTerrain {
        origin: tile(1, 0),
        dir: Dir8::N,
    };
    assert!(solve(&oob, &b).is_none());
}

#[test]
fn distance_and_direction() {
    let b = mini();
    assert_eq!(
        solve(
            &Question::Distance {
                a: tile(0, 0),
                b: tile(2, 2)
            },
            &b
        )
        .unwrap(),
        Answer::Int(2)
    );
    assert_eq!(
        solve(
            &Question::DirectionTo {
                origin: tile(0, 0),
                target: tile(2, 2)
            },
            &b
        )
        .unwrap(),
        Answer::Direction(Dir8::SE)
    );
}

#[test]
fn count_terrain_in_radius() {
    let b = mini();
    // radius 1 around center covers the whole 3x3; Grassland = 9 - Forest - Mountains = 7.
    let q = Question::CountTerrainInRadius {
        center: tile(1, 1),
        radius: 1,
        terrain: "Grassland".to_string(),
    };
    assert_eq!(solve(&q, &b).unwrap(), Answer::Int(7));
}

#[test]
fn nearest_resource() {
    let b = mini();
    // River is at (0,0); nearest from (2,2) is Chebyshev 2. A resource absent -> ill-posed.
    let q = Question::NearestResource {
        from: tile(2, 2),
        resource: "River".to_string(),
    };
    assert_eq!(solve(&q, &b).unwrap(), Answer::Int(2));
    let absent = Question::NearestResource {
        from: tile(0, 0),
        resource: "Gold".to_string(),
    };
    assert!(solve(&absent, &b).is_none());
}

#[test]
fn reachability_respects_budget_and_avoid() {
    let b = mini();
    // (0,0) -> (2,0) in 2 steps along the north edge, avoiding Mountains: reachable.
    let ok = Question::Reachable {
        from: tile(0, 0),
        to: tile(2, 0),
        budget: 2,
        avoid: "Mountains".to_string(),
        mover: None,
    };
    assert_eq!(solve(&ok, &b).unwrap(), Answer::Bool(true));
    // Same goal, budget 1: not reachable.
    let tight = Question::Reachable {
        from: tile(0, 0),
        to: tile(2, 0),
        budget: 1,
        avoid: "Mountains".to_string(),
        mover: None,
    };
    assert_eq!(solve(&tight, &b).unwrap(), Answer::Bool(false));
    // Goal is a Mountains tile -> cannot end there.
    let onto_avoid = Question::Reachable {
        from: tile(1, 1),
        to: tile(2, 2),
        budget: 5,
        avoid: "Mountains".to_string(),
        mover: None,
    };
    assert_eq!(solve(&onto_avoid, &b).unwrap(), Answer::Bool(false));
}
