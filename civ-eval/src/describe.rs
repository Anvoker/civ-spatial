//! Deterministic, LLM-free qualitative descriptions of a board region.
//!
//! "Compute exactly, describe coarsely" for the SPATIAL facts: the terrain histogram, water
//! fraction, and spatial concentration are computed exactly but emitted bucketed and map-like — no
//! exact per-terrain count, tile owner, or occupant *coordinate* is given (those stay in the leaves
//! via `scan`/`get_tile`/`scan_grid`). The OCCUPANT and RESOURCE facts, by contrast, are reported
//! exactly: each city by name + owner, units tallied per owner, and a total resource-deposit count —
//! the aggregates a player reasons over ("who is here, how many, how rich") without needing tile
//! positions. This keeps *where* things sit lossy while making *what/whose/how-many* precise. See
//! `interactive-board-access-design.md` §3.
//!
//! Everything here is derived from exact board data, so it is correct by construction; the tests
//! pin the *buckets and phrasing*, not ground-truth accuracy.

use std::collections::HashMap;

use civ_core::Board;

/// Sea/water terrains, for the "sea" phrasing (Freeciv `classic` names).
fn is_sea(terrain: &str) -> bool {
    matches!(terrain, "Ocean" | "Deep Ocean" | "Lake")
}

/// How much occupant/resource detail a region description carries.
///
/// The SPATIAL sentences (terrain character, territory tilt) are identical either way; only the
/// occupant and resource sentences differ.
///   - `Coarse` — the whole-board overview path: cities by name+owner, unit COUNTS per owner, and a
///     per-type resource tally. No coordinates. Kept small so it can summarise huge quadrants in the
///     prompt preamble without dumping hundreds of units.
///   - `CoarseCounts` — the `interactive-maxops` quadrant path: like `Coarse` but occupants are bare
///     COUNTS (`Cities: N, Units: M.`) with NO city names and NO per-owner unit tally — those now
///     live in the always-on occupant roster front-loaded into that surface's overview, so the
///     quadrant block must not re-enumerate them. Terrain, resources and territory tilt are identical
///     to `Coarse`.
///   - `Rich` — the `region_summary` tool path (one 10×10 sector at a time): every city with its
///     position/size/walls, every unit with its position and context-free att/def strength plus a
///     per-owner Σ, and every resource deposit with its position.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Detail {
    Coarse,
    CoarseCounts,
    Rich,
}

/// A coarse qualitative description of the **inclusive** tile box `(x0,y0)-(x1,y1)`. The box is
/// clamped to the board; a degenerate/out-of-bounds box yields a short sentinel. Used by the
/// whole-board overview; occupants/resources are reported as aggregates (no coordinates).
pub fn describe_region(board: &Board, x0: i32, y0: i32, x1: i32, y1: i32) -> String {
    describe_region_detail(board, x0, y0, x1, y1, Detail::Coarse)
}

/// A **rich** description of the inclusive box, for the `region_summary` tool (one sector at a
/// time). Same terrain/territory sentences as [`describe_region`], but every city, unit, and
/// resource deposit is listed with its exact position — units also with their context-free
/// attack/defense strengths and a per-owner Σ. Intentionally longer (more tokens) than the coarse
/// form; there is no length cap.
pub fn describe_region_rich(board: &Board, x0: i32, y0: i32, x1: i32, y1: i32) -> String {
    describe_region_detail(board, x0, y0, x1, y1, Detail::Rich)
}

/// A **counts-only** coarse description of the inclusive box, for the `interactive-maxops` quadrant
/// overview. Same terrain / resource-tally / territory sentences as [`describe_region`], but the
/// occupant sentence is bare COUNTS (`Cities: N, Units: M.`) — no city names, no per-owner unit
/// tally. Those occupants are front-loaded in that surface's always-on roster, so the quadrant block
/// only keeps the aggregate counts.
pub fn describe_region_counts(board: &Board, x0: i32, y0: i32, x1: i32, y1: i32) -> String {
    describe_region_detail(board, x0, y0, x1, y1, Detail::CoarseCounts)
}

