//! The round-trip reconstruction gate (DESIGN.md §4/§5). The encoding comparison is only valid
//! if every format carries the *same exact* map facts — a lower score must mean "presented
//! worse", never "info missing". The Oracle-100% gate does NOT check this (the Oracle never reads
//! the board block — it looks the answer up), so an information-incomplete format would ship
//! silently. THIS is the real enforcement: render each encoder, decode its own output back, and
//! assert the recovered facts equal the source `Board` for every in-bounds tile. It doubles as
//! encoder-correctness coverage: it catches *wrong* facts, not just missing ones.

use std::collections::{BTreeSet, HashMap};

use civ_core::board::{City, Player, Tile, Unit};
use civ_core::{parse_save, Board};
use civ_eval::{encoder_by_name, Interactive, QueryableSurface, RecoveredTile, STATIC_ENCODERS};

fn save_path() -> String {
    format!(
        "{}/../data/saves/myagent_T50.sav",
        env!("CARGO_MANIFEST_DIR")
    )
}

/// The ground-truth recovered facts for every in-bounds tile of `board`.
fn expected_map(board: &Board) -> HashMap<(i32, i32), RecoveredTile> {
    let mut m = HashMap::new();
    for y in 0..board.height {
        for x in 0..board.width {
            m.insert((x, y), RecoveredTile::expected(board, x, y));
        }
    }
    m
}

/// Render `board` under `encoding`, decode the block back, and assert the recovered facts match
/// the source for EVERY in-bounds tile (terrain exact; extras as a set; occupants/owner exact).
fn assert_round_trips(board: &Board, encoding: &str) {
    let enc = encoder_by_name(encoding).unwrap_or_else(|| panic!("unknown encoder {encoding}"));
    let block = enc.render(board);
    let recovered = enc
        .reconstruct(&block)
        .unwrap_or_else(|e| panic!("[{encoding}] reconstruct failed: {e}"));

    // Exactly one entry per in-bounds tile — no tiles dropped, none invented.
    assert_eq!(
        recovered.len(),
        (board.width * board.height) as usize,
        "[{encoding}] recovered {} tiles, expected {} ({}x{})",
        recovered.len(),
        board.width * board.height,
        board.width,
        board.height,
    );

    let got: HashMap<(i32, i32), RecoveredTile> = recovered.into_iter().collect();
    assert_eq!(
        got.len(),
        (board.width * board.height) as usize,
        "[{encoding}] duplicate coords"
    );

    let expected = expected_map(board);
    for (coord, want) in &expected {
        let have = got
            .get(coord)
            .unwrap_or_else(|| panic!("[{encoding}] missing tile {coord:?}"));
        assert_eq!(
            have, want,
            "[{encoding}] tile {coord:?} mismatch\n  recovered: {have:?}\n  expected:  {want:?}"
        );
    }
}

/// The hard gate on the real board: every registered static encoder must reconstruct T50 exactly.
#[test]
fn all_encoders_round_trip_t50() {
    let board = parse_save(save_path()).expect("parse save");
    for encoding in STATIC_ENCODERS {
        assert_round_trips(&board, encoding);
    }
}

/// The same gate on a three-state FOGGED T50 view: the `(fogged)` honesty marker must round-trip
/// (be stripped back to the same facts), and remembered-terrain / hidden-unit / kept-city masking
/// must leave every encoder information-complete against the MASKED board it renders.
#[test]
fn all_encoders_round_trip_fogged_t50() {
    let board = parse_save(save_path())
        .expect("parse save")
        .mask_to_known(1)
        .expect("fog player 1");
    assert!(board.visibility.is_some(), "board really is fogged");
    for encoding in STATIC_ENCODERS {
        assert_round_trips(&board, encoding);
    }
}

