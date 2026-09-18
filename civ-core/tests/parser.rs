//! Parser sanity checks against the T50 save — hand-verified facts read directly out of the
//! raw `.sav` ("sanity-check a few tiles by hand"). If the parser regresses, these pinpoint it.

use std::sync::OnceLock;

use civ_core::{parse_save, Board};

fn board() -> &'static Board {
    static BOARD: OnceLock<Board> = OnceLock::new();
    BOARD.get_or_init(|| {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../data/saves/myagent_T50.sav");
        parse_save(path).expect("parse T50 save")
    })
}

#[test]
fn dimensions() {
    let b = board();
    assert_eq!(b.width, 78);
    assert_eq!(b.height, 52);
    assert_eq!(b.tiles.len(), 52);
    assert!(b.tiles.iter().all(|r| r.len() == 78));
}

#[test]
fn poles_are_glacier() {
    // t0000 is all 'a' (Glacier); the north edge is entirely glacier.
    let b = board();
    assert!((0..b.width).all(|x| b.tile(x, 0).terrain == "Glacier"));
}

#[test]
fn ocean_decodes_from_space_and_colon() {
    // t0004 begins "::    gp..." -> (0,4),(1,4) Deep Ocean (':'); (2,4) Ocean (space).
    let b = board();
    assert_eq!(b.tile(0, 4).terrain, "Deep Ocean");
    assert_eq!(b.tile(1, 4).terrain, "Deep Ocean");
    assert_eq!(b.tile(2, 4).terrain, "Ocean");
}

#[test]
fn known_terrain_samples() {
    // t0003 = "     gg ..." -> (5,3),(6,3) Grassland.
    let b = board();
    assert_eq!(b.tile(5, 3).terrain, "Grassland");
    assert_eq!(b.tile(6, 3).terrain, "Grassland");
    assert!(b.iter_tiles().any(|t| t.terrain == "Mountains"));
}

#[test]
fn rivers_decode_from_bitplane() {
    // e03 packs (Railroad, River, Gold, Iron); River = bit value 2. Row y=9 has several.
    let b = board();
    let rivers: Vec<_> = b.iter_tiles().filter(|t| t.has("River")).collect();
    assert!(!rivers.is_empty(), "expected River extras to decode");
    assert!(rivers.iter().any(|t| t.y == 9));
}

#[test]
fn players() {
    let b = board();
    let names: std::collections::BTreeSet<_> = b.players.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Myagent", "Myagent1"].into_iter().collect());
    assert_eq!(b.player("Myagent").unwrap().nation, "Thracian");
}

#[test]
fn cities_use_yx_order() {
    // player0 city row "35,33,120,...,"COMMANDER"" -> y=35, x=33.
    let b = board();
    let mine = b.cities_of("Myagent");
    assert_eq!(mine.len(), 1);
    let c = mine[0];
    assert_eq!((c.x, c.y), (33, 35));
    assert_eq!(c.id, 120);
    assert_eq!(c.size, 12);
    assert_eq!(c.name, "COMMANDER");
    assert_eq!(b.cities_of("Myagent1").len(), 5);
}

#[test]
fn units_with_kind_and_hp() {
    // player0: Armor @ (24,25), Mech. Inf. @ (31,26), Chariot @ (36,27); each hp 30.
    let b = board();
    let by_pos: std::collections::HashMap<(i32, i32), &civ_core::Unit> = b
        .units_of("Myagent")
        .into_iter()
        .map(|u| ((u.x, u.y), u))
        .collect();
    assert_eq!(by_pos[&(24, 25)].kind, "Armor");
    assert_eq!(by_pos[&(31, 26)].kind, "Mech. Inf.");
    assert_eq!(by_pos[&(36, 27)].kind, "Chariot");
    assert_eq!(by_pos[&(24, 25)].hp, 30);
}