fn describe_region_detail(
    board: &Board,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    detail: Detail,
) -> String {
    let x0 = x0.max(0);
    let y0 = y0.max(0);
    let x1 = x1.min(board.width - 1);
    let y1 = y1.min(board.height - 1);
    if x0 > x1 || y0 > y1 {
        return "empty region (out of bounds)".to_string();
    }
    let total = ((x1 - x0 + 1) as i64 * (y1 - y0 + 1) as i64) as f64;

    // Exact terrain histogram (internal only — never emitted as counts).
    let mut terr: HashMap<&str, i32> = HashMap::new();
    let mut sea_tiles = 0i32;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let t = board.tile(x, y).terrain.as_str();
            *terr.entry(t).or_default() += 1;
            if is_sea(t) {
                sea_tiles += 1;
            }
        }
    }
    let mut items: Vec<(&str, i32)> = terr.into_iter().collect();
    items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

    // Sentence 1 — terrain character, plus sea concentration/coverage when non-trivial.
    let mut s1 = capitalize(&terrain_char(&items, total));
    let sea_frac = sea_tiles as f64 / total;
    if sea_frac >= 0.10 {
        let cov = coverage_bucket(sea_frac);
        match concentration(board, x0, y0, x1, y1, is_sea) {
            Some(w) => s1.push_str(&format!(", with sea {w} ({cov} of the region)")),
            None => s1.push_str(&format!(", with sea across {cov} of the region")),
        }
    }
    s1.push('.');

    // Sentence 2 — occupants (cities by name+owner, units per owner). Sentence 3 — resource
    // deposits total. Sentence 4 — territory tilt.
    let mut out = s1;
    let occ = match detail {
        Detail::Coarse => occupants_phrase(board, x0, y0, x1, y1),
        Detail::CoarseCounts => occupants_counts_phrase(board, x0, y0, x1, y1),
        Detail::Rich => occupants_phrase_rich(board, x0, y0, x1, y1),
    };
    if !occ.is_empty() {
        out.push(' ');
        out.push_str(&occ);
    }
    out.push(' ');
    match detail {
        Detail::Coarse | Detail::CoarseCounts => {
            out.push_str(&resources_phrase(board, x0, y0, x1, y1))
        }
        Detail::Rich => out.push_str(&resources_phrase_rich(board, x0, y0, x1, y1)),
    }
    let own = ownership_phrase(board, x0, y0, x1, y1, total);
    if !own.is_empty() {
        out.push(' ');
        out.push_str(&own);
    }
    out
}

/// The leading terrain characterization, using a bucketed dominance quantifier (never counts).
fn terrain_char(items: &[(&str, i32)], total: f64) -> String {
    let (dom, dom_n) = items[0];
    let dom_share = dom_n as f64 / total;
    if items.len() == 1 {
        return format!("entirely {dom}");
    }
    let (second, second_n) = items[1];
    let second_share = second_n as f64 / total;
    if dom_share > 0.65 {
        format!("predominantly {dom}")
    } else if dom_share - second_share < 0.15 {
        format!("a mix of {dom} and {second}")
    } else if dom_share >= 0.45 {
        format!("mostly {dom}")
    } else {
        format!("varied terrain, chiefly {dom}")
    }
}

/// A coarse coverage bucket for a fraction in `0.0..=1.0`.
fn coverage_bucket(frac: f64) -> &'static str {
    if frac < 0.10 {
        "a sliver"
    } else if frac < 0.20 {
        "a small part"
    } else if frac < 0.30 {
        "about a quarter"
    } else if frac < 0.42 {
        "about a third"
    } else if frac < 0.58 {
        "about half"
    } else if frac < 0.80 {
        "most"
    } else {
        "nearly all"
    }
}

