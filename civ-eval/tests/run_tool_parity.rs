//! Parity gate for the viewer tool-playground seam: `civ_eval::run_tool` run against a native
//! `Board` must return byte-identical output to the SAME board round-tripped through the viewer's
//! export-JSON shape and rebuilt with `Board::from_viewer_json`. This proves the browser path (which
//! feeds the wasm shim a serialized board) sees exactly what a model would, for every tool group.

use std::collections::{BTreeSet, HashMap};

use civ_core::{Board, City, Player, Tile, Unit};
use civ_eval::{run_tool, ToolCall};

/// Serialize a `Board` to the viewer export's per-board JSON shape (mirrors
/// `civ-cli::viewer_export::board_contents_json` + the width/height/players envelope, INCLUDING the
/// `improvements` fidelity field). This is exactly what `viewer/src/main.ts` serializes for the
/// current view before calling the wasm `run_tool`.
fn to_viewer_json(b: &Board) -> String {
    let q = |s: &str| {
        let mut out = String::from("\"");
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                _ => out.push(c),
            }
        }
        out.push('"');
        out
    };
    let mut s = format!(
        "{{\"source\":{},\"width\":{},\"height\":{},\"tiles\":[",
        q(&b.source),
        b.width,
        b.height
    );
    let mut first = true;
    for row in &b.tiles {
        for t in row {
            if !first {
                s.push(',');
            }
            first = false;
            let extras: Vec<String> = t.extras.iter().map(|e| q(e)).collect();
            let owner = t
                .owner
                .as_deref()
                .map(q)
                .unwrap_or_else(|| "null".to_string());
            s.push_str(&format!(
                "{{\"x\":{},\"y\":{},\"terrain\":{},\"extras\":[{}],\"owner\":{}}}",
                t.x,
                t.y,
                q(&t.terrain),
                extras.join(","),
                owner
            ));
        }
    }
    s.push_str("],\"cities\":[");
    for (i, c) in b.cities.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let imps: Vec<String> = c.improvements.iter().map(|m| q(m)).collect();
        s.push_str(&format!(
            "{{\"x\":{},\"y\":{},\"id\":{},\"name\":{},\"owner\":{},\"size\":{},\"improvements\":[{}]}}",
            c.x,
            c.y,
            c.id,
            q(&c.name),
            q(&c.owner),
            c.size,
            imps.join(",")
        ));
    }
    s.push_str("],\"units\":[");
    for (i, u) in b.units.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "{{\"x\":{},\"y\":{},\"id\":{},\"kind\":{},\"owner\":{},\"veteran\":{},\"hp\":{}}}",
            u.x,
            u.y,
            u.id,
            q(&u.kind),
            q(&u.owner),
            u.veteran,
            u.hp
        ));
    }
    s.push_str("],\"players\":[");
    for (i, p) in b.players.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "{{\"name\":{},\"nation\":{}}}",
            q(&p.name),
            q(&p.nation)
        ));
    }
    s.push_str("]}");
    s
}

fn tile(x: i32, y: i32, terrain: &str, extras: &[&str], owner: Option<&str>) -> Tile {
    Tile {
        x,
        y,
        terrain: terrain.to_string(),
        extras: extras.iter().map(|s| s.to_string()).collect(),
        owner: owner.map(str::to_string),
    }
}

/// A small mixed board: two players, a walled city, land + a strip of ocean, a couple of units, a
/// resource tile — enough to exercise every tool group non-trivially.
fn fixture() -> Board {
    let w = 8;
    let h = 6;
    let mut tiles = Vec::new();
    for y in 0..h {
        let mut row = Vec::new();
        for x in 0..w {
            // A column of ocean on the east edge (coastal access); hills patch; a forest patch.
            let terrain = if x == 7 {
                "Ocean"
            } else if (x, y) == (2, 2) || (x, y) == (3, 2) {
                "Hills"
            } else if (x, y) == (5, 4) {
                "Forest"
            } else {
                "Grassland"
            };
            let owner = if x <= 2 {
                Some("Alice")
            } else if (5..=6).contains(&x) {
                Some("Bob")
            } else {
                None
            };
            let extras: &[&str] = if (x, y) == (5, 4) { &["Wheat"] } else { &[] };
            row.push(tile(x, y, terrain, extras, owner));
        }
        tiles.push(row);
    }
    Board {
        width: w,
        height: h,
        tiles,
        cities: vec![
            City {
                x: 1,
                y: 1,
                id: 10,
                name: "Aptown".to_string(),
                owner: "Alice".to_string(),
                size: 6,
                improvements: ["City Walls".to_string(), "Palace".to_string()]
                    .into_iter()
                    .collect(),
            },
            City {
                x: 6,
                y: 4,
                id: 20,
                name: "Bobville".to_string(),
                owner: "Bob".to_string(),
                size: 4,
                improvements: BTreeSet::new(),
            },
        ],
        units: vec![
            // A garrison standing IN the walled city (1,1) so city_defense is non-zero and the wall
            // bonus is observable.
            Unit {
                x: 1,
                y: 1,
                id: 101,
                kind: "Phalanx".to_string(),
                owner: "Alice".to_string(),
                veteran: 0,
                hp: 10,
            },
            Unit {
                x: 2,
                y: 2,
                id: 100,
                kind: "Phalanx".to_string(),
                owner: "Alice".to_string(),
                veteran: 1,
                hp: 10,
            },
            Unit {
                x: 4,
                y: 3,
                id: 200,
                kind: "Legion".to_string(),
                owner: "Bob".to_string(),
                veteran: 0,
                hp: 10,
            },
            Unit {
                x: 6,
                y: 4,
                id: 201,
                kind: "Musketeers".to_string(),
                owner: "Bob".to_string(),
                veteran: 0,
                hp: 10,
            },
        ],
        players: vec![
            Player {
                id: 0,
                name: "Alice".to_string(),
                nation: "Romans".to_string(),
                is_alive: true,
            },
            Player {
                id: 1,
                name: "Bob".to_string(),
                nation: "Greeks".to_string(),
                is_alive: true,
            },
        ],
        known: HashMap::new(),
        visibility: None,
        ruleset: Some("classic".to_string()),
        turn: Some(42),
        source: "fixture.sav".to_string(),
    }
}

