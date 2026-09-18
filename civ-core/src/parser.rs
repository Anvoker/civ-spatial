//! Parser: Freeciv `.sav` (plaintext, INI-like) -> neutral [`Board`].
//!
//! The ONLY module that knows the Freeciv save format. Runs once per board. Everything
//! downstream speaks `Board`, not Freeciv.
//!
//! Format (verified against `data/saves/myagent_T50.sav`):
//!   - `[section]` headers; `key=value`; quoted strings; comma vectors; multi-line tables
//!     `key={"c0","c1",...` then CSV rows until a lone `}`.
//!   - `[savefile]`: `terrident` table (identifier char -> terrain name); `extras_vector`.
//!   - `[map]`: `t<YYYY>` terrain rows (decode via terrident; Ocean is the space char);
//!     `e<NN>_<YYYY>` extras bit-planes (plane NN packs 4 extras as one hex digit/tile:
//!     bit 1 -> extras[4*NN+0], 2 -> +1, 4 -> +2, 8 -> +3); `owner<YYYY>` territory map
//!     (player id or `-`).
//!   - `[player<i>]`: `name`, `nation`, city table `c` (keyed y,x,...), unit table `u`
//!     (keyed id,x,y,...,type_by_name,...,veteran,hp,...).
//!
//! Compressed saves (.sav.xz / .sav.zst) must be decompressed to plaintext first.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use crate::board::{Board, City, Player, Tile, Unit};

#[derive(Debug)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "save parse error: {}", self.0)
    }
}
impl std::error::Error for ParseError {}

fn err<T>(msg: impl Into<String>) -> Result<T, ParseError> {
    Err(ParseError(msg.into()))
}

/// Parse a save file at `path`.
pub fn parse_save(path: impl AsRef<Path>) -> Result<Board, ParseError> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path)
        .map_err(|e| ParseError(format!("reading {}: {e}", path.display())))?;
    let source = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    parse_str(&text, source)
}

/// Parse save `text` with a `source` provenance label. Split out for testability.
pub fn parse_str(text: &str, source: impl Into<String>) -> Result<Board, ParseError> {
    let lines: Vec<&str> = text.lines().collect();
    let sections = split_sections(&lines);

    let savefile = sections.get("savefile").map(Vec::as_slice).unwrap_or(&[]);
    let terrain_by_char = parse_terrident(savefile)?;
    let extras_vector = parse_extras_vector(savefile)?;
    let improvement_vector = parse_improvement_vector(savefile);

    // Ruleset directory (`[savefile] rulesetdir`) — used by the CLI to gate runs to `classic`, the
    // ruleset our solver constants target. The game turn (`[game] turn`) is carried as metadata.
    let ruleset = get_kv(savefile, "rulesetdir").map(|v| unquote(&v));
    let turn = sections
        .get("game")
        .and_then(|g| get_kv(g, "turn"))
        .and_then(|v| unquote(&v).trim().parse::<i32>().ok());

    let map = match sections.get("map") {
        Some(m) => m.as_slice(),
        None => return err("no [map] section"),
    };
    let terrain_rows = collect_terrain(map);
    if terrain_rows.is_empty() {
        return err("no terrain rows (t####=) in [map]");
    }
    let height = terrain_rows.len() as i32;
    let width = terrain_rows[0].chars().count() as i32;
    for (y, row) in terrain_rows.iter().enumerate() {
        if row.chars().count() as i32 != width {
            return err(format!(
                "terrain row {y} width {} != {width}",
                row.chars().count()
            ));
        }
    }
    let terrain_grid: Vec<Vec<char>> = terrain_rows.iter().map(|r| r.chars().collect()).collect();
    let extra_planes = collect_extra_planes(map);
    let owner_rows = collect_owner(map);

    let players = parse_players(&sections);
    let owner_name_by_id: HashMap<i32, String> =
        players.iter().map(|p| (p.id, p.name.clone())).collect();

    let owner_grid: Option<Vec<Vec<String>>> = if owner_rows.is_empty() {
        None
    } else {
        Some(
            owner_rows
                .iter()
                .map(|r| r.split(',').map(|s| s.to_string()).collect())
                .collect(),
        )
    };

    let mut tiles: Vec<Vec<Tile>> = Vec::with_capacity(height as usize);
    for y in 0..height {
        let mut row_tiles = Vec::with_capacity(width as usize);
        for x in 0..width {
            let ch = terrain_grid[y as usize][x as usize];
            let terrain = terrain_by_char
                .get(&ch)
                .cloned()
                .unwrap_or_else(|| format!("Unknown({ch:?})"));
            let extras = extras_at(x as usize, y as usize, &extra_planes, &extras_vector);
            let owner = owner_grid.as_ref().and_then(|g| {
                g.get(y as usize)
                    .and_then(|r| r.get(x as usize))
                    .and_then(|tok| match tok.as_str() {
                        "-" | "" => None,
                        t => t
                            .parse::<i32>()
                            .ok()
                            .and_then(|id| owner_name_by_id.get(&id))
                            .cloned(),
                    })
            });
            row_tiles.push(Tile {
                x,
                y,
                terrain,
                extras,
                owner,
            });
        }
        tiles.push(row_tiles);
    }

    let mut cities = Vec::new();
    let mut units = Vec::new();
    let mut known: HashMap<i32, Vec<Vec<bool>>> = HashMap::new();
    for p in &players {
        let sec = &sections[&format!("player{}", p.id)];
        cities.extend(parse_cities(sec, &p.name, &improvement_vector)?);
        units.extend(parse_units(sec, &p.name)?);
        if let Some(g) = collect_player_known(sec, width, height) {
            known.insert(p.id, g);
        }
    }

    Ok(Board {
        width,
        height,
        tiles,
        cities,
        units,
        players,
        known,
        visibility: None,
        ruleset,
        turn,
        source: source.into(),
    })
}

