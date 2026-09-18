//! Divergence harness: NEW classic-Dijkstra `ReachField` vs the OLD uniform 8-dir land BFS, on the
//! real boards. Reproduces the numbers in `analysis/findings-movement-dijkstra.md`.
//!
//! Run with:  `cargo test -p civ-eval --test reach_divergence -- --ignored --nocapture`
//! Ignored by default so it never runs in `just check` (it is analysis, not a gate).

use std::collections::VecDeque;

use civ_core::{parse_save, Board};
use civ_eval::rules::ReachField;

/// The OLD reachability model, verbatim: uniform 1 move-point per land tile, 8-connected, ocean
/// impassable, `turns = ceil(steps / move_rate)`, bounded at `horizon`.
fn old_turns(board: &Board, origins: &[(i32, i32)], move_rate: i32, horizon: i32) -> Vec<i32> {
    let w = board.width;
    let h = board.height;
    let mv = move_rate.max(1);
    let max_steps = horizon.max(0) * mv;
    let idx = |x: i32, y: i32| (y * w + x) as usize;
    let mut dist = vec![-1i32; (w * h) as usize];
    let mut turns = vec![-1i32; (w * h) as usize];
    let mut q = VecDeque::new();
    for &(ox, oy) in origins {
        if board.in_bounds(ox, oy) && dist[idx(ox, oy)] == -1 {
            dist[idx(ox, oy)] = 0;
            turns[idx(ox, oy)] = 0;
            q.push_back((ox, oy));
        }
    }
    while let Some((x, y)) = q.pop_front() {
        let d = dist[idx(x, y)];
        if d >= max_steps {
            continue;
        }
        for (_, (nx, ny)) in civ_core::geometry::neighbors(board, x, y) {
            if is_land(&board.tile(nx, ny).terrain) {
                let i = idx(nx, ny);
                if dist[i] == -1 {
                    dist[i] = d + 1;
                    turns[i] = (d + 1 + mv - 1) / mv;
                    q.push_back((nx, ny));
                }
            }
        }
    }
    turns
}

fn is_land(t: &str) -> bool {
    !matches!(t, "Ocean" | "Deep Ocean" | "Lake" | "Unknown")
}