/// Where the tiles matching `pred` concentrate within the box, via a 3×3 mass grid. `None` means
/// "spread out" (no single edge/corner holds a strong majority) — the caller phrases that as "across".
fn concentration<F: Fn(&str) -> bool>(
    board: &Board,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    pred: F,
) -> Option<String> {
    let w = x1 - x0 + 1;
    let h = y1 - y0 + 1;
    let mut col = [0i32; 3]; // west, mid, east
    let mut row = [0i32; 3]; // north, mid, south
    let mut tot = 0i32;
    for y in y0..=y1 {
        for x in x0..=x1 {
            if pred(board.tile(x, y).terrain.as_str()) {
                let cx = (((x - x0) * 3) / w).min(2) as usize;
                let cy = (((y - y0) * 3) / h).min(2) as usize;
                col[cx] += 1;
                row[cy] += 1;
                tot += 1;
            }
        }
    }
    if tot == 0 {
        return None;
    }
    let t = tot as f64;
    let strong = 0.55;
    // (compass word for corners, adjective for edges)
    let ew = if col[2] as f64 / t > strong {
        Some(("east", "eastern"))
    } else if col[0] as f64 / t > strong {
        Some(("west", "western"))
    } else {
        None
    };
    let ns = if row[2] as f64 / t > strong {
        Some(("south", "southern"))
    } else if row[0] as f64 / t > strong {
        Some(("north", "northern"))
    } else {
        None
    };
    match (ns, ew) {
        (Some((nsw, _)), Some((ews, _))) => Some(format!("in the {nsw}{ews} corner")),
        (Some((_, nse)), None) => Some(format!("along the {nse} edge")),
        (None, Some((_, ewe))) => Some(format!("along the {ewe} edge")),
        (None, None) => None,
    }
}

/// Occupant summary: every city named with its owner, and units tallied per owner. Exact on
/// identity and counts (what/whose/how-many); still no coordinates — those live in the leaves.
fn occupants_phrase(board: &Board, x0: i32, y0: i32, x1: i32, y1: i32) -> String {
    let inb = |x: i32, y: i32| x >= x0 && x <= x1 && y >= y0 && y <= y1;
    let mut cities: Vec<&civ_core::City> = board.cities.iter().filter(|c| inb(c.x, c.y)).collect();
    let units: Vec<&civ_core::Unit> = board.units.iter().filter(|u| inb(u.x, u.y)).collect();
    if cities.is_empty() && units.is_empty() {
        return "No cities or units.".to_string();
    }
    let mut out = String::new();
    if !cities.is_empty() {
        cities.sort_by_key(|c| (c.y, c.x)); // deterministic reading order (north-to-south)
        let list = cities
            .iter()
            .map(|c| format!("{} (owner {})", c.name, c.owner))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("Cities ({}): {list}.", cities.len()));
    }
    if !units.is_empty() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&format!(
            "Units ({}): {}.",
            units.len(),
            per_owner_counts(&units)
        ));
    }
    out
}

/// Counts-only occupant summary (the `interactive-maxops` quadrant path): just how many cities and
/// units fall in the box — `"Cities: 27, Units: 24."` — with NO names and NO per-owner tally. The
/// identities live in the front-loaded roster, so the quadrant block carries only the aggregate.
fn occupants_counts_phrase(board: &Board, x0: i32, y0: i32, x1: i32, y1: i32) -> String {
    let inb = |x: i32, y: i32| x >= x0 && x <= x1 && y >= y0 && y <= y1;
    let ncities = board.cities.iter().filter(|c| inb(c.x, c.y)).count();
    let nunits = board.units.iter().filter(|u| inb(u.x, u.y)).count();
    if ncities == 0 && nunits == 0 {
        return "No cities or units.".to_string();
    }
    format!("Cities: {ncities}, Units: {nunits}.")
}

