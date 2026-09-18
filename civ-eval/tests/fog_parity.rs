//! Three-state fog parity: the encoder (model view) and the solvers (ground truth) read the SAME
//! masked board, so a fogged enemy is invisible to BOTH. These tests prove the "mask upstream,
//! nothing reaches around it" seam — and that fog actually CHANGES the answer (not a trivial pass).

use std::collections::{BTreeSet, HashMap};

use civ_core::{Board, City, Player, Tile, Unit, Visibility};
use civ_eval::encoders::{Encoder, RawEncoder};
use civ_eval::rules::{city_defense, ThreatField};
use civ_eval::{score, Answer, Status};

/// A land row (`w` Grassland tiles), everything explored by self (player 0 "A"), plus occupants.
fn land_row(w: i32, cities: Vec<City>, units: Vec<Unit>) -> Board {
    let tiles: Vec<Vec<Tile>> = vec![(0..w)
        .map(|x| Tile {
            x,
            y: 0,
            terrain: "Grassland".to_string(),
            extras: BTreeSet::new(),
            owner: None,
        })
        .collect()];
    let mut known = HashMap::new();
    known.insert(0, vec![vec![true; w as usize]]); // self explored the whole row
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
        source: "fogparity".into(),
    }
}

fn warriors(x: i32, id: i32, owner: &str) -> Unit {
    Unit {
        x,
        y: 0,
        id,
        kind: "Warriors".into(),
        owner: owner.into(),
        veteran: 0,
        hp: 10,
    }
}
fn own_city(x: i32, id: i32) -> City {
    City {
        x,
        y: 0,
        id,
        name: format!("C{id}"),
        owner: "A".into(),
        size: 5,
        improvements: BTreeSet::new(),
    }
}

/// Self city at (0,0) (vision_radius_sq 5 → sees x≤2). An enemy Warriors placed at (3,0) is FOGGED
/// (explored, out of sight); at (2,0) it is VISIBLE. Assert that BOTH the encoder AND the ThreatField
/// solver ignore the fogged enemy, and both see it once it steps into sight — fog moves the answer.
#[test]
fn fogged_enemy_is_hidden_from_encoder_and_threatfield_alike() {
    // --- FOGGED: enemy at (3,0), out of the city's sight ---
    let fogged = land_row(6, vec![own_city(0, 1)], vec![warriors(3, 20, "B")])
        .mask_to_known(0)
        .expect("mask");
    assert_eq!(fogged.visibility_at(3, 0), Visibility::Fogged);

    // (a) The encoder (model view) mentions NO unit on the fogged tile, and tags it (fogged).
    let render = RawEncoder.render(&fogged);
    let line = render
        .lines()
        .find(|l| l.starts_with("(3, 0):"))
        .expect("fogged tile is listed");
    assert!(
        !line.contains("{unit"),
        "no unit rendered on fogged tile: {line:?}"
    );
    assert!(line.contains("(fogged)"), "fogged tile is tagged: {line:?}");
    // And the run's board list omits the hidden enemy entirely.
    assert!(
        !fogged.units.iter().any(|u| u.owner == "B"),
        "no enemy in masked board.units"
    );

    // (b) The ground-truth solver ALSO sees no threat from it (threat 0 at the city).
    let tf_fog = ThreatField::compute(&fogged, "A");
    assert_eq!(tf_fog.at(0, 0), 0.0, "fogged enemy raises no threat");

    // --- VISIBLE: same enemy stepped to (2,0), inside the city's sight ---
    let visible = land_row(6, vec![own_city(0, 1)], vec![warriors(2, 20, "B")])
        .mask_to_known(0)
        .expect("mask");
    assert_eq!(visible.visibility_at(2, 0), Visibility::Visible);
    assert!(
        visible.units.iter().any(|u| u.owner == "B"),
        "visible enemy kept"
    );
    let tf_vis = ThreatField::compute(&visible, "A");
    assert!(
        tf_vis.at(0, 0) > 0.0,
        "a visible enemy adjacent-reachable to the city DOES raise threat — fog changed the answer"
    );
}

/// Cross-check against the omniscient (unmasked) board: the SAME enemy at (3,0) is a real threat when
/// fog is off, confirming the fogged-case zero is caused by masking, not by the geometry being inert.
#[test]
fn unmasked_board_sees_the_same_enemy_as_a_threat() {
    let omniscient = land_row(6, vec![own_city(0, 1)], vec![warriors(3, 20, "B")]);
    let tf = ThreatField::compute(&omniscient, "A");
    assert!(
        tf.at(0, 0) > 0.0,
        "with no fog, the enemy at (3,0) threatens the city — so the fogged zero is the mask's doing"
    );
}

/// Negative oracle test for a fog-affected kind (`city-defense`). A fogged enemy city keeps its
/// last-known footprint but its GARRISON is hidden, so its defense reads 0 ("undefended as far as
/// you can see"); the unmasked truth shows a real defender. Then the scorer must REJECT the pre-fog
/// judgment: naming the (secretly-defended) enemy city as the better-defended one is *wrong* under
/// the fog-correct answer.
#[test]
fn city_defense_of_fogged_enemy_is_zero_and_scorer_rejects_the_prefog_pick() {
    let enemy = City {
        x: 7,
        y: 0,
        id: 2,
        name: "Bcity".into(),
        owner: "B".into(),
        size: 5,
        improvements: BTreeSet::new(),
    };
    // Own city at (0,0); enemy city at (7,0) with a Warriors garrison sitting on it (a FOGGED tile).
    let base = land_row(
        8,
        vec![own_city(0, 1), enemy],
        vec![warriors(0, 10, "A"), warriors(7, 30, "B")],
    );
    let masked = base.mask_to_known(0).expect("mask");
    assert_eq!(masked.visibility_at(7, 0), Visibility::Fogged);

    let enemy_masked = masked
        .cities
        .iter()
        .find(|c| c.id == 2)
        .expect("enemy city kept last-known");
    let enemy_true = base.cities.iter().find(|c| c.id == 2).unwrap();
    // Fog-correct: garrison hidden → 0. Ground truth: a real defender → positive. The answer MOVED.
    assert_eq!(
        city_defense(&masked, enemy_masked),
        0.0,
        "fogged garrison is hidden"
    );
    assert!(
        city_defense(&base, enemy_true) > 0.0,
        "true board shows the defender"
    );

    // Fog-correct city-defense comparison: own city (>0) out-defends the fogged enemy city (0).
    let answer = Answer::Choice {
        value: "the city \"C1\"".into(),
        options: vec!["the city \"C1\"".into(), "the city \"Bcity\"".into()],
    };
    assert_eq!(
        score(&answer, "Answer: the city \"C1\"").status,
        Status::Correct
    );
    // Naming the secretly-defended enemy city (the pre-fog intuition) is scored WRONG.
    assert_eq!(
        score(&answer, "Answer: the city \"Bcity\"").status,
        Status::Wrong,
        "the scorer must reject the pre-fog pick on a fog-affected kind"
    );
}