#[test]
fn cities_sit_on_land() {
    let b = board();
    for c in &b.cities {
        let t = b.tile(c.x, c.y);
        assert!(
            t.terrain != "Ocean" && t.terrain != "Deep Ocean",
            "city {} on {}",
            c.name,
            t.terrain
        );
    }
}

#[test]
fn crop_rebases_and_filters() {
    let b = board();
    // Crop a window that contains the player0 capital at (33,35).
    let c = b.crop(30, 32, 8, 8).expect("crop");
    assert_eq!((c.width, c.height), (8, 8));
    assert_eq!(c.tiles.len(), 8);
    // The capital moves from (33,35) to (3,3) and terrain is preserved.
    let cap = c.cities_of("Myagent");
    assert_eq!(cap.len(), 1);
    assert_eq!((cap[0].x, cap[0].y), (3, 3));
    assert_eq!(c.tile(3, 3).terrain, b.tile(33, 35).terrain);
    // A tile outside the window is gone; counts only include in-window objects.
    assert!(c.cities.iter().all(|city| c.in_bounds(city.x, city.y)));
    // Out-of-bounds crop is rejected.
    assert!(b.crop(75, 0, 10, 10).is_err());
}

#[test]
fn ownership_decoded() {
    // owner0007 has "1" around cols 26-31 -> tiles owned by Myagent1.
    let b = board();
    assert!(b
        .iter_tiles()
        .any(|t| t.owner.as_deref() == Some("Myagent1")));
}

#[test]
fn improvements_decode_palace_and_no_walls() {
    // player0 city "COMMANDER" has improvements bit 21 (Palace) set and nothing else. Verified
    // by hand from the raw bitstring "...010..." (idx 21 = Palace) against improvement_vector.
    let b = board();
    let commander = b.cities.iter().find(|c| c.name == "COMMANDER").unwrap();
    assert!(commander.has_palace(), "COMMANDER should hold the Palace");
    assert!(!commander.has_walls(), "COMMANDER has no City Walls");
    assert!(!commander.has_coastal_defense());
    // T50 has NO walled cities and NO Great Wall anywhere.
    assert!(
        b.cities.iter().all(|c| !c.has_walls()),
        "T50 has no walled cities"
    );
    assert!(b.cities.iter().all(|c| !c.has("Great Wall")));
    // capital_of resolves the true capital via the Palace marker.
    assert_eq!(b.capital_of("Myagent").unwrap().name, "COMMANDER");
}

/// Synthetic minimal save exercising the `improvements` decode on a *walled* city (real T50 has
/// none). Bit order comes from `improvement_vector`; the bitstring is one char per index.
#[test]
fn improvements_decode_synthetic_walled_city() {
    let save = "\
[savefile]
improvement_vector=\"City Walls\",\"Coastal Defense\",\"Palace\",\"Great Wall\"
extras_vector=\"River\"
terrident={\"name\",\"identifier\"
\"Grassland\",\"g\"
}
[map]
t0000=\"gg\"
[player0]
name=\"Alice\"
c={\"x\",\"y\",\"id\",\"name\",\"size\",\"improvements\"
0,0,5,\"Fortburg\",3,\"1011\"
1,0,6,\"Openton\",2,\"0000\"
}
";
    let b = civ_core::parse_str(save, "synthetic").expect("parse synthetic save");
    let fort = b.cities.iter().find(|c| c.name == "Fortburg").unwrap();
    // bits 0,2,3 set -> City Walls, Palace, Great Wall (not Coastal Defense at bit 1).
    assert!(fort.has_walls());
    assert!(fort.has_palace());
    assert!(fort.has("Great Wall"));
    assert!(!fort.has_coastal_defense());
    let open = b.cities.iter().find(|c| c.name == "Openton").unwrap();
    assert!(open.improvements.is_empty());
    assert!(!open.has_walls());
    // Palace resolves the capital even against a larger non-Palace city.
    assert_eq!(b.capital_of("Alice").unwrap().name, "Fortburg");
}