/// A small hand-built board that deliberately stresses the tricky corners of the contract:
/// a multi-word terrain ("Deep Ocean"), a tile with several extras, a city with a spaced name,
/// a unit, tiles whose terrain differs from the omittable default, and — critically — an
/// **owned-but-otherwise-plain default tile** at (4, 0). That last one is the case that would
/// silently drop its territory owner under ASCII (whose glyph grid encodes no owner and whose
/// DETAIL legend, before the `is_notable` fix, ignored owner). The origin/capital sits in the
/// middle so egocentric also exercises West/North (negative) offsets.
fn synthetic() -> Board {
    let default = "Ocean";
    let mut tiles = Vec::new();
    for y in 0..4 {
        let mut row = Vec::new();
        for x in 0..5 {
            row.push(Tile {
                x,
                y,
                terrain: default.to_string(),
                extras: BTreeSet::new(),
                owner: None,
            });
        }
        tiles.push(row);
    }
    let set = |tiles: &mut Vec<Vec<Tile>>,
               x: usize,
               y: usize,
               terrain: &str,
               extras: &[&str],
               owner: Option<&str>| {
        tiles[y][x] = Tile {
            x: x as i32,
            y: y as i32,
            terrain: terrain.to_string(),
            extras: extras.iter().map(|s| s.to_string()).collect(),
            owner: owner.map(str::to_string),
        };
    };
    // Capital tile (origin), owned.
    set(&mut tiles, 2, 2, "Grassland", &[], Some("Alice"));
    // Owned-but-plain non-default terrain, west of origin.
    set(&mut tiles, 1, 2, "Grassland", &[], Some("Alice"));
    // Multi-word terrain, no extras, north of origin.
    set(&mut tiles, 2, 1, "Deep Ocean", &[], None);
    // Non-default terrain with several extras + unit + owner, SE of origin.
    set(
        &mut tiles,
        3,
        3,
        "Forest",
        &["Iron", "River"],
        Some("Alice"),
    );
    // NW corner, non-default terrain with an extra.
    set(&mut tiles, 0, 0, "Plains", &["Wheat"], None);
    // THE critical case: default terrain (Ocean), no extras/city/unit, but OWNED.
    set(&mut tiles, 4, 0, "Ocean", &[], Some("Bob"));

    Board {
        width: 5,
        height: 4,
        tiles,
        known: std::collections::HashMap::new(),
        visibility: None,
        ruleset: None,
        turn: None,
        cities: vec![City {
            x: 2,
            y: 2,
            id: 1,
            name: "Cap City".to_string(),
            owner: "Alice".to_string(),
            size: 7,
            improvements: BTreeSet::new(),
        }],
        units: vec![
            // On the Forest tile (notable anyway).
            Unit {
                x: 3,
                y: 3,
                id: 10,
                kind: "Armor".to_string(),
                owner: "Alice".to_string(),
                veteran: 0,
                hp: 10,
            },
            // A SECOND unit STACKED on the same (3,3) tile — the stacking case: every encoder must
            // render both (each with its id) and recover both, in board order. First-/last-wins
            // rendering would silently drop one; the gate now catches that.
            Unit {
                x: 3,
                y: 3,
                id: 12,
                kind: "Musketeers".to_string(),
                owner: "Bob".to_string(),
                veteran: 0,
                hp: 10,
            },
            // On a DEFAULT-terrain tile — so the tile is listed only because of its occupant.
            Unit {
                x: 4,
                y: 3,
                id: 11,
                kind: "Trireme".to_string(),
                owner: "Bob".to_string(),
                veteran: 0,
                hp: 10,
            },
        ],
        players: vec![
            Player {
                id: 0,
                name: "Alice".to_string(),
                nation: "Rome".to_string(),
                is_alive: true,
            },
            Player {
                id: 1,
                name: "Bob".to_string(),
                nation: "Greece".to_string(),
                is_alive: true,
            },
        ],
        source: "synthetic".to_string(),
    }
}

#[test]
fn all_encoders_round_trip_synthetic() {
    let board = synthetic();
    for encoding in STATIC_ENCODERS {
        assert_round_trips(&board, encoding);
    }
}

/// The interactive surface's completeness (parity) gate: rebuilding the board from ONLY the tool
/// outputs (tiled `scan`) must recover every tile's exact facts. This is the tool-surface analogue
/// of the static reconstruction gate — it guarantees the queryable surface hides no information
/// (the model *could* fetch everything; it just shouldn't need to).
fn assert_surface_parity(board: &Board) {
    let recovered = Interactive
        .reconstruct_via_tools(board)
        .expect("interactive reconstruct_via_tools");
    assert_eq!(
        recovered.len(),
        (board.width * board.height) as usize,
        "interactive recovered {} tiles, expected {}",
        recovered.len(),
        board.width * board.height,
    );
    let got: HashMap<(i32, i32), RecoveredTile> = recovered.into_iter().collect();
    assert_eq!(
        got.len(),
        (board.width * board.height) as usize,
        "interactive duplicate coords"
    );
    for (coord, want) in &expected_map(board) {
        let have = got
            .get(coord)
            .unwrap_or_else(|| panic!("interactive missing tile {coord:?}"));
        assert_eq!(
            have, want,
            "interactive tile {coord:?} mismatch\n  recovered: {have:?}\n  expected:  {want:?}"
        );
    }
}

#[test]
fn interactive_surface_parity_t50() {
    let board = parse_save(save_path()).expect("parse save");
    assert_surface_parity(&board);
}

#[test]
fn interactive_surface_parity_synthetic() {
    assert_surface_parity(&synthetic());
}
