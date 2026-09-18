//! Fog-of-war: per-player known-map decoding and `Board::mask_to_known` against the real T50 save.

use civ_core::{parse_save, Board};

fn t50() -> Board {
    parse_save(format!(
        "{}/../data/saves/myagent_T50.sav",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("parse T50 save")
}

fn known_frac(b: &Board, id: i32) -> f64 {
    let g = &b.known[&id];
    let known: usize = g.iter().flatten().filter(|&&k| k).count();
    known as f64 / (b.width * b.height) as f64
}

#[test]
fn parses_per_player_known_maps() {
    let b = t50();
    assert!(
        b.known.contains_key(&0) && b.known.contains_key(&1),
        "both known maps decoded"
    );
    // Early game: player0 has seen very little, player1 a bit more (measured ~3.0% / ~8.5%).
    let (f0, f1) = (known_frac(&b, 0), known_frac(&b, 1));
    assert!(f0 > 0.0 && f0 < 0.10, "player0 known fraction {f0}");
    assert!(f1 > f0 && f1 < 0.20, "player1 known fraction {f1}");
}

#[test]
fn mask_hides_unexplored_and_shrinks_the_world() {
    let b = t50();
    let m = b.mask_to_known(0).expect("mask to player 0");
    assert_eq!(
        (m.width, m.height),
        (b.width, b.height),
        "dimensions preserved"
    );

    let total = (b.width * b.height) as usize;
    let unknown = m
        .tiles
        .iter()
        .flatten()
        .filter(|t| t.terrain == "Unknown")
        .count();
    assert!(
        unknown as f64 / total as f64 > 0.90,
        "mostly Unknown, got {unknown}/{total}"
    );

    // Extras (resources/features) collapse to only those on explored tiles.
    let extras = |bd: &Board| {
        bd.tiles
            .iter()
            .flatten()
            .filter(|t| !t.extras.is_empty())
            .count()
    };
    assert!(
        extras(&m) < extras(&b) / 3,
        "masked extras {} vs true {}",
        extras(&m),
        extras(&b)
    );

    // A player always sees its own city (it sits on an explored tile).
    assert!(
        !m.cities.is_empty(),
        "player0's own city must remain visible"
    );
    // Every surviving occupant stands on an explored (non-Unknown) tile.
    assert!(
        m.cities
            .iter()
            .all(|c| m.tiles[c.y as usize][c.x as usize].terrain != "Unknown")
            && m.units
                .iter()
                .all(|u| m.tiles[u.y as usize][u.x as usize].terrain != "Unknown"),
        "no occupant should sit on an Unknown tile"
    );

    // The masked view carries no further per-player maps, and masking an absent player errors.
    assert!(m.known.is_empty());
    assert!(b.mask_to_known(99).is_err(), "unknown player id must error");
}

// --------------------------------------------------------------------------
// three-state fog: visibility geometry + fogged/visible split
// --------------------------------------------------------------------------

use civ_core::{sq_map_distance, unit_vision_radius_sq, CITY_VISION_RADIUS_SQ};
use civ_core::{City, Player, Tile, Unit, Visibility};
use std::collections::{BTreeSet, HashMap};

/// A synthetic land board (1 row of `w` Grassland tiles) with a given known grid and occupants, so
/// visibility can be controlled exactly.
fn land_row(w: i32, known_true_upto_x: i32, cities: Vec<City>, units: Vec<Unit>) -> Board {
    let tiles: Vec<Vec<Tile>> = vec![(0..w)
        .map(|x| Tile {
            x,
            y: 0,
            terrain: "Grassland".to_string(),
            extras: BTreeSet::new(),
            owner: None,
        })
        .collect()];
    // Player 0 = self ("A"), player 1 = enemy ("B"). `known[0][0][x]` true for x < known_true_upto_x.
    let known_row: Vec<bool> = (0..w).map(|x| x < known_true_upto_x).collect();
    let mut known = HashMap::new();
    known.insert(0, vec![known_row]);
    Board {
        width: w,
        height: 1,
        tiles,
        cities,
        units,
        players: vec![
            Player {
                id: 0,
                name: "A".into(),
                nation: "Rome".into(),
                is_alive: true,
            },
            Player {
                id: 1,
                name: "B".into(),
                nation: "Greece".into(),
                is_alive: true,
            },
        ],
        known,
        visibility: None,
        ruleset: Some("classic".into()),
        turn: Some(1),
        source: "fogtest".into(),
    }
}

fn unit(x: i32, id: i32, kind: &str, owner: &str) -> Unit {
    Unit {
        x,
        y: 0,
        id,
        kind: kind.into(),
        owner: owner.into(),
        veteran: 0,
        hp: 10,
    }
}
fn city(x: i32, id: i32, owner: &str) -> City {
    City {
        x,
        y: 0,
        id,
        name: format!("C{id}"),
        owner: owner.into(),
        size: 5,
        improvements: BTreeSet::new(),
    }
}

#[test]
fn vision_uses_squared_euclidean_freeciv_metric() {
    // City base vision_radius_sq = 5 → sees x with x*x <= 5 → x in {0,1,2}; x=3 (sqdist 9) is out.
    assert_eq!(CITY_VISION_RADIUS_SQ, 5);
    assert_eq!(sq_map_distance((0, 0), (2, 0)), 4); // <= 5, visible
    assert_eq!(sq_map_distance((0, 0), (3, 0)), 9); // > 5, not visible
                                                    // Land unit vision_radius_sq = 2 → 3x3 block; a fast/air unit sees 8.
    assert_eq!(unit_vision_radius_sq("Warriors"), 2);
    assert_eq!(unit_vision_radius_sq("Explorer"), 2); // classic Explorer is 2, NOT wide-vision
    assert_eq!(unit_vision_radius_sq("Fighter"), 8);
    assert_eq!(unit_vision_radius_sq("AWACS"), 26);
}

#[test]
fn own_city_visible_disk_matches_radius_sq() {
    // Self city at (0,0); everything explored. Visible iff x*x <= 5.
    let b = land_row(10, 10, vec![city(0, 1, "A")], vec![]);
    let vis = b.visible_grid(0).expect("visible grid");
    for x in 0..10 {
        let want = (x * x) <= CITY_VISION_RADIUS_SQ;
        assert_eq!(vis[0][x as usize], want, "x={x} visibility");
    }
}

#[test]
fn three_state_split_drops_fogged_enemy_units_keeps_terrain_and_city() {
    // Self city at (0,0) (sees x=0,1,2). Enemy unit at (5,0) and enemy city at (7,0): both on
    // explored-but-unwatched (FOGGED) tiles. Tiles x=8,9 are never explored (UNEXPLORED).
    let b = land_row(
        10,
        8, // known for x in 0..8; x=8,9 unexplored
        vec![city(0, 1, "A"), city(7, 2, "B")],
        vec![unit(1, 10, "Warriors", "A"), unit(5, 20, "Warriors", "B")],
    );
    let m = b.mask_to_known(0).expect("mask");

    // States.
    assert_eq!(m.visibility_at(1, 0), Visibility::Visible);
    assert_eq!(m.visibility_at(5, 0), Visibility::Fogged);
    assert_eq!(m.visibility_at(7, 0), Visibility::Fogged);
    assert_eq!(m.visibility_at(9, 0), Visibility::Unexplored);

    // Visible ⊆ Known: every visible tile is explored.
    for x in 0..10 {
        if matches!(m.visibility_at(x, 0), Visibility::Visible) {
            assert!(b.known[&0][0][x as usize], "visible tile {x} must be known");
        }
    }

    // Fogged terrain is REMEMBERED (not Unknown); unexplored collapses to Unknown.
    assert_eq!(m.tile(5, 0).terrain, "Grassland");
    assert_eq!(m.tile(9, 0).terrain, "Unknown");

    // The hidden ENEMY unit on the fogged tile is dropped; the OWN unit (visible) survives.
    assert!(m.units.iter().any(|u| u.id == 10), "own unit kept");
    assert!(
        !m.units.iter().any(|u| u.id == 20),
        "fogged enemy unit must be hidden"
    );
    // The enemy CITY on a fogged tile is kept at last-known state.
    assert!(
        m.cities.iter().any(|c| c.id == 2 && c.owner == "B"),
        "fogged enemy city kept"
    );
    assert!(
        m.cities.iter().any(|c| c.id == 1 && c.owner == "A"),
        "own city kept"
    );
}

#[test]
fn enemy_unit_becomes_visible_when_in_sight() {
    // Same board but the enemy unit stands at (2,0) — inside the self city's sight → shown.
    let b = land_row(
        10,
        8,
        vec![city(0, 1, "A")],
        vec![unit(2, 20, "Warriors", "B")],
    );
    let m = b.mask_to_known(0).expect("mask");
    assert_eq!(m.visibility_at(2, 0), Visibility::Visible);
    assert!(
        m.units.iter().any(|u| u.id == 20),
        "a visible enemy unit must be shown"
    );
}

#[test]
fn no_reach_around_no_hidden_enemy_survives_the_mask() {
    // The load-bearing invariant: after masking, NO enemy unit sits on a fogged/unexplored tile.
    let b = land_row(
        10,
        8,
        vec![city(0, 1, "A")],
        vec![
            unit(1, 10, "Warriors", "A"),
            unit(2, 21, "Warriors", "B"), // visible enemy → kept
            unit(5, 22, "Warriors", "B"), // fogged enemy → dropped
        ],
    );
    let m = b.mask_to_known(0).expect("mask");
    for u in &m.units {
        if u.owner != "A" {
            assert_eq!(
                m.visibility_at(u.x, u.y),
                Visibility::Visible,
                "every surviving enemy unit must be on a currently-visible tile"
            );
        }
    }
}

#[test]
fn fog_perspective_validity() {
    let live_civ = Player {
        id: 0,
        name: "A".into(),
        nation: "Rome".into(),
        is_alive: true,
    };
    let dead_civ = Player {
        id: 1,
        name: "B".into(),
        nation: "Rome".into(),
        is_alive: false,
    };
    let live_pirate = Player {
        id: 2,
        name: "P".into(),
        nation: "Pirate".into(),
        is_alive: true,
    };
    let live_barb = Player {
        id: 3,
        name: "X".into(),
        nation: "Barbarian".into(),
        is_alive: true,
    };
    assert!(live_civ.is_valid_fog_perspective());
    assert!(!dead_civ.is_valid_fog_perspective());
    assert!(!live_pirate.is_valid_fog_perspective());
    assert!(!live_barb.is_valid_fog_perspective());
}