// --------------------------------------------------------------------------
// low-level helpers
// --------------------------------------------------------------------------

/// Split a Freeciv table/vector line on commas, honoring double-quoted fields (which may
/// contain commas) and doubled-quote escaping (`""` -> `"`). Surrounding quotes are removed.
fn split_csv(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                cur.push(c);
            }
        } else {
            match c {
                '"' => in_quotes = true,
                ',' => fields.push(std::mem::take(&mut cur)),
                _ => cur.push(c),
            }
        }
    }
    fields.push(cur);
    fields
}

/// Strip one layer of surrounding double quotes (without touching interior whitespace —
/// terrain rows use the space char as the Ocean code).
fn unquote(val: &str) -> String {
    let v = val.trim_end_matches(['\r', '\n']);
    let bytes = v.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"' {
        v[1..v.len() - 1].to_string()
    } else {
        v.to_string()
    }
}

fn split_sections(lines: &[&str]) -> HashMap<String, Vec<String>> {
    let mut sections: HashMap<String, Vec<String>> = HashMap::new();
    let mut current: Option<String> = None;
    for &line in lines {
        let t = line.trim_end();
        if let Some(name) = t.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            let name = name.to_string();
            sections.entry(name.clone()).or_default();
            current = Some(name);
        } else if let Some(name) = &current {
            sections.get_mut(name).unwrap().push(line.to_string());
        }
    }
    sections
}

/// First `key=value` line with an exact key match; returns the raw (possibly quoted) value.
fn get_kv(section: &[String], key: &str) -> Option<String> {
    let pfx = format!("{key}=");
    section
        .iter()
        .find_map(|line| line.strip_prefix(&pfx).map(|v| v.to_string()))
}