const RESOURCE_EXTRAS: &[&str] = &[
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

fn cheb(a: (i32, i32), b: (i32, i32)) -> i32 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

/// Compare the per-tile turn fields (multi-source from `origins`, move 1, horizon 6) and print the
/// divergence census + a few concrete drop examples.
fn nearest_owned_divergence(board: &Board, name: &str) {
    let w = board.width;
    let h = board.height;
    // Group cities by owner.
    let mut owners: Vec<String> = board.cities.iter().map(|c| c.owner.clone()).collect();
    owners.sort();
    owners.dedup();

    let mut union_reach = 0usize; // tiles reachable by either field
    let mut changed = 0usize; // turn count differs
    let mut dropped = 0usize; // new < old
    let mut raised = 0usize; // new > old
    let mut new_reach_old_not = 0usize; // rail/road opened a tile beyond the old horizon
    let mut examples: Vec<String> = Vec::new();

    for owner in &owners {
        let cities: Vec<(i32, i32)> = board
            .cities
            .iter()
            .filter(|c| &c.owner == owner)
            .map(|c| (c.x, c.y))
            .collect();
        if cities.is_empty() {
            continue;
        }
        let old = old_turns(board, &cities, 1, 6);
        let new = ReachField::from_origins(board, &cities, 1, 6);
        for y in 0..h {
            for x in 0..w {
                let o = old[(y * w + x) as usize];
                let n = new.turns_at(x, y);
                let n = n.unwrap_or(-1);
                if o < 0 && n < 0 {
                    continue;
                }
                union_reach += 1;
                if o != n {
                    changed += 1;
                    if n >= 0 && (o < 0 || n < o) {
                        dropped += 1;
                        if o < 0 {
                            new_reach_old_not += 1;
                        }
                        // Collect a few road/rail-driven examples on resource tiles.
                        let t = board.tile(x, y);
                        let is_res = t
                            .extras
                            .iter()
                            .any(|e| RESOURCE_EXTRAS.contains(&e.as_str()));
                        let outside_work =
                            cities.iter().map(|&c| cheb((x, y), c)).min().unwrap_or(9) > 2;
                        if is_res && outside_work && examples.len() < 6 {
                            let infra: Vec<&str> = ["Railroad", "Road", "River"]
                                .iter()
                                .filter(|k| t.has(k))
                                .copied()
                                .collect();
                            examples.push(format!(
                                "  ({x},{y}) owner={owner} terrain={} res+infra={:?}/{:?}: old={} new={}",
                                t.terrain,
                                t.extras.iter().filter(|e| RESOURCE_EXTRAS.contains(&e.as_str())).collect::<Vec<_>>(),
                                infra,
                                if o < 0 { "unreach".into() } else { o.to_string() },
                                n
                            ));
                        }
                    } else if o >= 0 && (n < 0 || n > o) {
                        raised += 1;
                    }
                }
            }
        }
    }
    println!("== nearest-owned (move 1, horizon 6) on {name} ==");
    println!(
        "  owners={} union-reachable tiles={union_reach}  changed={changed} ({:.1}%)  dropped(new<old)={dropped}  raised(new>old)={raised}  newly-opened-by-road/rail={new_reach_old_not}",
        owners.len(),
        100.0 * changed as f64 / union_reach.max(1) as f64
    );
    // Per-(owner,resource) ANSWER change: min turns to an unexploited resource of that type.
    let mut ans_total = 0usize;
    let mut ans_changed = 0usize;
    let mut new_hist = [0usize; 8]; // index 0..6 = turns, 7 = "none"
    let resources: Vec<String> = {
        let mut present: std::collections::BTreeSet<String> = Default::default();
        for t in board.iter_tiles() {
            for e in &t.extras {
                if RESOURCE_EXTRAS.contains(&e.as_str()) {
                    present.insert(e.clone());
                }
            }
        }
        present.into_iter().collect()
    };
    for owner in &owners {
        let cities: Vec<(i32, i32)> = board
            .cities
            .iter()
            .filter(|c| &c.owner == owner)
            .map(|c| (c.x, c.y))
            .collect();
        if cities.is_empty() {
            continue;
        }
        let old = old_turns(board, &cities, 1, 6);
        let new = ReachField::from_origins(board, &cities, 1, 6);
        for res in &resources {
            let targets: Vec<(i32, i32)> = board
                .iter_tiles()
                .filter(|t| t.has(res))
                .filter(|t| {
                    cities
                        .iter()
                        .map(|&c| cheb((t.x, t.y), c))
                        .min()
                        .unwrap_or(0)
                        > 2
                })
                .map(|t| (t.x, t.y))
                .collect();
            if targets.is_empty() {
                continue;
            }
            let old_ans = targets
                .iter()
                .filter_map(|&(x, y)| {
                    let v = old[(y * w + x) as usize];
                    (v >= 0).then_some(v)
                })
                .min();
            let new_ans = targets
                .iter()
                .filter_map(|&(x, y)| new.turns_at(x, y))
                .min();
            ans_total += 1;
            if old_ans != new_ans {
                ans_changed += 1;
            }
            match new_ans {
                Some(v) if (0..=6).contains(&v) => new_hist[v as usize] += 1,
                _ => new_hist[7] += 1,
            }
        }
    }
    println!(
        "  nearest-owned ANSWERS: {ans_changed}/{ans_total} (owner×resource) instances change value"
    );
    println!(
        "  new-answer value histogram [0,1,2,3,4,5,6,none] = {new_hist:?}  (trivial 0/1 = {})",
        new_hist[0] + new_hist[1]
    );
    for e in &examples {
        println!("{e}");
    }
    println!();
}

/// reachable-nearest divergence: each modeled LAND unit's own-move-rate field, move-rate & horizon 6.
fn reachable_nearest_divergence(board: &Board, name: &str) {
    let w = board.width;
    let h = board.height;
    let mut union_reach = 0usize;
    let mut changed = 0usize;
    let mut dropped = 0usize;
    let mut raised = 0usize;
    let mut units = 0usize;
    // A minimal move-rate table for the land types present (mirrors rules::unit_stat move_rate).
    let mv_of = |kind: &str| -> Option<i32> {
        match kind {
            "Warriors" | "Phalanx" | "Legion" | "Musketeers" | "Cannon" | "Riflemen"
            | "Alpine Troops" | "Archers" | "Pikemen" | "Catapult" | "Settlers" | "Workers"
            | "Engineers" | "Migrants" => Some(1),
            "Chariot" | "Cavalry" | "Horsemen" | "Knights" | "Dragoons" | "Explorer" => Some(2),
            "Armor" | "Mech. Inf." | "Partisan" => Some(3),
            _ => None,
        }
    };
    for u in &board.units {
        let Some(mv) = mv_of(&u.kind) else { continue };
        if mv > 1 {
            // fast units are where terrain (not just roads) can also diverge
        }
        units += 1;
        let old = old_turns(board, &[(u.x, u.y)], mv, 6);
        let new = ReachField::from_origins(board, &[(u.x, u.y)], mv, 6);
        for y in 0..h {
            for x in 0..w {
                let o = old[(y * w + x) as usize];
                let n = new.turns_at(x, y).unwrap_or(-1);
                if o < 0 && n < 0 {
                    continue;
                }
                union_reach += 1;
                if o != n {
                    changed += 1;
                    if n >= 0 && (o < 0 || n < o) {
                        dropped += 1;
                    } else {
                        raised += 1;
                    }
                }
            }
        }
    }
    println!("== reachable-nearest (unit move-rate, horizon 6) on {name} ==");
    println!(
        "  land-units={units} union-reachable tiles={union_reach}  changed={changed} ({:.1}%)  dropped(new<old)={dropped}  raised(new>old, terrain cost)={raised}",
        100.0 * changed as f64 / union_reach.max(1) as f64
    );
    println!();
}

fn density(board: &Board, name: &str) {
    let land = board.iter_tiles().filter(|t| is_land(&t.terrain)).count();
    let road = board.iter_tiles().filter(|t| t.has("Road")).count();
    let rail = board.iter_tiles().filter(|t| t.has("Railroad")).count();
    let river = board.iter_tiles().filter(|t| t.has("River")).count();
    println!(
        "== {name}: {}x{}  land={land}  Road={road} ({:.0}% of land)  Railroad={rail} ({:.0}%)  River={river} ({:.0}%) ==",
        board.width, board.height,
        100.0 * road as f64 / land.max(1) as f64,
        100.0 * rail as f64 / land.max(1) as f64,
        100.0 * river as f64 / land.max(1) as f64,
    );
}

#[test]
#[ignore = "analysis harness; run explicitly with --ignored --nocapture"]
fn divergence_report() {
    let base = concat!(env!("CARGO_MANIFEST_DIR"), "/../data/saves/");
    for (file, name) in [
        ("testcontroller_T677.sav", "T677"),
        ("myagent_T50.sav", "T50"),
    ] {
        let b = parse_save(format!("{base}{file}")).expect("parse");
        density(&b, name);
        nearest_owned_divergence(&b, name);
        reachable_nearest_divergence(&b, name);
    }
}