/// Units tallied per owner, most first: `"40 owner 0, 30 owner 4, 21 owner 5"`.
fn per_owner_counts(units: &[&civ_core::Unit]) -> String {
    let mut counts: HashMap<&str, i32> = HashMap::new();
    for u in units {
        *counts.entry(u.owner.as_str()).or_default() += 1;
    }
    let mut v: Vec<(&str, i32)> = counts.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    v.iter()
        .map(|(o, n)| format!("{n} owner {o}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Natural-resource deposits in the box, counted PER TYPE (Iron/Oil/Wine/…; excludes Road/Fortress/
/// etc.), using the same resource set as the "nearest resource" questions. Reported most-first with
/// a running total: `"Resource deposits (10): Oil 4, Wine 3, Iron 2, Coal 1."`.
fn resources_phrase(board: &Board, x0: i32, y0: i32, x1: i32, y1: i32) -> String {
    let mut counts: HashMap<&str, i32> = HashMap::new();
    let mut total = 0i32;
    for y in y0..=y1 {
        for x in x0..=x1 {
            for e in &board.tile(x, y).extras {
                if crate::generators::is_resource(e) {
                    *counts.entry(e.as_str()).or_default() += 1;
                    total += 1;
                }
            }
        }
    }
    if total == 0 {
        return "Resource deposits: none.".to_string();
    }
    let mut v: Vec<(&str, i32)> = counts.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let list = v
        .iter()
        .map(|(t, n)| format!("{t} {n}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("Resource deposits ({total}): {list}.")
}

/// Format an attack/defense strength to one decimal (e.g. `4.7`), matching the tool renderer.
fn fmt1(v: f64) -> String {
    format!("{v:.1}")
}

/// RICH occupant summary (the `region_summary` tool). Every city is listed with its position, owner,
/// size, and walls; every unit is listed per owner with its position and context-free att/def
/// strength plus a per-owner Σ over the modeled units. See [`describe_region_rich`].
fn occupants_phrase_rich(board: &Board, x0: i32, y0: i32, x1: i32, y1: i32) -> String {
    let inb = |x: i32, y: i32| x >= x0 && x <= x1 && y >= y0 && y <= y1;
    let mut cities: Vec<&civ_core::City> = board.cities.iter().filter(|c| inb(c.x, c.y)).collect();
    let units: Vec<&civ_core::Unit> = board.units.iter().filter(|u| inb(u.x, u.y)).collect();
    if cities.is_empty() && units.is_empty() {
        return "No cities or units.".to_string();
    }
    let mut out = String::new();

    if !cities.is_empty() {
        cities.sort_by_key(|c| (c.y, c.x)); // deterministic reading order (north-to-south)
        let list = cities
            .iter()
            .map(|c| {
                let mut s = format!(
                    "{} @({},{}) ({}, size {}",
                    c.name, c.x, c.y, c.owner, c.size
                );
                if crate::rules::city_is_walled(board, c) {
                    s.push_str(", walls");
                }
                s.push(')');
                s
            })
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("Cities ({}): {list}.", cities.len()));
    }

    if !units.is_empty() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&format!(
            "Units ({}) — {}.",
            units.len(),
            per_owner_rich(&units)
        ));
    }
    out
}

/// Units grouped per owner for the rich phrasing. Owners are ordered by their Σ attack (desc), then
/// owner name; units within an owner are ordered by (y, x). Unmodeled types (no `att_eff`/
/// `def_eff_base`) render as `att -/def -` and are excluded from the Σ.
fn per_owner_rich(units: &[&civ_core::Unit]) -> String {
    // Bucket unit refs by owner (preserving nothing about input order; we sort explicitly below).
    let mut by_owner: HashMap<&str, Vec<&civ_core::Unit>> = HashMap::new();
    for u in units {
        by_owner.entry(u.owner.as_str()).or_default().push(u);
    }

    // Compute each owner's Σ (over modeled units only) to drive the owner ordering.
    let mut owners: Vec<(&str, Vec<&civ_core::Unit>, f64, f64)> = by_owner
        .into_iter()
        .map(|(owner, mut us)| {
            us.sort_by_key(|u| (u.y, u.x));
            let sum_att: f64 = us.iter().filter_map(|u| crate::rules::att_eff(u)).sum();
            let sum_def: f64 = us
                .iter()
                .filter_map(|u| crate::rules::def_eff_base(u))
                .sum();
            (owner, us, sum_att, sum_def)
        })
        .collect();
    // Σ attack desc, then owner name asc.
    owners.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(b.0))
    });

    owners
        .iter()
        .map(|(owner, us, sum_att, sum_def)| {
            let list = us
                .iter()
                .map(|u| {
                    let att = crate::rules::att_eff(u)
                        .map(fmt1)
                        .unwrap_or_else(|| "-".to_string());
                    let def = crate::rules::def_eff_base(u)
                        .map(fmt1)
                        .unwrap_or_else(|| "-".to_string());
                    format!("{} @({},{}) att {att}/def {def}", u.kind, u.x, u.y)
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{owner}: {list} [Σ att {}, def {}]",
                fmt1(*sum_att),
                fmt1(*sum_def)
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// RICH resource summary (the `region_summary` tool): every deposit listed with its position, in
/// (y, x) reading order, keeping the running total. Uses the same resource set as
/// [`resources_phrase`].
fn resources_phrase_rich(board: &Board, x0: i32, y0: i32, x1: i32, y1: i32) -> String {
    let mut deposits: Vec<(i32, i32, &str)> = Vec::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            for e in &board.tile(x, y).extras {
                if crate::generators::is_resource(e) {
                    deposits.push((y, x, e.as_str()));
                }
            }
        }
    }
    if deposits.is_empty() {
        return "Resource deposits: none.".to_string();
    }
    // Iteration already yields (y, x) order; a stable sort keeps within-tile extras name-ordered.
    deposits.sort_by_key(|d| (d.0, d.1));
    let list = deposits
        .iter()
        .map(|(y, x, t)| format!("{t} @({x},{y})"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("Resource deposits ({}): {list}.", deposits.len())
}

/// Territory inclusion/tilt: whether the region is wholly inside one player's territory
/// ("Entirely"), mostly/partly one player's ("Mostly"/"Partly"), or contested.
fn ownership_phrase(board: &Board, x0: i32, y0: i32, x1: i32, y1: i32, total: f64) -> String {
    let mut counts: HashMap<&str, i32> = HashMap::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            if let Some(o) = &board.tile(x, y).owner {
                *counts.entry(o.as_str()).or_default() += 1;
            }
        }
    }
    let mut owned: Vec<(&str, i32)> = counts.into_iter().collect();
    if owned.is_empty() {
        return "Territory unclaimed.".to_string();
    }
    owned.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let (top, top_n) = owned[0];
    if owned.len() == 1 && top_n as f64 == total {
        // Full inclusion: every tile in the (clamped) region is claimed by this one player.
        format!("Entirely player {top}'s territory.")
    } else if top_n as f64 / total > 0.5 {
        format!("Mostly player {top}'s territory.")
    } else if owned.len() >= 2 {
        let mut names: Vec<&str> = owned.iter().map(|(o, _)| *o).collect();
        names.sort_unstable();
        format!("Contested territory (players {}).", names.join(", "))
    } else {
        format!("Partly player {top}'s territory.")
    }
}

/// Uppercase the first character.
fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use civ_core::{Board, City, Tile, Unit};
    use std::collections::BTreeSet;

    /// Build a board from a grid of single-char terrain codes via a legend; owner grid optional.
    fn mk_board(rows: &[&str], legend: &[(char, &str)]) -> Board {
        let height = rows.len() as i32;
        let width = rows[0].len() as i32;
        let terr = |ch: char| -> String {
            legend
                .iter()
                .find(|(c, _)| *c == ch)
                .map(|(_, t)| t.to_string())
                .unwrap()
        };
        let mut tiles = Vec::new();
        for (y, row) in rows.iter().enumerate() {
            let mut trow = Vec::new();
            for (x, ch) in row.chars().enumerate() {
                trow.push(Tile {
                    x: x as i32,
                    y: y as i32,
                    terrain: terr(ch),
                    extras: BTreeSet::new(),
                    owner: None,
                });
            }
            tiles.push(trow);
        }
        Board {
            width,
            height,
            tiles,
            cities: Vec::new(),
            units: Vec::new(),
            players: Vec::new(),
            known: std::collections::HashMap::new(),
            visibility: None,
            ruleset: None,
            turn: None,
            source: "test".to_string(),
        }
    }

    fn city(x: i32, y: i32, owner: &str) -> City {
        City {
            x,
            y,
            id: 0,
            name: "C".into(),
            owner: owner.into(),
            size: 1,
            improvements: BTreeSet::new(),
        }
    }
    fn unit(x: i32, y: i32, owner: &str) -> Unit {
        Unit {
            x,
            y,
            id: 0,
            kind: "Warriors".into(),
            owner: owner.into(),
            veteran: 0,
            hp: 10,
        }
    }
    /// A green (veteran 0) unit of a given type at full health — pass `hp` = that type's hitpoints
    /// (Warriors/Phalanx/Legion 10; Musketeers/Riflemen/Alpine 20; Armor 30) so `health` = 1.0 and
    /// att/def equal the raw table values.
    fn unit_kind(x: i32, y: i32, owner: &str, kind: &str, hp: i32) -> Unit {
        Unit {
            x,
            y,
            id: 0,
            kind: kind.into(),
            owner: owner.into(),
            veteran: 0,
            hp,
        }
    }

    #[test]
    fn hills_with_sea_on_the_east() {
        // 6×6: columns 0..=3 Hills, columns 4..=5 Ocean → Hills 24/36 (0.667), Ocean 12/36 (0.333).
        let rows = ["HHHHOO"; 6];
        let mut b = mk_board(&rows, &[('H', "Hills"), ('O', "Ocean")]);
        b.cities.push(city(1, 1, "0"));
        b.units.push(unit(2, 2, "0"));
        b.units.push(unit(3, 3, "0"));
        let d = describe_region(&b, 0, 0, 5, 5);
        assert!(d.starts_with("Predominantly Hills"), "got: {d}");
        assert!(d.contains("sea along the eastern edge"), "got: {d}");
        assert!(d.contains("about a third of the region"), "got: {d}");
        assert!(d.contains("Cities (1): C (owner 0)."), "got: {d}");
        assert!(d.contains("Units (2): 2 owner 0."), "got: {d}");
        assert!(d.contains("Resource deposits: none."), "got: {d}");
    }

    #[test]
    fn mixed_terrain_and_contested() {
        // 4×4: half Plains, half Grassland (a near-tie) → "a mix of ...".
        let rows = ["PPGG", "PPGG", "PPGG", "PPGG"];
        let mut b = mk_board(&rows, &[('P', "Plains"), ('G', "Grassland")]);
        // Two owners claim tiles → contested.
        b.tiles[0][0].owner = Some("0".into());
        b.tiles[3][3].owner = Some("2".into());
        b.cities.push(city(0, 0, "0"));
        b.cities.push(city(3, 3, "2"));
        let d = describe_region(&b, 0, 0, 3, 3);
        assert!(d.starts_with("A mix of Grassland and Plains"), "got: {d}");
        // Each city is named with its owner, in north-to-south reading order.
        assert!(
            d.contains("Cities (2): C (owner 0), C (owner 2)."),
            "got: {d}"
        );
        assert!(d.contains("Contested territory (players 0, 2)"), "got: {d}");
    }

    #[test]
    fn exact_cities_per_owner_units_and_resource_total() {
        // 4×4 Grassland with a mixed garrison and two resource deposits.
        let rows = ["GGGG", "GGGG", "GGGG", "GGGG"];
        let mut b = mk_board(&rows, &[('G', "Grassland")]);
        // Two named cities (owners 0 and 2), listed exactly.
        b.cities.push(City {
            name: "Rome".into(),
            ..city(0, 0, "0")
        });
        b.cities.push(City {
            name: "Qanqlis".into(),
            ..city(2, 1, "2")
        });
        // Five units: 3 owned by 0, 2 by owner 2 → tallied per owner, most first.
        for _ in 0..3 {
            b.units.push(unit(1, 1, "0"));
        }
        b.units.push(unit(3, 3, "2"));
        b.units.push(unit(3, 2, "2"));
        // Resources counted per type: two Iron + one Wine; a non-resource extra (Road) is ignored.
        b.tiles[0][1].extras.insert("Iron".into());
        b.tiles[1][3].extras.insert("Iron".into());
        b.tiles[2][2].extras.insert("Wine".into());
        b.tiles[0][0].extras.insert("Road".into());
        let d = describe_region(&b, 0, 0, 3, 3);
        assert!(
            d.contains("Cities (2): Rome (owner 0), Qanqlis (owner 2)."),
            "got: {d}"
        );
        assert!(d.contains("Units (5): 3 owner 0, 2 owner 2."), "got: {d}");
        // Most-first, then alphabetical: Iron 2 before Wine 1.
        assert!(
            d.contains("Resource deposits (3): Iron 2, Wine 1."),
            "got: {d}"
        );
    }

    #[test]
    fn counts_only_drops_names_and_per_owner_tally() {
        // Same 4×4 board as `exact_cities_per_owner...`: 2 cities, 5 units, 3 resource deposits.
        let rows = ["GGGG", "GGGG", "GGGG", "GGGG"];
        let mut b = mk_board(&rows, &[('G', "Grassland")]);
        b.cities.push(City {
            name: "Rome".into(),
            ..city(0, 0, "0")
        });
        b.cities.push(City {
            name: "Qanqlis".into(),
            ..city(2, 1, "2")
        });
        for _ in 0..3 {
            b.units.push(unit(1, 1, "0"));
        }
        b.units.push(unit(3, 3, "2"));
        b.units.push(unit(3, 2, "2"));
        b.tiles[0][1].extras.insert("Iron".into());
        b.tiles[1][3].extras.insert("Iron".into());
        b.tiles[2][2].extras.insert("Wine".into());
        let d = describe_region_counts(&b, 0, 0, 3, 3);
        // Bare counts, no names, no per-owner tally.
        assert!(d.contains("Cities: 2, Units: 5."), "got: {d}");
        assert!(!d.contains("Rome") && !d.contains("owner 0"), "got: {d}");
        // Terrain + resource tally are identical to the coarse path.
        assert!(d.starts_with("Entirely Grassland"), "got: {d}");
        assert!(d.contains("Resource deposits (3): Iron 2, Wine 1."), "got: {d}");
    }

    #[test]
    fn empty_and_uniform() {
        let rows = ["GGG", "GGG", "GGG"];
        let b = mk_board(&rows, &[('G', "Grassland")]);
        let d = describe_region(&b, 0, 0, 2, 2);
        assert!(d.starts_with("Entirely Grassland"), "got: {d}");
        assert!(d.contains("No cities or units."), "got: {d}");
        assert!(d.contains("Territory unclaimed."), "got: {d}");
        // Out-of-bounds / degenerate box.
        assert!(describe_region(&b, 5, 5, 9, 9).contains("empty region"));
    }

    #[test]
    fn entirely_one_players_territory() {
        // 3×3 all Grassland, every tile owned by player 0 → full inclusion.
        let rows = ["GGG", "GGG", "GGG"];
        let mut b = mk_board(&rows, &[('G', "Grassland")]);
        for y in 0..3 {
            for x in 0..3 {
                b.tiles[y][x].owner = Some("0".into());
            }
        }
        let d = describe_region(&b, 0, 0, 2, 2);
        assert!(d.contains("Entirely player 0's territory."), "got: {d}");
    }

    #[test]
    fn partial_inclusion_not_entirely() {
        // 3×3: player 0 owns 6 of 9 tiles (>0.5), rest unclaimed → "Mostly", not "Entirely".
        let rows = ["GGG", "GGG", "GGG"];
        let mut b = mk_board(&rows, &[('G', "Grassland")]);
        for y in 0..2 {
            for x in 0..3 {
                b.tiles[y][x].owner = Some("0".into());
            }
        }
        let d = describe_region(&b, 0, 0, 2, 2);
        assert!(!d.contains("Entirely player"), "got: {d}");
        assert!(d.contains("Mostly player 0's territory."), "got: {d}");

        // Player 0 owns only 2 of 9 (<=0.5), sole claimant → "Partly", not "Entirely".
        let mut b2 = mk_board(&rows, &[('G', "Grassland")]);
        b2.tiles[0][0].owner = Some("0".into());
        b2.tiles[0][1].owner = Some("0".into());
        let d2 = describe_region(&b2, 0, 0, 2, 2);
        assert!(!d2.contains("Entirely player"), "got: {d2}");
        assert!(d2.contains("Partly player 0's territory."), "got: {d2}");
    }

    #[test]
    fn rich_lists_positions_owner_sums_and_resource_positions() {
        // 8×8 Grassland. Sentence 1 (terrain) and the territory tilt are shared with the coarse
        // path; the rich path enriches occupants + resources with positions and att/def.
        let rows = ["GGGGGGGG"; 8];
        let mut b = mk_board(&rows, &[('G', "Grassland")]);

        // Two cities: an UNwalled one at (2,1) and a WALLED one (own City Walls) at (0,3).
        // Sorted by (y, x): (2,1) before (0,3).
        b.cities.push(City {
            name: "Kabah".into(),
            ..city(2, 1, "A")
        });
        b.cities.push(City {
            name: "Zama".into(),
            size: 3,
            improvements: ["City Walls".to_string()].into_iter().collect(),
            ..city(0, 3, "B")
        });

        // Owner A: Alpine Troops (att5/def5) @(5,4) and Musketeers (att3/def3) @(5,2) → Σ 8/8.
        // Owner B: Riflemen (att5/def4) @(7,2) → Σ 5/4. A's Σ att (8) > B's (5) → A listed first.
        b.units.push(unit_kind(5, 4, "A", "Alpine Troops", 20));
        b.units.push(unit_kind(5, 2, "A", "Musketeers", 20));
        b.units.push(unit_kind(7, 2, "B", "Riflemen", 20));

        // Resources: Wheat @(6,1) then Iron @(1,3) in (y, x) order; a Road extra is ignored.
        b.tiles[1][6].extras.insert("Wheat".into());
        b.tiles[3][1].extras.insert("Iron".into());
        b.tiles[0][0].extras.insert("Road".into());

        let d = describe_region_rich(&b, 0, 0, 7, 7);

        // Cities: position, owner, size, walls; unwalled first, walled annotated.
        assert!(
            d.contains("Cities (2): Kabah @(2,1) (A, size 1), Zama @(0,3) (B, size 3, walls)."),
            "got: {d}"
        );
        // Units: per-owner grouping, (y,x) order within owner, att/def, per-owner Σ, owner order.
        assert!(
            d.contains(
                "Units (3) — A: Musketeers @(5,2) att 3.0/def 3.0, \
                 Alpine Troops @(5,4) att 5.0/def 5.0 [Σ att 8.0, def 8.0]; \
                 B: Riflemen @(7,2) att 5.0/def 4.0 [Σ att 5.0, def 4.0]."
            ),
            "got: {d}"
        );
        // Resources: each deposit with its position, in (y,x) order.
        assert!(
            d.contains("Resource deposits (2): Wheat @(6,1), Iron @(1,3)."),
            "got: {d}"
        );
        // Shared sentences unchanged from the coarse path.
        assert!(d.starts_with("Entirely Grassland"), "got: {d}");
    }

    #[test]
    fn rich_unmodeled_unit_excluded_from_sum() {
        // A modeled Legion (att4/def2) and an UNMODELED Diplomat share owner A; Diplomat renders
        // `att -/def -` and is left out of the Σ, but still counts toward the (N) total.
        let rows = ["GGGG"; 4];
        let mut b = mk_board(&rows, &[('G', "Grassland")]);
        b.units.push(unit_kind(1, 1, "A", "Legion", 10));
        b.units.push(unit_kind(2, 2, "A", "Diplomat", 20));
        let d = describe_region_rich(&b, 0, 0, 3, 3);
        assert!(
            d.contains(
                "Units (2) — A: Legion @(1,1) att 4.0/def 2.0, \
                 Diplomat @(2,2) att -/def - [Σ att 4.0, def 2.0]."
            ),
            "got: {d}"
        );
    }

    #[test]
    fn rich_empty_region_matches_coarse_sentinels() {
        // With no occupants/resources the rich path still emits the same "none" sentinels.
        let rows = ["GGG", "GGG", "GGG"];
        let b = mk_board(&rows, &[('G', "Grassland")]);
        let d = describe_region_rich(&b, 0, 0, 2, 2);
        assert!(d.contains("No cities or units."), "got: {d}");
        assert!(d.contains("Resource deposits: none."), "got: {d}");
        assert!(d.contains("Territory unclaimed."), "got: {d}");
    }
}