/// Read a table `key={"c0",...` + rows until a lone `}`; returns (header, rows).
fn read_table(section: &[String], key: &str) -> Option<(Vec<String>, Vec<Vec<String>>)> {
    let prefix = format!("{key}={{");
    let i = section.iter().position(|l| l.starts_with(&prefix))?;
    let header = split_csv(&section[i][prefix.len()..]);
    let mut rows = Vec::new();
    for row_line in &section[i + 1..] {
        if row_line.trim() == "}" {
            break;
        }
        rows.push(split_csv(row_line));
    }
    Some((header, rows))
}

fn col(header: &[String], name: &str) -> Option<usize> {
    header.iter().position(|c| c == name)
}

// --------------------------------------------------------------------------
// legends
// --------------------------------------------------------------------------
fn parse_terrident(savefile: &[String]) -> Result<HashMap<char, String>, ParseError> {
    let (header, rows) = read_table(savefile, "terrident")
        .ok_or_else(|| ParseError("terrident legend not found in [savefile]".into()))?;
    let name_i = col(&header, "name")
        .ok_or_else(|| ParseError(format!("terrident missing 'name': {header:?}")))?;
    let id_i = col(&header, "identifier")
        .ok_or_else(|| ParseError(format!("terrident missing 'identifier': {header:?}")))?;
    let mut legend = HashMap::new();
    for row in rows {
        if row.len() <= name_i.max(id_i) {
            continue;
        }
        if let Some(ch) = row[id_i].chars().next() {
            legend.insert(ch, row[name_i].clone());
        }
    }
    Ok(legend)
}

fn parse_extras_vector(savefile: &[String]) -> Result<Vec<String>, ParseError> {
    let raw = get_kv(savefile, "extras_vector")
        .ok_or_else(|| ParseError("extras_vector not found in [savefile]".into()))?;
    Ok(split_csv(&raw))
}

/// The `improvement_vector` in `[savefile]` lists every building/wonder name in bit order; a
/// city's `improvements` bitstring is indexed against it. Absent (older saves) -> empty vector,
/// which decodes every city to an empty improvement set (a graceful no-op, not an error).
fn parse_improvement_vector(savefile: &[String]) -> Vec<String> {
    match get_kv(savefile, "improvement_vector") {
        Some(raw) => split_csv(&raw),
        None => Vec::new(),
    }
}

/// Decode a per-city `improvements` bitstring against `improvement_vector`. Unlike the tile-extra
/// bit-planes (4 extras packed per hex digit), this is a *plain* fixed-length binary string: one
/// `'0'`/`'1'` char per improvement index, char `i` set iff the city has `improvement_vector[i]`.
/// (Verified against `myagent_T50.sav`: 68-char strings vs. a 68-entry vector; City Walls=7,
/// Coastal Defense=8, Palace=21, Great Wall=47.) Any non-`'1'` char is treated as unset.
fn decode_improvements(bits: &str, improvement_vector: &[String]) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for (i, ch) in bits.chars().enumerate() {
        if ch == '1' {
            if let Some(name) = improvement_vector.get(i) {
                set.insert(name.clone());
            }
        }
    }
    set
}

// --------------------------------------------------------------------------
// map grid
// --------------------------------------------------------------------------

/// Parse a `<prefix>DDDD=value` line -> `(index, unquoted_value)`.
fn match_indexed(line: &str, prefix: char) -> Option<(usize, String)> {
    let rest = line.strip_prefix(prefix)?;
    if rest.len() < 5 {
        return None;
    }
    let (num, tail) = rest.split_at(4);
    if !num.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let val = tail.strip_prefix('=')?;
    Some((num.parse().ok()?, unquote(val)))
}

fn match_owner(line: &str) -> Option<(usize, String)> {
    let rest = line.strip_prefix("owner")?;
    if rest.len() < 5 {
        return None;
    }
    let (num, tail) = rest.split_at(4);
    if !num.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let val = tail.strip_prefix('=')?;
    Some((num.parse().ok()?, unquote(val)))
}