fn call(name: &str, args: &str) -> ToolCall {
    ToolCall {
        id: String::new(),
        name: name.to_string(),
        args_json: args.to_string(),
    }
}

#[test]
fn every_tool_group_round_trips_identically() {
    let native = fixture();
    let json = to_viewer_json(&native);
    let rebuilt = Board::from_viewer_json(&json).expect("from_viewer_json");

    // One representative (and a couple of edge) call per tool across all four dispatch groups.
    let calls = vec![
        // operators
        call("distance", "{\"x0\":0,\"y0\":0,\"x1\":3,\"y1\":4}"),
        call("travel_turns", "{\"x0\":0,\"y0\":0,\"x1\":6,\"y1\":5}"),
        call(
            "count",
            "{\"x0\":0,\"y0\":0,\"x1\":7,\"y1\":5,\"kind\":\"terrain\",\"feature\":\"Grassland\"}",
        ),
        call(
            "count",
            "{\"x0\":0,\"y0\":0,\"x1\":7,\"y1\":5,\"kind\":\"resource\",\"feature\":\"Wheat\"}",
        ),
        // maximal operators
        call("att_eff", "{\"unit\":200}"),
        call("def_eff", "{\"unit\":100}"),
        call("def_eff", "{\"unit\":100,\"bare\":true}"),
        call("win_prob", "{\"attacker\":200,\"defender\":100}"),
        call("city_defense", "{\"x\":1,\"y\":1}"),
        call("city_fall_prob", "{\"x\":1,\"y\":1,\"player\":\"Bob\"}"),
        call("garrison_defense", "{\"unit\":201,\"cx\":6,\"cy\":4}"),
        call("threat", "{\"x\":1,\"y\":1,\"player\":\"Alice\"}"),
        call("site_axes", "{\"x\":4,\"y\":1,\"player\":\"Alice\"}"),
        call("tile_cover", "{\"x\":2,\"y\":2,\"player\":\"Alice\"}"),
        call("support", "{\"x\":6,\"y\":4,\"player\":\"Bob\"}"),
        call("reach_turns", "{\"unit\":100,\"tx\":6,\"ty\":5}"),
        call("site_check", "{\"x\":2,\"y\":2,\"player\":\"Alice\"}"),
        // enumeration operators
        call(
            "list_tiles",
            "{\"x0\":0,\"y0\":0,\"x1\":7,\"y1\":5,\"terrain\":\"Grassland\"}",
        ),
        call("list_owned_cities", "{\"player\":\"Alice\"}"),
        call("list_owned_units", "{\"player\":\"Bob\"}"),
        call(
            "travel_turns_from_owned",
            "{\"player\":\"Alice\",\"tx\":6,\"ty\":5}",
        ),
        // interactive / fetch verbs
        call("region_summary", "{\"x\":3,\"y\":2}"),
        call("scan_region", "{\"x\":3,\"y\":2}"),
        call("scan", "{\"x0\":0,\"y0\":0,\"x1\":7,\"y1\":5}"),
        call("scan_grid", "{\"x0\":0,\"y0\":0,\"x1\":7,\"y1\":5}"),
        call("get_tile", "{\"x\":5,\"y\":4}"),
        call("list_cities", "{}"),
        call("list_units", "{}"),
        // unknown tool -> terminal error string
        call("no_such_tool", "{}"),
    ];

    for c in &calls {
        let a = run_tool(&native, c);
        let b = run_tool(&rebuilt, c);
        assert_eq!(
            a, b,
            "tool {:?} diverged after round-trip:\n native: {a}\n rebuilt: {b}",
            c.name
        );
        assert!(!a.is_empty(), "tool {:?} returned empty", c.name);
    }

    // Spot-check a couple of exact values so the test also pins real outputs (not just equality).
    assert_eq!(
        run_tool(
            &native,
            &call("distance", "{\"x0\":0,\"y0\":0,\"x1\":3,\"y1\":4}")
        ),
        "4"
    );
    assert_eq!(
        run_tool(
            &native,
            &call(
                "count",
                "{\"x0\":0,\"y0\":0,\"x1\":7,\"y1\":5,\"kind\":\"resource\",\"feature\":\"Wheat\"}"
            )
        ),
        "1"
    );
    assert!(run_tool(&native, &call("no_such_tool", "{}")).starts_with("error: unknown tool"));
}

#[test]
fn improvements_survive_the_round_trip() {
    // City Walls must survive export->import, or city_defense would under-compute. Compare a walled
    // city's defense between the native board and the rebuilt one, and confirm dropping the wall
    // actually changes the number (so the field is load-bearing, not inert).
    let native = fixture();
    let rebuilt = Board::from_viewer_json(&to_viewer_json(&native)).unwrap();
    let cd = |b: &Board| run_tool(b, &call("city_defense", "{\"x\":1,\"y\":1}"));
    assert_eq!(cd(&native), cd(&rebuilt));

    let mut no_walls = native.clone();
    no_walls.cities[0].improvements.clear();
    assert_ne!(
        cd(&native),
        cd(&no_walls),
        "city_defense should differ with vs without City Walls (the improvements field is load-bearing)"
    );
}