/// Parse `e<NN>_<YYYY>=value` -> `(plane, y, value)`.
fn match_extra(line: &str) -> Option<(usize, usize, String)> {
    let rest = line.strip_prefix('e')?;
    if rest.len() < 8 {
        return None;
    }
    let (plane, r2) = rest.split_at(2);
    if !plane.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let r3 = r2.strip_prefix('_')?;
    if r3.len() < 5 {
        return None;
    }
    let (y, tail) = r3.split_at(4);
    if !y.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let val = tail.strip_prefix('=')?;
    Some((plane.parse().ok()?, y.parse().ok()?, unquote(val)))
}

/// Collect rows keyed by a 4-digit y index into a dense Vec ordered by y.
fn dense_by_y(map: HashMap<usize, String>) -> Vec<String> {
    if map.is_empty() {
        return Vec::new();
    }
    let max_y = *map.keys().max().unwrap();
    (0..=max_y)
        .map(|y| map.get(&y).cloned().unwrap_or_default())
        .collect()
}

fn collect_terrain(section: &[String]) -> Vec<String> {
    let mut by_y = HashMap::new();
    for line in section {
        if let Some((y, v)) = match_indexed(line, 't') {
            by_y.insert(y, v);
        }
    }
    dense_by_y(by_y)
}

fn collect_owner(section: &[String]) -> Vec<String> {
    let mut by_y = HashMap::new();
    for line in section {
        if let Some((y, v)) = match_owner(line) {
            by_y.insert(y, v);
        }
    }
    dense_by_y(by_y)
}

/// Per-player known-terrain map (fog of war): the `map_t####` rows in a `[player<i>]` section,
/// where `'u'` marks a never-explored tile (all other chars — terrain identifiers, and `' '` for
/// known ocean — mean explored). Returns a `height`-row × `width`-col grid of bools (true = the
/// player has explored the tile), or `None` if the section carries no `map_t` rows.
fn collect_player_known(section: &[String], width: i32, height: i32) -> Option<Vec<Vec<bool>>> {
    let mut by_y: HashMap<usize, String> = HashMap::new();
    for line in section {
        if let Some(rest) = line.strip_prefix("map_t") {
            if rest.len() >= 5 && rest.as_bytes()[..4].iter().all(u8::is_ascii_digit) {
                let (num, tail) = rest.split_at(4);
                if let (Ok(y), Some(val)) = (num.parse::<usize>(), tail.strip_prefix('=')) {
                    by_y.insert(y, unquote(val));
                }
            }
        }
    }
    if by_y.is_empty() {
        return None;
    }
    let rows = dense_by_y(by_y);
    Some(
        (0..height as usize)
            .map(|y| {
                let bytes = rows.get(y).map(String::as_str).unwrap_or("").as_bytes();
                (0..width as usize)
                    .map(|x| bytes.get(x).map(|&b| b != b'u').unwrap_or(false))
                    .collect()
            })
            .collect(),
    )
}

/// `{plane -> rows-of-chars-by-y}` for every `e<NN>_` plane.
fn collect_extra_planes(section: &[String]) -> HashMap<usize, Vec<Vec<char>>> {
    let mut planes: HashMap<usize, HashMap<usize, String>> = HashMap::new();
    for line in section {
        if let Some((plane, y, v)) = match_extra(line) {
            planes.entry(plane).or_default().insert(y, v);
        }
    }
    planes
        .into_iter()
        .map(|(plane, by_y)| {
            let rows = dense_by_y(by_y)
                .into_iter()
                .map(|s| s.chars().collect())
                .collect();
            (plane, rows)
        })
        .collect()
}

fn extras_at(
    x: usize,
    y: usize,
    planes: &HashMap<usize, Vec<Vec<char>>>,
    extras_vector: &[String],
) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for (plane, rows) in planes {
        let Some(row) = rows.get(y) else { continue };
        let Some(&ch) = row.get(x) else { continue };
        let Some(bits) = ch.to_digit(16) else {
            continue;
        };
        for b in 0..4u32 {
            if bits & (1 << b) != 0 {
                let idx = 4 * plane + b as usize;
                if idx < extras_vector.len() {
                    set.insert(extras_vector[idx].clone());
                }
            }
        }
    }
    set
}

// --------------------------------------------------------------------------
// players, cities, units
// --------------------------------------------------------------------------
fn parse_players(sections: &HashMap<String, Vec<String>>) -> Vec<Player> {
    let mut players = Vec::new();
    let mut i = 0;
    while let Some(sec) = sections.get(&format!("player{i}")) {
        let name = get_kv(sec, "name")
            .map(|v| unquote(&v))
            .unwrap_or_else(|| format!("player{i}"));
        let nation = get_kv(sec, "nation")
            .map(|v| unquote(&v))
            .unwrap_or_default();
        // `is_alive=TRUE|FALSE`; absent (older saves) is treated as alive.
        let is_alive = get_kv(sec, "is_alive")
            .map(|v| unquote(&v).trim().eq_ignore_ascii_case("true"))
            .unwrap_or(true);
        players.push(Player {
            id: i,
            name,
            nation,
            is_alive,
        });
        i += 1;
    }
    players
}

fn parse_cities(
    section: &[String],
    owner: &str,
    improvement_vector: &[String],
) -> Result<Vec<City>, ParseError> {
    let Some((header, rows)) = read_table(section, "c") else {
        return Ok(Vec::new());
    };
    let (xi, yi, idi) = match (col(&header, "x"), col(&header, "y"), col(&header, "id")) {
        (Some(a), Some(b), Some(c)) => (a, b, c),
        _ => return err(format!("city table missing x/y/id: {header:?}")),
    };
    let name_i = col(&header, "name");
    let size_i = col(&header, "size");
    let imp_i = col(&header, "improvements");
    let mut out = Vec::new();
    for row in rows {
        let need = xi.max(yi).max(idi);
        if row.len() <= need {
            continue;
        }
        let improvements = imp_i
            .and_then(|i| row.get(i))
            .map(|bits| decode_improvements(bits, improvement_vector))
            .unwrap_or_default();
        out.push(City {
            x: parse_i32(&row[xi]),
            y: parse_i32(&row[yi]),
            id: parse_i32(&row[idi]),
            name: name_i.and_then(|i| row.get(i)).cloned().unwrap_or_default(),
            owner: owner.to_string(),
            size: size_i
                .and_then(|i| row.get(i))
                .map(|s| parse_i32(s))
                .unwrap_or(0),
            improvements,
        });
    }
    Ok(out)
}

fn parse_units(section: &[String], owner: &str) -> Result<Vec<Unit>, ParseError> {
    let Some((header, rows)) = read_table(section, "u") else {
        return Ok(Vec::new());
    };
    let (idi, xi, yi) = match (col(&header, "id"), col(&header, "x"), col(&header, "y")) {
        (Some(a), Some(b), Some(c)) => (a, b, c),
        _ => return err(format!("unit table missing id/x/y: {header:?}")),
    };
    let kind_i = col(&header, "type_by_name");
    let vet_i = col(&header, "veteran");
    let hp_i = col(&header, "hp");
    let mut out = Vec::new();
    for row in rows {
        let need = idi.max(xi).max(yi);
        if row.len() <= need {
            continue;
        }
        out.push(Unit {
            x: parse_i32(&row[xi]),
            y: parse_i32(&row[yi]),
            id: parse_i32(&row[idi]),
            kind: kind_i.and_then(|i| row.get(i)).cloned().unwrap_or_default(),
            owner: owner.to_string(),
            veteran: vet_i
                .and_then(|i| row.get(i))
                .map(|s| parse_i32(s))
                .unwrap_or(0),
            hp: hp_i
                .and_then(|i| row.get(i))
                .map(|s| parse_i32(s))
                .unwrap_or(0),
        });
    }
    Ok(out)
}

fn parse_i32(s: &str) -> i32 {
    s.trim().parse().unwrap_or(0)
}
