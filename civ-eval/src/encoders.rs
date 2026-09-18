//! Encoders — the independent variable. An encoding is a *board-access strategy* (DESIGN.md
//! §8). v1 implements the static branch (Board -> prompt string). Each encoder also renders
//! `Referent`s in its own vocabulary so a question reads naturally under it.
//!
//! All three v1 encoders are *information-complete*: every fact needed to answer any T0/T1
//! question is present. They differ only in how spatial/relational structure is laid out —
//! which is the thing being compared.

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};

use civ_core::geometry::{chebyshev, neighbors, ALL_DIRS};
use civ_core::{Board, City, Unit};

use crate::model::{ToolCall, ToolDef};

/// A handle to a thing on the board that a question refers to. Stored abstractly so each
/// encoder can name it natively (coordinates now; "3 E of your capital" for egocentric later).
#[derive(Debug, Clone)]
pub enum Referent {
    Tile { x: i32, y: i32 },
    City { id: i32 },
    Unit { id: i32 },
}

impl Referent {
    /// Stable key for question ids.
    pub fn key(&self) -> String {
        match self {
            Referent::Tile { x, y } => format!("{x},{y}"),
            Referent::City { id } => format!("c{id}"),
            Referent::Unit { id } => format!("u{id}"),
        }
    }

    /// Resolve to board coordinates, or `None` if the referent doesn't exist.
    pub fn resolve(&self, board: &Board) -> Option<(i32, i32)> {
        match self {
            Referent::Tile { x, y } if board.in_bounds(*x, *y) => Some((*x, *y)),
            Referent::Tile { .. } => None,
            Referent::City { id } => board
                .cities
                .iter()
                .find(|c| c.id == *id)
                .map(|c| (c.x, c.y)),
            Referent::Unit { id } => board.units.iter().find(|u| u.id == *id).map(|u| (u.x, u.y)),
        }
    }

    /// Resolve a `Unit` referent to the board unit, or `None` (also `None` for non-unit kinds).
    pub fn unit<'a>(&self, board: &'a Board) -> Option<&'a Unit> {
        match self {
            Referent::Unit { id } => board.units.iter().find(|u| u.id == *id),
            _ => None,
        }
    }

    /// Resolve a `City` referent to the board city, or `None` (also `None` for non-city kinds).
    pub fn city<'a>(&self, board: &'a Board) -> Option<&'a City> {
        match self {
            Referent::City { id } => board.cities.iter().find(|c| c.id == *id),
            _ => None,
        }
    }
}

/// The closed-option label for a unit in a T2 `Choice` answer: `"<type> at (x, y)"`. The default
/// `render_referent` for a unit ("the <type> at (x, y)") contains this as a substring, so a model
/// echoing the question's phrasing under any coordinate encoder matches it. Location
/// disambiguates two same-type units.
pub fn unit_choice_label(u: &Unit) -> String {
    format!("{} at ({}, {})", u.kind, u.x, u.y)
}

/// The closed-option label for a city in a T2 `Choice` answer: its name (distinctive and salient;
/// it appears in every encoder's city rendering).
pub fn city_choice_label(c: &City) -> String {
    c.name.clone()
}

/// A board-access strategy. v1 encoders are all static (produce a prompt string).
pub trait Encoder {
    fn name(&self) -> &'static str;

    /// The board block placed in the prompt.
    fn render(&self, board: &Board) -> String;

    /// How this encoding names a referent inside question text. Default = coordinates/names,
    /// which suits every v1 encoder; egocentric/label encodings override this later.
    fn render_referent(&self, board: &Board, r: &Referent) -> String {
        match r {
            Referent::Tile { x, y } => format!("tile ({x}, {y})"),
            Referent::City { id } => board
                .cities
                .iter()
                .find(|c| c.id == *id)
                .map(|c| format!("the city \"{}\" at ({}, {})", c.name, c.x, c.y))
                .unwrap_or_else(|| format!("city #{id}")),
            Referent::Unit { id } => board
                .units
                .iter()
                .find(|u| u.id == *id)
                // Id appended at the END so `unit_choice_label` ("<type> at (x, y)") stays a
                // substring, AND so the model can address this unit by id in the by-id operators
                // (att_eff/def_eff/reach_turns), which matters when it is buried in a stack.
                .map(|u| format!("the {} at ({}, {}) (unit #{})", u.kind, u.x, u.y, u.id))
                .unwrap_or_else(|| format!("unit #{id}")),
        }
    }

    /// Round-trip obligation (the information-completeness gate, DESIGN.md §4/§5). Decode this
    /// encoder's **own** [`render`](Encoder::render) output back to the per-tile facts it carries,
    /// for **every in-bounds tile** — so a lost or corrupted fact is a compile-*and*-test failure,
    /// not a silent one. This is a *required* method with no default body: you cannot add a new
    /// static encoder without also providing its decoder.
    ///
    /// `block` is the exact string produced by `render`; dimensions and default-terrain are parsed
    /// back out of the block's own header (the block is self-describing). Returns one
    /// `((x, y), RecoveredTile)` per in-bounds tile, or `Err` if the block cannot be decoded (e.g.
    /// an ambiguous glyph whose terrain is unrecoverable — see the `ascii` caveat).
    ///
    /// NOTE: the obligation is for *static* string encoders. The interactive/queryable branch's
    /// equivalent — a `get_tile` parity check — is deferred (future work).
    fn reconstruct(&self, block: &str) -> Result<RecoveredBoard, String>;
}

/// The full set of per-tile facts recovered from one encoder's output: one `((x, y), facts)`
/// entry per in-bounds tile, in row-major order.
pub type RecoveredBoard = Vec<((i32, i32), RecoveredTile)>;

// --------------------------------------------------------------------------
// recovered facts — the structured form of what `tile_facts` renders
// --------------------------------------------------------------------------

/// A city recovered from an encoder's output. Mirrors the `{city "<name>" size <n> owner <o>}`
/// segment of [`tile_facts`]; the question-relevant subset of [`City`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveredCity {
    pub name: String,
    pub size: i32,
    pub owner: String,
}

/// A unit recovered from an encoder's output. Mirrors the `{unit #<id> <kind> owner <o>}` segment.
/// The `id` is carried so a specific unit on a STACKED tile can be named and addressed (operators
/// resolve their unit subject by id, not by first-match on the coordinate).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveredUnit {
    pub id: i32,
    pub kind: String,
    pub owner: String,
}

/// The per-tile facts recovered by decoding an encoder's output — exactly what [`tile_facts`]
/// renders, in structured form. Compared *semantically* (not byte-exact) against the source
/// `Board` by the reconstruction gate: terrain exact, extras as a set, city/owner exact, and
/// **every** unit on the tile (with its id) in board order — a stacked tile lists all its units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveredTile {
    pub terrain: String,
    pub extras: BTreeSet<String>,
    pub city: Option<RecoveredCity>,
    pub units: Vec<RecoveredUnit>,
    pub owner: Option<String>,
}

impl RecoveredTile {
    /// The facts of a plain default tile: the stated default terrain, nothing else. Used to
    /// pre-fill list-style grids where unlisted in-bounds tiles are the default.
    fn plain(default_terrain: &str) -> Self {
        RecoveredTile {
            terrain: default_terrain.to_string(),
            extras: BTreeSet::new(),
            city: None,
            units: Vec::new(),
            owner: None,
        }
    }

    /// The expected recovered facts for a source-board tile — the ground truth the gate compares
    /// against. Kept next to the decoders so encoder and check share one notion of "the facts".
    pub fn expected(board: &Board, x: i32, y: i32) -> Self {
        let t = board.tile(x, y);
        RecoveredTile {
            terrain: t.terrain.clone(),
            extras: t.extras.clone(),
            city: board
                .cities
                .iter()
                .find(|c| c.x == x && c.y == y)
                .map(|c| RecoveredCity {
                    name: c.name.clone(),
                    size: c.size,
                    owner: c.owner.clone(),
                }),
            units: board
                .units
                .iter()
                .filter(|u| u.x == x && u.y == y)
                .map(|u| RecoveredUnit {
                    id: u.id,
                    kind: u.kind.clone(),
                    owner: u.owner.clone(),
                })
                .collect(),
            owner: t.owner.clone(),
        }
    }
}

// --------------------------------------------------------------------------
// shared decode helpers — parse the structured backing of `tile_facts`
// --------------------------------------------------------------------------

/// The integer immediately preceding `marker` in `s` (e.g. the width before `" wide"`).
fn int_before(s: &str, marker: &str) -> Result<i32, String> {
    let idx = s
        .find(marker)
        .ok_or_else(|| format!("header missing {marker:?}"))?;
    let mut digits: Vec<char> = s[..idx]
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.reverse();
    let digits: String = digits.into_iter().collect();
    digits
        .parse()
        .map_err(|_| format!("no integer before {marker:?}"))
}

/// The substring between the first `open` and the following `close`, if both are present.
fn between<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = s.find(open)? + open.len();
    let rest = &s[start..];
    let end = rest.find(close)?;
    Some(&rest[..end])
}

/// Parse the `<width> wide ... <height> tall` dimensions shared by every static header.
fn parse_dims(block: &str) -> Result<(i32, i32), String> {
    Ok((int_before(block, " wide")?, int_before(block, " tall")?))
}

/// Parse the stated default terrain from a list-style header ("... below is <default> with no
/// special features.").
fn parse_default_terrain(block: &str) -> Result<String, String> {
    between(block, "below is ", " with no special features")
        .map(str::to_string)
        .ok_or_else(|| "header missing default-terrain clause".to_string())
}

/// Decode one `tile_facts` payload (the text after the coordinate/offset prefix) into structured
/// facts. The segment order is fixed: `terrain [extras] {city ...} {unit ...} <territory ...>`,
/// each with unambiguous delimiters.
fn parse_tile_facts(s: &str) -> Result<RecoveredTile, String> {
    let s = s.trim();
    // Strip the trailing three-state-fog honesty marker (always the last segment; see `tile_facts`).
    // `RecoveredTile` carries no fog flag — fogged-ness is a derived view property, not a tile fact,
    // and the reconstruction gate compares only the facts, which round-trip identically either way.
    let s = s.strip_suffix("(fogged)").map(str::trim_end).unwrap_or(s);
    // Terrain is everything up to the first optional-segment marker (terrain names may contain
    // spaces, e.g. "Deep Ocean", but never '[', '{' or '<').
    let cut = [" [", " {", " <"]
        .iter()
        .filter_map(|m| s.find(m))
        .min()
        .unwrap_or(s.len());
    let terrain = s[..cut].trim().to_string();
    if terrain.is_empty() {
        return Err(format!("empty terrain in facts {s:?}"));
    }
    let rest = &s[cut..];

    let mut extras = BTreeSet::new();
    if let Some(body) = between(rest, " [", "]") {
        for e in body.split(", ") {
            if !e.is_empty() {
                extras.insert(e.to_string());
            }
        }
    }

    let city = between(rest, "{city ", "}").map(parse_city).transpose()?;
    // Every `{unit ...}` segment on the tile, left-to-right (== board order, since the renderer
    // emits them in board order) — a stacked tile carries several. First-match would drop the rest.
    let mut units = Vec::new();
    let mut scan = rest;
    while let Some(open) = scan.find("{unit ") {
        let seg_start = open + "{unit ".len();
        let Some(rel_close) = scan[seg_start..].find('}') else {
            break;
        };
        units.push(parse_unit(&scan[seg_start..seg_start + rel_close])?);
        scan = &scan[seg_start + rel_close + 1..];
    }
    let owner = between(rest, "<territory ", ">").map(str::to_string);

    Ok(RecoveredTile {
        terrain,
        extras,
        city,
        units,
        owner,
    })
}

/// Parse the inside of `{city "<name>" size <n> owner <o>}` (the `"<name>" size <n> owner <o>`).
fn parse_city(seg: &str) -> Result<RecoveredCity, String> {
    let q1 = seg
        .find('"')
        .ok_or("city facts missing opening name quote")?;
    let q2 = seg[q1 + 1..]
        .find('"')
        .ok_or("city facts missing closing name quote")?
        + q1
        + 1;
    let name = seg[q1 + 1..q2].to_string();
    let after = &seg[q2 + 1..];
    let sz = after.find("size ").ok_or("city facts missing size")? + "size ".len();
    let owner_at = after[sz..]
        .find(" owner ")
        .ok_or("city facts missing owner")?;
    let size: i32 = after[sz..sz + owner_at]
        .trim()
        .parse()
        .map_err(|_| format!("bad city size in {seg:?}"))?;
    let owner = after[sz + owner_at + " owner ".len()..].trim().to_string();
    Ok(RecoveredCity { name, size, owner })
}

/// Parse the inside of `{unit #<id> <kind> owner <o>}` (the `#<id> <kind> owner <o>`). `kind` may
/// contain spaces; the id leads (as `#<digits>`) and `kind` ends at the ` owner ` delimiter.
fn parse_unit(seg: &str) -> Result<RecoveredUnit, String> {
    let seg = seg.trim();
    let rest = seg
        .strip_prefix('#')
        .ok_or_else(|| format!("unit facts missing #id in {seg:?}"))?;
    let sp = rest
        .find(' ')
        .ok_or_else(|| format!("unit facts missing id/kind gap in {seg:?}"))?;
    let id: i32 = rest[..sp]
        .parse()
        .map_err(|_| format!("bad unit id in {seg:?}"))?;
    let body = &rest[sp + 1..];
    let at = body.find(" owner ").ok_or("unit facts missing owner")?;
    Ok(RecoveredUnit {
        id,
        kind: body[..at].trim().to_string(),
        owner: body[at + " owner ".len()..].trim().to_string(),
    })
}

/// Parse a `"(x, y): <facts>"` line (raw's tile lines and ascii's DETAIL lines, after trimming).
/// Returns `None` for lines that are not coordinate entries (headers, blanks).
fn parse_coord_line(line: &str) -> Option<((i32, i32), &str)> {
    let line = line.trim_start();
    if !line.starts_with('(') {
        return None;
    }
    let close = line.find(')')?;
    let (x, y) = parse_pair(&line[1..close])?;
    let facts = line[close + 1..]
        .trim_start()
        .strip_prefix(':')?
        .trim_start();
    Some(((x, y), facts))
}

/// Parse a `"x, y"` (or `"x,y"`) integer pair.
fn parse_pair(s: &str) -> Option<(i32, i32)> {
    let (a, b) = s.split_once(',')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

/// A dense `height x width` grid pre-filled with `fill`, addressed row-major.
struct RecoveredGrid {
    width: i32,
    height: i32,
    tiles: Vec<RecoveredTile>,
}

impl RecoveredGrid {
    fn filled(width: i32, height: i32, fill: RecoveredTile) -> Self {
        RecoveredGrid {
            width,
            height,
            tiles: vec![fill; (width * height) as usize],
        }
    }

    fn set(&mut self, x: i32, y: i32, facts: RecoveredTile) -> Result<(), String> {
        if x < 0 || x >= self.width || y < 0 || y >= self.height {
            return Err(format!("decoded tile ({x}, {y}) is out of bounds"));
        }
        self.tiles[(y * self.width + x) as usize] = facts;
        Ok(())
    }

    /// Emit every in-bounds tile in row-major order.
    fn into_vec(self) -> RecoveredBoard {
        let width = self.width;
        self.tiles
            .into_iter()
            .enumerate()
            .map(|(i, t)| ((i as i32 % width, i as i32 / width), t))
            .collect()
    }
}

// --------------------------------------------------------------------------
// shared tile-content rendering (the canonical TileFacts)
// --------------------------------------------------------------------------
struct Occupants<'a> {
    cities: HashMap<(i32, i32), &'a City>,
    /// ALL units on each tile, in board order (a stacked tile holds several). Keyed for O(1)
    /// per-tile lookup; the render lists every one, so no unit is dropped on dense boards.
    units: HashMap<(i32, i32), Vec<&'a Unit>>,
}

impl<'a> Occupants<'a> {
    fn of(board: &'a Board) -> Self {
        let mut units: HashMap<(i32, i32), Vec<&'a Unit>> = HashMap::new();
        for u in &board.units {
            units.entry((u.x, u.y)).or_default().push(u);
        }
        Occupants {
            cities: board.cities.iter().map(|c| ((c.x, c.y), c)).collect(),
            units,
        }
    }
}

/// The canonical, information-complete description of a tile's contents. Every encoder renders
/// the same facts; they differ only in placement.
fn tile_facts(board: &Board, x: i32, y: i32, occ: &Occupants) -> String {
    let t = board.tile(x, y);
    let mut s = t.terrain.clone();
    if !t.extras.is_empty() {
        let extras: Vec<&str> = t.extras.iter().map(String::as_str).collect();
        s.push_str(&format!(" [{}]", extras.join(", ")));
    }
    if let Some(c) = occ.cities.get(&(x, y)) {
        s.push_str(&format!(
            " {{city \"{}\" size {} owner {}}}",
            c.name, c.size, c.owner
        ));
    }
    if let Some(us) = occ.units.get(&(x, y)) {
        // Every unit on the tile, each with its id — so the model can name a specific stacked unit
        // (e.g. `att_eff(unit=3956)`). Info-completeness over token cost on dense boards, by design.
        for u in us {
            s.push_str(&format!(" {{unit #{} {} owner {}}}", u.id, u.kind, u.owner));
        }
    }
    if let Some(o) = &t.owner {
        s.push_str(&format!(" <territory {o}>"));
    }
    // Honesty marker (three-state fog): a FOGGED tile shows remembered terrain/extras/owner and any
    // last-known enemy city, but its live unit occupants are hidden — so an absent unit here must NOT
    // be read as "safe/empty". Always the LAST segment, so decoders can strip it in one step. On an
    // omniscient (non-fog) board `is_fogged` is always false → byte-identical to before.
    if board.is_fogged(x, y) {
        s.push_str(" (fogged)");
    }
    s
}

/// A one-line preamble describing the `(fogged)` marker, emitted in every encoder's header when the
/// board is a masked (fog-of-war) view, and empty otherwise. Explains fog globally so the rule holds
/// even for fogged default-terrain tiles that are omitted from a list-style render.
fn fog_preamble(board: &Board) -> String {
    if board.visibility.is_some() {
        "This is a FOG-OF-WAR view from one player's perspective. A tile tagged (fogged) is \
         explored but not currently in sight: its terrain is remembered and any enemy city is shown \
         at its last-known state, but live unit positions there are hidden — an absent unit on a \
         fogged or unlisted explored tile does NOT mean the tile is empty. Tiles shown as Unknown \
         were never explored.\n"
            .to_string()
    } else {
        String::new()
    }
}

/// Whether a tile is worth listing in the ASCII detail legend. Includes tiles with a *territory
/// owner*: the ASCII glyph grid encodes only terrain/unit/city markers, so an owned-but-otherwise
/// plain tile would otherwise drop its owner (an information-completeness gap the reconstruction
/// gate would catch). Listing it in DETAIL keeps ASCII complete for territory owner too.
fn is_notable(board: &Board, x: i32, y: i32, occ: &Occupants) -> bool {
    let t = board.tile(x, y);
    occ.cities.contains_key(&(x, y))
        || occ.units.contains_key(&(x, y))
        || !t.extras.is_empty()
        || t.owner.is_some()
        // A fogged LAND tile is listed even when otherwise plain, so its "(fogged)" tag reaches the
        // model (a hidden land unit could be there). Fogged water carries no hidden-land-unit signal
        // and is left to the glyph + header note, keeping a heavily-explored board's DETAIL bounded.
        || (board.is_fogged(x, y) && crate::question::is_land(&t.terrain))
}

/// The most frequent terrain on the board — used as an omittable "default" so list-style
/// encodings don't enumerate open ocean tile by tile. Information-complete: the default is
/// stated, and the grid dimensions let a reader tell an unlisted in-bounds tile from an edge.
pub(crate) fn most_common_terrain(board: &Board) -> String {
    let mut counts: HashMap<&str, u32> = HashMap::new();
    for t in board.iter_tiles() {
        *counts.entry(t.terrain.as_str()).or_default() += 1;
    }
    // B3: break count ties deterministically by terrain name (ascending), matching the histogram
    // tiebreak idiom used elsewhere (`.then_with(|| a.0.cmp(b.0))`). A bare `max_by_key` over a std
    // `HashMap` left the omitted "default" terrain at the mercy of per-map-random iteration order,
    // so a tie could resolve differently across runs *and* across calls — breaking both the
    // question-set determinism guarantee and generator/encoder agreement on the default. `min_by_key`
    // over `(Reverse(count), name)` picks the highest count, then the name-order winner, stably.
    counts
        .into_iter()
        .min_by_key(|(name, n)| (std::cmp::Reverse(*n), *name))
        .map(|(t, _)| t.to_string())
        .unwrap_or_default()
}

/// A tile that is exactly the default terrain with nothing on it — safe to omit from a list
/// because it is fully recovered from the stated default.
fn is_plain_default(board: &Board, x: i32, y: i32, occ: &Occupants, default: &str) -> bool {
    let t = board.tile(x, y);
    t.terrain == default
        && t.extras.is_empty()
        && t.owner.is_none()
        && !occ.cities.contains_key(&(x, y))
        && !occ.units.contains_key(&(x, y))
        // A fogged LAND tile is never omitted as "plain default": it must be listed so its "(fogged)"
        // tag reaches the model (see `is_notable`). Fogged water/default tiles stay omittable.
        && !(board.is_fogged(x, y) && crate::question::is_land(&t.terrain))
}

// --------------------------------------------------------------------------
// raw coordinate list — the naive baseline: every tile, one line
// --------------------------------------------------------------------------
pub struct RawEncoder;

impl Encoder for RawEncoder {
    fn name(&self) -> &'static str {
        "raw"
    }

    fn render(&self, board: &Board) -> String {
        let occ = Occupants::of(board);
        let default = most_common_terrain(board);
        let mut out = String::new();
        out.push_str(&fog_preamble(board));
        out.push_str(&format!(
            "BOARD (raw tile list). Grid is {} wide (x: 0..{}) by {} tall (y: 0..{}). \
             x increases east, y increases south; y=0 is the north edge. \
             Any in-bounds tile NOT listed below is {default} with no special features. \
             One line per listed tile:\n\n",
            board.width,
            board.width - 1,
            board.height,
            board.height - 1
        ));
        for y in 0..board.height {
            for x in 0..board.width {
                if is_plain_default(board, x, y, &occ, &default) {
                    continue;
                }
                out.push_str(&format!("({x}, {y}): {}\n", tile_facts(board, x, y, &occ)));
            }
        }
        out
    }

    fn reconstruct(&self, block: &str) -> Result<RecoveredBoard, String> {
        let (width, height) = parse_dims(block)?;
        let default = parse_default_terrain(block)?;
        let mut grid = RecoveredGrid::filled(width, height, RecoveredTile::plain(&default));
        for line in block.lines() {
            if let Some(((x, y), facts)) = parse_coord_line(line) {
                grid.set(x, y, parse_tile_facts(facts)?)?;
            }
        }
        Ok(grid.into_vec())
    }
}

// --------------------------------------------------------------------------
// annotated ASCII map — glyph grid + coordinate-keyed detail legend
// --------------------------------------------------------------------------
pub struct AsciiEncoder;

/// The terrain <-> glyph table. Single source of truth for both rendering ([`terrain_glyph`]) and
/// decoding ([`glyph_terrain`]) so the two can never drift. NOTE: this map is the boundary of
/// ASCII's information-completeness — a terrain NOT listed here renders as `'?'` and, on a
/// *non-notable* tile (no unit/city/extras/owner), is unrecoverable from the glyph alone. All
/// terrains present in the T50 board are covered; see the crate's reconstruction test.
const TERRAIN_GLYPHS: &[(&str, char)] = &[
    ("Grassland", 'g'),
    ("Plains", 'p'),
    ("Forest", 'f'),
    ("Hills", 'h'),
    ("Mountains", 'm'),
    ("Desert", 'd'),
    ("Jungle", 'j'),
    ("Swamp", 's'),
    ("Tundra", 't'),
    ("Glacier", 'a'),
    ("Lake", '+'),
    ("Ocean", '.'),
    ("Deep Ocean", ':'),
    ("Inaccessible", '#'),
    ("Unknown", '?'), // fog of war: an unexplored tile (only present on `--fog` masked boards)
];

fn terrain_glyph(terrain: &str) -> char {
    TERRAIN_GLYPHS
        .iter()
        .find(|(t, _)| *t == terrain)
        .map_or('?', |(_, g)| *g)
}

/// The inverse of [`terrain_glyph`] for the *unambiguous* glyphs. Returns `None` for `'?'`
/// (unmapped terrain) and for the `'@'`/`'C'` markers (unit/city override the terrain glyph) —
/// those tiles' terrain must come from the DETAIL legend instead.
fn glyph_terrain(g: char) -> Option<&'static str> {
    TERRAIN_GLYPHS
        .iter()
        .find(|(_, c)| *c == g)
        .map(|(t, _)| *t)
}

impl Encoder for AsciiEncoder {
    fn name(&self) -> &'static str {
        "ascii"
    }

    fn render(&self, board: &Board) -> String {
        let occ = Occupants::of(board);
        let mut out = String::new();
        out.push_str(&fog_preamble(board));
        out.push_str(&format!(
            "BOARD (ASCII map). {} wide by {} tall. x increases east (columns, labeled along \
             the top), y increases south (rows, labeled at left); y=0 is the north edge.\n",
            board.width, board.height
        ));
        out.push_str(
            "Terrain glyphs: g=Grassland p=Plains f=Forest h=Hills m=Mountains d=Desert \
             j=Jungle s=Swamp t=Tundra a=Glacier +=Lake .=Ocean :=Deep Ocean.\n",
        );
        out.push_str(
            "Markers: @=a unit, C=a city (these override the terrain glyph). The full contents \
             of every tile that has a unit, city, or special feature are listed by coordinate \
             in the DETAIL section below.\n\n",
        );

        // Column ruler (tens then units).
        let gutter = "    "; // 4 chars: aligns with "{:>3} " row labels
        let mut tens = String::from(gutter);
        let mut units = String::from(gutter);
        for x in 0..board.width {
            tens.push(if x % 10 == 0 {
                std::char::from_digit((x / 10 % 10) as u32, 10).unwrap()
            } else {
                ' '
            });
            units.push(std::char::from_digit((x % 10) as u32, 10).unwrap());
        }
        out.push_str(&tens);
        out.push('\n');
        out.push_str(&units);
        out.push('\n');

        for y in 0..board.height {
            out.push_str(&format!("{y:>3} "));
            for x in 0..board.width {
                let g = if occ.units.contains_key(&(x, y)) {
                    '@'
                } else if occ.cities.contains_key(&(x, y)) {
                    'C'
                } else {
                    terrain_glyph(&board.tile(x, y).terrain)
                };
                out.push(g);
            }
            out.push('\n');
        }

        out.push_str("\nDETAIL (full contents of every notable tile, by coordinate):\n");
        for y in 0..board.height {
            for x in 0..board.width {
                if is_notable(board, x, y, &occ) {
                    out.push_str(&format!(
                        "  ({x}, {y}): {}\n",
                        tile_facts(board, x, y, &occ)
                    ));
                }
            }
        }
        out
    }

    fn reconstruct(&self, block: &str) -> Result<RecoveredBoard, String> {
        let (width, height) = parse_dims(block)?;

        // The DETAIL legend is authoritative for every notable tile (terrain included). Anything
        // not in DETAIL is a non-notable tile whose terrain must come from the glyph grid.
        let mut detail: HashMap<(i32, i32), RecoveredTile> = HashMap::new();
        let in_detail = block.split("\nDETAIL").nth(1).unwrap_or("");
        for line in in_detail.lines() {
            if let Some(((x, y), facts)) = parse_coord_line(line) {
                detail.insert((x, y), parse_tile_facts(facts)?);
            }
        }

        let mut grid = RecoveredGrid::filled(width, height, RecoveredTile::plain(""));
        for line in block.lines() {
            // A grid row is `"{y:>3} {glyphs}"`: a 4-char label+space then exactly `width` glyphs.
            if line.len() != width as usize + 4 {
                continue;
            }
            let Ok(y) = line[..4].trim().parse::<i32>() else {
                continue;
            };
            if y < 0 || y >= height {
                continue;
            }
            for (x, g) in line[4..].chars().enumerate() {
                let x = x as i32;
                if let Some(facts) = detail.get(&(x, y)) {
                    grid.set(x, y, facts.clone())?;
                } else if let Some(terrain) = glyph_terrain(g) {
                    grid.set(x, y, RecoveredTile::plain(terrain))?;
                } else {
                    // '?', '@' or 'C' on a tile absent from DETAIL: terrain unrecoverable. For
                    // '@'/'C' this cannot happen (units/cities are notable → in DETAIL); for '?'
                    // it means the glyph map does not cover this terrain — a real completeness gap.
                    return Err(format!(
                        "ascii: tile ({x}, {y}) has ambiguous glyph {g:?} and no DETAIL entry — \
                         terrain unrecoverable (glyph map does not cover this terrain)"
                    ));
                }
            }
        }
        Ok(grid.into_vec())
    }
}

// --------------------------------------------------------------------------
// adjacency list — tiles with precomputed neighbor relations
// --------------------------------------------------------------------------
pub struct AdjacencyEncoder;

impl Encoder for AdjacencyEncoder {
    fn name(&self) -> &'static str {
        "adjacency"
    }

    fn render(&self, board: &Board) -> String {
        let occ = Occupants::of(board);
        let default = most_common_terrain(board);
        let mut out = String::new();
        out.push_str(&fog_preamble(board));
        out.push_str(&format!(
            "BOARD (adjacency list). {} wide by {} tall; y=0 is the north edge. Directions: \
             N,NE,E,SE,S,SW,W,NW. Any in-bounds tile NOT given its own entry below is {default} \
             with no special features. For each listed tile, its contents are given, then its \
             non-{default} neighbors by direction; a neighbor direction that is in-bounds but \
             not shown is {default}, and a direction off the board edge has no tile.\n\n",
            board.width, board.height
        ));
        for y in 0..board.height {
            for x in 0..board.width {
                if is_plain_default(board, x, y, &occ, &default) {
                    continue;
                }
                out.push_str(&format!("({x}, {y}) {}", tile_facts(board, x, y, &occ)));
                out.push_str(" | neighbors:");
                // Preserve ALL_DIRS order for determinism; omit default-terrain neighbors
                // (recoverable from the stated rule) to cut redundancy.
                let nbrs: HashMap<_, _> = neighbors(board, x, y).into_iter().collect();
                for d in ALL_DIRS {
                    if let Some(&(nx, ny)) = nbrs.get(&d) {
                        let t = &board.tile(nx, ny).terrain;
                        if *t != default {
                            out.push_str(&format!(" {}=({nx},{ny}):{}", d.short(), t));
                        }
                    }
                }
                out.push('\n');
            }
        }
        out
    }

    fn reconstruct(&self, block: &str) -> Result<RecoveredBoard, String> {
        let (width, height) = parse_dims(block)?;
        let default = parse_default_terrain(block)?;
        let mut grid = RecoveredGrid::filled(width, height, RecoveredTile::plain(&default));
        for line in block.lines() {
            // Entry lines are `"(x, y) <facts> | neighbors: ..."`. Neighbor annotations are
            // redundant (each non-default neighbor has its own entry) — only the own facts matter.
            let line = line.trim();
            if !line.starts_with('(') {
                continue;
            }
            let Some(close) = line.find(')') else {
                continue;
            };
            let Some((x, y)) = parse_pair(&line[1..close]) else {
                continue;
            };
            let after = &line[close + 1..];
            let facts = match after.find(" | neighbors:") {
                Some(i) => &after[..i],
                None => after,
            };
            grid.set(x, y, parse_tile_facts(facts)?)?;
        }
        Ok(grid.into_vec())
    }
}

// --------------------------------------------------------------------------
// egocentric — everything positioned relative to a chosen ORIGIN, not absolute
// --------------------------------------------------------------------------
pub struct EgocentricEncoder;

/// Choose the egocentric origin. "Primary player" = the first player (in board order) that
/// owns at least one city; the origin is that player's **capital** (largest city, ties by
/// lowest id — [`Board::capital_of`]). Falls back to the board's first city, and finally to
/// the north-west corner tile if the board has no cities at all. Returns
/// `(x, y, human-label)`.
fn egocentric_origin(board: &Board) -> (i32, i32, String) {
    let city = board
        .players
        .iter()
        .find_map(|p| board.capital_of(&p.name))
        .or_else(|| board.cities.first());
    match city {
        Some(c) => (c.x, c.y, format!("the city \"{}\"", c.name)),
        None => (0, 0, "the north-west corner tile".to_string()),
    }
}

/// Compact offset key used in the board block, e.g. `3E 2S`, `2S`, `3E`, or `0 (Origin)`.
/// `dx > 0` is east, `dy > 0` is south (y increases south). A zero axis is omitted.
fn offset_key(dx: i32, dy: i32) -> String {
    let ew = match dx.cmp(&0) {
        Ordering::Greater => format!("{dx}E"),
        Ordering::Less => format!("{}W", -dx),
        Ordering::Equal => String::new(),
    };
    let ns = match dy.cmp(&0) {
        Ordering::Greater => format!("{dy}S"),
        Ordering::Less => format!("{}N", -dy),
        Ordering::Equal => String::new(),
    };
    match (ew.is_empty(), ns.is_empty()) {
        (true, true) => "0 (Origin)".to_string(),
        (false, true) => ew,
        (true, false) => ns,
        (false, false) => format!("{ew} {ns}"),
    }
}

/// Inverse of [`offset_key`]: parse a compact offset key back to `(dx, dy)`. Accepts
/// `"0 (Origin)"`, single-axis keys (`"3E"`, `"2S"`), and two-axis keys (`"3E 2S"`). Returns
/// `Err` for anything that is not a well-formed offset (used to reject header prose).
fn parse_offset(key: &str) -> Result<(i32, i32), String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("empty offset key".to_string());
    }
    if key.starts_with('0') {
        // The Origin renders as "0 (Origin)".
        return Ok((0, 0));
    }
    let (mut dx, mut dy) = (0, 0);
    for tok in key.split_whitespace() {
        let (num, dir) = tok.split_at(tok.len() - 1);
        let n: i32 = num
            .parse()
            .map_err(|_| format!("bad offset token {tok:?}"))?;
        match dir {
            "E" => dx = n,
            "W" => dx = -n,
            "S" => dy = n,
            "N" => dy = -n,
            _ => return Err(format!("bad offset direction in {tok:?}")),
        }
    }
    Ok((dx, dy))
}

/// A prose egocentric phrase for a tile referent, e.g. "the tile 3 East and 2 South of the
/// Origin", or "the Origin tile" when the referent *is* the origin.
fn egocentric_phrase(dx: i32, dy: i32) -> String {
    let mut parts = Vec::new();
    match dx.cmp(&0) {
        Ordering::Greater => parts.push(format!("{dx} East")),
        Ordering::Less => parts.push(format!("{} West", -dx)),
        Ordering::Equal => {}
    }
    match dy.cmp(&0) {
        Ordering::Greater => parts.push(format!("{dy} South")),
        Ordering::Less => parts.push(format!("{} North", -dy)),
        Ordering::Equal => {}
    }
    if parts.is_empty() {
        "the Origin tile".to_string()
    } else {
        format!("the tile {} of the Origin", parts.join(" and "))
    }
}

/// A short parenthetical locating a named object relative to the origin, e.g. "the Origin" or
/// "3E 2S from the Origin". Used so City/Unit referents stay *named* but egocentric.
fn egocentric_paren(dx: i32, dy: i32) -> String {
    if dx == 0 && dy == 0 {
        "the Origin".to_string()
    } else {
        format!("{} from the Origin", offset_key(dx, dy))
    }
}

impl Encoder for EgocentricEncoder {
    fn name(&self) -> &'static str {
        "egocentric"
    }

    fn render(&self, board: &Board) -> String {
        let occ = Occupants::of(board);
        let default = most_common_terrain(board);
        let (ox, oy, label) = egocentric_origin(board);
        let mut out = String::new();
        out.push_str(&fog_preamble(board));
        out.push_str(&format!(
            "BOARD (egocentric list). Grid is {} wide by {} tall. Origin is {label} at absolute \
             ({ox}, {oy}); all positions below are given as steps East/West and North/South \
             from the Origin. E = east (+x), W = west (-x), N = north (toward y=0), S = south; \
             an axis with 0 steps is omitted (a tile on the Origin's own row shows only its \
             E/W part, and vice versa). Distances and compass directions are frame-invariant \
             (N is always toward y=0). Any in-bounds tile NOT listed below is {default} with no \
             special features. One line per listed tile, \"<offset>: <contents>\":\n\n",
            board.width, board.height,
        ));
        for y in 0..board.height {
            for x in 0..board.width {
                if is_plain_default(board, x, y, &occ, &default) {
                    continue;
                }
                out.push_str(&format!(
                    "{}: {}\n",
                    offset_key(x - ox, y - oy),
                    tile_facts(board, x, y, &occ)
                ));
            }
        }
        out
    }

    fn reconstruct(&self, block: &str) -> Result<RecoveredBoard, String> {
        let (width, height) = parse_dims(block)?;
        let default = parse_default_terrain(block)?;
        // The header states the Origin's absolute coordinates; offsets are relative to it.
        let origin = between(block, "at absolute (", ")")
            .and_then(parse_pair)
            .ok_or("egocentric header missing \"at absolute (x, y)\"")?;
        let mut grid = RecoveredGrid::filled(width, height, RecoveredTile::plain(&default));
        for line in block.lines() {
            // Tile lines are `"<offset>: <facts>"`; header prose is filtered out by rejecting any
            // prefix that is not a valid offset key.
            let Some((key, facts)) = line.split_once(": ") else {
                continue;
            };
            let Ok((dx, dy)) = parse_offset(key) else {
                continue;
            };
            grid.set(origin.0 + dx, origin.1 + dy, parse_tile_facts(facts)?)?;
        }
        Ok(grid.into_vec())
    }

    fn render_referent(&self, board: &Board, r: &Referent) -> String {
        let (ox, oy, _) = egocentric_origin(board);
        match r {
            Referent::Tile { x, y } => egocentric_phrase(x - ox, y - oy),
            Referent::City { id } => board
                .cities
                .iter()
                .find(|c| c.id == *id)
                .map(|c| {
                    format!(
                        "the city \"{}\" ({})",
                        c.name,
                        egocentric_paren(c.x - ox, c.y - oy)
                    )
                })
                .unwrap_or_else(|| format!("city #{id}")),
            Referent::Unit { id } => board
                .units
                .iter()
                .find(|u| u.id == *id)
                .map(|u| format!("the {} ({})", u.kind, egocentric_paren(u.x - ox, u.y - oy)))
                .unwrap_or_else(|| format!("unit #{id}")),
        }
    }
}

// --------------------------------------------------------------------------
// hierarchical — fixed-sector region summaries over complete raw leaves
// --------------------------------------------------------------------------
//
// A region-summary *tree* (two levels: board → fixed sectors → tiles). The top surface is a
// per-sector summary the model can read as pre-aggregated structure (the finding in
// `findings-deepseek-fullboard.md` is that region-count/aggregation is the residual hard skill;
// this format targets it by presenting counts already grouped by a fixed partition). The bottom
// level is a COMPLETE, raw-style listing of every non-default tile grouped by its sector — that
// backing is what makes the format information-complete and reconstructable, and is the *only*
// thing `reconstruct` reads (the summary surface is ignored on decode).
//
// DISCIPLINE (DESIGN.md §4/§6): every summary is over the FIXED partition only — a sector
// terrain histogram is an *observation* over a whole sector. The format never emits a
// question-shaped aggregate (e.g. "forest tiles within radius R of (x,y)"): a query window that
// straddles sector boundaries must still be mapped onto sectors and summed by the model, with
// partial-sector overlaps resolved from the leaves. So the surface pre-aggregates the partition,
// never the answer.
pub struct HierarchicalEncoder;

/// Fixed sector edge length (tiles). The board is partitioned row-major into a grid of
/// `SECTOR`×`SECTOR` sectors; edge sectors are truncated where the board size is not a multiple
/// of `SECTOR`.
const SECTOR: i32 = 10;

/// Number of sectors needed to cover `span` tiles at `SECTOR` per sector (ceil division;
/// `i32::div_ceil` is not yet stable on this toolchain).
fn sector_count(span: i32) -> i32 {
    (span + SECTOR - 1) / SECTOR
}

/// The inclusive tile bbox of the `SECTOR`×`SECTOR` (10×10) window CENTERED on `(cx, cy)`, SHIFTED
/// so it lies fully on the board: the window keeps its full size, and any overrun past an edge is
/// pushed back onto the opposite side (so a near-edge center still yields a complete 10×10 view).
/// When the board is smaller than the window in a dimension, that axis clamps to the whole extent.
/// This is what `region_summary(x, y)` summarizes; the viewer's fetched-region overlay reuses it so
/// the drawn box matches the summarized area exactly.
pub fn region_window(board: &Board, cx: i32, cy: i32) -> (i32, i32, i32, i32) {
    // Per-axis: size-tile window, `size/2` west/north of the center, shifted to stay in [0, span).
    let axis = |c: i32, span: i32| -> (i32, i32) {
        let size = SECTOR.min(span);
        let lo = (c - size / 2).clamp(0, span - size);
        (lo, lo + size - 1)
    };
    let (x0, x1) = axis(cx, board.width);
    let (y0, y1) = axis(cy, board.height);
    (x0, y0, x1, y1)
}

/// One fixed sector: its (col, row) index and inclusive tile bbox `(x0, y0)-(x1, y1)`.
struct Sector {
    sx: i32,
    sy: i32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

impl HierarchicalEncoder {
    /// The fixed sector partition of `board`, in row-major order.
    fn sectors(board: &Board) -> Vec<Sector> {
        let n_sx = sector_count(board.width);
        let n_sy = sector_count(board.height);
        let mut out = Vec::with_capacity((n_sx * n_sy) as usize);
        for sy in 0..n_sy {
            for sx in 0..n_sx {
                out.push(Sector {
                    sx,
                    sy,
                    x0: sx * SECTOR,
                    y0: sy * SECTOR,
                    x1: ((sx + 1) * SECTOR).min(board.width) - 1,
                    y1: ((sy + 1) * SECTOR).min(board.height) - 1,
                });
            }
        }
        out
    }
}

impl Encoder for HierarchicalEncoder {
    fn name(&self) -> &'static str {
        "hierarchical"
    }

    fn render(&self, board: &Board) -> String {
        let occ = Occupants::of(board);
        let default = most_common_terrain(board);
        let sectors = Self::sectors(board);
        let n_sx = sector_count(board.width);
        let n_sy = sector_count(board.height);

        let mut out = String::new();
        out.push_str(&fog_preamble(board));
        out.push_str(&format!(
            "BOARD (hierarchical region-summary). Grid is {} wide (x: 0..{}) by {} tall (y: 0..{}). \
             x increases east, y increases south; y=0 is the north edge. The board is partitioned \
             into a FIXED {n_sx}x{n_sy} grid of sectors of {SECTOR}x{SECTOR} tiles each (row-major; \
             edge sectors are truncated where the board size is not a multiple of {SECTOR}). Sector \
             RrCc is row r, column c, covering x in [c*{SECTOR}, c*{SECTOR}+{SECTOR}) and y in \
             [r*{SECTOR}, r*{SECTOR}+{SECTOR}); each sector's exact bbox is stated on its line. \
             PART 1 summarizes each fixed sector: a terrain histogram (tile counts per terrain over \
             the WHOLE sector), the occupants (cities/units by coordinate), and the special features \
             present. These are observations over the fixed partition, NOT answers: a query window \
             that only partially overlaps a sector must be resolved tile-by-tile from PART 2. \
             PART 2 lists the exact contents of every non-default tile, grouped by sector. \
             Any in-bounds tile not listed below is {default} with no special features.\n\n",
            board.width,
            board.width - 1,
            board.height,
            board.height - 1,
        ));

        // PART 1 — per-sector summaries over the fixed partition.
        out.push_str("PART 1 — SECTOR SUMMARIES (fixed partition; observations only):\n");
        for s in &sectors {
            out.push_str(&format!(
                "Sector R{}C{} bbox ({},{})-({},{}):\n",
                s.sy, s.sx, s.x0, s.y0, s.x1, s.y1
            ));

            // Terrain histogram over the whole sector (includes the default terrain — an
            // observation, not a precomputed region-count answer).
            let mut terr: HashMap<&str, i32> = HashMap::new();
            let mut extras: HashMap<&str, i32> = HashMap::new();
            for y in s.y0..=s.y1 {
                for x in s.x0..=s.x1 {
                    let t = board.tile(x, y);
                    *terr.entry(t.terrain.as_str()).or_default() += 1;
                    for e in &t.extras {
                        *extras.entry(e.as_str()).or_default() += 1;
                    }
                }
            }
            out.push_str("  terrain: ");
            out.push_str(&histogram_line(&terr));
            out.push('\n');

            // Occupants by coordinate.
            let mut cities: Vec<&City> = board
                .cities
                .iter()
                .filter(|c| in_bbox(c.x, c.y, s))
                .collect();
            cities.sort_by_key(|c| (c.y, c.x));
            if !cities.is_empty() {
                let parts: Vec<String> = cities
                    .iter()
                    .map(|c| {
                        format!(
                            "\"{}\" (owner {}, size {}) @ ({},{})",
                            c.name, c.owner, c.size, c.x, c.y
                        )
                    })
                    .collect();
                out.push_str(&format!("  cities: {}\n", parts.join("; ")));
            }
            let mut units: Vec<&Unit> = board
                .units
                .iter()
                .filter(|u| in_bbox(u.x, u.y, s))
                .collect();
            units.sort_by_key(|u| (u.y, u.x));
            if !units.is_empty() {
                let parts: Vec<String> = units
                    .iter()
                    .map(|u| {
                        format!(
                            "#{} {} (owner {}) @ ({},{})",
                            u.id, u.kind, u.owner, u.x, u.y
                        )
                    })
                    .collect();
                out.push_str(&format!("  units: {}\n", parts.join("; ")));
            }
            if !extras.is_empty() {
                out.push_str(&format!("  extras: {}\n", histogram_line(&extras)));
            }
        }

        // PART 2 — complete leaves: every non-default tile's exact facts, grouped by sector.
        out.push_str(
            "\nPART 2 — COMPLETE LEAVES (exact facts of every non-default tile, grouped by sector):\n",
        );
        for s in &sectors {
            let mut lines: Vec<String> = Vec::new();
            for y in s.y0..=s.y1 {
                for x in s.x0..=s.x1 {
                    if is_plain_default(board, x, y, &occ, &default) {
                        continue;
                    }
                    lines.push(format!("  ({x}, {y}): {}", tile_facts(board, x, y, &occ)));
                }
            }
            if lines.is_empty() {
                continue;
            }
            out.push_str(&format!("Sector R{}C{}:\n", s.sy, s.sx));
            for l in lines {
                out.push_str(&l);
                out.push('\n');
            }
        }
        out
    }

    fn reconstruct(&self, block: &str) -> Result<RecoveredBoard, String> {
        let (width, height) = parse_dims(block)?;
        let default = parse_default_terrain(block)?;
        // Only PART 2 (the complete leaves) is authoritative; the PART 1 summary surface — which
        // also mentions coordinates (bboxes, occupants) — is ignored on decode.
        let leaves = block
            .split("COMPLETE LEAVES")
            .nth(1)
            .ok_or("hierarchical: missing PART 2 (COMPLETE LEAVES) section")?;
        let mut grid = RecoveredGrid::filled(width, height, RecoveredTile::plain(&default));
        for line in leaves.lines() {
            if let Some(((x, y), facts)) = parse_coord_line(line) {
                grid.set(x, y, parse_tile_facts(facts)?)?;
            }
        }
        Ok(grid.into_vec())
    }
}

/// Whether `(x, y)` falls inside a sector's inclusive bbox.
fn in_bbox(x: i32, y: i32, s: &Sector) -> bool {
    x >= s.x0 && x <= s.x1 && y >= s.y0 && y <= s.y1
}

/// Render a `name count` histogram deterministically: by count descending, ties by name ascending.
fn histogram_line(counts: &HashMap<&str, i32>) -> String {
    let mut items: Vec<(&str, i32)> = counts.iter().map(|(k, v)| (*k, *v)).collect();
    items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    items
        .iter()
        .map(|(k, v)| format!("{k} {v}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Look up an encoder by name (the CLI `--encoding` value).
pub fn encoder_by_name(name: &str) -> Option<Box<dyn Encoder>> {
    match name {
        "raw" => Some(Box::new(RawEncoder)),
        "ascii" => Some(Box::new(AsciiEncoder)),
        "adjacency" => Some(Box::new(AdjacencyEncoder)),
        "egocentric" => Some(Box::new(EgocentricEncoder)),
        "hierarchical" => Some(Box::new(HierarchicalEncoder)),
        _ => None,
    }
}

/// The v1 comparison trio, in canonical order. Still the `verify-oracle` default, so adjacency's
/// generation→prompt→extraction→scoring contract stays gated even though it no longer runs by default.
pub const V1_ENCODERS: [&str; 3] = ["raw", "ascii", "adjacency"];

/// The default encoder set for live `run`s — the working set of interest: `raw` (the frontier),
/// `ascii` (the model-flipper), and `hierarchical` (the scale contender — ties raw on accuracy at T677,
/// the aggregation-encoder we keep probing as questions get more global). Adjacency is deliberately
/// omitted: the T677 crossover confirmed it stays strictly dominated by raw (equal accuracy at ~4.3×
/// the tokens), so it is opt-in via `--encoding adjacency`. All encoder code paths are fully retained
/// (registered in [`encoder_by_name`], covered by `verify-oracle` / the tests).
pub const DEFAULT_ENCODERS: [&str; 3] = ["raw", "ascii", "hierarchical"];

/// Every static string encoder, in canonical order. All must satisfy the round-trip
/// reconstruction obligation (see [`Encoder::reconstruct`] and the crate's reconstruction test).
pub const STATIC_ENCODERS: [&str; 5] = ["raw", "ascii", "adjacency", "egocentric", "hierarchical"];

// --------------------------------------------------------------------------
// interactive — a QUERYABLE board-access surface (tool loop, not a static render)
// --------------------------------------------------------------------------
//
// The board-access mode where hierarchy's real payoff lives: the model gets a cheap overview, then
// *queries* coarse-to-fine, paying only for what it fetches. See `interactive-board-access-design.md`.
// This file provides the offline half — the overview, the observation-only verbs, and the
// reconstruct-via-tools parity gate. The multi-turn driver + provider tool plumbing live in the
// runner / `remote` (behind the `remote` feature).

/// A board seen through queries rather than one static block. `overview` is the cheap initial
/// context; `tools`/`execute` are the observation-only verbs; `reconstruct_via_tools` is the
/// completeness (parity) gate — the tool outputs must suffice to rebuild the exact board.
pub trait QueryableSurface {
    fn name(&self) -> &str;
    fn overview(&self, board: &Board) -> String;
    fn tools(&self) -> Vec<ToolDef>;
    /// Run one verb against the board, returning its text result (an error string on bad input —
    /// surfaced to the model so it can retry, never a panic).
    fn execute(&self, board: &Board, call: &ToolCall) -> String;
    /// Rebuild the board using only tool outputs (tiling `scan`), then parse them back — proves the
    /// surface hides no information. The static Oracle-100% invariant stays on the text encoders.
    fn reconstruct_via_tools(&self, board: &Board) -> Result<RecoveredBoard, String>;

    /// The system-prompt head for this surface: the framing, how its tools work, and the
    /// answer-format line, ending with the label that introduces the board text the runner appends
    /// (`overview`, then any rules). Kept on the surface so each mode teaches its OWN verbs. The
    /// default is the perception/fetch guidance used by the `interactive` surface; the operator
    /// (calculator) surfaces override it to describe the measurement tools instead.
    fn system_preamble(&self) -> String {
        String::from(
            "You are answering a question about a Freeciv game board. You CANNOT see the whole board at \
             once — you explore it through tools, zooming from a coarse overview down to exact tiles.\n\
             Coordinates: x increases east (0..width-1), y increases south (0..height-1); y=0 is the \
             north edge.\n\n\
             Tools (costs differ — summaries are cheap and approximate; scan/scan_grid/get_tile are exact \
             but you pay per tile):\n\
             - region_summary(x,y): the EXACT occupants (every city + unit with strengths), resources, and dominant terrain of the 10x10 area centered on (x,y), in ONE call (shifted to stay on-board at an edge).\n\
             - scan_grid(x0,y0,x1,y1): a compact glyph grid + exact detail for an inclusive box, in ONE \
             call (no size cap — it can cover a whole area at ~1 char/tile).\n\
             - scan(x0,y0,x1,y1): exact facts of every non-default tile in an inclusive box (max 12x12).\n\
             - get_tile(x,y): exact facts of one tile.\n\n\
             Work economically: read the overview, use region_summary to locate the relevant area, then \
             fetch exact facts. If the region relevant to the question is small, pull it in a single \
             scan_grid call rather than many piecemeal queries — one grid over the whole relevant area is \
             cheaper and less error-prone than stitching together small scans. Use get_tile for a lone \
             tile.\n\
             When ready, reply with a line starting exactly `Answer: ` followed by your answer, and \
             nothing after it.\n\n\
             Board overview:\n",
        )
    }
}

/// The v1 interactive surface: `overview` + `region_summary` + `scan` (area-capped) + `get_tile`.
pub struct Interactive;

/// Max side length of a single `scan` box (so "scan the whole board" costs visibly / takes many calls).
const SCAN_MAX: i32 = 12;

impl Interactive {
    /// Every non-default tile's exact facts in the inclusive box, one `"(x, y): <facts>"` per line.
    fn scan_lines(board: &Board, x0: i32, y0: i32, x1: i32, y1: i32) -> String {
        let occ = Occupants::of(board);
        let default = most_common_terrain(board);
        let mut out = String::new();
        for y in y0..=y1 {
            for x in x0..=x1 {
                if is_plain_default(board, x, y, &occ, &default) {
                    continue;
                }
                out.push_str(&format!("({x}, {y}): {}\n", tile_facts(board, x, y, &occ)));
            }
        }
        if out.is_empty() {
            format!("(no non-default tiles in {x0},{y0}-{x1},{y1}; all are {default})\n")
        } else {
            out
        }
    }

    /// Compact bird's-eye of the inclusive box in ONE call: a glyph grid (one char per tile, same
    /// table as the `ascii` encoder) plus a DETAIL appendix carrying the exact facts a glyph can't
    /// hold (resources, city/unit contents, territory owner). Grid + DETAIL together are
    /// information-complete. Unlike `scan` (one line per non-default tile, capped small), this pays
    /// only ~1 char/tile, so the whole relevant area — up to the whole board — fits in a single fetch.
    fn grid_lines(board: &Board, x0: i32, y0: i32, x1: i32, y1: i32) -> String {
        let occ = Occupants::of(board);
        let mut out = String::new();
        out.push_str(&format!(
            "Grid ({x0},{y0})-({x1},{y1}). Glyphs: g=Grassland p=Plains f=Forest h=Hills \
             m=Mountains d=Desert j=Jungle s=Swamp t=Tundra a=Glacier +=Lake .=Ocean :=Deep Ocean \
             ?=Unknown; @=a unit, C=a city (a marker overrides the terrain glyph). x increases \
             east, y increases south.\n"
        ));
        // Column ruler (absolute x): a 4-char gutter aligns with the "{:>3} " row labels below.
        let gutter = "    ";
        let mut tens = String::from(gutter);
        let mut units = String::from(gutter);
        for x in x0..=x1 {
            tens.push(if x % 10 == 0 {
                std::char::from_digit((x / 10 % 10) as u32, 10).unwrap()
            } else {
                ' '
            });
            units.push(std::char::from_digit((x % 10) as u32, 10).unwrap());
        }
        out.push_str(&tens);
        out.push('\n');
        out.push_str(&units);
        out.push('\n');
        for y in y0..=y1 {
            out.push_str(&format!("{y:>3} "));
            for x in x0..=x1 {
                let g = if occ.units.contains_key(&(x, y)) {
                    '@'
                } else if occ.cities.contains_key(&(x, y)) {
                    'C'
                } else {
                    terrain_glyph(&board.tile(x, y).terrain)
                };
                out.push(g);
            }
            out.push('\n');
        }
        // DETAIL appendix: exact facts of every notable tile (what a single glyph can't carry).
        let mut detail = String::new();
        for y in y0..=y1 {
            for x in x0..=x1 {
                if is_notable(board, x, y, &occ) {
                    detail.push_str(&format!(
                        "  ({x}, {y}): {}\n",
                        tile_facts(board, x, y, &occ)
                    ));
                }
            }
        }
        if detail.is_empty() {
            out.push_str("DETAIL: none (every tile is plain terrain, as shown by its glyph).\n");
        } else {
            out.push_str("DETAIL (exact facts of notable tiles in this box):\n");
            out.push_str(&detail);
        }
        out
    }
}

/// The ORIGINAL `region_summary` tool description — kept verbatim so the A/B control arm
/// (`--legacy-rs-desc`) can restore the pre-redescribe wording that opened with "Qualitative
/// description…". The last smoke run called `region_summary` 0 times: the model read "Qualitative"
/// and reached for `get_tile` instead, even though the tool returns EXACT occupants/strengths.
/// [`REGION_SUMMARY_DESC_ENRICHED`] is the shipped default; this is the control it is measured against.
pub const REGION_SUMMARY_DESC_LEGACY: &str = "Qualitative description of the 10x10 area CENTERED on (x, y): dominant \
    terrain and water/edge concentration, then the EXACT occupants and resources — \
    every city (position, owner, size, walls), every visible unit grouped by owner \
    (type, position, context-free att/def strength, and a per-owner Σ), and every \
    resource deposit (type + position) — plus territory tilt. You may center it \
    anywhere; if the window would run off an edge it is shifted back on so you \
    always get a full 10x10 view (smaller only if the board itself is smaller).";

/// The shipped DEFAULT `region_summary` description: FOREGROUNDS the exact bulk-perception data the
/// tool now returns (the full occupant roster + every resource), so it reads as the precise one-call
/// area dump it is — NOT "qualitative". [`REGION_SUMMARY_DESC_LEGACY`] is the pre-redescribe control.
pub const REGION_SUMMARY_DESC_ENRICHED: &str = "EXACT bulk perception of a 10x10 window CENTERED on (x, y) — the one call that dumps a \
    whole area at once. Returns the precise occupants and resources: every CITY (position, owner, \
    size, walls); every visible UNIT grouped by owner (type, position, exact context-free att/def \
    strength, with a per-owner Σ); every RESOURCE deposit (type + position); plus the dominant \
    terrain and the territory tilt. Center it ANYWHERE on the board — if the 10x10 window would run \
    off an edge it slides back on so you still get a full view (smaller only if the board itself is \
    smaller). Reach for this over get_tile whenever you want everything in an area in one shot.";

impl QueryableSurface for Interactive {
    fn name(&self) -> &str {
        "interactive"
    }

    fn overview(&self, board: &Board) -> String {
        let default = most_common_terrain(board);
        let (w, h) = (board.width, board.height);
        let (mx, my) = (w / 2, h / 2);
        let d = crate::describe::describe_region;
        format!(
            "INTERACTIVE BOARD. {w} wide (x 0..{}) by {h} tall (y 0..{}); x increases east, y \
             increases south, y=0 is the north edge. Unlisted tiles are {default}. You see the \
             board only through tools — call them to zoom from coarse to fine.\n\
             Coarse overview by quadrant:\n\
             \x20 NW (0,0)-({},{}) : {}\n\
             \x20 NE ({},0)-({},{}) : {}\n\
             \x20 SW (0,{})-({},{}) : {}\n\
             \x20 SE ({},{})-({},{}) : {}\n\
             Call region_summary(x, y) for an EXACT {SECTOR}x{SECTOR} dump of every occupant \
             (cities + units with strengths) and resource centered anywhere on \
             the board (it is shifted to stay on-board at an edge). \
             For a compact bird's-eye of a whole area in ONE call, use scan_grid (a glyph grid plus \
             exact detail); use scan / get_tile for pinpoint facts. Use list_cities / list_units to \
             enumerate every visible city / unit by coordinate (a directory — filter by owner to \
             find your own).",
            w - 1, h - 1,
            mx - 1, my - 1, d(board, 0, 0, mx - 1, my - 1),
            mx, w - 1, my - 1, d(board, mx, 0, w - 1, my - 1),
            my, mx - 1, h - 1, d(board, 0, my, mx - 1, h - 1),
            mx, my, w - 1, h - 1, d(board, mx, my, w - 1, h - 1),
        )
    }

    fn tools(&self) -> Vec<ToolDef> {
        vec![
            ToolDef {
                name: "region_summary",
                description: REGION_SUMMARY_DESC_ENRICHED.to_string(),
                params_schema:
                    "{\"type\":\"object\",\"properties\":{\"x\":{\"type\":\"integer\"},\
                     \"y\":{\"type\":\"integer\"}},\"required\":[\"x\",\"y\"]}"
                        .to_string(),
            },
            ToolDef {
                name: "scan",
                description: format!(
                    "Exact facts of every non-default tile in an inclusive box (max {SCAN_MAX}x{SCAN_MAX}). \
                     Precise but you pay per tile."
                ),
                params_schema:
                    "{\"type\":\"object\",\"properties\":{\"x0\":{\"type\":\"integer\"},\
                     \"y0\":{\"type\":\"integer\"},\"x1\":{\"type\":\"integer\"},\
                     \"y1\":{\"type\":\"integer\"}},\"required\":[\"x0\",\"y0\",\"x1\",\"y1\"]}"
                        .to_string(),
            },
            ToolDef {
                name: "scan_grid",
                description: "Compact bird's-eye of an inclusive box in ONE call: a glyph grid (1 \
                    char/tile) plus a DETAIL list of exact facts for notable tiles. No size cap — a \
                    single call can cover the whole relevant area (or the whole board). Cheaper per \
                    tile than scan; prefer it when the region of interest is small enough to grasp \
                    at once."
                    .to_string(),
                params_schema:
                    "{\"type\":\"object\",\"properties\":{\"x0\":{\"type\":\"integer\"},\
                     \"y0\":{\"type\":\"integer\"},\"x1\":{\"type\":\"integer\"},\
                     \"y1\":{\"type\":\"integer\"}},\"required\":[\"x0\",\"y0\",\"x1\",\"y1\"]}"
                        .to_string(),
            },
            ToolDef {
                name: "get_tile",
                description: "Exact facts of one tile (x, y).".to_string(),
                params_schema:
                    "{\"type\":\"object\",\"properties\":{\"x\":{\"type\":\"integer\"},\
                     \"y\":{\"type\":\"integer\"}},\"required\":[\"x\",\"y\"]}"
                        .to_string(),
            },
            ToolDef {
                name: "list_cities",
                description: "List every VISIBLE city — name, owner, coordinates, size — as a directory \
                    (cheaper than hunting with scan). Filter by owner to find your own cities."
                    .to_string(),
                params_schema: "{\"type\":\"object\",\"properties\":{}}".to_string(),
            },
            ToolDef {
                name: "list_units",
                description: "List every VISIBLE unit — type, owner, coordinates — as a directory. \
                    Filter by owner to find your own units."
                    .to_string(),
                params_schema: "{\"type\":\"object\",\"properties\":{}}".to_string(),
            },
        ]
    }

    fn execute(&self, board: &Board, call: &ToolCall) -> String {
        match call.name.as_str() {
            "region_summary" => {
                let (Some(x), Some(y)) =
                    (arg_i32(&call.args_json, "x"), arg_i32(&call.args_json, "y"))
                else {
                    return "error: region_summary requires integers 'x' and 'y' (the window center)"
                        .to_string();
                };
                if !board.in_bounds(x, y) {
                    return format!(
                        "error: region_summary center ({x}, {y}) is off the board (it is {}x{})",
                        board.width, board.height
                    );
                }
                let (x0, y0, x1, y1) = region_window(board, x, y);
                format!(
                    "10x10 around ({x},{y}) — tiles ({x0},{y0})-({x1},{y1}): {}",
                    crate::describe::describe_region_rich(board, x0, y0, x1, y1)
                )
            }
            "scan" => {
                let (Some(x0), Some(y0), Some(x1), Some(y1)) = (
                    arg_i32(&call.args_json, "x0"),
                    arg_i32(&call.args_json, "y0"),
                    arg_i32(&call.args_json, "x1"),
                    arg_i32(&call.args_json, "y1"),
                ) else {
                    return "error: scan requires integers x0,y0,x1,y1".to_string();
                };
                if x0 > x1 || y0 > y1 {
                    return "error: scan needs x0<=x1 and y0<=y1".to_string();
                }
                if !board.in_bounds(x0, y0) || !board.in_bounds(x1, y1) {
                    return format!(
                        "error: scan box out of bounds (board is {}x{})",
                        board.width, board.height
                    );
                }
                if (x1 - x0 + 1) > SCAN_MAX || (y1 - y0 + 1) > SCAN_MAX {
                    return format!("error: scan box too large (max {SCAN_MAX}x{SCAN_MAX}); narrow it and scan in pieces");
                }
                Interactive::scan_lines(board, x0, y0, x1, y1)
            }
            "scan_grid" => {
                let (Some(x0), Some(y0), Some(x1), Some(y1)) = (
                    arg_i32(&call.args_json, "x0"),
                    arg_i32(&call.args_json, "y0"),
                    arg_i32(&call.args_json, "x1"),
                    arg_i32(&call.args_json, "y1"),
                ) else {
                    return "error: scan_grid requires integers x0,y0,x1,y1".to_string();
                };
                if x0 > x1 || y0 > y1 {
                    return "error: scan_grid needs x0<=x1 and y0<=y1".to_string();
                }
                if !board.in_bounds(x0, y0) || !board.in_bounds(x1, y1) {
                    return format!(
                        "error: scan_grid box out of bounds (board is {}x{})",
                        board.width, board.height
                    );
                }
                Interactive::grid_lines(board, x0, y0, x1, y1)
            }
            "get_tile" => {
                let (Some(x), Some(y)) =
                    (arg_i32(&call.args_json, "x"), arg_i32(&call.args_json, "y"))
                else {
                    return "error: get_tile requires integers x and y".to_string();
                };
                if !board.in_bounds(x, y) {
                    return format!(
                        "error: ({x},{y}) out of bounds (board is {}x{})",
                        board.width, board.height
                    );
                }
                let occ = Occupants::of(board);
                format!("({x}, {y}): {}", tile_facts(board, x, y, &occ))
            }
            "list_cities" => {
                if board.cities.is_empty() {
                    return "(no cities visible)".to_string();
                }
                let mut cs: Vec<&City> = board.cities.iter().collect();
                cs.sort_by_key(|c| (c.y, c.x));
                cs.iter()
                    .map(|c| {
                        format!(
                            "\"{}\" owner {} at ({}, {}) size {}",
                            c.name, c.owner, c.x, c.y, c.size
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            "list_units" => {
                if board.units.is_empty() {
                    return "(no units visible)".to_string();
                }
                let mut us: Vec<&Unit> = board.units.iter().collect();
                us.sort_by_key(|u| (u.y, u.x));
                us.iter()
                    .map(|u| {
                        format!(
                            "#{} {} owner {} at ({}, {})",
                            u.id, u.kind, u.owner, u.x, u.y
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            other => format!("error: unknown tool {other:?}"),
        }
    }

    fn reconstruct_via_tools(&self, board: &Board) -> Result<RecoveredBoard, String> {
        let default = most_common_terrain(board);
        let (w, h) = (board.width, board.height);
        let mut grid: Vec<Vec<RecoveredTile>> = (0..h)
            .map(|_| (0..w).map(|_| RecoveredTile::plain(&default)).collect())
            .collect();
        // Tile the board with in-bounds scan calls (<= SCAN_MAX per side), parse each listed leaf.
        let mut y0 = 0;
        while y0 < h {
            let y1 = (y0 + SCAN_MAX - 1).min(h - 1);
            let mut x0 = 0;
            while x0 < w {
                let x1 = (x0 + SCAN_MAX - 1).min(w - 1);
                let call = ToolCall {
                    id: String::new(),
                    name: "scan".to_string(),
                    args_json: format!("{{\"x0\":{x0},\"y0\":{y0},\"x1\":{x1},\"y1\":{y1}}}"),
                };
                for line in self.execute(board, &call).lines() {
                    if let Some(((x, y), facts)) = parse_coord_line(line) {
                        grid[y as usize][x as usize] = parse_tile_facts(facts)?;
                    }
                }
                x0 += SCAN_MAX;
            }
            y0 += SCAN_MAX;
        }
        let mut out = Vec::with_capacity((w * h) as usize);
        for (y, row) in grid.into_iter().enumerate() {
            for (x, t) in row.into_iter().enumerate() {
                out.push(((x as i32, y as i32), t));
            }
        }
        Ok(out)
    }
}

/// Read an integer field from a flat JSON object string (`{"x":5,...}`). Tolerant of whitespace.
fn arg_i32(args: &str, key: &str) -> Option<i32> {
    let k = format!("\"{key}\"");
    let after = &args[args.find(&k)? + k.len()..];
    let tail = after[after.find(':')? + 1..].trim_start();
    let end = tail
        .find(|c: char| !(c.is_ascii_digit() || c == '-'))
        .unwrap_or(tail.len());
    tail[..end].parse().ok()
}

/// Read a boolean field from a flat JSON object string (`{"bare":true}`). Absent or non-`true`
/// reads as `false`, so an omitted flag defaults off.
fn arg_bool(args: &str, key: &str) -> bool {
    let k = format!("\"{key}\"");
    let Some(after) = args.find(&k).map(|i| &args[i + k.len()..]) else {
        return false;
    };
    let Some(tail) = after.find(':').map(|i| after[i + 1..].trim_start()) else {
        return false;
    };
    tail.starts_with("true")
}

/// Read a string field from a flat JSON object string (`{"kind":"terrain"}`).
fn arg_str(args: &str, key: &str) -> Option<String> {
    let k = format!("\"{key}\"");
    let after = &args[args.find(&k)? + k.len()..];
    let tail = &after[after.find(':')? + 1..];
    let q1 = tail.find('"')?;
    let q2 = tail[q1 + 1..].find('"')? + q1 + 1;
    Some(tail[q1 + 1..q2].to_string())
}

// --------------------------------------------------------------------------
// primitive spatial operators — the "basic calculator"
// (interactive-compute-forks-design.md §7 + §1-4 dial)
// --------------------------------------------------------------------------
//
// Deterministic PRIMITIVE measurement verbs the model can call so it never has to do the spatial
// arithmetic in its head. PRIMITIVES ONLY: each returns an exact MEASUREMENT, never a
// selection/answer (no `nearest`, no `best_site`, no dominance solver) — the "choose-among-K" step
// is deliberately WITHHELD (design §3-4, §7), which is what keeps the surviving decision questions
// unsolvable by the calculator alone. Every operator reuses the SAME function the matching question
// kind's ground-truth solver uses, so the calculator is exactly faithful to the scored answers.

/// Chebyshev (king-move) distance between two points — the metric of the `distance` question kind
/// (`civ_core::geometry::chebyshev`, the shared grid semantics).
pub(crate) fn op_distance(x0: i32, y0: i32, x1: i32, y1: i32) -> i32 {
    civ_core::geometry::chebyshev((x0, y0), (x1, y1))
}

/// Count of tiles whose terrain is `terrain` inside the inclusive box, clipped to the board. This
/// is exactly the count `region-count`/`best-site` compute over a Chebyshev-radius window: a
/// Chebyshev ball of radius `r` about `(cx,cy)` IS the inclusive box `(cx-r,cy-r)-(cx+r,cy+r)`
/// clipped to the board (`geometry::tiles_within`), and both filter on `board.tile(x,y).terrain`.
pub(crate) fn op_count_terrain(
    board: &Board,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    terrain: &str,
) -> i64 {
    let mut n = 0i64;
    for y in y0..=y1 {
        for x in x0..=x1 {
            if board.in_bounds(x, y) && board.tile(x, y).terrain == terrain {
                n += 1;
            }
        }
    }
    n
}

/// Count of tiles bearing the `resource` deposit inside the inclusive box, clipped to the board —
/// the same `Tile::has(resource)` membership the `nearest`/`nearest-owned` resource questions use.
/// The caller (`execute`) gates `resource` through [`crate::generators::is_resource`] so a
/// non-resource extra name is rejected rather than silently counted.
pub(crate) fn op_count_resource(
    board: &Board,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    resource: &str,
) -> i64 {
    let mut n = 0i64;
    for y in y0..=y1 {
        for x in x0..=x1 {
            if board.in_bounds(x, y) && board.tile(x, y).has(resource) {
                n += 1;
            }
        }
    }
    n
}

/// Land travel cost in turns from `(x0,y0)` to `(x1,y1)` for a 1-tile/turn land unit, using the
/// SAME terrain-aware land Dijkstra as the reachability questions ([`crate::rules::ReachField`]):
/// 8-connected, ocean impassable, uniform 1 move-point per land tile. The horizon is the whole
/// board, so the only `None` is a genuinely unreachable target (a different land component / across
/// water). At `move_rate = 1` the returned turns equal the raw step-distance that BOTH reach
/// questions are built on (`nearest-owned` uses move 1 verbatim; `reachable-nearest` scales it to a
/// unit that moves `M` tiles/turn as `ceil(turns / M)`).
///
/// When `mover` is `Some(player)` the path is TACTICAL for that player
/// ([`crate::rules::ReachField::from_origins_tactical`]): it additionally cannot enter enemy-occupied
/// tiles or enemy cities and obeys classic enemy Zones of Control — so a fogged run (which threads
/// its perspective player here) is ZOC-aware by default. `None` keeps the pure-geographic fallback
/// for an ownerless point-to-point measurement.
pub(crate) fn op_travel_turns(
    board: &Board,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    mover: Option<&str>,
) -> Option<i32> {
    let horizon = board.width.saturating_mul(board.height).max(1);
    let reach = match mover {
        Some(p) => {
            crate::rules::ReachField::from_origins_tactical(board, &[(x0, y0)], 1, horizon, p)
        }
        None => crate::rules::ReachField::from_origins(board, &[(x0, y0)], 1, horizon),
    };
    reach.turns_at(x1, y1)
}

/// JSON-Schema for the box operators (`count_terrain`/`count_resource` add a `feature` string).
const XYXY_SCHEMA: &str = "{\"type\":\"object\",\"properties\":{\"x0\":{\"type\":\"integer\"},\
     \"y0\":{\"type\":\"integer\"},\"x1\":{\"type\":\"integer\"},\
     \"y1\":{\"type\":\"integer\"}},\"required\":[\"x0\",\"y0\",\"x1\",\"y1\"]}";

/// `travel_turns` schema: the four corners plus an OPTIONAL `player` that turns the measurement
/// ZOC-aware (enemy Zones of Control + enemy-occupied / enemy-city blocking) for that player.
const TRAVEL_TURNS_SCHEMA: &str =
    "{\"type\":\"object\",\"properties\":{\"x0\":{\"type\":\"integer\"},\
     \"y0\":{\"type\":\"integer\"},\"x1\":{\"type\":\"integer\"},\
     \"y1\":{\"type\":\"integer\"},\"player\":{\"type\":\"string\"}},\
     \"required\":[\"x0\",\"y0\",\"x1\",\"y1\"]}";

/// The four PRIMITIVE operator tool definitions (the calculator). Shared by every surface that
/// exposes the operators, so `raw-ops` and `interactive-ops` present byte-identical verbs.
fn operator_tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "distance",
            description: "Chebyshev (king-move) distance in tiles between (x0,y0) and (x1,y1)."
                .to_string(),
            params_schema: XYXY_SCHEMA.to_string(),
        },
        ToolDef {
            name: "travel_turns",
            description: "Land travel cost in turns from (x0,y0) to (x1,y1) for a 1-tile/turn land \
                unit, over land only (ocean impassable; 8-connected). Returns \"unreachable\" if no \
                land path exists. A unit that moves M tiles/turn needs ceil(turns / M). Pass the \
                optional 'player' to make the path TACTICAL for that player: it then also respects \
                enemy Zones of Control and cannot cross enemy-occupied tiles or enemy cities \
                (omit 'player' for a pure-geography measurement)."
                .to_string(),
            params_schema: TRAVEL_TURNS_SCHEMA.to_string(),
        },
        ToolDef {
            name: "count",
            description: "Count of tiles matching 'feature' inside the inclusive box \
                (x0,y0)-(x1,y1), clipped to the board. kind=\"terrain\" counts tiles whose terrain \
                is 'feature' (e.g. Grassland); kind=\"resource\" counts tiles bearing the resource \
                deposit 'feature' (e.g. Wheat)."
                .to_string(),
            params_schema: "{\"type\":\"object\",\"properties\":{\"x0\":{\"type\":\"integer\"},\
                \"y0\":{\"type\":\"integer\"},\"x1\":{\"type\":\"integer\"},\
                \"y1\":{\"type\":\"integer\"},\"kind\":{\"type\":\"string\",\
                \"enum\":[\"terrain\",\"resource\"]},\"feature\":{\"type\":\"string\"}},\
                \"required\":[\"x0\",\"y0\",\"x1\",\"y1\",\"kind\",\"feature\"]}"
                .to_string(),
        },
    ]
}

/// A concise bulleted description of the operator verbs, injected into the calculator surfaces'
/// system preamble.
const OPERATOR_TOOLS_DOC: &str = "\
    - distance(x0,y0,x1,y1): Chebyshev (king-move) distance in tiles between two points.\n\
    - travel_turns(x0,y0,x1,y1,player?): land travel cost in turns from the first point to the \
    second for a 1-tile/turn land unit (ocean impassable, 8-connected); \"unreachable\" if no land \
    path. A unit moving M tiles/turn needs ceil(turns / M). Give the optional player to also respect \
    that player's enemy Zones of Control and enemy-occupied / enemy-city blocking.\n\
    - count(x0,y0,x1,y1,kind,feature): how many tiles in the inclusive box match feature — \
    kind=\"terrain\" counts tiles whose terrain is feature, kind=\"resource\" counts tiles bearing \
    that resource deposit.\n";

/// Parse the four box corners from a call, or return the standard error message.
fn box_args(call: &ToolCall, verb: &str) -> Result<(i32, i32, i32, i32), String> {
    let (Some(x0), Some(y0), Some(x1), Some(y1)) = (
        arg_i32(&call.args_json, "x0"),
        arg_i32(&call.args_json, "y0"),
        arg_i32(&call.args_json, "x1"),
        arg_i32(&call.args_json, "y1"),
    ) else {
        return Err(format!("error: {verb} requires integers x0,y0,x1,y1"));
    };
    Ok((x0, y0, x1, y1))
}

/// Dispatch one operator verb, or `None` if `call` is not an operator (so a composite surface can
/// fall through to its other verbs). Never panics — bad input returns an `error:` string.
fn execute_operator(board: &Board, call: &ToolCall) -> Option<String> {
    match call.name.as_str() {
        "distance" => Some(match box_args(call, "distance") {
            Err(e) => e,
            Ok((x0, y0, x1, y1)) => {
                if !board.in_bounds(x0, y0) || !board.in_bounds(x1, y1) {
                    format!(
                        "error: distance endpoints out of bounds (board is {}x{})",
                        board.width, board.height
                    )
                } else {
                    op_distance(x0, y0, x1, y1).to_string()
                }
            }
        }),
        "travel_turns" => Some(match box_args(call, "travel_turns") {
            Err(e) => e,
            Ok((x0, y0, x1, y1)) => {
                if !board.in_bounds(x0, y0) || !board.in_bounds(x1, y1) {
                    format!(
                        "error: travel_turns endpoints out of bounds (board is {}x{})",
                        board.width, board.height
                    )
                } else {
                    // Optional `player`: with it, the path respects that player's enemy Zones of
                    // Control and enemy-occupied / enemy-city blocking; without it, pure geography.
                    let mover = arg_str(&call.args_json, "player");
                    match op_travel_turns(board, x0, y0, x1, y1, mover.as_deref()) {
                        Some(t) => t.to_string(),
                        None => "unreachable".to_string(),
                    }
                }
            }
        }),
        "count" => Some(match box_args(call, "count") {
            Err(e) => e,
            Ok((x0, y0, x1, y1)) => {
                let Some(feature) = arg_str(&call.args_json, "feature") else {
                    return Some("error: count requires a string 'feature'".to_string());
                };
                if x0 > x1 || y0 > y1 {
                    "error: count needs x0<=x1 and y0<=y1".to_string()
                } else {
                    match arg_str(&call.args_json, "kind").as_deref() {
                        Some("terrain") => {
                            op_count_terrain(board, x0, y0, x1, y1, &feature).to_string()
                        }
                        Some("resource") => {
                            if !crate::generators::is_resource(&feature) {
                                format!("error: {feature:?} is not a known resource deposit")
                            } else {
                                op_count_resource(board, x0, y0, x1, y1, &feature).to_string()
                            }
                        }
                        _ => "error: count requires kind = \"terrain\" or \"resource\"".to_string(),
                    }
                }
            }
        }),
        _ => None,
    }
}

// --------------------------------------------------------------------------
// maximal measurement operators — the "maximal calculator"
// (maximal-calculator-corpus.md; interactive-compute-forks-design.md §7)
// --------------------------------------------------------------------------
//
// The FULL measurement operator set on top of the four primitives: strength (att_eff / def_eff),
// city defense, the graded threat field, the settle-site working-radius axes, and unit-scaled
// reach. Each returns an exact MEASUREMENT over the FROZEN board and reuses the SAME `rules.rs`
// function the matching solver calls, so the calculator is faithful to the scored answer (the
// measurement-parity gate, see tests). SELECTION operators stay WITHHELD — no `best_site`,
// `most_threatened`, `dominates`, `can_vacate`, `attack_axes`, or `city_defense_vacated` — which is
// what keeps the surviving decision kinds (settle-site, cf-vacate, adv-assault-target,
// triage-reinforce, compare-two-attacks) unsolvable by the calculator alone. UNIT-subject operators
// (att_eff/def_eff/garrison_defense/reach_turns) address their unit BY ID — the render prints every
// unit with its `#id`, and stacking (up to 37 units on one late-game tile) makes a first-match on
// the coordinate resolve to the WRONG occupant. CITY-subject operators stay coordinate-addressed:
// Freeciv permits only one city per tile, so a coordinate names a city unambiguously.

/// The unit with the given `id`, if present. The unit operators resolve their subject through this
/// (not first-match on the tile) so a unit buried in a stack is addressed exactly.
fn unit_by_id(board: &Board, id: i32) -> Option<&Unit> {
    board.units.iter().find(|u| u.id == id)
}

/// The city on `(x, y)`, if any. (One city per tile, so a coordinate is an unambiguous key.)
fn city_at(board: &Board, x: i32, y: i32) -> Option<&City> {
    board.cities.iter().find(|c| c.x == x && c.y == y)
}

/// ATTACK strength of the unit with id `id` — `rules::att_eff` verbatim. `None` if no such unit or
/// its type is unmodeled.
pub(crate) fn op_att_eff(board: &Board, id: i32) -> Option<f64> {
    crate::rules::att_eff(unit_by_id(board, id)?)
}

/// The attacker's WIN PROBABILITY striking the defender where the defender stands — `rules::
/// attack_win_prob` verbatim (the HP/firepower race odds, folding in the same veteran/terrain/
/// fortification stack as att_eff/def_eff). This is the odds form of the `compare-two-attacks`
/// favorability axis; it also reproduces the `cf-vacate` / `triage-reinforce` "does the city fall"
/// odds when `defender` is the garrison standing in its own city (its `def_eff` then carries the
/// city-center/walls context). `None` if either id is missing/unmodeled or the defender has no
/// contextual defense.
pub(crate) fn op_win_prob(board: &Board, attacker: i32, defender: i32) -> Option<f64> {
    crate::rules::attack_win_prob(
        board,
        unit_by_id(board, attacker)?,
        unit_by_id(board, defender)?,
    )
}

/// Contextual DEFENSE of the unit with id `id` where it stands (terrain + own-city center/walls) —
/// `rules::unit_def_on_own_tile` verbatim. `None` if no such unit or its type is unmodeled.
pub(crate) fn op_def_eff(board: &Board, id: i32) -> Option<f64> {
    crate::rules::unit_def_on_own_tile(board, unit_by_id(board, id)?)
}

/// Context-free DEFENSE of the unit with id `id` — `rules::def_eff_base` verbatim (base defense ×
/// veteran × health, NO terrain or fortification). This is the exact axis the `unit-strength`
/// solver's DEFENSE comparison uses (`rules::strength(_, StrengthAxis::Defense)`), so a model
/// answering that kind through the calculator gets the ground-truth value rather than the
/// terrain-inflated contextual [`op_def_eff`]. `None` if no such unit or its type is unmodeled.
pub(crate) fn op_def_eff_base(board: &Board, id: i32) -> Option<f64> {
    crate::rules::def_eff_base(unit_by_id(board, id)?)
}

/// The city@`(x, y)`'s defensive strength — `rules::city_defense` verbatim (best defender's
/// `def_eff` incl. terrain/center/walls; 0 if undefended). `None` if no city there.
pub(crate) fn op_city_defense(board: &Board, x: i32, y: i32) -> Option<f64> {
    Some(crate::rules::city_defense(board, city_at(board, x, y)?))
}

/// The probability the city@`(x, y)` FALLS this turn to its scariest incoming attacker, from
/// `player`'s viewpoint — `rules::city_capture_prob` verbatim (the SAME "ease of capture" odds the
/// `t3-threat` solver's argmax and the `adv-assault-target` capture axis score). `player` is the
/// viewpoint whose ENEMIES are the attackers; pass the city's OWN owner to reproduce the scored
/// capture probability (the parity gate). `None` if no city is on the tile.
pub(crate) fn op_city_fall_prob(board: &Board, player: &str, x: i32, y: i32) -> Option<f64> {
    let city = city_at(board, x, y)?;
    let field = crate::rules::ThreatField::compute(board, player);
    Some(crate::rules::city_capture_prob(board, city, &field))
}

/// The `def_eff` the unit with id `unit_id` WOULD have garrisoning the city@`(cx, cy)` —
/// `rules::def_eff_if_garrisoned` verbatim (the counterfactual "after" defense for triage-reinforce).
/// The unit subject is BY ID; the city subject stays by coordinate (one city per tile). `None` if
/// there is no such unit/city or the unit type is unmodeled.
pub(crate) fn op_garrison_defense(board: &Board, unit_id: i32, cx: i32, cy: i32) -> Option<f64> {
    let u = unit_by_id(board, unit_id)?;
    let c = city_at(board, cx, cy)?;
    crate::rules::def_eff_if_garrisoned(u, board, c)
}

/// Graded incoming enemy-LAND threat at `(x, y)` from `player`'s viewpoint — `rules::threat_at`
/// (`ThreatField::at`) verbatim: the scariest single reachable enemy attacker's `att_eff` faded by
/// the turns it needs, 0 past the horizon.
pub(crate) fn op_threat(board: &Board, player: &str, x: i32, y: i32) -> f64 {
    crate::rules::threat_at(board, player, (x, y))
}

/// The four settle-site axes of `(x, y)` for `player` over the working radius — `rules::site_axes`
/// verbatim (`food / production / resources` tallies + `safety` = -threat).
pub(crate) fn op_site_axes(board: &Board, player: &str, x: i32, y: i32) -> crate::rules::SiteAxes {
    crate::rules::site_axes(board, player, (x, y))
}

/// Land travel cost in turns for the unit with id `unit_id` to reach `(tx, ty)`, at that unit's own
/// move rate over the whole board — the unit-scaled generalisation of `travel_turns` (the
/// `reachable-nearest` machinery). The subject is resolved BY ID, so its OWN origin, move rate and
/// CLASS are used (fixing the stacking bug where a first-match resolved to the wrong occupant, and
/// the latent bug where a SEA unit was land-pathed). Outer `None` = no such unit, its type is
/// unmodeled, or it is NOT a land unit (this operator models land movement only, mirroring the
/// `reachable-nearest` solver); inner `None` = unreachable within the whole-board horizon.
pub(crate) fn op_reach_turns(board: &Board, unit_id: i32, tx: i32, ty: i32) -> Option<Option<i32>> {
    let u = unit_by_id(board, unit_id)?;
    let stat = crate::rules::unit_stat(&u.kind)?;
    if stat.class != crate::rules::UnitClass::Land {
        return None; // a land-travel task; non-land subjects are ill-posed (as the solver treats them)
    }
    let horizon = board.width.saturating_mul(board.height).max(1);
    // TACTICAL reach for THIS unit's owner: enemy-occupied tiles, enemy cities and enemy Zones of
    // Control block the path (parity with the `reachable-nearest` solver, which does the same).
    let reach = crate::rules::ReachField::from_origins_tactical(
        board,
        &[(u.x, u.y)],
        stat.move_rate,
        horizon,
        &u.owner,
    );
    Some(reach.turns_at(tx, ty))
}

/// Predicate — the tile at `(x, y)` is DEFENSIBLE (positive terrain defense bonus) — the exact
/// `rules::is_defensible` predicate the `constraint-site` solver ANDs. This is one of the three
/// `constraint_site_ok` conjuncts, exposed so the model can compose the intersection itself.
pub(crate) fn op_is_defensible(board: &Board, x: i32, y: i32) -> bool {
    crate::rules::is_defensible(board, (x, y))
}

/// Predicate — some tile within [`rules::NEAR_WATER_RADIUS`] (Chebyshev) of `(x, y)`, including it,
/// is open water — the exact `rules::is_within_water_radius` conjunct of `constraint_site_ok`.
pub(crate) fn op_within_water_radius(board: &Board, x: i32, y: i32) -> bool {
    crate::rules::is_within_water_radius(board, (x, y))
}

/// Predicate — the tile at `(x, y)` is NOT in enemy territory for `player` (unclaimed or owned by
/// `player`) — the exact `rules::not_enemy_territory` conjunct of `constraint_site_ok`.
pub(crate) fn op_not_enemy_territory(board: &Board, player: &str, x: i32, y: i32) -> bool {
    crate::rules::not_enemy_territory(board, player, (x, y))
}

/// The COVER axis of a retreat to `(x, y)` for `player` — `rules::retreat_cover` verbatim (the
/// tile's `defense_multiplier`: terrain + extras, plus own-city center/walls if `player` holds a
/// city there). The `t3-retreat` cover axis, previously unexposed.
pub(crate) fn op_tile_cover(board: &Board, player: &str, x: i32, y: i32) -> f64 {
    crate::rules::retreat_cover(board, player, (x, y))
}

/// The SUPPORT axis of a retreat to `(x, y)` for `player` — `rules::retreat_support` verbatim: the
/// count of `player`'s cities + units within `SUPPORT_RADIUS`. `exclude` drops one unit id (the
/// retreating unit, so it is not counted as its own backup); `None` counts all. The `t3-retreat`
/// support axis, previously unexposed.
pub(crate) fn op_support(board: &Board, player: &str, x: i32, y: i32, exclude: Option<i32>) -> i64 {
    crate::rules::retreat_support(board, player, (x, y), exclude)
}

/// Render a predicate operator's boolean as `"yes"`/`"no"` for the tool text. The parity gate
/// compares the bool op functions, never this string.
fn fmt_bool(v: bool) -> String {
    if v { "yes" } else { "no" }.to_string()
}

/// 3-decimal float with trailing zeros trimmed (`4.0 -> "4"`, `1.3333 -> "1.333"`). Only affects
/// the tool's TEXT output; the parity gate compares the f64 op functions, never these strings.
fn fmt_f64(v: f64) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// JSON-Schema for a single `(x, y)` operator (city-subject / tile-scoped).
const XY_SCHEMA: &str = "{\"type\":\"object\",\"properties\":{\"x\":{\"type\":\"integer\"},\
     \"y\":{\"type\":\"integer\"}},\"required\":[\"x\",\"y\"]}";

/// JSON-Schema for a unit-subject operator: a single unit `id` (read off the board render's
/// `{unit #<id> ...}` markers). Used by att_eff.
const UNIT_SCHEMA: &str = "{\"type\":\"object\",\"properties\":{\"unit\":{\"type\":\"integer\"}},\
     \"required\":[\"unit\"]}";

/// JSON-Schema for `def_eff`: a unit `id` plus an optional `bare` flag (true = context-free base
/// defense, absent/false = contextual defense on the unit's current tile).
const UNIT_BARE_SCHEMA: &str =
    "{\"type\":\"object\",\"properties\":{\"unit\":{\"type\":\"integer\"},\
     \"bare\":{\"type\":\"boolean\"}},\"required\":[\"unit\"]}";

/// JSON-Schema for a player-scoped `(x, y, player)` operator (threat / site_axes).
const XY_PLAYER_SCHEMA: &str = "{\"type\":\"object\",\"properties\":{\"x\":{\"type\":\"integer\"},\
     \"y\":{\"type\":\"integer\"},\"player\":{\"type\":\"string\"}},\
     \"required\":[\"x\",\"y\",\"player\"]}";

/// JSON-Schema for the `win_prob` operator: an `attacker` and a `defender` unit id.
const ATTACKER_DEFENDER_SCHEMA: &str =
    "{\"type\":\"object\",\"properties\":{\"attacker\":{\"type\":\"integer\"},\
     \"defender\":{\"type\":\"integer\"}},\"required\":[\"attacker\",\"defender\"]}";

/// The maximal measurement operator tool definitions (the full calculator = the four primitives
/// plus these). Shared by `raw-maxops` and `interactive-maxops` so both present identical verbs.
fn maximal_operator_tools() -> Vec<ToolDef> {
    let mut t = operator_tools();
    t.extend([
        ToolDef {
            name: "att_eff",
            description: "ATTACK strength of the unit with the given id (unit type x veteran x \
                health). 'unit' is the unit id shown in the board as {unit #<id> ...}. Error if no \
                unit has that id."
                .to_string(),
            params_schema: UNIT_SCHEMA.to_string(),
        },
        ToolDef {
            name: "def_eff",
            description: "DEFENSE strength of the unit with the given id. Default (bare omitted or \
                false) is CONTEXTUAL — where it stands: base defense x terrain/fortification, plus \
                city-center and City Walls if it is in one of its own cities. Pass bare=true for the \
                CONTEXT-FREE base defense (base def x veteran x health, NO terrain/fortification) — \
                the intrinsic value for a bare unit-vs-unit comparison. 'unit' is the unit id shown \
                in the board as {unit #<id> ...}. Error if no unit has that id."
                .to_string(),
            params_schema: UNIT_BARE_SCHEMA.to_string(),
        },
        ToolDef {
            name: "win_prob",
            description: "The ATTACKER's win probability (0..1) if the unit with id 'attacker' \
                strikes the unit with id 'defender' where the defender stands. Resolves the \
                HP/firepower combat: each round the attacker wins with p = A/(A+D) (A/D are the \
                att_eff/def_eff, with veteran/terrain/fortification/walls folded in), and it must \
                destroy the defender before being destroyed. Use this for a real kill chance — a big \
                strength ratio can still be a coin-flip when the defender has more hitpoints. Both \
                ids are shown in the board as {unit #<id> ...}. Error if either is missing/unmodeled."
                .to_string(),
            params_schema: ATTACKER_DEFENDER_SCHEMA.to_string(),
        },
        ToolDef {
            name: "city_defense",
            description: "Defensive strength of the city on (x, y): the def_eff of its single best \
                defender on the city tile (terrain, city-center, and City Walls included); 0 if \
                undefended. Returns 'unknown' for a city under fog (garrison hidden). Error if no \
                city is there."
                .to_string(),
            params_schema: XY_SCHEMA.to_string(),
        },
        ToolDef {
            name: "city_fall_prob",
            description: "The probability (0..1) the city on (x, y) FALLS this turn to its scariest \
                incoming enemy attacker, from player's viewpoint: that attacker's win probability \
                against the city's best defender, combining the threat and combat-odds model. An \
                undefended city an enemy can reach falls for certain (1); a city no enemy can reach \
                in time is 0. Pass the city's own owner as 'player'. Returns 'unknown' for a city \
                under fog (garrison hidden). Error if no city is there."
                .to_string(),
            params_schema: XY_PLAYER_SCHEMA.to_string(),
        },
        ToolDef {
            name: "garrison_defense",
            description: "The def_eff the unit with id 'unit' WOULD have if it garrisoned the city \
                on (cx, cy) (its base defense scaled by that city tile's terrain/center/walls) — the \
                city's defense after reinforcing it with that unit. 'unit' is the unit id from the \
                board; the city is addressed by coordinate. Error if either is missing."
                .to_string(),
            params_schema: "{\"type\":\"object\",\"properties\":{\"unit\":{\"type\":\"integer\"},\
                \"cx\":{\"type\":\"integer\"},\
                \"cy\":{\"type\":\"integer\"}},\"required\":[\"unit\",\"cx\",\"cy\"]}"
                .to_string(),
        },
        ToolDef {
            name: "threat",
            description: "Incoming enemy LAND threat at (x, y) from player's viewpoint: the \
                scariest single enemy attacker's ATTACK strength divided by the turns it needs to \
                bring an attack to bear (air/sea excluded; 0 past the ~6-turn horizon)."
                .to_string(),
            params_schema: XY_PLAYER_SCHEMA.to_string(),
        },
        // NOTE (2026-08-17): `site_axes` was RETIRED from the maxops menu when settle-site/best-site
        // were dropped from the run — it is the one tool no in-run kind uses (see
        // `analysis/question-design-review.md`). The `run_tool` dispatch handler is kept (so the
        // viewer playground + `run_tool_parity` test are unaffected, and the oracle solver still
        // computes the axes internally); only the model-facing advertisement is removed. Re-add this
        // ToolDef + the prose catalogue line if settle-site ever returns to the run.
        ToolDef {
            name: "tile_cover",
            description: "The retreat COVER of tile (x, y) for player: the tile's defensive \
                multiplier (terrain + fortification, plus the city-center and City Walls bonuses if \
                player holds a city on it). Higher = a tougher tile to be attacked on."
                .to_string(),
            params_schema: XY_PLAYER_SCHEMA.to_string(),
        },
        ToolDef {
            name: "support",
            description: "The retreat SUPPORT at tile (x, y) for player: the number of player's own \
                cities and units within 2 tiles (Chebyshev) of (x, y). Optionally pass 'exclude' = a \
                unit id to leave that unit out of the count (e.g. the unit you are moving, so it is \
                not counted as its own backup)."
                .to_string(),
            params_schema: "{\"type\":\"object\",\"properties\":{\"x\":{\"type\":\"integer\"},\
                \"y\":{\"type\":\"integer\"},\"player\":{\"type\":\"string\"},\
                \"exclude\":{\"type\":\"integer\"}},\"required\":[\"x\",\"y\",\"player\"]}"
                .to_string(),
        },
        ToolDef {
            name: "reach_turns",
            description: "Land travel cost in turns for the unit with id 'unit' to reach (tx, ty) at \
                that unit's own move rate (ocean impassable, 8-connected); \"unreachable\" if no \
                land path. The path is TACTICAL for the unit's owner: it respects enemy Zones of \
                Control and cannot cross enemy-occupied tiles or enemy cities. 'unit' is the unit id \
                from the board; the operator uses that unit's own position, move rate and class \
                (land units only). Error if no unit has that id or it is not a land unit."
                .to_string(),
            params_schema: "{\"type\":\"object\",\"properties\":{\"unit\":{\"type\":\"integer\"},\
                \"tx\":{\"type\":\"integer\"},\
                \"ty\":{\"type\":\"integer\"}},\"required\":[\"unit\",\"tx\",\"ty\"]}"
                .to_string(),
        },
        ToolDef {
            name: "site_check",
            description: "THREE of the four site-constraint predicates for tile (x, y) from player's \
                viewpoint, reported together: defensible (terrain grants a positive defense bonus — \
                Forest, Jungle, Swamp, Hills, Mountains), within_water (some tile within Chebyshev 2 \
                of (x,y), including it, is open water — coastal access), and not_enemy (the tile is \
                unclaimed or owned by player, not by another player). Returns each as yes/no plus \
                ok = all three of THESE true. NOTE: ok does NOT include the minimum city-spacing \
                requirement — a tile can be ok here yet still be an invalid site because it is too \
                close to an existing city, which you must check yourself from the city positions."
                .to_string(),
            params_schema: XY_PLAYER_SCHEMA.to_string(),
        },
    ]);
    t
}

/// Bulleted description of the maximal operators, appended to the calculator surfaces' preamble
/// after [`OPERATOR_TOOLS_DOC`].
const MAXIMAL_OPERATOR_TOOLS_DOC: &str = "\
    - att_eff(unit): ATTACK strength of the unit with that id.\n\
    - def_eff(unit,bare?): DEFENSE strength of the unit with that id. Default is contextual — where \
    it stands (terrain + own-city center/walls); pass bare=true for the context-free base defense \
    (base def x veteran x health, NO terrain/fortification) — the intrinsic value for a bare \
    unit-vs-unit defense comparison.\n\
    - win_prob(attacker,defender): the attacker's win probability (0..1) in the HP/firepower combat \
    if 'attacker' strikes 'defender' where it stands — a real kill chance, not a strength ratio.\n\
    - city_defense(x,y): the city's defense = its best defender's def_eff (terrain/center/walls); 0 \
    if undefended; returns 'unknown' for a city under fog (garrison hidden). (City subject, so by \
    coordinate.)\n\
    - city_fall_prob(x,y,player): the probability (0..1) the city on (x,y) FALLS this turn to its \
    scariest incoming attacker — that attacker's win probability vs the city's best defender \
    (undefended-in-reach = 1, unreachable = 0); returns 'unknown' for a city under fog (garrison \
    hidden). Pass the city's own owner as player.\n\
    - garrison_defense(unit,cx,cy): the def_eff the unit with that id would have if it garrisoned the \
    city on (cx,cy).\n\
    - threat(x,y,player): incoming enemy LAND force at the tile from player's viewpoint (scariest \
    reachable attacker, faded by arrival turns).\n\
    - tile_cover(x,y,player): the retreat COVER of the tile = its defensive multiplier (terrain + \
    fortification, plus own-city center/walls if player has a city there).\n\
    - support(x,y,player,exclude?): the retreat SUPPORT = count of player's cities + units within 2 \
    tiles (Chebyshev) of (x,y). Pass exclude=<unit id> to omit that unit (e.g. the one retreating, \
    so it is not counted as its own backup).\n\
    - reach_turns(unit,tx,ty): land travel turns for the unit with that id to reach (tx,ty) at its \
    own move rate (land units only), respecting enemy Zones of Control and enemy-occupied / \
    enemy-city blocking for the unit's owner.\n\
    - site_check(x,y,player): THREE of the four site-constraint predicates for the tile, together — \
    defensible (terrain defense bonus: Forest, Jungle, Swamp, Hills, Mountains), within_water (open \
    water within Chebyshev 2, coastal access), not_enemy (unclaimed or owned by player). Reports \
    each yes/no plus ok = all three of THESE. ok does NOT cover the minimum city-spacing rule — an \
    ok=yes tile can still be invalid for being too close to a city; compute that yourself.\n";

/// Read the `(x, y)` of a single-tile operator, or the standard error.
fn xy_args(call: &ToolCall, verb: &str) -> Result<(i32, i32), String> {
    let (Some(x), Some(y)) = (arg_i32(&call.args_json, "x"), arg_i32(&call.args_json, "y")) else {
        return Err(format!("error: {verb} requires integers x, y"));
    };
    Ok((x, y))
}

/// Read the single `unit` id of a unit-subject operator, or the standard error.
fn unit_arg(call: &ToolCall, verb: &str) -> Result<i32, String> {
    arg_i32(&call.args_json, "unit")
        .ok_or_else(|| format!("error: {verb} requires an integer 'unit' id"))
}

/// Read the `attacker` and `defender` unit ids of the `win_prob` operator, or the standard error.
fn attacker_defender_args(call: &ToolCall) -> Result<(i32, i32), String> {
    let (Some(a), Some(d)) = (
        arg_i32(&call.args_json, "attacker"),
        arg_i32(&call.args_json, "defender"),
    ) else {
        return Err("error: win_prob requires integer 'attacker' and 'defender' ids".to_string());
    };
    Ok((a, d))
}

/// Read a `(unit, a, b)` triple (e.g. `unit,cx,cy` or `unit,tx,ty`) for a unit-subject operator
/// whose other two args are the named coordinate pair, or the standard error.
fn unit_pair_args(call: &ToolCall, verb: &str, k: [&str; 2]) -> Result<(i32, i32, i32), String> {
    let (Some(u), Some(a), Some(b)) = (
        arg_i32(&call.args_json, "unit"),
        arg_i32(&call.args_json, k[0]),
        arg_i32(&call.args_json, k[1]),
    ) else {
        return Err(format!(
            "error: {verb} requires integers unit, {}, {}",
            k[0], k[1]
        ));
    };
    Ok((u, a, b))
}

/// Dispatch one MAXIMAL operator verb. Tries the four primitives first (so the whole calculator is
/// one dispatch), then the maximal ops; `None` if `call` is none of them (a composite surface then
/// falls through to its other verbs). Never panics — bad input returns an `error:` string.
fn execute_maximal_operator(board: &Board, call: &ToolCall) -> Option<String> {
    if let Some(s) = execute_operator(board, call) {
        return Some(s);
    }
    let oob = |x: i32, y: i32| {
        format!(
            "error: ({x}, {y}) out of bounds (board is {}x{})",
            board.width, board.height
        )
    };
    match call.name.as_str() {
        "att_eff" => Some(match unit_arg(call, "att_eff") {
            Err(e) => e,
            Ok(id) => match op_att_eff(board, id) {
                Some(v) => fmt_f64(v),
                None => no_modeled_unit(board, id),
            },
        }),
        "def_eff" => Some(match unit_arg(call, "def_eff") {
            Err(e) => e,
            // bare=true → context-free base defense; default → contextual defense on the unit's tile.
            Ok(id) => {
                let v = if arg_bool(&call.args_json, "bare") {
                    op_def_eff_base(board, id)
                } else {
                    op_def_eff(board, id)
                };
                match v {
                    Some(v) => fmt_f64(v),
                    None => no_modeled_unit(board, id),
                }
            }
        }),
        "win_prob" => Some(match attacker_defender_args(call) {
            Err(e) => e,
            Ok((att, def)) => match op_win_prob(board, att, def) {
                Some(v) => fmt_f64(v),
                None => format!(
                    "error: win_prob needs a modeled attacker #{att} and a defender #{def} with a \
                     positive contextual defense"
                ),
            },
        }),
        "city_defense" => Some(match xy_args(call, "city_defense") {
            Err(e) => e,
            Ok((x, y)) if !board.in_bounds(x, y) => oob(x, y),
            // A fogged enemy city shows its last-known shell but its live garrison is masked out, so
            // the masked board would read as "undefended" (0) — a misleading number. Report unknown.
            Ok((x, y)) if board.is_fogged(x, y) && city_at(board, x, y).is_some() => format!(
                "unknown: city at ({x}, {y}) is under fog — its garrison is hidden, so its defense \
                 cannot be measured"
            ),
            Ok((x, y)) => match op_city_defense(board, x, y) {
                Some(v) => fmt_f64(v),
                None => format!("error: no city at ({x}, {y})"),
            },
        }),
        "city_fall_prob" => Some(match xy_args(call, "city_fall_prob") {
            Err(e) => e,
            Ok((x, y)) if !board.in_bounds(x, y) => oob(x, y),
            // As city_defense: a fogged city's hidden garrison makes the fall probability unmeasurable.
            Ok((x, y)) if board.is_fogged(x, y) && city_at(board, x, y).is_some() => format!(
                "unknown: city at ({x}, {y}) is under fog — its garrison is hidden, so its defense \
                 cannot be measured"
            ),
            Ok((x, y)) => match arg_str(&call.args_json, "player") {
                Some(p) => match op_city_fall_prob(board, &p, x, y) {
                    Some(v) => fmt_f64(v),
                    None => format!("error: no city at ({x}, {y})"),
                },
                None => "error: city_fall_prob requires a string 'player'".to_string(),
            },
        }),
        "garrison_defense" => Some(
            match unit_pair_args(call, "garrison_defense", ["cx", "cy"]) {
                Err(e) => e,
                Ok((id, cx, cy)) => match op_garrison_defense(board, id, cx, cy) {
                    Some(v) => fmt_f64(v),
                    None => format!(
                        "error: need a modeled unit with id #{id} and a city at ({cx}, {cy})"
                    ),
                },
            },
        ),
        "threat" => Some(match xy_args(call, "threat") {
            Err(e) => e,
            Ok((x, y)) if !board.in_bounds(x, y) => oob(x, y),
            Ok((x, y)) => match arg_str(&call.args_json, "player") {
                Some(p) => fmt_f64(op_threat(board, &p, x, y)),
                None => "error: threat requires a string 'player'".to_string(),
            },
        }),
        "site_axes" => Some(match xy_args(call, "site_axes") {
            Err(e) => e,
            Ok((x, y)) if !board.in_bounds(x, y) => oob(x, y),
            Ok((x, y)) => match arg_str(&call.args_json, "player") {
                Some(p) => {
                    let a = op_site_axes(board, &p, x, y);
                    format!(
                        "food={} production={} resources={} safety={}",
                        a.food,
                        a.production,
                        a.resources,
                        fmt_f64(a.safety)
                    )
                }
                None => "error: site_axes requires a string 'player'".to_string(),
            },
        }),
        "tile_cover" => Some(match xy_args(call, "tile_cover") {
            Err(e) => e,
            Ok((x, y)) if !board.in_bounds(x, y) => oob(x, y),
            Ok((x, y)) => match arg_str(&call.args_json, "player") {
                Some(p) => fmt_f64(op_tile_cover(board, &p, x, y)),
                None => "error: tile_cover requires a string 'player'".to_string(),
            },
        }),
        "support" => Some(match xy_args(call, "support") {
            Err(e) => e,
            Ok((x, y)) if !board.in_bounds(x, y) => oob(x, y),
            Ok((x, y)) => match arg_str(&call.args_json, "player") {
                // Optional 'exclude' unit id (e.g. the retreating unit) dropped from the tally.
                Some(p) => {
                    op_support(board, &p, x, y, arg_i32(&call.args_json, "exclude")).to_string()
                }
                None => "error: support requires a string 'player'".to_string(),
            },
        }),
        "reach_turns" => Some(match unit_pair_args(call, "reach_turns", ["tx", "ty"]) {
            Err(e) => e,
            Ok((_, tx, ty)) if !board.in_bounds(tx, ty) => oob(tx, ty),
            Ok((id, tx, ty)) => match reach_turns_error(board, id) {
                Some(e) => e,
                None => match op_reach_turns(board, id, tx, ty) {
                    Some(Some(t)) => t.to_string(),
                    // Guarded above, so the subject is a modeled land unit here.
                    Some(None) | None => "unreachable".to_string(),
                },
            },
        }),
        "site_check" => Some(match xy_args(call, "site_check") {
            Err(e) => e,
            Ok((x, y)) if !board.in_bounds(x, y) => oob(x, y),
            Ok((x, y)) => match arg_str(&call.args_json, "player") {
                Some(p) => {
                    // All three constraint-site conjuncts, reported separably plus their AND, so the
                    // model can both read each predicate and prove no candidate satisfies all three.
                    let defensible = op_is_defensible(board, x, y);
                    let within_water = op_within_water_radius(board, x, y);
                    let not_enemy = op_not_enemy_territory(board, &p, x, y);
                    format!(
                        "defensible={} within_water={} not_enemy={} ok={}",
                        fmt_bool(defensible),
                        fmt_bool(within_water),
                        fmt_bool(not_enemy),
                        fmt_bool(defensible && within_water && not_enemy),
                    )
                }
                None => "error: site_check requires a string 'player'".to_string(),
            },
        }),
        _ => None,
    }
}

/// The `error:` string for an att_eff/def_eff whose `unit` id resolves to nothing modeled —
/// distinguishing "no such unit" from "present but its type is unmodeled" for a clearer tool result.
fn no_modeled_unit(board: &Board, id: i32) -> String {
    match unit_by_id(board, id) {
        None => format!("error: no unit with id #{id} on the board"),
        Some(u) => format!("error: unit #{id} ({}) is not a modeled unit type", u.kind),
    }
}

/// If the `unit` id is a bad subject for `reach_turns` (missing, unmodeled, or NOT a land unit),
/// the `error:` string explaining why; `None` if it is a valid land-unit subject. Keeps the SEA/AIR
/// case explicit rather than silently land-pathing a ship (the latent bug the root-cause flagged).
fn reach_turns_error(board: &Board, id: i32) -> Option<String> {
    let Some(u) = unit_by_id(board, id) else {
        return Some(format!("error: no unit with id #{id} on the board"));
    };
    let Some(stat) = crate::rules::unit_stat(&u.kind) else {
        return Some(format!(
            "error: unit #{id} ({}) is not a modeled unit type",
            u.kind
        ));
    };
    if stat.class != crate::rules::UnitClass::Land {
        return Some(format!(
            "error: reach_turns models LAND movement; unit #{id} ({}) is a {} unit",
            u.kind,
            stat.class.as_str()
        ));
    }
    None
}

// --------------------------------------------------------------------------
// enumeration / index operators — the "locating calculator"
// (analysis/findings-scale-ceiling.md: the ~0.75 wall is LOCATING, not computing)
// --------------------------------------------------------------------------
//
// Every measurement operator above returns a SCALAR for a position the model must already have; on
// large boards ~92% of failures are LOCATING (pointing the operators at the wrong tile/city, or
// over-fetching to find them). These operators return POSITIONS instead: predicate-filtered
// coordinates that push the LOCATING step into the tool. They stay strictly ENUMERATIVE — each
// returns the full matching set (or a player's raw roster) and NEVER a choice, an argmin, a
// dominance verdict, or the answer to a decision kind; the JUDGMENT/composition over the returned
// positions remains the model's, exactly as the withheld-selection thesis requires. Each reuses the
// same board membership the solvers use, so the parity gate holds. `list_tiles` SUBSUMES the scalar
// `count_terrain`/`count_resource` (its length IS the count), so those two are retired on the
// enumeration surfaces in its favour (no menu bloat — see the enum surfaces below).

/// Max coordinates `list_tiles` will return before asking the model to narrow — bounds the tool
/// output (a scalar count never had this cost). Small predicate/region sweeps (resource positions,
/// a settle radius, a region-count window ≤49) are far under it; only a whole-board unqualified
/// terrain dump approaches it, and narrowing is the right response.
const LIST_MAX: usize = 1024;

/// Coordinates of every in-bounds tile in the inclusive box `(x0,y0)-(x1,y1)` matching the terrain
/// and/or resource predicate, in row-major (y then x) order. `terrain`/`resource` are ANDed when
/// both are given; at least one must be present (enforced by the caller). Iterates the SAME clipped
/// region and the SAME `terrain==`/`Tile::has` predicates as [`op_count_terrain`]/[`op_count_resource`],
/// so `op_list_tiles(box, Some(t), None).len() == op_count_terrain(box, t)` exactly (the subsumption
/// the parity gate proves) — and likewise for a resource predicate.
pub(crate) fn op_list_tiles(
    board: &Board,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    terrain: Option<&str>,
    resource: Option<&str>,
) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            if !board.in_bounds(x, y) {
                continue;
            }
            let t = board.tile(x, y);
            if terrain.is_some_and(|w| t.terrain != w) {
                continue;
            }
            if resource.is_some_and(|r| !t.has(r)) {
                continue;
            }
            out.push((x, y));
        }
    }
    out
}

/// Like [`op_list_tiles`], but ADDITIONALLY drops every tile lying within Chebyshev `exclude_radius`
/// of ANY city owned by `exclude_owner` — the "unexploited"/eligible set for the `nearest-owned`
/// kind. Mirrors the `NearestOwnedResource` solver's exclusion predicate EXACTLY (`question.rs`):
/// the solver keeps a resource tile iff `min over own cities of chebyshev(tile, city) > 2`, i.e.
/// it excludes tiles within the Chebyshev-2 working radius of any own city. Here a tile is kept iff
/// EVERY `exclude_owner` city is strictly farther than `exclude_radius` (equivalently `min > radius`),
/// so passing `exclude_radius == 2` reproduces the solver's set. Own-city membership is
/// `City::owner == exclude_owner` verbatim (all such cities, same as the solver); only the excluding
/// player's OWN cities count, so a resource next to an ENEMY city is retained. When `exclude_owner`
/// owns no city, nothing is excluded (the base predicate set is returned unchanged). The travel-turn
/// reach/horizon and the final min-argmin stay the model's job — this operator is a pure FILTER over
/// the SAME board membership the solver iterates, so the returned set is faithful to the scored
/// candidate set (proven by the parity test).
#[allow(clippy::too_many_arguments)]
pub(crate) fn op_list_tiles_excluding_owned(
    board: &Board,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    terrain: Option<&str>,
    resource: Option<&str>,
    exclude_owner: &str,
    exclude_radius: i32,
) -> Vec<(i32, i32)> {
    let cities: Vec<(i32, i32)> = board
        .cities
        .iter()
        .filter(|c| c.owner == exclude_owner)
        .map(|c| (c.x, c.y))
        .collect();
    op_list_tiles(board, x0, y0, x1, y1, terrain, resource)
        .into_iter()
        .filter(|&(tx, ty)| {
            cities
                .iter()
                .all(|&c| chebyshev((tx, ty), c) > exclude_radius)
        })
        .collect()
}

/// Min land-travel turns (settler-like move 1, 8-connected, land-only) from `player`'s NEAREST own
/// city to `(tx, ty)`, within [`crate::generators::NEAREST_OWNED_HORIZON`] turns — the MULTI-SOURCE
/// analogue of [`op_travel_turns`], and the EXACT inner op of the `NearestOwnedResource` solver
/// (`question.rs`): `ReachField::from_origins_tactical(board, own_cities, 1, horizon, player)
/// .turns_at(tx, ty)` — ZOC-aware for `player`.
/// Returns `None` when no own city can land-reach the tile within the horizon (including when
/// `player` owns no city → empty origin set → nothing reachable, matching the solver's ill-posed
/// case). The eligible-set filter (`list_tiles` with `exclude_owner`/`exclude_radius`) and the final
/// argmin over eligible tiles stay the model's job; this operator supplies only the per-tile
/// nearest-city cost that the point-to-point `travel_turns` forced the model to reconstruct
/// city-by-city (the tool-parity gap in `analysis/findings-tool-parity.md`). The baked-in horizon
/// keeps it in strict one-shot parity with the solver, which builds its field at the same horizon.
pub(crate) fn op_travel_turns_from_owned(
    board: &Board,
    player: &str,
    tx: i32,
    ty: i32,
) -> Option<i32> {
    let cities: Vec<(i32, i32)> = board
        .cities
        .iter()
        .filter(|c| c.owner == player)
        .map(|c| (c.x, c.y))
        .collect();
    // TACTICAL multi-source reach for `player` (own cities are the origins): enemy Zones of Control
    // and enemy-occupied / enemy-city blocking apply, in strict parity with the `nearest-owned`
    // solver, which builds its field the same way.
    let reach = crate::rules::ReachField::from_origins_tactical(
        board,
        &cities,
        1,
        crate::generators::NEAREST_OWNED_HORIZON,
        player,
    );
    reach.turns_at(tx, ty)
}

/// The cities owned by `player`, sorted by `(y, x)` — the raw-side analogue of the interactive
/// `list_cities` verb, filtered to one owner. Membership is `City::owner == player` verbatim.
fn owned_cities_sorted<'a>(board: &'a Board, player: &str) -> Vec<&'a City> {
    let mut cs: Vec<&City> = board.cities.iter().filter(|c| c.owner == player).collect();
    cs.sort_by_key(|c| (c.y, c.x));
    cs
}

/// The units owned by `player`, sorted by `(y, x)` — the raw-side analogue of `list_units`,
/// filtered to one owner. Membership is `Unit::owner == player` verbatim.
fn owned_units_sorted<'a>(board: &'a Board, player: &str) -> Vec<&'a Unit> {
    let mut us: Vec<&Unit> = board.units.iter().filter(|u| u.owner == player).collect();
    us.sort_by_key(|u| (u.y, u.x));
    us
}

/// Coordinates of `player`'s cities (row-major). The parity target for `list_owned_cities`.
pub(crate) fn op_owned_cities(board: &Board, player: &str) -> Vec<(i32, i32)> {
    owned_cities_sorted(board, player)
        .iter()
        .map(|c| (c.x, c.y))
        .collect()
}

/// JSON-Schema for `list_tiles`: the four box corners plus optional `terrain`/`resource` predicates.
const LIST_TILES_SCHEMA: &str =
    "{\"type\":\"object\",\"properties\":{\"x0\":{\"type\":\"integer\"},\
     \"y0\":{\"type\":\"integer\"},\"x1\":{\"type\":\"integer\"},\"y1\":{\"type\":\"integer\"},\
     \"terrain\":{\"type\":\"string\"},\"resource\":{\"type\":\"string\"},\
     \"exclude_owner\":{\"type\":\"string\"},\"exclude_radius\":{\"type\":\"integer\"}},\
     \"required\":[\"x0\",\"y0\",\"x1\",\"y1\"]}";

/// JSON-Schema for a player-scoped roster operator (`list_owned_cities`/`list_owned_units`).
const PLAYER_SCHEMA: &str =
    "{\"type\":\"object\",\"properties\":{\"player\":{\"type\":\"string\"}},\"required\":[\"player\"]}";

/// JSON-Schema for `travel_turns_from_owned`: a player plus a single target tile `(tx, ty)`.
const PLAYER_XY_SCHEMA: &str =
    "{\"type\":\"object\",\"properties\":{\"player\":{\"type\":\"string\"},\
     \"tx\":{\"type\":\"integer\"},\"ty\":{\"type\":\"integer\"}},\
     \"required\":[\"player\",\"tx\",\"ty\"]}";

/// The enumeration/index operator tool definitions. Shared by `raw-maxops-enum` and
/// `interactive-maxops-enum` so both present byte-identical enumeration verbs.
fn enumeration_operator_tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "list_tiles",
            description: "The COORDINATES of every tile in the inclusive box (x0,y0)-(x1,y1) \
                (clipped to the board) matching the given predicate: pass 'terrain' and/or \
                'resource' (at least one; both = tiles matching BOTH). Returns the positions AND \
                their count, so its length replaces count_terrain/count_resource. Optionally pass \
                BOTH 'exclude_owner' (a player) and 'exclude_radius' (an integer) to additionally \
                DROP every matching tile within that Chebyshev radius of any city that player owns — \
                e.g. exclude_owner=<you>, exclude_radius=2 returns only resource tiles OUTSIDE your \
                cities' working radius (the 'unexploited' set), doing the proximity exclusion inside \
                the tool. Up to 1024 tiles; narrow the box or add a predicate if it reports more."
                .to_string(),
            params_schema: LIST_TILES_SCHEMA.to_string(),
        },
        ToolDef {
            name: "list_owned_cities",
            description: "The COORDINATES of every city owned by the given player, and the count."
                .to_string(),
            params_schema: PLAYER_SCHEMA.to_string(),
        },
        ToolDef {
            name: "list_owned_units",
            description: "Every unit owned by the given player as '#<id> (x, y)', and the count. \
                Pass a returned id to att_eff/def_eff/reach_turns for that unit's stats/reach."
                .to_string(),
            params_schema: PLAYER_SCHEMA.to_string(),
        },
        ToolDef {
            name: "travel_turns_from_owned",
            description: "Land travel cost in turns from the NEAREST city owned by 'player' to \
                (tx,ty) for a 1-tile/turn land unit (ocean impassable, 8-connected), or \
                \"unreachable\" if no city that player owns can reach it within 6 turns. The path is \
                TACTICAL for 'player': it respects enemy Zones of Control and cannot cross \
                enemy-occupied tiles or enemy cities. This is travel_turns measured from that \
                player's WHOLE set of cities at once — call it ONCE per candidate tile to get that \
                tile's distance from the nearest own city, instead of looping travel_turns over \
                every (tile, city) pair."
                .to_string(),
            params_schema: PLAYER_XY_SCHEMA.to_string(),
        },
    ]
}

/// Bulleted description of the enumeration operators, appended to the enum surfaces' preamble.
const ENUMERATION_OPERATOR_TOOLS_DOC: &str = "\
    - list_tiles(x0,y0,x1,y1,terrain?,resource?,exclude_owner?,exclude_radius?): the COORDINATES of \
    every tile in the inclusive box matching the terrain and/or resource predicate (give at least \
    one; both are ANDed). The list's length is the count, so this replaces the \
    count tool. Optionally give BOTH exclude_owner (a player) and exclude_radius \
    (an integer) to also DROP tiles within that Chebyshev radius of any of that player's cities: \
    exclude_owner=<you>, exclude_radius=2 yields resource tiles OUTSIDE your cities' working radius \
    (the unexploited set) — let the tool do the proximity exclusion instead of checking each \
    resource against each city by hand. Box may span the whole board (clipped); up to 1024 results.\n\
    - list_owned_cities(player): the coordinates of every city that player owns.\n\
    - list_owned_units(player): every unit that player owns as '#<id> (x, y)' — feed an id to the \
    unit operators (att_eff/def_eff/reach_turns).\n\
    - travel_turns_from_owned(player,tx,ty): land travel turns from that player's NEAREST city to \
    (tx,ty) (1 tile/turn, ocean impassable, 8-connected), or \"unreachable\" beyond 6 turns. Call it \
    ONCE per candidate tile to rank tiles by distance from your nearest city — don't loop \
    travel_turns over every (tile, city) pair.\n";

/// Enumeration doc for the INTERACTIVE enum surface (`interactive-maxops-enum`): identical to
/// [`ENUMERATION_OPERATOR_TOOLS_DOC`] but WITHOUT the `list_owned_cities`/`list_owned_units` bullets.
/// On the interactive-maxops family the front-loaded roster already enumerates every visible entity
/// by coordinate (with unit ids), so the owned-enumerators are redundant there and are dropped from
/// the menu — this doc must not advertise tools the surface no longer offers.
const INTERACTIVE_ENUMERATION_OPERATOR_TOOLS_DOC: &str = "\
    - list_tiles(x0,y0,x1,y1,terrain?,resource?,exclude_owner?,exclude_radius?): the COORDINATES of \
    every tile in the inclusive box matching the terrain and/or resource predicate (give at least \
    one; both are ANDed). The list's length is the count, so this replaces the \
    count tool. Optionally give BOTH exclude_owner (a player) and exclude_radius \
    (an integer) to also DROP tiles within that Chebyshev radius of any of that player's cities: \
    exclude_owner=<you>, exclude_radius=2 yields resource tiles OUTSIDE your cities' working radius \
    (the unexploited set) — let the tool do the proximity exclusion instead of checking each \
    resource against each city by hand. Box may span the whole board (clipped); up to 1024 results.\n\
    - travel_turns_from_owned(player,tx,ty): land travel turns from that player's NEAREST city to \
    (tx,ty) (1 tile/turn, ocean impassable, 8-connected), or \"unreachable\" beyond 6 turns. Call it \
    ONCE per candidate tile to rank tiles by distance from your nearest city — don't loop \
    travel_turns over every (tile, city) pair.\n";

/// Documentation for the measurement half of the enum surfaces (the primitives WITHOUT the retired
/// `count_terrain`/`count_resource`, plus the maximal operators). Kept separate from
/// [`OPERATOR_TOOLS_DOC`] so the shipped `raw-ops`/`interactive-ops` preambles stay untouched.
const ENUM_SURFACE_MEASURE_DOC: &str = "\
    - distance(x0,y0,x1,y1): Chebyshev (king-move) distance in tiles between two points.\n\
    - travel_turns(x0,y0,x1,y1): land travel cost in turns from the first point to the second for a \
    1-tile/turn land unit (ocean impassable, 8-connected); \"unreachable\" if no land path. A unit \
    moving M tiles/turn needs ceil(turns / M).\n";

/// Dispatch one ENUMERATION operator verb, or `None` if `call` is not one (so a composite surface
/// can fall through). Never panics — bad input returns an `error:` string.
fn execute_enumeration_operator(board: &Board, call: &ToolCall) -> Option<String> {
    match call.name.as_str() {
        "list_tiles" => Some(match box_args(call, "list_tiles") {
            Err(e) => e,
            Ok((x0, y0, x1, y1)) => {
                if x0 > x1 || y0 > y1 {
                    return Some("error: list_tiles needs x0<=x1 and y0<=y1".to_string());
                }
                let terrain = arg_str(&call.args_json, "terrain");
                let resource = arg_str(&call.args_json, "resource");
                if terrain.is_none() && resource.is_none() {
                    return Some(
                        "error: list_tiles requires at least one of 'terrain' or 'resource'"
                            .to_string(),
                    );
                }
                if let Some(r) = &resource {
                    if !crate::generators::is_resource(r) {
                        return Some(format!("error: {r:?} is not a known resource deposit"));
                    }
                }
                // Optional proximity-exclusion filter: BOTH exclude_owner and exclude_radius, or
                // NEITHER. One without the other is a malformed call (clear error, never a panic).
                let exclude_owner = arg_str(&call.args_json, "exclude_owner");
                let exclude_radius = arg_i32(&call.args_json, "exclude_radius");
                let coords = match (exclude_owner, exclude_radius) {
                    (None, None) => op_list_tiles(
                        board,
                        x0,
                        y0,
                        x1,
                        y1,
                        terrain.as_deref(),
                        resource.as_deref(),
                    ),
                    (Some(owner), Some(radius)) => {
                        if radius < 0 {
                            return Some(
                                "error: list_tiles exclude_radius must be >= 0".to_string(),
                            );
                        }
                        op_list_tiles_excluding_owned(
                            board,
                            x0,
                            y0,
                            x1,
                            y1,
                            terrain.as_deref(),
                            resource.as_deref(),
                            &owner,
                            radius,
                        )
                    }
                    _ => {
                        return Some(
                            "error: list_tiles exclusion needs BOTH 'exclude_owner' and \
                             'exclude_radius' (or neither)"
                                .to_string(),
                        )
                    }
                };
                if coords.len() > LIST_MAX {
                    return Some(format!(
                        "error: list_tiles matched {} tiles (max {LIST_MAX}); narrow the box or add \
                         a predicate",
                        coords.len()
                    ));
                }
                fmt_coords("tiles", &coords)
            }
        }),
        "list_owned_cities" => Some(match arg_str(&call.args_json, "player") {
            None => "error: list_owned_cities requires a string 'player'".to_string(),
            Some(p) => fmt_coords("cities", &op_owned_cities(board, &p)),
        }),
        "list_owned_units" => Some(match arg_str(&call.args_json, "player") {
            None => "error: list_owned_units requires a string 'player'".to_string(),
            // Units carry their id (not just a coordinate): the unit operators are addressed BY ID,
            // and a coordinate alone can't name one unit in a stack. So the locate→measure chain is
            // list_owned_units -> #id -> att_eff/def_eff/reach_turns(unit=#id).
            Some(p) => fmt_units(&owned_units_sorted(board, &p)),
        }),
        "travel_turns_from_owned" => Some(
            match (
                arg_str(&call.args_json, "player"),
                arg_i32(&call.args_json, "tx"),
                arg_i32(&call.args_json, "ty"),
            ) {
                (Some(p), Some(tx), Some(ty)) => {
                    if !board.in_bounds(tx, ty) {
                        format!(
                            "error: travel_turns_from_owned target out of bounds (board is {}x{})",
                            board.width, board.height
                        )
                    } else {
                        match op_travel_turns_from_owned(board, &p, tx, ty) {
                            Some(t) => t.to_string(),
                            None => "unreachable".to_string(),
                        }
                    }
                }
                _ => "error: travel_turns_from_owned requires 'player' (string), 'tx' and 'ty' \
                      (integers)"
                    .to_string(),
            },
        ),
        _ => None,
    }
}

/// Format an enumeration result as `"N <noun>: (x, y), ..."` (or `"0 <noun>"`), so the count leads
/// (the model takes it directly) and the positions follow.
fn fmt_coords(noun: &str, coords: &[(i32, i32)]) -> String {
    if coords.is_empty() {
        return format!("0 {noun}");
    }
    let body = coords
        .iter()
        .map(|(x, y)| format!("({x}, {y})"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{} {noun}: {body}", coords.len())
}

/// Format a unit roster as `"N units: #<id> (x, y), ..."` (or `"0 units"`) — id first so the model
/// can feed it straight into the by-id unit operators, position second so it can also locate.
fn fmt_units(units: &[&Unit]) -> String {
    if units.is_empty() {
        return "0 units".to_string();
    }
    let body = units
        .iter()
        .map(|u| format!("#{} ({}, {})", u.id, u.x, u.y))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{} units: {body}", units.len())
}

/// The MAXIMAL measurement calculator with `count_terrain`/`count_resource` RETIRED (subsumed by
/// `list_tiles`) and the enumeration/index group ADDED — the operator set of the enumeration
/// surfaces. Shared by `raw-maxops-enum` and `interactive-maxops-enum`.
fn maximal_enum_operator_tools() -> Vec<ToolDef> {
    let mut t: Vec<ToolDef> = maximal_operator_tools()
        .into_iter()
        .filter(|d| d.name != "count")
        .collect();
    t.extend(enumeration_operator_tools());
    t
}

/// Dispatch one operator on the ENUMERATION surfaces: the maximal calculator MINUS the retired
/// `count_terrain`/`count_resource` (both fall through to the surface's unknown-tool handling on
/// this surface), PLUS the enumeration/index group. `None` if `call` is none of them.
fn execute_maximal_enum_operator(board: &Board, call: &ToolCall) -> Option<String> {
    // count is retired here (subsumed by list_tiles); make it unknown rather than answered.
    if call.name == "count" {
        return None;
    }
    if let Some(s) = execute_enumeration_operator(board, call) {
        return Some(s);
    }
    execute_maximal_operator(board, call)
}

/// Run ANY of the eval's model-facing tools against `board`, returning the tool's exact output string
/// — the SINGLE dispatch that unions the whole tool surface. This is what the offline viewer's tool
/// playground calls (through the `civ-wasm` shim) so a human can invoke any tool, parameterized,
/// against whatever board is displayed, with byte-for-byte parity to what a model would see.
///
/// It OR-combines the four dispatchers in priority order:
///   1. [`execute_maximal_operator`] — the measurement calculator, which itself chains
///      [`execute_operator`] first (so `distance`/`travel_turns`/`count_terrain`/`count_resource`
///      are covered here, NOT retired as they are on the enum surfaces).
///   2. [`execute_enumeration_operator`] — the locating calculator (`list_tiles`, `list_owned_*`,
///      `travel_turns_from_owned`).
///   3. [`Interactive::execute`] — the perception/fetch verbs (`region_summary`, `scan`, `scan_grid`,
///      `get_tile`, `list_cities`, `list_units`), which also owns the terminal `error: unknown tool`
///      string for anything unrecognized.
///
/// Never panics — every dispatcher returns an `error:` string on bad input. Ambient state is nil:
/// "which player" is always an explicit tool argument, so `board` is the only context needed.
pub fn run_tool(board: &Board, call: &ToolCall) -> String {
    if let Some(s) = execute_scan_region(board, call) {
        return s;
    }
    if let Some(s) = execute_maximal_operator(board, call) {
        return s;
    }
    if let Some(s) = execute_enumeration_operator(board, call) {
        return s;
    }
    Interactive.execute(board, call)
}

// --------------------------------------------------------------------------
// trimmed spatial "calculator" — the dispersed-kind surfaces (raw-calc / raw-calc-enum)
// --------------------------------------------------------------------------
//
// The three dispersed decision kinds (nearest-owned, reachable-nearest, region-count) only ever use
// SPATIAL-LOGISTICS operators — distance, travel_turns, reach_turns, and count/list. The six
// combat/valuation operators (att_eff/def_eff/city_defense/garrison_defense/threat/site_axes) are
// dead weight for them and mildly worsen tool-dither. These two surfaces reuse the SAME operator
// IMPLEMENTATIONS as raw-maxops / raw-maxops-enum but expose ONLY the spatial ones — a maximally
// clean experiment with no dead ops. `raw-calc` = `raw-ops` PLUS the unit move-rate op `reach_turns`
// the primitive set lacked; `raw-calc-enum` = distance/travel_turns/reach_turns PLUS the enumeration
// group with count_* retired for list_tiles (same rule as raw-maxops-enum). No combat ops, no fetch
// verbs. Per B1, the unit-subject operator `reach_turns` addresses its unit BY ID (read from the
// render's `{unit #<id> ...}` markers), not by coordinate.

/// The minimal spatial calculator: the four primitives PLUS `reach_turns` (unit-scaled travel), and
/// nothing else. The `reach_turns` def is reused verbatim from [`maximal_operator_tools`].
fn calc_operator_tools() -> Vec<ToolDef> {
    let mut t = operator_tools();
    t.extend(
        maximal_operator_tools()
            .into_iter()
            .filter(|d| d.name == "reach_turns"),
    );
    t
}

/// The spatial calculator with `count_terrain`/`count_resource` RETIRED (subsumed by `list_tiles`)
/// and the enumeration/index group ADDED: distance, travel_turns, reach_turns, list_tiles,
/// list_owned_cities, list_owned_units. The enum analogue of [`calc_operator_tools`].
fn calc_enum_operator_tools() -> Vec<ToolDef> {
    let mut t: Vec<ToolDef> = calc_operator_tools()
        .into_iter()
        .filter(|d| d.name != "count")
        .collect();
    t.extend(enumeration_operator_tools());
    t
}

/// Bulleted description of `reach_turns` (the one maximal op the calc surfaces carry), appended to
/// their preamble after the primitive/enumeration docs. Documents the B1 by-id addressing.
const REACH_TURNS_DOC: &str = "\
    - reach_turns(unit,tx,ty): land travel turns for the unit with that id to reach (tx,ty) at its \
    own move rate (ocean impassable, 8-connected; land units only), respecting enemy Zones of \
    Control and enemy-occupied / enemy-city blocking for the unit's owner. It takes a UNIT ID — the \
    number shown in the board as {unit #<id> ...}, not a coordinate, since a tile may hold several \
    stacked units.\n";

/// Enumeration doc for the trimmed `raw-calc-enum` surface: identical to
/// [`ENUMERATION_OPERATOR_TOOLS_DOC`] except the `list_owned_units` feed-hint names ONLY
/// `reach_turns` — att_eff/def_eff are NOT exposed here, so the preamble must not claim them.
const CALC_ENUMERATION_OPERATOR_TOOLS_DOC: &str = "\
    - list_tiles(x0,y0,x1,y1,terrain?,resource?,exclude_owner?,exclude_radius?): the COORDINATES of \
    every tile in the inclusive box matching the terrain and/or resource predicate (give at least \
    one; both are ANDed). The list's length is the count, so this replaces the \
    count tool. Optionally give BOTH exclude_owner (a player) and exclude_radius \
    (an integer) to also DROP tiles within that Chebyshev radius of any of that player's cities: \
    exclude_owner=<you>, exclude_radius=2 yields resource tiles OUTSIDE your cities' working radius \
    (the unexploited set) — let the tool do the proximity exclusion instead of checking each \
    resource against each city by hand. Box may span the whole board (clipped); up to 1024 results.\n\
    - list_owned_cities(player): the coordinates of every city that player owns.\n\
    - list_owned_units(player): every unit that player owns as '#<id> (x, y)' — feed an id to \
    reach_turns.\n\
    - travel_turns_from_owned(player,tx,ty): land travel turns from that player's NEAREST city to \
    (tx,ty) (1 tile/turn, ocean impassable, 8-connected), or \"unreachable\" beyond 6 turns. Call it \
    ONCE per candidate tile to rank tiles by distance from your nearest city — don't loop \
    travel_turns over every (tile, city) pair.\n";

/// Dispatch one operator on the trimmed calculator surfaces: the four primitives PLUS `reach_turns`.
/// The combat/valuation operators stay WITHHELD (fall through to the surface's unknown-tool error).
/// `None` if `call` is none of the five.
fn execute_calc_operator(board: &Board, call: &ToolCall) -> Option<String> {
    if let Some(s) = execute_operator(board, call) {
        return Some(s);
    }
    // Only reach_turns is borrowed from the maximal set; att_eff/def_eff/... stay withheld.
    if call.name == "reach_turns" {
        return execute_maximal_operator(board, call);
    }
    None
}

/// Dispatch on the enum calc surface: count_* retired (fall through to unknown), then the
/// enumeration/index group, then the trimmed spatial calculator (distance/travel_turns/reach_turns).
/// `None` if `call` is none of the six.
fn execute_calc_enum_operator(board: &Board, call: &ToolCall) -> Option<String> {
    // count is retired here (subsumed by list_tiles); make it unknown rather than answered.
    if call.name == "count" {
        return None;
    }
    if let Some(s) = execute_enumeration_operator(board, call) {
        return Some(s);
    }
    execute_calc_operator(board, call)
}

// --------------------------------------------------------------------------
// raw-ops — the FULL raw board block as context + the operator calculator (NO fetch verbs)
// --------------------------------------------------------------------------
//
// The centerpiece calculator surface: "model sees everything + a calculator". `overview` is the
// complete `RawEncoder` render (information-complete on its own), and the ONLY tools are the four
// primitive operators — no perception/fetch verbs. Tests whether the interactive deficit is
// arithmetic (a calculator over a fully-visible board closes it) vs genuine spatial reasoning.
pub struct RawOps;

impl QueryableSurface for RawOps {
    fn name(&self) -> &str {
        "raw-ops"
    }

    fn overview(&self, board: &Board) -> String {
        RawEncoder.render(board)
    }

    fn tools(&self) -> Vec<ToolDef> {
        operator_tools()
    }

    fn execute(&self, board: &Board, call: &ToolCall) -> String {
        execute_operator(board, call)
            .unwrap_or_else(|| format!("error: unknown tool {:?} (this surface exposes only the operators: distance, travel_turns, count)", call.name))
    }

    fn reconstruct_via_tools(&self, board: &Board) -> Result<RecoveredBoard, String> {
        // The overview IS the full raw board block, so completeness is proven by decoding it with
        // the raw encoder's own decoder — the operators carry no per-tile facts of their own.
        RawEncoder.reconstruct(&self.overview(board))
    }

    fn system_preamble(&self) -> String {
        format!(
            "You are answering a question about a Freeciv game board. The COMPLETE board is given \
             below — you can see every tile at once. You ALSO have a set of deterministic \
             measurement tools (a calculator) that compute exact spatial quantities for you, so you \
             never have to do the spatial arithmetic in your head.\n\
             Coordinates: x increases east (0..width-1), y increases south (0..height-1); y=0 is the \
             north edge.\n\n\
             Measurement tools (each returns an exact value computed from the board):\n\
             {OPERATOR_TOOLS_DOC}\n\
             Prefer calling these tools to measure distances, counts, and travel costs rather than \
             counting or estimating by eye. When ready, reply with a line starting exactly \
             `Answer: ` followed by your answer, and nothing after it.\n\n\
             Board (every tile):\n"
        )
    }
}

// --------------------------------------------------------------------------
// interactive-ops — the interactive fetch surface PLUS the operator calculator
// --------------------------------------------------------------------------
//
// The existing `Interactive` perception verbs (overview/region_summary/scan_grid/scan/get_tile/
// list_*) AND the four operators. The model both navigates coarse-to-fine and offloads the
// arithmetic. Everything but the operators delegates to `Interactive`, so its behavior is identical
// to the `interactive` surface plus the extra verbs.
pub struct InteractiveOps;

impl QueryableSurface for InteractiveOps {
    fn name(&self) -> &str {
        "interactive-ops"
    }

    fn overview(&self, board: &Board) -> String {
        Interactive.overview(board)
    }

    fn tools(&self) -> Vec<ToolDef> {
        let mut t = Interactive.tools();
        t.extend(operator_tools());
        t
    }

    fn execute(&self, board: &Board, call: &ToolCall) -> String {
        // Operators first; anything else is a fetch verb handled by the interactive surface (which
        // also owns the "unknown tool" error).
        execute_operator(board, call).unwrap_or_else(|| Interactive.execute(board, call))
    }

    fn reconstruct_via_tools(&self, board: &Board) -> Result<RecoveredBoard, String> {
        // The fetch verbs are inherited verbatim, so completeness is the interactive surface's gate.
        Interactive.reconstruct_via_tools(board)
    }

    fn system_preamble(&self) -> String {
        format!(
            "You are answering a question about a Freeciv game board. You CANNOT see the whole board \
             at once — you explore it through tools, zooming from a coarse overview down to exact \
             tiles. You ALSO have a calculator: deterministic measurement tools that compute exact \
             spatial quantities so you never have to do the arithmetic in your head.\n\
             Coordinates: x increases east (0..width-1), y increases south (0..height-1); y=0 is the \
             north edge.\n\n\
             Perception tools (costs differ — summaries are cheap and approximate; \
             scan/scan_grid/get_tile are exact but you pay per tile):\n\
             - region_summary(x,y): the EXACT occupants (every city + unit with strengths), resources, and dominant terrain of the 10x10 area centered on (x,y), in ONE call (shifted to stay on-board at an edge).\n\
             - scan_grid(x0,y0,x1,y1): a compact glyph grid + exact detail for an inclusive box, in \
             ONE call (no size cap — it can cover a whole area at ~1 char/tile).\n\
             - scan(x0,y0,x1,y1): exact facts of every non-default tile in an inclusive box (max 12x12).\n\
             - get_tile(x,y): exact facts of one tile.\n\
             - list_cities / list_units: enumerate every visible city / unit by coordinate.\n\n\
             Measurement tools (each returns an exact value computed from the board):\n\
             {OPERATOR_TOOLS_DOC}\n\
             Fetch the facts you need, then use the measurement tools to compute distances, counts, \
             and travel costs rather than doing the arithmetic by eye. When ready, reply with a line \
             starting exactly `Answer: ` followed by your answer, and nothing after it.\n\n\
             Board overview:\n"
        )
    }
}

// --------------------------------------------------------------------------
// raw-maxops — the FULL raw board block + the MAXIMAL calculator (NO fetch verbs)
// --------------------------------------------------------------------------
//
// The centrepiece maximal-calculator surface: "model sees everything + the full measurement
// calculator". `overview` is the complete `RawEncoder` render (information-complete on its own);
// the tools are the four primitives PLUS the maximal measurement operators (att_eff/def_eff/
// city_defense/garrison_defense/threat/site_axes/reach_turns) — but no perception/fetch verbs and no
// SELECTION operator. The primary arm for the surviving decision corpus (global judgments over full
// perception), mirroring `raw-ops`.
pub struct RawMaxOps;

impl QueryableSurface for RawMaxOps {
    fn name(&self) -> &str {
        "raw-maxops"
    }

    fn overview(&self, board: &Board) -> String {
        RawEncoder.render(board)
    }

    fn tools(&self) -> Vec<ToolDef> {
        maximal_operator_tools()
    }

    fn execute(&self, board: &Board, call: &ToolCall) -> String {
        execute_maximal_operator(board, call).unwrap_or_else(|| {
            let tools = maximal_operator_tools()
                .iter()
                .map(|t| t.name)
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "error: unknown tool {:?} (this surface exposes the measurement calculator only: \
                 {tools})",
                call.name
            )
        })
    }

    fn reconstruct_via_tools(&self, board: &Board) -> Result<RecoveredBoard, String> {
        // The overview IS the full raw board block; the operators carry no per-tile facts of their
        // own, so completeness is the raw encoder's own decoder (as for `raw-ops`).
        RawEncoder.reconstruct(&self.overview(board))
    }

    fn system_preamble(&self) -> String {
        format!(
            "You are answering a question about a Freeciv game board. The full board block below is \
             given at once (no exploration needed); fogged and never-explored tiles are marked in \
             it as described. You ALSO have a full set of deterministic \
             MEASUREMENT tools (a calculator) that compute exact spatial and combat quantities for \
             you, so you never have to do the arithmetic in your head. The tools MEASURE the board \
             as it is; the DECISION or JUDGMENT the question asks for is yours to make from those \
             measurements.\n\
             Coordinates: x increases east (0..width-1), y increases south (0..height-1); y=0 is the \
             north edge.\n\n\
             Measurement tools (each returns an exact value computed from the board):\n\
             {OPERATOR_TOOLS_DOC}{MAXIMAL_OPERATOR_TOOLS_DOC}\n\
             Unit operators (att_eff/def_eff/garrison_defense/reach_turns) take a UNIT ID — the \
             number in each {{unit #<id> ...}} marker in the board. A tile can hold several stacked \
             units, so pass the id of the exact unit you mean, not its coordinate.\n\
             Prefer calling these tools to measure distances, counts, travel costs, strengths, \
             threats, and site axes rather than estimating by eye. When ready, reply with a line \
             starting exactly `Answer: ` followed by your answer, and nothing after it.\n\n\
             Board (every tile):\n"
        )
    }
}

// --------------------------------------------------------------------------
// interactive-maxops perception surface: front-loaded roster + scan_region
// --------------------------------------------------------------------------
//
// Shared by `interactive-maxops` and `interactive-maxops-enum`. Their perception surface differs
// from the plain `interactive` family in three ways: (1) the occupant ROSTER (every visible unit +
// city) is ALWAYS front-loaded into the overview — no `--frontload-roster` flag needed; (2) the
// quadrant block carries only COUNTS, not per-name enumeration (the roster owns identities); and
// (3) the SOLE terrain-perception verb is `scan_region` (a per-tile TERRAIN listing; occupants stay
// in the roster): `region_summary`, `list_cities`/`list_units`, AND `scan`/`get_tile`/`scan_grid` are
// dropped from the menu. scan/get_tile were used wastefully once `scan_region` + the roster exist;
// scan_grid's only edge was a hidden-force accuracy bump that turned out to be a `scan_region` fog
// FIDELITY GAP (it dropped fogged terrain as "Unknown") — once scan_region was fixed to list fogged
// tiles, scan_region-only matched scan_region+scan_grid on hidden-force on both boards, so scan_grid
// earns nothing here (`findings-scangrid-drop-budgetaware.md`). Every dropped verb's execute/dispatch
// is retained for the plain `interactive`/`interactive-ops` surfaces and the run_tool parity gate.

/// The `scan_region(x,y)` description: a per-tile terrain layer of the sliding-clamp 10×10 window
/// (occupants are lightweight flags only — names/sizes/strengths live in the roster / the ops).
pub const SCAN_REGION_DESC: &str = "Per-tile TERRAIN layer of the 10x10 window CENTERED on (x, y): one line per KNOWN \
    tile giving its terrain, any resource, map features (River/Road/Railroad/Fortress/Hut/...), the \
    bordering territory owner, and a lightweight [city]/[unit] occupancy FLAG — NO names, sizes or \
    strengths (those are in the occupant roster at the top of the board overview, and via the \
    att_eff/def_eff/city_defense ops). Unknown/fogged tiles are SKIPPED; the header line states the \
    window bounds, the dominant terrain, and how many tiles are hidden. Center it ANYWHERE — if the \
    10x10 window would run off an edge it slides back on so you still get a full view (smaller only \
    if the board itself is smaller). Use it to read the lay of an area in one call; occupants are \
    already in the roster.";

/// One `scan_region` line for a KNOWN tile: `(x, y): Terrain[, Resource…][, feature…][, borders
/// Owner][, [city]][, [unit]]`. Resources precede other map features; each group is name-sorted
/// (extras is a `BTreeSet`). Occupancy is a bare flag — no identities (they are in the roster).
fn scan_region_tile_line(board: &Board, x: i32, y: i32, occ: &Occupants) -> String {
    let t = board.tile(x, y);
    let mut parts: Vec<String> = vec![t.terrain.clone()];
    for e in &t.extras {
        if crate::generators::is_resource(e) {
            parts.push(e.clone());
        }
    }
    for e in &t.extras {
        if !crate::generators::is_resource(e) {
            parts.push(e.clone());
        }
    }
    if let Some(o) = &t.owner {
        parts.push(format!("borders {o}"));
    }
    if occ.cities.contains_key(&(x, y)) {
        parts.push("[city]".to_string());
    }
    // Live units on a FOGGED tile are hidden by the perspective mask (they are absent from
    // `board.units`, so `occ.units` has no entry) — that is the whole point of the fog kinds, so we
    // never flag `[unit]` there even defensively. On a currently-visible tile the flag shows.
    if !board.is_fogged(x, y) && occ.units.contains_key(&(x, y)) {
        parts.push("[unit]".to_string());
    }
    // Three-state fog: a fogged-but-explored tile shows remembered terrain/resource/owner/[city]
    // (units hidden), tagged so the model reads it as remembered, not current. Mirrors `tile_facts`.
    if board.is_fogged(x, y) {
        parts.push("(fogged)".to_string());
    }
    format!("({x}, {y}): {}", parts.join(", "))
}

/// The `scan_region` result: a header (window bounds, dominant terrain, unexplored-tile count)
/// followed by one line per EXPLORED tile. Under three-state fog an explored-but-fogged tile's
/// terrain/resource/owner/last-known city ARE known (only live units are hidden), so fogged tiles
/// are LISTED (tagged `(fogged)`) — the fog kinds (e.g. hidden-force) reason over exactly that
/// remembered terrain/territory substrate, which the old "skip every fogged tile" behavior discarded,
/// making the model misread fogged enemy land as empty ocean (see
/// `analysis/findings-scangrid-drop-budgetaware.md`). Only NEVER-explored ("Unknown") tiles are
/// omitted; that count is ALWAYS printed — even zero. `(cx, cy)` is the requested center; the window
/// slides to stay on-board via [`region_window`].
fn scan_region_text(board: &Board, cx: i32, cy: i32) -> String {
    let (x0, y0, x1, y1) = region_window(board, cx, cy);
    let occ = Occupants::of(board);
    // A tile is EXPLORED (shown) when it has real remembered terrain — whether currently visible or
    // fogged. Only never-explored tiles ("Unknown") are omitted.
    let explored = |x: i32, y: i32| board.tile(x, y).terrain != "Unknown";

    // Dominant terrain over the EXPLORED tiles (name-tiebreak, matching most_common_terrain).
    let mut terr: HashMap<&str, i32> = HashMap::new();
    let mut unexplored = 0i32;
    for y in y0..=y1 {
        for x in x0..=x1 {
            if explored(x, y) {
                *terr.entry(board.tile(x, y).terrain.as_str()).or_default() += 1;
            } else {
                unexplored += 1;
            }
        }
    }
    let dominant = terr
        .into_iter()
        .min_by_key(|(name, n)| (std::cmp::Reverse(*n), *name))
        .map(|(t, _)| t.to_string())
        .unwrap_or_else(|| "none (all unexplored)".to_string());

    let (ww, hh) = (x1 - x0 + 1, y1 - y0 + 1);
    let mut out = format!(
        "scan_region around ({cx},{cy}) — terrain layer of window ({x0},{y0})-({x1},{y1}), \
         {ww}x{hh} tiles; dominant terrain {dominant}; {unexplored} tiles Unknown (never explored, \
         not shown). Fogged tiles ARE listed (tagged (fogged)): terrain/owner remembered, live units \
         hidden — an absent [unit] on a fogged tile does NOT mean it is empty.\n"
    );
    let mut lines = String::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            if explored(x, y) {
                lines.push_str(&scan_region_tile_line(board, x, y, &occ));
                lines.push('\n');
            }
        }
    }
    if lines.is_empty() {
        out.push_str("(every tile in this window is Unknown/never explored — nothing to show)\n");
    } else {
        out.push_str(&lines);
    }
    out
}

/// Dispatch `scan_region`; `None` for any other verb so composite surfaces fall through. Never
/// panics — bad input returns an `error:` string.
fn execute_scan_region(board: &Board, call: &ToolCall) -> Option<String> {
    if call.name != "scan_region" {
        return None;
    }
    let (Some(x), Some(y)) = (arg_i32(&call.args_json, "x"), arg_i32(&call.args_json, "y")) else {
        return Some(
            "error: scan_region requires integers 'x' and 'y' (the window center)".to_string(),
        );
    };
    if !board.in_bounds(x, y) {
        return Some(format!(
            "error: scan_region center ({x}, {y}) is off the board (it is {}x{})",
            board.width, board.height
        ));
    }
    Some(scan_region_text(board, x, y))
}

/// The always-on occupant roster front-loaded into the interactive-maxops overview: the same
/// `list_units`/`list_cities` directory under the `KNOWN UNITS`/`KNOWN CITIES` headers, but with each
/// CITY line enriched with size AND walls (city detail lives here now that `scan_region` carries only
/// terrain). Unit lines keep type/owner/position/id (strengths stay behind the att_eff/def_eff ops).
fn interactive_maxops_roster(board: &Board) -> String {
    let units = if board.units.is_empty() {
        "(no units visible)".to_string()
    } else {
        let mut us: Vec<&Unit> = board.units.iter().collect();
        us.sort_by_key(|u| (u.y, u.x));
        us.iter()
            .map(|u| format!("#{} {} owner {} at ({}, {})", u.id, u.kind, u.owner, u.x, u.y))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let cities = if board.cities.is_empty() {
        "(no cities visible)".to_string()
    } else {
        let mut cs: Vec<&City> = board.cities.iter().collect();
        cs.sort_by_key(|c| (c.y, c.x));
        cs.iter()
            .map(|c| {
                let mut s = format!(
                    "\"{}\" owner {} at ({}, {}) size {}",
                    c.name, c.owner, c.x, c.y, c.size
                );
                if crate::rules::city_is_walled(board, c) {
                    s.push_str(", walls");
                }
                s
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "KNOWN UNITS (every visible unit — type, owner, position, id; the full directory, provided \
         so you never need to fetch it):\n{units}\n\n\
         KNOWN CITIES (every visible city — name, owner, position, size, walls; the full directory):\n\
         {cities}\n\n"
    )
}

/// The interactive-maxops fetch tool menu: `scan_region` (per-tile terrain, fog-aware) is the SOLE
/// terrain-perception verb. `region_summary`, `list_cities` and `list_units` are retired (the roster
/// front-loads occupants), and `scan`/`get_tile`/`scan_grid` are dropped too. scan_grid's only earned
/// justification was a hidden-force accuracy edge — but that turned out to be a `scan_region` FIDELITY
/// GAP (it was discarding fogged-but-explored terrain as "Unknown"); once `scan_region` was fixed to
/// list fogged tiles, scan_region-only MATCHED scan_region+scan_grid on hidden-force on both boards, so
/// scan_grid earns nothing here (`analysis/findings-scangrid-drop-budgetaware.md`). Every dropped
/// verb's execute/dispatch stays intact — the plain `interactive`/`interactive-ops` surfaces still
/// expose them, and the run_tool parity gate calls them by name — they are only off THIS menu.
fn interactive_maxops_fetch_tools() -> Vec<ToolDef> {
    let mut t: Vec<ToolDef> = Interactive
        .tools()
        .into_iter()
        .filter(|d| {
            d.name != "region_summary"
                && d.name != "list_cities"
                && d.name != "list_units"
                && d.name != "scan"
                && d.name != "get_tile"
                && d.name != "scan_grid"
        })
        .collect();
    t.insert(
        0,
        ToolDef {
            name: "scan_region",
            description: SCAN_REGION_DESC.to_string(),
            params_schema:
                "{\"type\":\"object\",\"properties\":{\"x\":{\"type\":\"integer\"},\
                 \"y\":{\"type\":\"integer\"}},\"required\":[\"x\",\"y\"]}"
                    .to_string(),
        },
    );
    t
}

/// The interactive-maxops overview: the front-loaded roster, then the quadrant block with COUNTS
/// only (no per-name enumeration), teaching `scan_region` for per-tile terrain. Shared by
/// `interactive-maxops` and `interactive-maxops-enum`.
fn interactive_maxops_overview(board: &Board) -> String {
    let (w, h) = (board.width, board.height);
    let (mx, my) = (w / 2, h / 2);
    let d = crate::describe::describe_region_counts;
    format!(
        "{roster}INTERACTIVE BOARD. {w} wide (x 0..{}) by {h} tall (y 0..{}); x increases east, y \
         increases south, y=0 is the north edge. Every visible city \
         and unit is already listed in the roster above; the board terrain you see only through \
         tools — call them to zoom from coarse to fine.\n\
         Coarse overview by quadrant (terrain, territory and occupant COUNTS only — the occupants \
         themselves are named in the roster above):\n\
         \x20 NW (0,0)-({},{}) : {}\n\
         \x20 NE ({},0)-({},{}) : {}\n\
         \x20 SW (0,{})-({},{}) : {}\n\
         \x20 SE ({},{})-({},{}) : {}\n\
         Call scan_region(x, y) for a per-tile TERRAIN + resource listing of the {SECTOR}x{SECTOR} \
         area centered anywhere on the board (it is shifted to stay on-board at an edge; occupants \
         are in the roster). Fogged-but-explored tiles are listed too (terrain/owner remembered, live \
         units hidden), so a fogged area is never a blank — reason over its remembered terrain.",
        w - 1, h - 1,
        mx - 1, my - 1, d(board, 0, 0, mx - 1, my - 1),
        mx, w - 1, my - 1, d(board, mx, 0, w - 1, my - 1),
        my, mx - 1, h - 1, d(board, 0, my, mx - 1, h - 1),
        mx, my, w - 1, h - 1, d(board, mx, my, w - 1, h - 1),
        roster = interactive_maxops_roster(board),
    )
}

// --------------------------------------------------------------------------
// interactive-maxops — the interactive fetch surface PLUS the MAXIMAL calculator
// --------------------------------------------------------------------------
//
// The `Interactive` perception verbs (roster front-loaded, scan_region in place of region_summary,
// list_cities/list_units retired) AND the full measurement calculator. The model both navigates
// coarse-to-fine and offloads every measurement; the perception × compute factorial's max-compute
// cell.
pub struct InteractiveMaxOps;

impl QueryableSurface for InteractiveMaxOps {
    fn name(&self) -> &str {
        "interactive-maxops"
    }

    fn overview(&self, board: &Board) -> String {
        interactive_maxops_overview(board)
    }

    fn tools(&self) -> Vec<ToolDef> {
        let mut t = interactive_maxops_fetch_tools();
        t.extend(maximal_operator_tools());
        t
    }

    fn execute(&self, board: &Board, call: &ToolCall) -> String {
        // scan_region first, then the operators (the whole calculator); anything else is a fetch
        // verb owned by Interactive (which still resolves the retired region_summary/list_* for the
        // roster renderer and back-compat, even though they are off this surface's menu).
        if let Some(s) = execute_scan_region(board, call) {
            return s;
        }
        execute_maximal_operator(board, call).unwrap_or_else(|| Interactive.execute(board, call))
    }

    fn reconstruct_via_tools(&self, board: &Board) -> Result<RecoveredBoard, String> {
        // The fetch verbs are inherited verbatim, so completeness is the interactive surface's gate.
        Interactive.reconstruct_via_tools(board)
    }

    fn system_preamble(&self) -> String {
        format!(
            "You are answering a question about a Freeciv game board. You CANNOT see the whole board \
             at once — you explore it through tools, zooming from a coarse overview down to exact \
             tiles. You ALSO have a full calculator: deterministic MEASUREMENT tools that compute \
             exact spatial and combat quantities so you never have to do the arithmetic in your \
             head. The tools MEASURE the board; the DECISION the question asks for is yours to make \
             from those measurements.\n\
             Coordinates: x increases east (0..width-1), y increases south (0..height-1); y=0 is the \
             north edge.\n\n\
             The full occupant ROSTER (every visible unit and city) is front-loaded at the top of \
             the board overview below, so you never need to fetch it.\n\
             Perception tool (terrain-only — occupant identities are in the roster):\n\
             - scan_region(x,y): a per-tile TERRAIN + resource listing of the 10x10 area centered on \
             (x,y) — one line per EXPLORED tile (terrain, resource, features, bordering owner, and a \
             lightweight [city]/[unit] flag), in ONE call (shifted to stay on-board at an edge). \
             Fogged-but-explored tiles ARE listed (terrain/owner remembered, tagged (fogged); live \
             units hidden) — a fogged area is never blank. Only never-explored tiles are omitted. \
             Occupant identities are in the roster, not here.\n\n\
             Measurement tools (each returns an exact value computed from the board):\n\
             {OPERATOR_TOOLS_DOC}{MAXIMAL_OPERATOR_TOOLS_DOC}\n\
             Unit operators (att_eff/def_eff/garrison_defense/reach_turns) take a UNIT ID — the \
             number after the `#` in each roster line (e.g. `#703` -> id 703). A tile \
             can hold several stacked units, so pass the id of the exact unit you mean, not its \
             coordinate.\n\
             Fetch the facts you need, then use the measurement tools to compute quantities rather \
             than doing the arithmetic by eye. When ready, reply with a line starting exactly \
             `Answer: ` followed by your answer, and nothing after it.\n\n\
             Board overview:\n"
        )
    }
}

// --------------------------------------------------------------------------
// raw-maxops-enum — the FULL raw board block + the MAXIMAL calculator with count_* RETIRED and the
// ENUMERATION/INDEX group ADDED (NO fetch verbs)
// --------------------------------------------------------------------------
//
// The ablation partner of `raw-maxops`: the SAME full-perception substrate and the SAME maximal
// measurement operators, but with `count_terrain`/`count_resource` swapped for `list_tiles` (which
// subsumes them) and the positional enumerators (`list_tiles`, `list_owned_cities`,
// `list_owned_units`) added. This isolates a clean toggle — "does positional enumeration break the
// LOCATING ceiling?" — on an identical measurement+perception base. The PRIMARY enumeration arm:
// the scale finding says the wall is global LOCATING over a huge board, which full perception plus
// cheap enumeration is meant to break. 12 tools total.
pub struct RawMaxOpsEnum;

impl QueryableSurface for RawMaxOpsEnum {
    fn name(&self) -> &str {
        "raw-maxops-enum"
    }

    fn overview(&self, board: &Board) -> String {
        RawEncoder.render(board)
    }

    fn tools(&self) -> Vec<ToolDef> {
        maximal_enum_operator_tools()
    }

    fn execute(&self, board: &Board, call: &ToolCall) -> String {
        execute_maximal_enum_operator(board, call).unwrap_or_else(|| {
            format!(
                "error: unknown tool {:?} (this surface exposes the measurement + enumeration \
                 calculator: distance, travel_turns, att_eff, def_eff, city_defense, \
                 garrison_defense, threat, reach_turns, list_tiles, list_owned_cities, \
                 list_owned_units, travel_turns_from_owned — count is retired, use list_tiles)",
                call.name
            )
        })
    }

    fn reconstruct_via_tools(&self, board: &Board) -> Result<RecoveredBoard, String> {
        // The overview IS the full raw board block; the operators carry no per-tile facts of their
        // own, so completeness is the raw encoder's own decoder (as for `raw-ops`/`raw-maxops`).
        RawEncoder.reconstruct(&self.overview(board))
    }

    fn system_preamble(&self) -> String {
        format!(
            "You are answering a question about a Freeciv game board. The COMPLETE board is given \
             below — you can see every tile at once. You ALSO have a full set of deterministic \
             tools: MEASUREMENT tools that compute exact spatial and combat quantities, and \
             ENUMERATION tools that return the exact COORDINATES of tiles/cities/units matching a \
             predicate (so you never have to locate entities by eye out of the board). The tools \
             MEASURE and LOCATE; the DECISION or JUDGMENT the question asks for is yours to make \
             from what they return.\n\
             Coordinates: x increases east (0..width-1), y increases south (0..height-1); y=0 is the \
             north edge.\n\n\
             Tools (each returns exact values computed from the board):\n\
             {ENUM_SURFACE_MEASURE_DOC}{MAXIMAL_OPERATOR_TOOLS_DOC}{ENUMERATION_OPERATOR_TOOLS_DOC}\n\
             Unit operators (att_eff/def_eff/garrison_defense/reach_turns) take a UNIT ID — the \
             number in each {{unit #<id> ...}} marker in the board (list_owned_units also returns \
             ids). A tile can hold several stacked units, so pass the id of the exact unit you mean, \
             not its coordinate.\n\
             Use list_tiles to find and COUNT matching tiles (its length is the count), \
             list_owned_cities/list_owned_units to locate a player's entities, and the measurement \
             tools for distances, travel costs, strengths, threats, and site axes — rather than \
             reading positions or estimating by eye. When a question asks for resource tiles OUTSIDE \
             the working radius of your cities (unexploited resources), call \
             list_tiles(resource=..., exclude_owner=<you>, exclude_radius=2) so the tool drops the \
             already-worked tiles for you — do NOT fetch every resource and every city and check the \
             pairwise distances by hand. When ready, reply with a line starting exactly \
             `Answer: ` followed by your answer, and nothing after it.\n\n\
             Board (every tile):\n"
        )
    }
}

// --------------------------------------------------------------------------
// interactive-maxops-enum — the interactive fetch surface PLUS the MAXIMAL+ENUMERATION calculator
// --------------------------------------------------------------------------
//
// The interactive analogue of `raw-maxops-enum`, added for factorial symmetry. Expected to be
// LOWER-VALUE, possibly counterproductive: interactive's measured failure mode is over-fetch/dither
// (findings-scale-ceiling.md), and a larger menu — the fetch verbs' `list_cities`/`list_units` now
// SIT ALONGSIDE the enumerators' player-filtered `list_owned_cities`/`list_owned_units` — widens
// that surface. The enumeration operators are kept IDENTICAL to the raw substrate (the whole point
// is that the ONLY difference between the two arms is perception). count_* are retired here too.
pub struct InteractiveMaxOpsEnum;

impl QueryableSurface for InteractiveMaxOpsEnum {
    fn name(&self) -> &str {
        "interactive-maxops-enum"
    }

    fn overview(&self, board: &Board) -> String {
        interactive_maxops_overview(board)
    }

    fn tools(&self) -> Vec<ToolDef> {
        let mut t = interactive_maxops_fetch_tools();
        // Drop the player-scoped owned-enumerators: on this interactive substrate the front-loaded
        // roster already enumerates every visible entity by coordinate (with unit ids), so
        // list_owned_cities/list_owned_units only widen an already-over-fetching menu. They are KEPT
        // on the raw enum surface (raw-maxops-enum), which has NO roster and there they are the sole
        // owned-enumerator. The dispatch below still resolves them if called.
        t.extend(
            maximal_enum_operator_tools()
                .into_iter()
                .filter(|d| d.name != "list_owned_cities" && d.name != "list_owned_units"),
        );
        t
    }

    fn execute(&self, board: &Board, call: &ToolCall) -> String {
        // scan_region first, then the enum+maximal calculator (count_* fall through as retired);
        // anything else is a fetch verb owned by Interactive (which also owns the "unknown tool"
        // error, including count_*). list_owned_* are off tools() above but still resolve here.
        if let Some(s) = execute_scan_region(board, call) {
            return s;
        }
        execute_maximal_enum_operator(board, call)
            .unwrap_or_else(|| Interactive.execute(board, call))
    }

    fn reconstruct_via_tools(&self, board: &Board) -> Result<RecoveredBoard, String> {
        // The fetch verbs are inherited verbatim, so completeness is the interactive surface's gate.
        Interactive.reconstruct_via_tools(board)
    }

    fn system_preamble(&self) -> String {
        format!(
            "You are answering a question about a Freeciv game board. You CANNOT see the whole board \
             at once — you explore it through tools, zooming from a coarse overview down to exact \
             tiles. You ALSO have a full calculator: MEASUREMENT tools that compute exact spatial \
             and combat quantities, and ENUMERATION tools that return the exact COORDINATES of \
             tiles/cities/units matching a predicate. The tools MEASURE and LOCATE; the DECISION the \
             question asks for is yours to make from what they return.\n\
             Coordinates: x increases east (0..width-1), y increases south (0..height-1); y=0 is the \
             north edge.\n\n\
             The full occupant ROSTER (every visible unit and city) is front-loaded at the top of \
             the board overview below, so you never need to fetch it.\n\
             Perception tool (terrain-only — occupant identities are in the roster):\n\
             - scan_region(x,y): a per-tile TERRAIN + resource listing of the 10x10 area centered on \
             (x,y) — one line per EXPLORED tile (terrain, resource, features, bordering owner, and a \
             lightweight [city]/[unit] flag), in ONE call (shifted to stay on-board at an edge). \
             Fogged-but-explored tiles ARE listed (terrain/owner remembered, tagged (fogged); live \
             units hidden) — a fogged area is never blank. Only never-explored tiles are omitted. \
             Occupant identities are in the roster, not here.\n\n\
             Measurement + enumeration tools (each returns exact values computed from the board):\n\
             {ENUM_SURFACE_MEASURE_DOC}{MAXIMAL_OPERATOR_TOOLS_DOC}{INTERACTIVE_ENUMERATION_OPERATOR_TOOLS_DOC}\n\
             Unit operators (att_eff/def_eff/garrison_defense/reach_turns) take a UNIT ID — the \
             number after the `#` in each roster line (e.g. `#703` -> id 703). A tile \
             can hold several stacked units, so pass the id of the exact unit you mean, not its \
             coordinate.\n\
             Fetch or enumerate the facts you need, then use the measurement tools to compute \
             quantities rather than doing the arithmetic by eye. When a question asks for resource \
             tiles OUTSIDE the working radius of your cities (unexploited resources), call \
             list_tiles(resource=..., exclude_owner=<you>, exclude_radius=2) so the tool drops the \
             already-worked tiles for you — do NOT fetch every resource and every city and check the \
             pairwise distances by hand. When ready, reply with a line \
             starting exactly `Answer: ` followed by your answer, and nothing after it.\n\n\
             Board overview:\n"
        )
    }
}

// --------------------------------------------------------------------------
// raw-calc — the FULL raw board block + the TRIMMED spatial calculator (NO fetch verbs)
// --------------------------------------------------------------------------
//
// The clean dispersed-kind surface: full perception + EXACTLY the five spatial-logistics operators
// (distance, travel_turns, count_terrain, count_resource, reach_turns) the dispersed kinds
// (nearest-owned, reachable-nearest, region-count) actually invoke. This is `raw-ops` PLUS the
// unit-move-rate op `reach_turns` the primitive set lacked, with NO combat/valuation ops, NO
// enumeration, NO fetch verbs — no dead tools to dither over. Mirrors `raw-ops`/`raw-maxops`:
// overview is the full render, completeness is the raw decoder.
pub struct RawCalc;

impl QueryableSurface for RawCalc {
    fn name(&self) -> &str {
        "raw-calc"
    }

    fn overview(&self, board: &Board) -> String {
        RawEncoder.render(board)
    }

    fn tools(&self) -> Vec<ToolDef> {
        calc_operator_tools()
    }

    fn execute(&self, board: &Board, call: &ToolCall) -> String {
        execute_calc_operator(board, call).unwrap_or_else(|| {
            format!(
                "error: unknown tool {:?} (this surface exposes the spatial calculator only: \
                 distance, travel_turns, count, reach_turns)",
                call.name
            )
        })
    }

    fn reconstruct_via_tools(&self, board: &Board) -> Result<RecoveredBoard, String> {
        // The overview IS the full raw board block; the operators carry no per-tile facts of their
        // own, so completeness is the raw encoder's own decoder (as for `raw-ops`/`raw-maxops`).
        RawEncoder.reconstruct(&self.overview(board))
    }

    fn system_preamble(&self) -> String {
        format!(
            "You are answering a question about a Freeciv game board. The COMPLETE board is given \
             below — you can see every tile at once. You ALSO have a set of deterministic \
             measurement tools (a calculator) that compute exact spatial quantities for you, so you \
             never have to do the spatial arithmetic in your head.\n\
             Coordinates: x increases east (0..width-1), y increases south (0..height-1); y=0 is the \
             north edge.\n\n\
             Measurement tools (each returns an exact value computed from the board):\n\
             {OPERATOR_TOOLS_DOC}{REACH_TURNS_DOC}\n\
             The reach_turns operator takes a UNIT ID — the number in each {{unit #<id> ...}} marker \
             in the board. A tile can hold several stacked units, so pass the id of the exact unit \
             you mean, not its coordinate.\n\
             Prefer calling these tools to measure distances, counts, and travel costs rather than \
             counting or estimating by eye. When ready, reply with a line starting exactly \
             `Answer: ` followed by your answer, and nothing after it.\n\n\
             Board (every tile):\n"
        )
    }
}

// --------------------------------------------------------------------------
// raw-calc-enum — the FULL raw board block + the TRIMMED spatial calculator with count_* RETIRED and
// the ENUMERATION/INDEX group ADDED (NO fetch verbs)
// --------------------------------------------------------------------------
//
// The enum partner of `raw-calc`: full perception + distance/travel_turns/reach_turns with
// count_terrain/count_resource swapped for `list_tiles` (which subsumes them) and the positional
// enumerators (`list_tiles`, `list_owned_cities`, `list_owned_units`) added. Still NO combat/
// valuation ops and NO fetch verbs — the maximally clean enumeration surface for the dispersed
// kinds. 6 tools total.
pub struct RawCalcEnum;

impl QueryableSurface for RawCalcEnum {
    fn name(&self) -> &str {
        "raw-calc-enum"
    }

    fn overview(&self, board: &Board) -> String {
        RawEncoder.render(board)
    }

    fn tools(&self) -> Vec<ToolDef> {
        calc_enum_operator_tools()
    }

    fn execute(&self, board: &Board, call: &ToolCall) -> String {
        execute_calc_enum_operator(board, call).unwrap_or_else(|| {
            format!(
                "error: unknown tool {:?} (this surface exposes the spatial + enumeration \
                 calculator: distance, travel_turns, reach_turns, list_tiles, list_owned_cities, \
                 list_owned_units, travel_turns_from_owned — count is retired, use list_tiles)",
                call.name
            )
        })
    }

    fn reconstruct_via_tools(&self, board: &Board) -> Result<RecoveredBoard, String> {
        // The overview IS the full raw board block; the operators carry no per-tile facts of their
        // own, so completeness is the raw encoder's own decoder (as for `raw-ops`/`raw-maxops`).
        RawEncoder.reconstruct(&self.overview(board))
    }

    fn system_preamble(&self) -> String {
        format!(
            "You are answering a question about a Freeciv game board. The COMPLETE board is given \
             below — you can see every tile at once. You ALSO have a set of deterministic tools: \
             MEASUREMENT tools that compute exact spatial quantities, and ENUMERATION tools that \
             return the exact COORDINATES of tiles/cities/units matching a predicate (so you never \
             have to locate entities by eye out of the board).\n\
             Coordinates: x increases east (0..width-1), y increases south (0..height-1); y=0 is the \
             north edge.\n\n\
             Tools (each returns exact values computed from the board):\n\
             {ENUM_SURFACE_MEASURE_DOC}{REACH_TURNS_DOC}{CALC_ENUMERATION_OPERATOR_TOOLS_DOC}\n\
             The reach_turns operator takes a UNIT ID — the number in each {{unit #<id> ...}} marker \
             in the board (list_owned_units also returns ids). A tile can hold several stacked \
             units, so pass the id of the exact unit you mean, not its coordinate.\n\
             Use list_tiles to find and COUNT matching tiles (its length is the count), \
             list_owned_cities/list_owned_units to locate a player's entities, and the measurement \
             tools for distances and travel costs — rather than reading positions or estimating by \
             eye. When a question asks for resource tiles OUTSIDE the working radius of your cities \
             (unexploited resources), call list_tiles(resource=..., exclude_owner=<you>, \
             exclude_radius=2) so the tool drops the already-worked tiles for you — do NOT fetch \
             every resource and every city and check the pairwise distances by hand. When ready, \
             reply with a line starting exactly `Answer: ` followed by your answer, and nothing \
             after it.\n\n\
             Board (every tile):\n"
        )
    }
}

/// Look up a queryable (tool-loop) board-access surface by its `--encoding` name. Returns a
/// `Sync` trait object so the runner can share it across the concurrent interactive workers.
pub fn queryable_surface(name: &str) -> Option<Box<dyn QueryableSurface + Sync>> {
    match name {
        "interactive" => Some(Box::new(Interactive)),
        "raw-ops" => Some(Box::new(RawOps)),
        "interactive-ops" => Some(Box::new(InteractiveOps)),
        "raw-maxops" => Some(Box::new(RawMaxOps)),
        "interactive-maxops" => Some(Box::new(InteractiveMaxOps)),
        "raw-maxops-enum" => Some(Box::new(RawMaxOpsEnum)),
        "interactive-maxops-enum" => Some(Box::new(InteractiveMaxOpsEnum)),
        "raw-calc" => Some(Box::new(RawCalc)),
        "raw-calc-enum" => Some(Box::new(RawCalcEnum)),
        _ => None,
    }
}

/// The queryable surface names (drive the multi-turn tool loop, not a static render). Used by the
/// CLI to route `--encoding` and to keep them out of the static board-block-size report.
pub const QUERYABLE_SURFACES: [&str; 9] = [
    "interactive",
    "raw-ops",
    "interactive-ops",
    "raw-maxops",
    "interactive-maxops",
    "raw-maxops-enum",
    "interactive-maxops-enum",
    "raw-calc",
    "raw-calc-enum",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ToolCall;
    use crate::question::{solve, Answer, Question};
    use crate::rules::ReachField;
    use civ_core::board::{City, Player, Tile, Unit, Visibility};
    use std::collections::BTreeSet;

    /// A `width`x1 land strip of the given terrains (one per column), no city/unit.
    fn terrain_strip(terrains: &[&str]) -> Board {
        let mut row = Vec::new();
        for (x, t) in terrains.iter().enumerate() {
            row.push(Tile {
                x: x as i32,
                y: 0,
                terrain: t.to_string(),
                extras: BTreeSet::new(),
                owner: None,
            });
        }
        Board {
            width: terrains.len() as i32,
            height: 1,
            tiles: vec![row],
            cities: vec![],
            units: vec![],
            players: vec![Player {
                id: 0,
                name: "A".to_string(),
                nation: String::new(),
                is_alive: true,
            }],
            known: std::collections::HashMap::new(),
            visibility: None,
            ruleset: None,
            turn: None,
            source: "strip".to_string(),
        }
    }

    #[test]
    fn most_common_terrain_tie_breaks_deterministically_by_name() {
        // A deliberate 2-2 terrain-count tie between Plains and Grassland. The omitted "default"
        // must resolve the same way on every call (B3) — the name-ascending winner, Grassland
        // (`G` < `P`) — never at the mercy of HashMap iteration order.
        let board = terrain_strip(&["Plains", "Grassland", "Plains", "Grassland"]);
        let first = most_common_terrain(&board);
        assert_eq!(
            first, "Grassland",
            "tie must resolve to the name-order winner"
        );
        for _ in 0..50 {
            assert_eq!(
                most_common_terrain(&board),
                first,
                "tie-break must be stable across calls"
            );
        }
    }

    /// Build a raw operator call with box args and an optional string `feature` field.
    fn op_call(name: &'static str, x0: i32, y0: i32, x1: i32, y1: i32, extra: &str) -> ToolCall {
        let args = format!("{{\"x0\":{x0},\"y0\":{y0},\"x1\":{x1},\"y1\":{y1}{extra}}}");
        ToolCall {
            id: String::new(),
            name: name.to_string(),
            args_json: args,
        }
    }

    // ---- operator-parity gate: each operator == the solver/machinery it mirrors ----

    #[test]
    fn op_distance_parity_with_distance_question() {
        let b = mini();
        // Faithful to the shared Chebyshev metric AND to the `distance` question's ground truth.
        assert_eq!(
            op_distance(1, 1, 3, 0),
            civ_core::geometry::chebyshev((1, 1), (3, 0))
        );
        let q = Question::Distance {
            a: Referent::Tile { x: 1, y: 1 },
            b: Referent::Tile { x: 3, y: 0 },
        };
        assert_eq!(
            solve(&q, &b),
            Some(Answer::Int(op_distance(1, 1, 3, 0) as i64))
        );
    }

    #[test]
    fn op_count_terrain_parity_with_region_count() {
        let b = mini(); // 4x4 Grassland, Forest at (3,0)
                        // A Chebyshev radius-2 window about (1,1) is the inclusive box (-1,-1)-(3,3) clipped.
        let (cx, cy, r) = (1, 1, 2);
        let direct = civ_core::geometry::tiles_within(&b, (cx, cy), r)
            .into_iter()
            .filter(|&(x, y)| b.tile(x, y).terrain == "Grassland")
            .count() as i64;
        assert_eq!(
            op_count_terrain(&b, cx - r, cy - r, cx + r, cy + r, "Grassland"),
            direct
        );
        // ...and equal to the `region-count` question's ground truth.
        let q = Question::CountTerrainInRadius {
            center: Referent::Tile { x: cx, y: cy },
            radius: r,
            terrain: "Grassland".to_string(),
        };
        assert_eq!(solve(&q, &b), Some(Answer::Int(direct)));
        assert_eq!(op_count_terrain(&b, 0, 0, 3, 3, "Forest"), 1);
    }

    #[test]
    fn op_count_resource_parity_with_direct_count() {
        let b = mini(); // Iron at (3,0)
        let direct = b
            .iter_tiles()
            .filter(|t| (0..=3).contains(&t.x) && (0..=3).contains(&t.y) && t.has("Iron"))
            .count() as i64;
        assert_eq!(op_count_resource(&b, 0, 0, 3, 3, "Iron"), direct);
        assert_eq!(direct, 1);
        // A non-resource extra name is rejected by the surface (is_resource gate), never counted.
        let err = RawOps.execute(
            &b,
            &op_call(
                "count",
                0,
                0,
                3,
                3,
                ",\"kind\":\"resource\",\"feature\":\"Road\"",
            ),
        );
        assert!(err.starts_with("error:"), "{err}");
        let ok = RawOps.execute(
            &b,
            &op_call(
                "count",
                0,
                0,
                3,
                3,
                ",\"kind\":\"resource\",\"feature\":\"Iron\"",
            ),
        );
        assert_eq!(ok, "1");
    }

    #[test]
    fn op_travel_turns_parity_with_reach_bfs() {
        let b = mini(); // all land, 4x4
                        // Faithful to the SAME ReachField BFS at the documented params (move 1, whole-board horizon).
        let horizon = b.width * b.height;
        let reach = ReachField::from_origins(&b, &[(0, 0)], 1, horizon);
        for (dx, dy) in [(3, 0), (3, 3), (1, 1)] {
            assert_eq!(
                op_travel_turns(&b, 0, 0, dx, dy, None),
                reach.turns_at(dx, dy)
            );
        }
        assert_eq!(op_travel_turns(&b, 0, 0, 3, 3, None), Some(3)); // 8-connected all-land diagonal
    }

    #[test]
    fn op_travel_turns_unreachable_across_water() {
        let b = terrain_strip(&["Grassland", "Ocean", "Grassland"]);
        assert_eq!(op_travel_turns(&b, 0, 0, 0, 0, None), Some(0));
        assert_eq!(op_travel_turns(&b, 0, 0, 2, 0, None), None); // land component split by ocean
        let s = RawOps.execute(&b, &op_call("travel_turns", 0, 0, 2, 0, ""));
        assert_eq!(s, "unreachable");
    }

    #[test]
    fn op_travel_turns_matches_reachable_nearest_for_a_move1_unit() {
        // 5x1 land strip; a Warriors (move 1) at (0,0), Iron at (4,0).
        let mut b = terrain_strip(&[
            "Grassland",
            "Grassland",
            "Grassland",
            "Grassland",
            "Grassland",
        ]);
        b.tiles[0][4].extras.insert("Iron".to_string());
        b.units.push(Unit {
            x: 0,
            y: 0,
            id: 7,
            kind: "Warriors".to_string(),
            owner: "A".to_string(),
            veteran: 0,
            hp: 10,
        });
        let q = Question::ReachableNearestResource {
            unit: Referent::Unit { id: 7 },
            resource: "Iron".to_string(),
            horizon: 6,
        };
        // The reachable-nearest ground truth for a move-1 unit is exactly travel_turns to the resource.
        assert_eq!(solve(&q, &b), Some(Answer::OptionalInt(Some(4))));
        assert_eq!(op_travel_turns(&b, 0, 0, 4, 0, None), Some(4));
        // The tactical (owner-attributed) reach agrees on an enemy-free strip.
        assert_eq!(op_travel_turns(&b, 0, 0, 4, 0, Some("A")), Some(4));
    }

    /// `travel_turns` with a `player` is ZOC-aware: an enemy unit's Zone of Control that walls off
    /// the only corridor makes the far tile cost more (or become unreachable), while the ownerless
    /// (geographic) call is blind to it. Also: an enemy unit/city tile cannot be entered.
    #[test]
    fn op_travel_turns_tactical_respects_zoc_and_blocking() {
        // 3-wide, 3-tall all-land board. Mover "A" at (0,1). An enemy "B" Warriors at (1,0) exerts
        // ZOC on the whole top corridor; a second enemy at (1,2) closes the bottom — together they
        // pinch column 1 so A cannot slip past to column 2 on row 1 (both (0,1)->(1,1) neighbours
        // sit in enemy ZOC and (1,1) is doubly controlled).
        let mut tiles = Vec::new();
        for y in 0..3 {
            let mut row = Vec::new();
            for x in 0..3 {
                row.push(Tile {
                    x,
                    y,
                    terrain: "Grassland".to_string(),
                    extras: BTreeSet::new(),
                    owner: None,
                });
            }
            tiles.push(row);
        }
        let u = |x, y, id, owner: &str| Unit {
            x,
            y,
            id,
            kind: "Warriors".to_string(),
            owner: owner.to_string(),
            veteran: 0,
            hp: 10,
        };
        let b = Board {
            width: 3,
            height: 3,
            tiles,
            cities: vec![],
            units: vec![u(0, 1, 1, "A"), u(1, 0, 2, "B"), u(1, 2, 3, "B")],
            players: vec![
                Player {
                    id: 0,
                    name: "A".to_string(),
                    nation: String::new(),
                    is_alive: true,
                },
                Player {
                    id: 1,
                    name: "B".to_string(),
                    nation: String::new(),
                    is_alive: true,
                },
            ],
            known: std::collections::HashMap::new(),
            visibility: None,
            ruleset: None,
            turn: None,
            source: "zoc".to_string(),
        };
        // Geographic: (0,1) -> (2,1) is 2 diagonal steps.
        assert_eq!(op_travel_turns(&b, 0, 1, 2, 1, None), Some(2));
        // Cannot step ONTO the enemy unit at (1,0).
        assert_eq!(op_travel_turns(&b, 0, 1, 1, 0, Some("A")), None);
        // Tactical for A: the ZOC pinch forbids A from ever leaving column 0, so column 2 is
        // unreachable even though it is geographically 2 tiles away.
        assert_eq!(op_travel_turns(&b, 0, 1, 2, 1, Some("A")), None);
    }

    #[test]
    fn raw_ops_surface_shape() {
        let b = mini();
        // Overview is the full raw board block (model "sees everything").
        assert_eq!(RawOps.overview(&b), RawEncoder.render(&b));
        // Only the four operator verbs, no fetch verbs.
        let names: Vec<&str> = RawOps.tools().iter().map(|t| t.name).collect();
        assert_eq!(names, vec!["distance", "travel_turns", "count"]);
        // Completeness gate passes (the raw block decodes back to the board).
        assert!(RawOps.reconstruct_via_tools(&b).is_ok());
        // A fetch verb is unknown here.
        assert!(RawOps
            .execute(
                &b,
                &ToolCall {
                    id: String::new(),
                    name: "get_tile".to_string(),
                    args_json: "{\"x\":0,\"y\":0}".to_string()
                }
            )
            .starts_with("error:"));
        assert_eq!(
            RawOps.execute(&b, &op_call("distance", 1, 1, 3, 0, "")),
            "2"
        );
    }

    #[test]
    fn interactive_ops_has_fetch_and_operator_verbs() {
        let b = mini();
        let names: Vec<&str> = InteractiveOps.tools().iter().map(|t| t.name).collect();
        for v in [
            "region_summary",
            "scan",
            "scan_grid",
            "get_tile",
            "list_cities",
            "list_units",
        ] {
            assert!(names.contains(&v), "missing fetch verb {v}");
        }
        for v in ["distance", "travel_turns", "count"] {
            assert!(names.contains(&v), "missing operator {v}");
        }
        // A fetch verb still works (delegates to Interactive)...
        assert!(InteractiveOps
            .execute(
                &b,
                &ToolCall {
                    id: String::new(),
                    name: "get_tile".to_string(),
                    args_json: "{\"x\":3,\"y\":0}".to_string()
                }
            )
            .contains("Forest [Iron]"));
        // ...and an operator works too.
        assert_eq!(
            InteractiveOps.execute(
                &b,
                &op_call(
                    "count",
                    0,
                    0,
                    3,
                    3,
                    ",\"kind\":\"terrain\",\"feature\":\"Forest\""
                )
            ),
            "1"
        );
    }

    // ---- maximal-calculator operators: parity gate + surface shape ----

    /// A 10x1 Grassland strip for the maximal combat operators: player A's city "Home" at (5,0)
    /// with a Phalanx defender on it (id 10), an A Warriors reserve at (0,0) (id 11), and an enemy
    /// B Legion at (7,0) (id 12).
    fn combat_strip() -> Board {
        let mut row = Vec::new();
        for x in 0..10 {
            row.push(Tile {
                x,
                y: 0,
                terrain: "Grassland".to_string(),
                extras: BTreeSet::new(),
                owner: None,
            });
        }
        let u = |x, id, kind: &str, owner: &str| Unit {
            x,
            y: 0,
            id,
            kind: kind.to_string(),
            owner: owner.to_string(),
            veteran: 0,
            hp: 10,
        };
        Board {
            width: 10,
            height: 1,
            tiles: vec![row],
            cities: vec![City {
                x: 5,
                y: 0,
                id: 1,
                name: "Home".to_string(),
                owner: "A".to_string(),
                size: 3,
                improvements: BTreeSet::new(),
            }],
            units: vec![
                u(5, 10, "Phalanx", "A"),
                u(0, 11, "Warriors", "A"),
                u(7, 12, "Legion", "B"),
            ],
            players: vec![
                Player {
                    id: 0,
                    name: "A".to_string(),
                    nation: String::new(),
                    is_alive: true,
                },
                Player {
                    id: 1,
                    name: "B".to_string(),
                    nation: String::new(),
                    is_alive: true,
                },
            ],
            known: std::collections::HashMap::new(),
            visibility: None,
            ruleset: None,
            turn: None,
            source: "combat".to_string(),
        }
    }

    fn tc(name: &str, args: &str) -> ToolCall {
        ToolCall {
            id: String::new(),
            name: name.to_string(),
            args_json: args.to_string(),
        }
    }

    #[test]
    fn maximal_ops_parity_with_rules() {
        let b = combat_strip();
        let legion = b.units.iter().find(|u| u.id == 12).unwrap();
        let phalanx = b.units.iter().find(|u| u.id == 10).unwrap();
        let warriors = b.units.iter().find(|u| u.id == 11).unwrap();
        let home = &b.cities[0];
        // Each operator == the exact rules.rs function the matching solver calls. UNIT operators are
        // addressed by id (legion=12, phalanx=10, warriors=11); the city op by coordinate.
        assert_eq!(op_att_eff(&b, 12), crate::rules::att_eff(legion));
        assert_eq!(
            op_def_eff(&b, 12),
            crate::rules::unit_def_on_own_tile(&b, legion)
        );
        assert_eq!(
            op_def_eff(&b, 10),
            crate::rules::unit_def_on_own_tile(&b, phalanx)
        );
        assert_eq!(
            op_city_defense(&b, 5, 0),
            Some(crate::rules::city_defense(&b, home))
        );
        assert_eq!(
            op_garrison_defense(&b, 11, 5, 0),
            crate::rules::def_eff_if_garrisoned(warriors, &b, home)
        );
        assert_eq!(
            op_threat(&b, "A", 5, 0),
            crate::rules::threat_at(&b, "A", (5, 0))
        );
        assert_eq!(
            op_site_axes(&b, "A", 5, 0),
            crate::rules::site_axes(&b, "A", (5, 0))
        );
        // reach_turns == a TACTICAL ReachField at the unit's own move rate / owner over the
        // whole-board horizon (the enemy Legion #12 is owned by "B").
        let stat = crate::rules::unit_stat("Legion").unwrap();
        let horizon = b.width * b.height;
        let reach = ReachField::from_origins_tactical(&b, &[(7, 0)], stat.move_rate, horizon, "B");
        // (5,0) holds player A's city — the enemy Legion cannot enter it, so the tactical reach is
        // None where the old enemy-blind reach would have returned a turn count.
        assert_eq!(reach.turns_at(5, 0), None);
        assert_eq!(op_reach_turns(&b, 12, 5, 0), Some(reach.turns_at(5, 0)));
        // A tile the Legion CAN reach still parity-matches the op.
        assert_eq!(op_reach_turns(&b, 12, 9, 0), Some(reach.turns_at(9, 0)));
    }

    /// FIX 1 — the context-free `def_eff_base` op equals the exact axis the `unit-strength` solver's
    /// DEFENSE comparison reads (`rules::def_eff_base` == `rules::strength(_, Defense)`), and
    /// DIVERGES from the contextual `def_eff` on a tile that grants a defensive bonus (the bug: a
    /// model answering unit-strength via the tool would otherwise get a terrain/city-inflated value).
    #[test]
    fn op_def_eff_base_parity_with_unit_strength_solver() {
        let b = combat_strip();
        for &id in &[10, 11, 12] {
            let u = b.units.iter().find(|u| u.id == id).unwrap();
            assert_eq!(op_def_eff_base(&b, id), crate::rules::def_eff_base(u));
            // The op == the very axis the unit-strength DEFENSE solver evaluates.
            assert_eq!(
                op_def_eff_base(&b, id),
                crate::rules::strength(u, crate::rules::StrengthAxis::Defense)
            );
        }
        // Phalanx #10 stands on its own city center (5,0): the contextual def_eff is inflated by the
        // city-center bonus (3) while the ground-truth unit-strength axis is the bare base (2). The
        // new op returns the base; the pre-existing op returns the contextual value. This gap is
        // exactly what FIX 1 removes for the unit-strength kind.
        assert_eq!(op_def_eff_base(&b, 10), Some(2.0));
        assert_eq!(op_def_eff(&b, 10), Some(3.0));
        assert_ne!(op_def_eff_base(&b, 10), op_def_eff(&b, 10));
        // Off-tile (Legion #12 on plain Grassland) the two coincide — the divergence is context-only.
        assert_eq!(op_def_eff_base(&b, 12), op_def_eff(&b, 12));
    }

    /// A 4x2 board for the constraint-site predicates: Hills at (0,0) (defensible), Ocean at (1,0)
    /// (coastal source), an enemy-B tile at (2,0), an A-owned tile at (3,0); everything else is
    /// unclaimed Grassland.
    fn constraint_board() -> Board {
        let t = |x, y, terrain: &str, owner: Option<&str>| Tile {
            x,
            y,
            terrain: terrain.to_string(),
            extras: BTreeSet::new(),
            owner: owner.map(|s| s.to_string()),
        };
        let row0 = vec![
            t(0, 0, "Hills", None),
            t(1, 0, "Ocean", None),
            t(2, 0, "Grassland", Some("B")),
            t(3, 0, "Grassland", Some("A")),
        ];
        let row1 = vec![
            t(0, 1, "Grassland", None),
            t(1, 1, "Grassland", None),
            t(2, 1, "Grassland", None),
            t(3, 1, "Grassland", None),
        ];
        Board {
            width: 4,
            height: 2,
            tiles: vec![row0, row1],
            cities: vec![],
            units: vec![],
            players: vec![
                Player {
                    id: 0,
                    name: "A".to_string(),
                    nation: String::new(),
                    is_alive: true,
                },
                Player {
                    id: 1,
                    name: "B".to_string(),
                    nation: String::new(),
                    is_alive: true,
                },
            ],
            known: std::collections::HashMap::new(),
            visibility: None,
            ruleset: None,
            turn: None,
            source: "constraint".to_string(),
        }
    }

    /// FIX 2 — each of the three constraint-site predicate ops equals the exact `rules` predicate
    /// the `constraint-site` solver ANDs, and their conjunction reproduces `constraint_site_ok`
    /// verbatim over every tile (the model composes the intersection the solver computes).
    #[test]
    fn op_constraint_predicates_parity_with_solver() {
        let b = constraint_board();
        for y in 0..b.height {
            for x in 0..b.width {
                assert_eq!(
                    op_is_defensible(&b, x, y),
                    crate::rules::is_defensible(&b, (x, y))
                );
                assert_eq!(
                    op_within_water_radius(&b, x, y),
                    crate::rules::is_within_water_radius(&b, (x, y))
                );
                assert_eq!(
                    op_not_enemy_territory(&b, "A", x, y),
                    crate::rules::not_enemy_territory(&b, "A", (x, y))
                );
                // The three ops AND'd == the constraint-site conjunction the solver evaluates.
                let composed = op_is_defensible(&b, x, y)
                    && op_within_water_radius(&b, x, y)
                    && op_not_enemy_territory(&b, "A", x, y);
                assert_eq!(composed, crate::rules::constraint_site_ok(&b, "A", (x, y)));
            }
        }
        // (0,0) Hills, adjacent to Ocean, unclaimed → satisfies all three; the distractors fail one.
        assert!(crate::rules::constraint_site_ok(&b, "A", (0, 0)));
        assert!(!op_not_enemy_territory(&b, "A", 2, 0)); // owned by enemy B
        assert!(!op_is_defensible(&b, 3, 0)); // flat Grassland is not defensible
    }

    /// A 5x5 board for the t3-retreat cover/support axes: Hills at the retreat dest (2,2), player A's
    /// city "Fort" at (3,3), A's retreating unit #1 at (2,3) and another A unit #2 at (1,2) — the
    /// city and both units sit within SUPPORT_RADIUS (2) of the dest.
    fn retreat_board() -> Board {
        let mut tiles = Vec::new();
        for y in 0..5 {
            let mut row = Vec::new();
            for x in 0..5 {
                let terrain = if (x, y) == (2, 2) {
                    "Hills"
                } else {
                    "Grassland"
                };
                row.push(Tile {
                    x,
                    y,
                    terrain: terrain.to_string(),
                    extras: BTreeSet::new(),
                    owner: None,
                });
            }
            tiles.push(row);
        }
        let u = |x, y, id, kind: &str, owner: &str| Unit {
            x,
            y,
            id,
            kind: kind.to_string(),
            owner: owner.to_string(),
            veteran: 0,
            hp: 10,
        };
        Board {
            width: 5,
            height: 5,
            tiles,
            cities: vec![City {
                x: 3,
                y: 3,
                id: 1,
                name: "Fort".to_string(),
                owner: "A".to_string(),
                size: 2,
                improvements: BTreeSet::new(),
            }],
            units: vec![u(2, 3, 1, "Warriors", "A"), u(1, 2, 2, "Warriors", "A")],
            players: vec![Player {
                id: 0,
                name: "A".to_string(),
                nation: String::new(),
                is_alive: true,
            }],
            known: std::collections::HashMap::new(),
            visibility: None,
            ruleset: None,
            turn: None,
            source: "retreat".to_string(),
        }
    }

    /// FIX 3 — the tile-cover and support ops equal the `t3-retreat` solver's cover and support axes
    /// verbatim (`rules::retreat_cover` / `rules::retreat_support`, the very functions
    /// `retreat_axes_with` now calls), and the support op's `exclude` reproduces the solver's
    /// self-exclusion of the retreating unit.
    #[test]
    fn op_retreat_cover_support_parity_with_solver() {
        let b = retreat_board();
        let dest = (2, 2);
        let uid = 1; // the retreating unit at (2,3), within SUPPORT_RADIUS of the dest
        let field = crate::rules::ThreatField::compute(&b, "A");
        let axes = crate::rules::retreat_axes_with(&b, dest, "A", uid, &field);
        // cover op == the RetreatAxes cover axis (the tile's defense_multiplier), verbatim.
        assert_eq!(op_tile_cover(&b, "A", dest.0, dest.1), axes.cover);
        assert_eq!(
            op_tile_cover(&b, "A", dest.0, dest.1),
            crate::rules::retreat_cover(&b, "A", dest)
        );
        // support op excluding the retreating unit == the RetreatAxes support axis, verbatim.
        assert_eq!(op_support(&b, "A", dest.0, dest.1, Some(uid)), axes.support);
        // Own city Fort@(3,3) + own unit #2@(1,2) within radius 2 → 2; the retreating unit excluded.
        assert_eq!(op_support(&b, "A", dest.0, dest.1, Some(uid)), 2);
        // Counting ALL units (no exclusion) adds the retreating unit back — exclusion is load-bearing.
        assert_eq!(op_support(&b, "A", dest.0, dest.1, None), 3);
        // Hills grants real cover (> 1.0, whereas flat Grassland would be exactly 1.0).
        assert!(op_tile_cover(&b, "A", dest.0, dest.1) > 1.0);
    }

    /// A 6x1 Grassland strip with a STACKED tile at (1,0) that reproduces the T677 unit-stacking
    /// bug in miniature: an UNMODELED unit FIRST (Freight #100, so a first-match-by-coordinate
    /// dead-ends on it), the modeled SUBJECT deeper (Mech. Inf. #200, move 3, Land), and a faster
    /// modeled unit also stacked (AEGIS Cruiser #300, move 5, Sea). Iron sits at (5,0) as a
    /// reachable-nearest target. `combat_strip()` (one unit per tile) can never exercise this.
    fn stacked_strip() -> Board {
        let mut row = Vec::new();
        for x in 0..6 {
            row.push(Tile {
                x,
                y: 0,
                terrain: "Grassland".to_string(),
                extras: BTreeSet::new(),
                owner: None,
            });
        }
        row[5].extras.insert("Iron".to_string());
        let u = |x, id, kind: &str, owner: &str| Unit {
            x,
            y: 0,
            id,
            kind: kind.to_string(),
            owner: owner.to_string(),
            veteran: 0,
            hp: 10,
        };
        Board {
            width: 6,
            height: 1,
            tiles: vec![row],
            cities: vec![],
            // Board order = stack order at (1,0): Freight FIRST, then the subject, then the AEGIS.
            units: vec![
                u(1, 100, "Freight", "A"),       // unmodeled, first occupant
                u(1, 200, "Mech. Inf.", "A"),    // the modeled SUBJECT (move 3, Land)
                u(1, 300, "AEGIS Cruiser", "A"), // faster, but SEA (move 5) — must not land-path
            ],
            players: vec![Player {
                id: 0,
                name: "A".to_string(),
                nation: String::new(),
                is_alive: true,
            }],
            known: std::collections::HashMap::new(),
            visibility: None,
            ruleset: None,
            turn: None,
            source: "stacked".to_string(),
        }
    }

    /// Task D — the STACKED-tile invariant the old parity gate (combat_strip, one unit/tile) is
    /// blind to: (1) the render lists every stacked unit with its id; (2) each unit operator
    /// addressed by the SUBJECT's id returns the SUBJECT's value, never the first occupant's;
    /// (3) reach_turns refuses the stacked SEA unit instead of land-pathing it; (4) the operator
    /// result == the solver result for a unit-subject item on the stacked board; (5) the
    /// reconstruction gate round-trips the stacked tile (all three units recovered, in order).
    #[test]
    fn stacked_tile_operators_resolve_subject_by_id() {
        let b = stacked_strip();
        let freight = b.units.iter().find(|u| u.id == 100).unwrap();
        let mech = b.units.iter().find(|u| u.id == 200).unwrap();
        let aegis = b.units.iter().find(|u| u.id == 300).unwrap();

        // (1) The render lists ALL three stacked units on (1,0), each with its id — so the model can
        //     name the exact one. (Last-wins/first-wins rendering would show only one of the three.)
        let render = RawEncoder.render(&b);
        let line = render
            .lines()
            .find(|l| l.starts_with("(1, 0):"))
            .expect("tile (1,0) listed");
        assert!(line.contains("{unit #100 Freight owner A}"), "line: {line}");
        assert!(
            line.contains("{unit #200 Mech. Inf. owner A}"),
            "line: {line}"
        );
        assert!(
            line.contains("{unit #300 AEGIS Cruiser owner A}"),
            "line: {line}"
        );

        // (2) att_eff/def_eff BY the subject's id == the subject's rules value — NOT the first
        //     occupant's. First occupant (Freight #100) is unmodeled -> None (a first-match operator
        //     would dead-end here); a DIFFERENT modeled stackmate (AEGIS #300) gives a DIFFERENT
        //     value, so "by id" is demonstrably picking the right unit, not just any modeled one.
        assert_eq!(op_att_eff(&b, 200), crate::rules::att_eff(mech));
        assert_eq!(
            op_def_eff(&b, 200),
            crate::rules::unit_def_on_own_tile(&b, mech)
        );
        assert_eq!(op_att_eff(&b, 100), None, "Freight is unmodeled");
        assert_eq!(op_att_eff(&b, 300), crate::rules::att_eff(aegis));
        assert_ne!(
            op_att_eff(&b, 200),
            op_att_eff(&b, 300),
            "subject != stackmate"
        );
        // Through the surface: the subject measures, the unmodeled first occupant errors.
        assert_eq!(
            RawMaxOps.execute(&b, &tc("att_eff", "{\"unit\":200}")),
            fmt_f64(crate::rules::att_eff(mech).unwrap())
        );
        assert!(RawMaxOps
            .execute(&b, &tc("att_eff", "{\"unit\":100}"))
            .starts_with("error:"));

        // (3) reach_turns uses the SUBJECT's own origin/move-rate/class. Subject (Mech, move 3) to
        //     Iron at (5,0): 4 tiles -> ceil(4/3) = 2 turns. The faster stacked SEA unit (AEGIS) is
        //     REFUSED (class guard) rather than land-pathed — the latent bug the root-cause flagged.
        let horizon = b.width * b.height;
        // All units here are owner "A", so the tactical reach coincides with the geographic one.
        let mech_reach = ReachField::from_origins_tactical(&b, &[(1, 0)], 3, horizon, "A");
        assert_eq!(
            op_reach_turns(&b, 200, 5, 0),
            Some(mech_reach.turns_at(5, 0))
        );
        assert_eq!(op_reach_turns(&b, 200, 5, 0), Some(Some(2)));
        assert_eq!(
            op_reach_turns(&b, 300, 5, 0),
            None,
            "AEGIS is Sea — no land reach"
        );
        assert_eq!(op_reach_turns(&b, 100, 5, 0), None, "Freight is unmodeled");
        let sea_err = RawMaxOps.execute(&b, &tc("reach_turns", "{\"unit\":300,\"tx\":5,\"ty\":0}"));
        assert!(
            sea_err.contains("Sea"),
            "sea unit should be refused, got: {sea_err}"
        );
        assert_eq!(
            RawMaxOps.execute(&b, &tc("reach_turns", "{\"unit\":200,\"tx\":5,\"ty\":0}")),
            "2"
        );

        // (4) verify-oracle style: the operator result == the SOLVER result for a unit-subject item
        //     (reachable-nearest, subject = Mech #200) on the same stacked board.
        let q = Question::ReachableNearestResource {
            unit: Referent::Unit { id: 200 },
            resource: "Iron".to_string(),
            horizon,
        };
        assert_eq!(solve(&q, &b), Some(Answer::OptionalInt(Some(2))));
        assert_eq!(op_reach_turns(&b, 200, 5, 0), Some(Some(2)));

        // (5) reconstruction gate round-trips the stacked tile: expected() enumerates all three
        //     units (in board order, with ids), and the raw decoder recovers exactly those.
        let expected = RecoveredTile::expected(&b, 1, 0);
        assert_eq!(expected.units.len(), 3);
        assert_eq!(
            expected.units.iter().map(|u| u.id).collect::<Vec<_>>(),
            vec![100, 200, 300]
        );
        let recovered: std::collections::HashMap<(i32, i32), RecoveredTile> = RawEncoder
            .reconstruct(&render)
            .unwrap()
            .into_iter()
            .collect();
        assert_eq!(recovered.get(&(1, 0)).unwrap(), &expected);
        // The surface completeness gates pass on the stacked board too.
        assert!(RawMaxOps.reconstruct_via_tools(&b).is_ok());
        assert!(RawMaxOpsEnum.reconstruct_via_tools(&b).is_ok());
        assert!(InteractiveMaxOps.reconstruct_via_tools(&b).is_ok());

        // Ensure `freight` is genuinely unmodeled (guards the fixture's premise).
        assert!(crate::rules::unit_stat(&freight.kind).is_none());
    }

    /// Ripple check: a unit-subject question NAMES its subject by id, so the model can supply that
    /// id to the by-id operators — while still containing `unit_choice_label` as a substring (the T2
    /// closed-option matching invariant). Both must hold on the SAME rendering.
    #[test]
    fn unit_referent_exposes_id_and_keeps_choice_label() {
        let b = stacked_strip();
        let mech = b.units.iter().find(|u| u.id == 200).unwrap();
        let named = RawEncoder.render_referent(&b, &Referent::Unit { id: 200 });
        assert!(
            named.contains("(unit #200)"),
            "subject id must be in the prompt: {named}"
        );
        assert!(
            named.contains(&unit_choice_label(mech)),
            "choice-label substring preserved: {named}"
        );
        // The full reachable-nearest prompt carries the id too (its subject is a Unit referent).
        let q = Question::ReachableNearestResource {
            unit: Referent::Unit { id: 200 },
            resource: "Iron".to_string(),
            horizon: 6,
        };
        assert!(q.render(&RawEncoder, &b).contains("(unit #200)"));
    }

    #[test]
    fn maximal_ops_reproduce_attack_axes() {
        // compare-two-attacks: favorability = win_prob(attacker, target) (the HP/firepower odds),
        // threat_removed = att_eff(target) — the operators reproduce `rules::attack_axes` exactly.
        let b = combat_strip();
        let legion = b.units.iter().find(|u| u.id == 12).unwrap(); // B attacker at (7,0)
        let phalanx = b.units.iter().find(|u| u.id == 10).unwrap(); // A target in Home at (5,0)
        let ax = crate::rules::attack_axes(&b, legion, phalanx).unwrap();
        // The win_prob op IS the favorability axis, verbatim.
        let fav = op_win_prob(&b, 12, 10).unwrap();
        let thr = op_att_eff(&b, 10).unwrap();
        assert!(
            (ax.favorability - fav).abs() < 1e-12,
            "fav {} vs {}",
            ax.favorability,
            fav
        );
        assert!((ax.threat_removed - thr).abs() < 1e-12);
        // And a probability really is one — the raw att_eff/def_eff ratio it replaces is not.
        assert!((0.0..=1.0).contains(&fav), "fav {fav} not a probability");
    }

    #[test]
    fn op_win_prob_parity_and_matches_solver() {
        // win_prob(attacker, defender) == rules::attack_win_prob verbatim (the compare-two-attacks
        // favorability and the cf-vacate/triage "does the city fall" odds share this one op).
        let b = combat_strip();
        let legion = b.units.iter().find(|u| u.id == 12).unwrap();
        let phalanx = b.units.iter().find(|u| u.id == 10).unwrap();
        assert_eq!(
            op_win_prob(&b, 12, 10),
            crate::rules::attack_win_prob(&b, legion, phalanx)
        );
        // A defender standing in its own city: win_prob reproduces the ThreatField city-fall odds
        // used by cf-vacate/triage (same att_eff, same city-context def_eff, same hp/firepower).
        let home = &b.cities[0];
        let field = crate::rules::ThreatField::compute(&b, "A");
        if let (Some((def_u, def_p)), Some(att)) = (
            crate::rules::best_defender(&b, home),
            field.attacker_at(home.x, home.y),
        ) {
            let via_field = crate::rules::city_fall_prob(&field, home.x, home.y, def_u, def_p);
            // Find the attacker unit the field selected (the Legion) and call the op by ids.
            let att_id = b
                .units
                .iter()
                .find(|u| {
                    crate::rules::att_eff(u).map(|a| (a - att.power).abs() < 1e-9) == Some(true)
                        && u.owner != "A"
                })
                .map(|u| u.id)
                .unwrap();
            assert_eq!(op_win_prob(&b, att_id, def_u.id), Some(via_field));
        }
        // Missing / unmodeled ids error out cleanly (None), never panic.
        assert_eq!(op_win_prob(&b, 999, 10), None);
    }

    #[test]
    fn op_city_fall_prob_parity_matches_solver() {
        // city_fall_prob(x, y, owner) == rules::city_capture_prob for that city — the SAME value the
        // t3-threat solver argmaxes and the adv-assault-target capture axis scores (tool-parity).
        let b = combat_strip();
        let home = &b.cities[0]; // "Home", owner "A" at (5, 0)
        let field = crate::rules::ThreatField::compute(&b, &home.owner);
        let want = crate::rules::city_capture_prob(&b, home, &field);
        assert_eq!(
            op_city_fall_prob(&b, &home.owner, home.x, home.y),
            Some(want)
        );
        // The tool text is the formatted probability (city subject addressed by coordinate).
        assert_eq!(
            RawMaxOps.execute(
                &b,
                &tc("city_fall_prob", "{\"x\":5,\"y\":0,\"player\":\"A\"}")
            ),
            fmt_f64(want)
        );
        // An empty tile (a unit but no city) → clean error, never a panic.
        assert_eq!(op_city_fall_prob(&b, "A", 0, 0), None);
        assert!(RawMaxOps
            .execute(
                &b,
                &tc("city_fall_prob", "{\"x\":0,\"y\":0,\"player\":\"A\"}")
            )
            .starts_with("error:"));
        // Missing the 'player' arg errors cleanly too.
        assert!(RawMaxOps
            .execute(&b, &tc("city_fall_prob", "{\"x\":5,\"y\":0}"))
            .starts_with("error:"));
    }

    #[test]
    fn maximal_ops_reproduce_cf_vacate_inputs() {
        // cf-vacate: the solver reads the WITH-garrison city defense and the incoming threat; the
        // operators reproduce both (the counterfactual "without best defender" stays withheld).
        let b = combat_strip();
        let home = &b.cities[0];
        let (with_best, _without) = crate::rules::city_defense_vacated(&b, home);
        assert_eq!(op_city_defense(&b, 5, 0), Some(with_best));
        assert_eq!(
            op_threat(&b, "A", 5, 0),
            crate::rules::threat_at(&b, "A", (5, 0))
        );
    }

    #[test]
    fn raw_maxops_surface_shape() {
        let b = combat_strip();
        // Overview is the full raw board block (model "sees everything").
        assert_eq!(RawMaxOps.overview(&b), RawEncoder.render(&b));
        let names: Vec<&str> = RawMaxOps.tools().iter().map(|t| t.name).collect();
        for v in [
            "distance",
            "travel_turns",
            "count",
            "att_eff",
            "def_eff",
            "win_prob",
            "city_defense",
            "city_fall_prob",
            "garrison_defense",
            "threat",
            "reach_turns",
            "site_check",
        ] {
            assert!(names.contains(&v), "missing operator {v}");
        }
        // No perception/fetch verbs on this surface.
        for v in [
            "get_tile",
            "scan",
            "scan_grid",
            "region_summary",
            "list_cities",
            "list_units",
        ] {
            assert!(!names.contains(&v), "unexpected fetch verb {v}");
        }
        // Completeness gate passes (the raw block decodes back to the board).
        assert!(RawMaxOps.reconstruct_via_tools(&b).is_ok());
        // A fetch verb is unknown here; so is a WITHHELD selection operator.
        assert!(RawMaxOps
            .execute(&b, &tc("get_tile", "{\"x\":0,\"y\":0}"))
            .starts_with("error:"));
        assert!(RawMaxOps
            .execute(&b, &tc("most_threatened", "{}"))
            .starts_with("error:"));
        // Operators return the measured values (formatted). att_eff is addressed by unit id (12=Legion).
        assert_eq!(RawMaxOps.execute(&b, &tc("att_eff", "{\"unit\":12}")), "4");
        assert_eq!(
            RawMaxOps.execute(&b, &tc("city_defense", "{\"x\":5,\"y\":0}")),
            "3"
        );
        assert_eq!(
            RawMaxOps.execute(&b, &tc("threat", "{\"x\":5,\"y\":0,\"player\":\"A\"}")),
            "4"
        );
        // site_axes is RETIRED from the advertised menu (settle-site/best-site left the run,
        // 2026-08-17), so the C2 loop guard (runner.rs) blocks the model from naming it. The low-level
        // execute/run_tool dispatch is deliberately retained for the viewer playground + the
        // run_tool_parity gate, so a direct execute still computes it — it just isn't offered.
        assert!(
            !names.contains(&"site_axes"),
            "site_axes should be off the advertised maxops menu"
        );
        assert_eq!(
            RawMaxOps.execute(&b, &tc("site_axes", "{\"x\":5,\"y\":0,\"player\":\"A\"}")),
            "food=5 production=0 resources=0 safety=-4"
        );
    }

    #[test]
    fn interactive_maxops_has_fetch_and_all_operators() {
        let b = combat_strip();
        let names: Vec<&str> = InteractiveMaxOps.tools().iter().map(|t| t.name).collect();
        // scan_region is the SOLE terrain-perception verb on this menu.
        assert!(names.contains(&"scan_region"), "missing fetch verb scan_region");
        // region_summary + the list_* fetch VERBS are retired (roster front-loads occupants), and
        // scan/get_tile/scan_grid are dropped too — scan/get_tile were used wastefully, and scan_grid's
        // hidden-force edge was a scan_region fog-fidelity gap now fixed (findings-scangrid-drop-*).
        for v in ["region_summary", "list_cities", "list_units", "scan", "get_tile", "scan_grid"] {
            assert!(!names.contains(&v), "verb {v} should be off the interactive-maxops menu");
        }
        for v in [
            "distance",
            "travel_turns",
            "count",
            "att_eff",
            "def_eff",
            "win_prob",
            "city_defense",
            "city_fall_prob",
            "garrison_defense",
            "threat",
            "reach_turns",
            "site_check",
        ] {
            assert!(names.contains(&v), "missing operator {v}");
        }
        // A fetch verb still works (delegates to Interactive)...
        assert!(InteractiveMaxOps
            .execute(&b, &tc("get_tile", "{\"x\":5,\"y\":0}"))
            .contains("city \"Home\""));
        // ...and a maximal operator works too.
        assert_eq!(
            InteractiveMaxOps.execute(&b, &tc("city_defense", "{\"x\":5,\"y\":0}")),
            "3"
        );
    }

    #[test]
    fn scan_region_lists_known_tiles_with_flags_and_hidden_count() {
        let b = mini(); // 4x4 Grassland; city "Cap" @(1,1); Forest+Iron @(3,0).
        let out = InteractiveMaxOps.execute(&b, &tc("scan_region", "{\"x\":1,\"y\":1}"));
        // Header: window bounds, dominant terrain, hidden count (ALWAYS present, even zero).
        assert!(out.contains("window (0,0)-(3,3)"), "out:\n{out}");
        assert!(out.contains("dominant terrain Grassland"), "out:\n{out}");
        assert!(out.contains("0 tiles Unknown (never explored"), "out:\n{out}");
        // A resource tile shows terrain + resource, no occupant.
        assert!(out.contains("(3, 0): Forest, Iron"), "out:\n{out}");
        // The city tile shows only a lightweight [city] flag — NO name/size/strength.
        assert!(out.contains("(1, 1): Grassland, [city]"), "out:\n{out}");
        assert!(!out.contains("Cap"), "scan_region must not name occupants:\n{out}");
        // Every KNOWN tile is listed (terrain layer), so a plain corner tile appears too.
        assert!(out.contains("(0, 0): Grassland"), "out:\n{out}");
    }

    #[test]
    fn scan_region_lists_fogged_tiles_with_terrain_and_territory_hiding_units() {
        // Regression for the fog-fidelity gap (findings-scangrid-drop-budgetaware.md): scan_region used
        // to DROP every fogged tile as "Unknown", so hidden-force saw fogged enemy land as empty. Now a
        // fogged tile is LISTED (terrain/owner/(fogged)) with its live units hidden; only NEVER-explored
        // tiles are omitted.
        let mut b = mini(); // 4x4; city @(1,1); Forest+Iron @(3,0).
        // (2,2): fogged enemy LAND (Hills, bordered by Enemy) with a live unit still in board.units —
        // the perspective mask would remove it; the render must hide it regardless.
        b.tiles[2][2].terrain = "Hills".to_string();
        b.tiles[2][2].owner = Some("Enemy".to_string());
        b.units.push(Unit { x: 2, y: 2, id: 77, kind: "Legion".to_string(), owner: "Enemy".to_string(), veteran: 0, hp: 10 });
        // (0,3): never explored.
        b.tiles[3][0].terrain = "Unknown".to_string();
        // Build a visibility grid: everything Visible except (2,2) Fogged and (0,3) Unexplored.
        let mut vis = vec![vec![Visibility::Visible; 4]; 4];
        vis[2][2] = Visibility::Fogged;
        vis[3][0] = Visibility::Unexplored;
        b.visibility = Some(vis);

        let out = InteractiveMaxOps.execute(&b, &tc("scan_region", "{\"x\":2,\"y\":2}"));
        // The fogged land tile is LISTED with terrain + territory + the (fogged) tag...
        assert!(out.contains("(2, 2): Hills, borders Enemy, (fogged)"), "out:\n{out}");
        // ...but its live unit is HIDDEN (that is the hidden-force premise).
        assert!(!out.contains("Legion"), "fogged unit must stay hidden:\n{out}");
        assert!(!out.contains("#77"), "fogged unit id must stay hidden:\n{out}");
        // Only the never-explored tile is counted as Unknown (not the fogged one).
        assert!(out.contains("1 tiles Unknown (never explored"), "out:\n{out}");
    }

    #[test]
    fn interactive_maxops_roster_front_loads_with_size_and_walls() {
        let b = combat_strip(); // 10x1; city "Home" (unwalled) @(5,0); 3 units.
        let ov = InteractiveMaxOps.overview(&b);
        // The roster LEADS the overview and carries both directories.
        assert!(ov.starts_with("KNOWN UNITS"), "roster must lead the overview:\n{ov}");
        assert!(ov.contains("KNOWN CITIES"), "ov:\n{ov}");
        // City line: name, owner, position, size (no walls here).
        assert!(ov.contains("\"Home\" owner A at (5, 0) size 3"), "ov:\n{ov}");
        // Unit line: type/owner/position/id.
        assert!(ov.contains("#10 Phalanx owner A at (5, 0)"), "ov:\n{ov}");
        // Quadrant block keeps COUNTS, drops the per-name enumeration.
        assert!(ov.contains("Units:"), "ov:\n{ov}");
        assert!(!ov.contains("Home (owner"), "quadrant must not re-name cities:\n{ov}");

        // A walled city gains the ", walls" suffix in the roster.
        let mut walled = combat_strip();
        walled.cities[0].improvements.insert("City Walls".to_string());
        let ov2 = InteractiveMaxOps.overview(&walled);
        assert!(
            ov2.contains("\"Home\" owner A at (5, 0) size 3, walls"),
            "ov2:\n{ov2}"
        );
    }

    #[test]
    fn maximal_op_error_paths() {
        let b = combat_strip();
        assert!(RawMaxOps
            .execute(&b, &tc("att_eff", "{\"unit\":999}"))
            .starts_with("error:")); // no such unit id
        assert!(RawMaxOps
            .execute(&b, &tc("city_defense", "{\"x\":0,\"y\":0}"))
            .starts_with("error:")); // no city
        assert!(RawMaxOps
            .execute(&b, &tc("threat", "{\"x\":5,\"y\":0}"))
            .starts_with("error:")); // missing player
        assert!(RawMaxOps
            .execute(&b, &tc("att_eff", "{\"x\":99,\"y\":0}"))
            .starts_with("error:")); // missing 'unit' arg
    }

    // ---- enumeration/index operators: parity gate + subsumption + surface shape ----

    #[test]
    fn op_list_tiles_parity_and_subsumes_counts() {
        let b = mini(); // 4x4: Forest+Iron at (3,0), else Grassland
                        // SUBSUMPTION: list_tiles length == the scalar count it replaces, over the SAME box —
                        // proven for a terrain predicate and a resource predicate.
        for (x0, y0, x1, y1) in [(0, 0, 3, 3), (1, 1, 3, 3), (0, 0, 0, 0)] {
            assert_eq!(
                op_list_tiles(&b, x0, y0, x1, y1, Some("Grassland"), None).len() as i64,
                op_count_terrain(&b, x0, y0, x1, y1, "Grassland"),
            );
            assert_eq!(
                op_list_tiles(&b, x0, y0, x1, y1, Some("Forest"), None).len() as i64,
                op_count_terrain(&b, x0, y0, x1, y1, "Forest"),
            );
            assert_eq!(
                op_list_tiles(&b, x0, y0, x1, y1, None, Some("Iron")).len() as i64,
                op_count_resource(&b, x0, y0, x1, y1, "Iron"),
            );
        }
        // The POSITIONS are the actual matching tiles, row-major.
        assert_eq!(
            op_list_tiles(&b, 0, 0, 3, 3, Some("Forest"), None),
            vec![(3, 0)]
        );
        assert_eq!(
            op_list_tiles(&b, 0, 0, 3, 3, None, Some("Iron")),
            vec![(3, 0)]
        );
        // Both predicates are ANDed (Forest AND Iron -> the one tile; Grassland AND Iron -> none).
        assert_eq!(
            op_list_tiles(&b, 0, 0, 3, 3, Some("Forest"), Some("Iron")),
            vec![(3, 0)]
        );
        assert!(op_list_tiles(&b, 0, 0, 3, 3, Some("Grassland"), Some("Iron")).is_empty());
        // Out-of-bounds corners are clipped (as count_terrain clips), never a panic.
        assert_eq!(
            op_list_tiles(&b, -5, -5, 99, 99, Some("Forest"), None),
            op_list_tiles(&b, 0, 0, 3, 3, Some("Forest"), None),
        );
    }

    /// A 20x5 board for the nearest-owned exclusion filter. Player "p" owns two cities (with a
    /// stacked garrison on one, honoring the B1 lesson — real stacked shapes, not one-unit tiles),
    /// enemy "e" owns one, plus Gold deposits at controlled Chebyshev distances and one Gold on a
    /// FOGGED ("Unknown") tile. Mirrors the `NearestOwnedResource` fixture shape.
    fn owned_exclusion_board() -> Board {
        let golds = [(3, 4), (6, 2), (10, 4), (16, 3), (13, 0), (19, 2)];
        let mut tiles = Vec::new();
        for y in 0..5 {
            let mut row = Vec::new();
            for x in 0..20 {
                // One fogged tile at (13,0): Unknown terrain is non-land (unreachable), but it still
                // CARRIES its Gold — so both the solver and the filter must include it as a
                // candidate (the reach step, not the filter, later drops it).
                let terrain = if (x, y) == (13, 0) {
                    "Unknown"
                } else {
                    "Grassland"
                };
                let mut extras = BTreeSet::new();
                if golds.contains(&(x, y)) {
                    extras.insert("Gold".to_string());
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
            width: 20,
            height: 5,
            tiles,
            cities: vec![
                City {
                    x: 3,
                    y: 2,
                    id: 1,
                    name: "A".to_string(),
                    owner: "p".to_string(),
                    size: 3,
                    improvements: BTreeSet::new(),
                },
                City {
                    x: 10,
                    y: 2,
                    id: 2,
                    name: "B".to_string(),
                    owner: "p".to_string(),
                    size: 3,
                    improvements: BTreeSet::new(),
                },
                City {
                    x: 16,
                    y: 2,
                    id: 3,
                    name: "E".to_string(),
                    owner: "e".to_string(),
                    size: 3,
                    improvements: BTreeSet::new(),
                },
            ],
            // Two units stacked on the kept resource tile (6,2) and one on city A — the filter is over
            // CITIES only, so units must not perturb its result (part of what parity proves).
            units: vec![
                Unit {
                    x: 6,
                    y: 2,
                    id: 7,
                    kind: "Warriors".to_string(),
                    owner: "p".to_string(),
                    veteran: 0,
                    hp: 10,
                },
                Unit {
                    x: 6,
                    y: 2,
                    id: 8,
                    kind: "Warriors".to_string(),
                    owner: "p".to_string(),
                    veteran: 0,
                    hp: 10,
                },
                Unit {
                    x: 3,
                    y: 2,
                    id: 9,
                    kind: "Warriors".to_string(),
                    owner: "p".to_string(),
                    veteran: 0,
                    hp: 10,
                },
            ],
            players: vec![
                Player {
                    id: 0,
                    name: "p".to_string(),
                    nation: String::new(),
                    is_alive: true,
                },
                Player {
                    id: 1,
                    name: "e".to_string(),
                    nation: String::new(),
                    is_alive: true,
                },
            ],
            known: std::collections::HashMap::new(),
            visibility: None,
            ruleset: None,
            turn: None,
            source: "owned-exclusion".to_string(),
        }
    }

    /// FAITHFULNESS gate: the positional-exclusion filter returns EXACTLY the `nearest-owned`
    /// solver's eligible-candidate set (resource tiles whose min Chebyshev to any OWN city is > 2),
    /// so the tool's returned set is faithful to what the scored answer is computed over. Proven
    /// three ways: (a) against the hand-computed set, (b) against an independent replay of the
    /// solver's exclusion predicate on the SAME board, and (c) end-to-end — feeding the filtered set
    /// through the solver's ReachField reproduces `solve()`'s exact answer.
    #[test]
    fn op_list_tiles_excluding_owned_mirrors_nearest_owned_solver() {
        use crate::question::{solve, Answer, Question};
        let b = owned_exclusion_board();

        // (a) Whole-board box, Gold, exclude the caller's own cities within Chebyshev 2 (the solver's
        // radius). (3,4) is Cheby-2 of A and (10,4) is Cheby-2 of B → excluded; (16,3) sits next to
        // the ENEMY city E but is Cheby-6 from the nearest OWN city → KEPT (only own cities exclude);
        // (13,0) is a fogged Gold, still a candidate. Row-major (y then x).
        let got = op_list_tiles_excluding_owned(&b, 0, 0, 19, 4, None, Some("Gold"), "p", 2);
        assert_eq!(got, vec![(13, 0), (6, 2), (19, 2), (16, 3)]);

        // (b) Independent replay of the solver's exclusion predicate over the SAME board.
        let own: Vec<(i32, i32)> = b
            .cities
            .iter()
            .filter(|c| c.owner == "p")
            .map(|c| (c.x, c.y))
            .collect();
        let mut expect: Vec<(i32, i32)> = b
            .iter_tiles()
            .filter(|t| t.has("Gold"))
            .filter(|t| own.iter().map(|&c| chebyshev((t.x, t.y), c)).min().unwrap() > 2)
            .map(|t| (t.x, t.y))
            .collect();
        expect.sort_by_key(|&(x, y)| (y, x));
        let mut got_sorted = got.clone();
        got_sorted.sort_by_key(|&(x, y)| (y, x));
        assert_eq!(
            got_sorted, expect,
            "filter set must equal the solver's eligible set"
        );

        // (c) End-to-end: the min travel-turns (move 1) over the FILTERED set == solve()'s answer.
        // (6,2) is 3 land turns from city A (nearest); the fogged (13,0) is unreachable and dropped
        // by the reach step, exactly as in the solver.
        let horizon = 20;
        // The solver is TACTICAL for "p" (enemy city E blocks its own tile; no enemy units here, so
        // no ZOC) — mirror it so the parity holds by construction.
        let reach = crate::rules::ReachField::from_origins_tactical(&b, &own, 1, horizon, "p");
        let via_filter = got.iter().filter_map(|&(x, y)| reach.turns_at(x, y)).min();
        let q = Question::NearestOwnedResource {
            player: "p".to_string(),
            resource: "Gold".to_string(),
            horizon,
        };
        assert_eq!(
            solve(&q, &b),
            Some(Answer::OptionalInt(via_filter.map(|t| t as i64)))
        );
        assert_eq!(
            via_filter,
            Some(3),
            "nearest unexploited Gold is 3 land turns away"
        );
    }

    /// The exclusion filter is reachable through the enum surfaces' `list_tiles` dispatch, and its
    /// malformed forms return a clear `error:` (never a panic): one exclude arg without the other,
    /// and a negative radius. Also confirms neither exclude arg = the plain (unfiltered) behavior.
    #[test]
    fn list_tiles_exclusion_dispatch_and_errors() {
        let b = owned_exclusion_board();
        // Both exclude args present → the filtered set, count-led, row-major.
        assert_eq!(
            RawCalcEnum.execute(
                &b,
                &tc("list_tiles", "{\"x0\":0,\"y0\":0,\"x1\":19,\"y1\":4,\"resource\":\"Gold\",\"exclude_owner\":\"p\",\"exclude_radius\":2}")
            ),
            "4 tiles: (13, 0), (6, 2), (19, 2), (16, 3)"
        );
        // Neither exclude arg → unfiltered: all six Gold tiles.
        assert_eq!(
            RawCalcEnum.execute(
                &b,
                &tc(
                    "list_tiles",
                    "{\"x0\":0,\"y0\":0,\"x1\":19,\"y1\":4,\"resource\":\"Gold\"}"
                )
            ),
            "6 tiles: (13, 0), (6, 2), (19, 2), (16, 3), (3, 4), (10, 4)"
        );
        // Only exclude_owner (no radius) → error, not a silent unfiltered result.
        assert!(RawCalcEnum
            .execute(&b, &tc("list_tiles", "{\"x0\":0,\"y0\":0,\"x1\":19,\"y1\":4,\"resource\":\"Gold\",\"exclude_owner\":\"p\"}"))
            .starts_with("error:"));
        // Only exclude_radius (no owner) → error.
        assert!(RawCalcEnum
            .execute(&b, &tc("list_tiles", "{\"x0\":0,\"y0\":0,\"x1\":19,\"y1\":4,\"resource\":\"Gold\",\"exclude_radius\":2}"))
            .starts_with("error:"));
        // Negative radius → error.
        assert!(RawCalcEnum
            .execute(&b, &tc("list_tiles", "{\"x0\":0,\"y0\":0,\"x1\":19,\"y1\":4,\"resource\":\"Gold\",\"exclude_owner\":\"p\",\"exclude_radius\":-1}"))
            .starts_with("error:"));
        // The same exclusion is exposed on the other two enum surfaces (byte-identical enum verbs).
        assert_eq!(
            RawMaxOpsEnum.execute(
                &b,
                &tc("list_tiles", "{\"x0\":0,\"y0\":0,\"x1\":19,\"y1\":4,\"resource\":\"Gold\",\"exclude_owner\":\"p\",\"exclude_radius\":2}")
            ),
            "4 tiles: (13, 0), (6, 2), (19, 2), (16, 3)"
        );
    }

    /// FAITHFULNESS gate: `travel_turns_from_owned` is the solver's inner op — its per-tile value
    /// equals `ReachField::from_origins(own_cities, 1, NEAREST_OWNED_HORIZON).turns_at(...)`, and the
    /// intended model path (list_tiles(exclude…) → per-tile travel_turns_from_owned → min) reproduces
    /// `solve(NearestOwnedResource)` exactly. Covers reachable, beyond-horizon, fogged/non-land, and
    /// no-own-cities cases on the same fixture the exclusion filter is parity-tested on.
    #[test]
    fn op_travel_turns_from_owned_mirrors_solver_reach() {
        use crate::question::{solve, Answer, Question};
        let b = owned_exclusion_board();
        let own: Vec<(i32, i32)> = b
            .cities
            .iter()
            .filter(|c| c.owner == "p")
            .map(|c| (c.x, c.y))
            .collect();
        let h = crate::generators::NEAREST_OWNED_HORIZON;

        // (a) Per-tile value parity vs the solver's own multi-source field at the SAME horizon
        // (TACTICAL for "p", matching the op).
        let reach = crate::rules::ReachField::from_origins_tactical(&b, &own, 1, h, "p");
        for &(x, y) in &[(6, 2), (16, 3), (19, 2), (13, 0), (3, 4), (10, 4)] {
            assert_eq!(
                op_travel_turns_from_owned(&b, "p", x, y),
                reach.turns_at(x, y)
            );
        }
        // Spot values: (6,2)=3 from nearest city A; (16,3)=6 (== horizon, still reachable);
        // (19,2)=9 land turns → beyond horizon 6 → None; (13,0) fogged non-land → None.
        assert_eq!(op_travel_turns_from_owned(&b, "p", 6, 2), Some(3));
        assert_eq!(op_travel_turns_from_owned(&b, "p", 16, 3), Some(6));
        assert_eq!(op_travel_turns_from_owned(&b, "p", 19, 2), None);
        assert_eq!(op_travel_turns_from_owned(&b, "p", 13, 0), None);
        // No own cities → empty origin set → nothing reachable (matches the solver's ill-posed case).
        assert_eq!(op_travel_turns_from_owned(&b, "nobody", 6, 2), None);

        // (b) End-to-end: eligible set → per-tile travel_turns_from_owned → min == solve().
        let eligible = op_list_tiles_excluding_owned(&b, 0, 0, 19, 4, None, Some("Gold"), "p", 2);
        let via_tool = eligible
            .iter()
            .filter_map(|&(x, y)| op_travel_turns_from_owned(&b, "p", x, y))
            .min();
        let q = Question::NearestOwnedResource {
            player: "p".to_string(),
            resource: "Gold".to_string(),
            horizon: h,
        };
        assert_eq!(
            solve(&q, &b),
            Some(Answer::OptionalInt(via_tool.map(|t| t as i64)))
        );
        assert_eq!(
            via_tool,
            Some(3),
            "nearest unexploited Gold is 3 land turns from the nearest own city"
        );
    }

    /// `travel_turns_from_owned` is reachable through the enum surfaces' dispatch (byte-identical on
    /// all three), returns `"unreachable"` past the horizon / off-land, and errors (never panics) on
    /// malformed input.
    #[test]
    fn travel_turns_from_owned_dispatch_and_errors() {
        let b = owned_exclusion_board();
        // Reachable within horizon → the integer, bare.
        assert_eq!(
            RawCalcEnum.execute(
                &b,
                &tc(
                    "travel_turns_from_owned",
                    "{\"player\":\"p\",\"tx\":6,\"ty\":2}"
                )
            ),
            "3"
        );
        // Beyond horizon 6 → "unreachable".
        assert_eq!(
            RawCalcEnum.execute(
                &b,
                &tc(
                    "travel_turns_from_owned",
                    "{\"player\":\"p\",\"tx\":19,\"ty\":2}"
                )
            ),
            "unreachable"
        );
        // Fogged / non-land target → "unreachable".
        assert_eq!(
            RawCalcEnum.execute(
                &b,
                &tc(
                    "travel_turns_from_owned",
                    "{\"player\":\"p\",\"tx\":13,\"ty\":0}"
                )
            ),
            "unreachable"
        );
        // Missing player → error, not a stray value.
        assert!(RawCalcEnum
            .execute(&b, &tc("travel_turns_from_owned", "{\"tx\":6,\"ty\":2}"))
            .starts_with("error:"));
        // Out-of-bounds target → error, never a panic.
        assert!(RawCalcEnum
            .execute(
                &b,
                &tc(
                    "travel_turns_from_owned",
                    "{\"player\":\"p\",\"tx\":99,\"ty\":0}"
                )
            )
            .starts_with("error:"));
        // Exposed byte-identically on the maximal enum surface too.
        assert_eq!(
            RawMaxOpsEnum.execute(
                &b,
                &tc(
                    "travel_turns_from_owned",
                    "{\"player\":\"p\",\"tx\":6,\"ty\":2}"
                )
            ),
            "3"
        );
    }

    #[test]
    fn op_owned_rosters_parity_with_board_membership() {
        let b = combat_strip(); // city Home(A)@(5,0); units A@(0,0),A@(5,0), B@(7,0)
                                // Each roster == exactly the coords of that owner's cities/units the board holds.
        let a_cities: Vec<(i32, i32)> = b
            .cities
            .iter()
            .filter(|c| c.owner == "A")
            .map(|c| (c.x, c.y))
            .collect();
        assert_eq!(op_owned_cities(&b, "A"), a_cities);
        assert_eq!(op_owned_cities(&b, "A"), vec![(5, 0)]);
        assert!(op_owned_cities(&b, "B").is_empty());
        // Unit rosters carry the id AND coordinate, sorted row-major (y,x): Warriors#11@(0,0), Phalanx#10@(5,0).
        let a_units: Vec<(i32, i32, i32)> = owned_units_sorted(&b, "A")
            .iter()
            .map(|u| (u.id, u.x, u.y))
            .collect();
        assert_eq!(a_units, vec![(11, 0, 0), (10, 5, 0)]);
        assert_eq!(
            fmt_units(&owned_units_sorted(&b, "A")),
            "2 units: #11 (0, 0), #10 (5, 0)"
        );
        assert_eq!(
            fmt_units(&owned_units_sorted(&b, "B")),
            "1 units: #12 (7, 0)"
        );
        assert_eq!(fmt_units(&owned_units_sorted(&b, "Nobody")), "0 units");
    }

    #[test]
    fn raw_maxops_enum_surface_shape() {
        let b = combat_strip();
        assert_eq!(RawMaxOpsEnum.overview(&b), RawEncoder.render(&b));
        let names: Vec<&str> = RawMaxOpsEnum.tools().iter().map(|t| t.name).collect();
        // The intended 17-verb enumeration menu, in one place (site_axes retired 2026-08-17).
        assert_eq!(
            names,
            vec![
                "distance",
                "travel_turns",
                "att_eff",
                "def_eff",
                "win_prob",
                "city_defense",
                "city_fall_prob",
                "garrison_defense",
                "threat",
                "tile_cover",
                "support",
                "reach_turns",
                "site_check",
                "list_tiles",
                "list_owned_cities",
                "list_owned_units",
                "travel_turns_from_owned",
            ]
        );
        // count is RETIRED here (subsumed by list_tiles) and no fetch verbs are exposed.
        for v in [
            "count",
            "get_tile",
            "scan",
            "scan_grid",
            "region_summary",
            "list_cities",
            "list_units",
        ] {
            assert!(!names.contains(&v), "unexpected verb {v}");
        }
        assert!(RawMaxOpsEnum.reconstruct_via_tools(&b).is_ok());
        // A retired count verb is now an error, as is a fetch verb and a withheld selection op.
        assert!(RawMaxOpsEnum
            .execute(
                &b,
                &tc(
                    "count",
                    "{\"x0\":0,\"y0\":0,\"x1\":9,\"y1\":0,\"kind\":\"terrain\",\"feature\":\"Grassland\"}"
                )
            )
            .starts_with("error:"));
        assert!(RawMaxOpsEnum
            .execute(&b, &tc("get_tile", "{\"x\":0,\"y\":0}"))
            .starts_with("error:"));
        assert!(RawMaxOpsEnum
            .execute(&b, &tc("most_threatened", "{}"))
            .starts_with("error:"));
        // The enumerators and an inherited measurement op all work.
        assert_eq!(
            RawMaxOpsEnum.execute(&b, &tc("list_tiles", "{\"x0\":0,\"y0\":0,\"x1\":9,\"y1\":0,\"terrain\":\"Grassland\"}")),
            "10 tiles: (0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (5, 0), (6, 0), (7, 0), (8, 0), (9, 0)"
        );
        assert_eq!(
            RawMaxOpsEnum.execute(&b, &tc("list_owned_cities", "{\"player\":\"A\"}")),
            "1 cities: (5, 0)"
        );
        // Units carry their id (Warriors #11 @(0,0), Phalanx #10 @(5,0)) so they can be fed to the by-id ops.
        assert_eq!(
            RawMaxOpsEnum.execute(&b, &tc("list_owned_units", "{\"player\":\"A\"}")),
            "2 units: #11 (0, 0), #10 (5, 0)"
        );
        assert_eq!(
            RawMaxOpsEnum.execute(&b, &tc("city_defense", "{\"x\":5,\"y\":0}")),
            "3"
        );
    }

    #[test]
    fn raw_calc_surface_shape() {
        let b = combat_strip();
        // Overview is the full raw board block (model "sees everything").
        assert_eq!(RawCalc.overview(&b), RawEncoder.render(&b));
        let names: Vec<&str> = RawCalc.tools().iter().map(|t| t.name).collect();
        // EXACTLY the four spatial operators, in order.
        assert_eq!(
            names,
            vec!["distance", "travel_turns", "count", "reach_turns"]
        );
        // No combat/valuation ops, no enumeration, no fetch verbs.
        for v in [
            "att_eff",
            "def_eff",
            "city_defense",
            "garrison_defense",
            "threat",
            "site_axes",
            "list_tiles",
            "list_owned_cities",
            "list_owned_units",
            "get_tile",
            "scan",
            "scan_grid",
            "region_summary",
            "list_cities",
            "list_units",
        ] {
            assert!(!names.contains(&v), "unexpected verb {v}");
        }
        // Completeness gate passes (the raw block decodes back to the board).
        assert!(RawCalc.reconstruct_via_tools(&b).is_ok());
        // A WITHHELD combat op, and a fetch verb, are both errors.
        assert!(RawCalc
            .execute(&b, &tc("att_eff", "{\"unit\":10}"))
            .starts_with("error:"));
        assert!(RawCalc
            .execute(&b, &tc("threat", "{\"x\":5,\"y\":0,\"player\":\"A\"}"))
            .starts_with("error:"));
        assert!(RawCalc
            .execute(&b, &tc("get_tile", "{\"x\":0,\"y\":0}"))
            .starts_with("error:"));
        // The five exposed operators all return their measured values. reach_turns is by unit id
        // (Warriors #11 @(0,0), move 1, to (5,0) over a land strip = 5 turns).
        assert_eq!(
            RawCalc.execute(&b, &op_call("distance", 0, 0, 5, 0, "")),
            "5"
        );
        assert_eq!(
            RawCalc.execute(&b, &tc("reach_turns", "{\"unit\":11,\"tx\":5,\"ty\":0}")),
            "5"
        );
    }

    #[test]
    fn raw_calc_enum_surface_shape() {
        let b = combat_strip();
        assert_eq!(RawCalcEnum.overview(&b), RawEncoder.render(&b));
        let names: Vec<&str> = RawCalcEnum.tools().iter().map(|t| t.name).collect();
        // EXACTLY the seven-verb menu, in order.
        assert_eq!(
            names,
            vec![
                "distance",
                "travel_turns",
                "reach_turns",
                "list_tiles",
                "list_owned_cities",
                "list_owned_units",
                "travel_turns_from_owned",
            ]
        );
        // count RETIRED, combat/valuation ops withheld, no fetch verbs.
        for v in [
            "count",
            "att_eff",
            "def_eff",
            "city_defense",
            "garrison_defense",
            "threat",
            "site_axes",
            "get_tile",
            "scan",
            "scan_grid",
            "region_summary",
            "list_cities",
            "list_units",
        ] {
            assert!(!names.contains(&v), "unexpected verb {v}");
        }
        assert!(RawCalcEnum.reconstruct_via_tools(&b).is_ok());
        // A retired count verb, a withheld combat op, and a fetch verb are all errors.
        assert!(RawCalcEnum
            .execute(
                &b,
                &tc(
                    "count",
                    "{\"x0\":0,\"y0\":0,\"x1\":9,\"y1\":0,\"kind\":\"terrain\",\"feature\":\"Grassland\"}"
                )
            )
            .starts_with("error:"));
        assert!(RawCalcEnum
            .execute(&b, &tc("threat", "{\"x\":5,\"y\":0,\"player\":\"A\"}"))
            .starts_with("error:"));
        assert!(RawCalcEnum
            .execute(&b, &tc("get_tile", "{\"x\":0,\"y\":0}"))
            .starts_with("error:"));
        // The six exposed operators all work.
        assert_eq!(
            RawCalcEnum.execute(&b, &tc("list_tiles", "{\"x0\":0,\"y0\":0,\"x1\":9,\"y1\":0,\"terrain\":\"Grassland\"}")),
            "10 tiles: (0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (5, 0), (6, 0), (7, 0), (8, 0), (9, 0)"
        );
        assert_eq!(
            RawCalcEnum.execute(&b, &tc("list_owned_cities", "{\"player\":\"A\"}")),
            "1 cities: (5, 0)"
        );
        assert_eq!(
            RawCalcEnum.execute(&b, &tc("list_owned_units", "{\"player\":\"A\"}")),
            "2 units: #11 (0, 0), #10 (5, 0)"
        );
        assert_eq!(
            RawCalcEnum.execute(&b, &tc("reach_turns", "{\"unit\":11,\"tx\":5,\"ty\":0}")),
            "5"
        );
    }

    #[test]
    fn list_tiles_error_paths() {
        let b = combat_strip();
        // At least one predicate is required.
        assert!(RawMaxOpsEnum
            .execute(
                &b,
                &tc("list_tiles", "{\"x0\":0,\"y0\":0,\"x1\":9,\"y1\":0}")
            )
            .starts_with("error:"));
        // A non-resource extra is rejected (same is_resource gate as count_resource).
        assert!(RawMaxOpsEnum
            .execute(
                &b,
                &tc(
                    "list_tiles",
                    "{\"x0\":0,\"y0\":0,\"x1\":9,\"y1\":0,\"resource\":\"Road\"}"
                )
            )
            .starts_with("error:"));
        // Reversed box is rejected; missing player is rejected.
        assert!(RawMaxOpsEnum
            .execute(
                &b,
                &tc(
                    "list_tiles",
                    "{\"x0\":9,\"y0\":0,\"x1\":0,\"y1\":0,\"terrain\":\"Grassland\"}"
                )
            )
            .starts_with("error:"));
        assert!(RawMaxOpsEnum
            .execute(&b, &tc("list_owned_cities", "{}"))
            .starts_with("error:"));
        // A zero-match enumeration reads as "0 tiles" / "(no ... owned by ...)", not an error.
        assert_eq!(
            RawMaxOpsEnum.execute(
                &b,
                &tc(
                    "list_tiles",
                    "{\"x0\":0,\"y0\":0,\"x1\":9,\"y1\":0,\"terrain\":\"Ocean\"}"
                )
            ),
            "0 tiles"
        );
        assert_eq!(
            RawMaxOpsEnum.execute(&b, &tc("list_owned_units", "{\"player\":\"Z\"}")),
            "0 units"
        );
    }

    #[test]
    fn interactive_maxops_enum_has_fetch_and_enum_calculator() {
        let b = combat_strip();
        let names: Vec<&str> = InteractiveMaxOpsEnum
            .tools()
            .iter()
            .map(|t| t.name)
            .collect();
        // scan_region is the SOLE terrain-perception verb on this menu.
        assert!(names.contains(&"scan_region"), "missing fetch verb scan_region");
        // region_summary + the list_* fetch VERBS are retired (roster front-loads occupants), and
        // scan/get_tile/scan_grid are dropped too (scan_grid's hidden-force edge was a scan_region
        // fog-fidelity gap, now fixed — findings-scangrid-drop-*).
        for v in ["region_summary", "list_cities", "list_units", "scan", "get_tile", "scan_grid"] {
            assert!(!names.contains(&v), "verb {v} should be off the interactive-maxops-enum menu");
        }
        for v in [
            "distance",
            "travel_turns",
            "att_eff",
            "def_eff",
            "city_defense",
            "city_fall_prob",
            "garrison_defense",
            "threat",
            "reach_turns",
            "site_check",
            "list_tiles",
        ] {
            assert!(names.contains(&v), "missing operator {v}");
        }
        // The player-scoped owned-enumerators are DROPPED from the interactive menu (redundant with
        // the front-loaded roster); they stay on raw-maxops-enum. See tools().
        for v in ["list_owned_cities", "list_owned_units"] {
            assert!(!names.contains(&v), "owned-enumerator {v} should be off the interactive menu");
        }
        // count stays retired even on the interactive substrate (identical enum calculator).
        assert!(!names.contains(&"count"), "count op should be retired");
        assert!(InteractiveMaxOpsEnum
            .execute(
                &b,
                &tc(
                    "count",
                    "{\"x0\":0,\"y0\":0,\"x1\":9,\"y1\":0,\"kind\":\"terrain\",\"feature\":\"Grassland\"}"
                )
            )
            .starts_with("error:"));
        // A fetch verb still works (delegates to Interactive)...
        assert!(InteractiveMaxOpsEnum
            .execute(&b, &tc("get_tile", "{\"x\":5,\"y\":0}"))
            .contains("city \"Home\""));
        // ...and both an enumerator (still present) and a maximal op work.
        let lt = InteractiveMaxOpsEnum
            .execute(&b, &tc("list_tiles", "{\"x0\":0,\"y0\":0,\"x1\":9,\"y1\":0,\"terrain\":\"Grassland\"}"));
        assert!(!lt.starts_with("error:"), "list_tiles should still dispatch: {lt}");
        assert_eq!(
            InteractiveMaxOpsEnum.execute(&b, &tc("city_defense", "{\"x\":5,\"y\":0}")),
            "3"
        );
        assert!(InteractiveMaxOpsEnum.reconstruct_via_tools(&b).is_ok());
    }

    /// 4x4 all-Grassland board. Capital "Cap" (owner Alice) sits at (1,1) → the Origin.
    /// (3,0) is Forest bearing Iron: offset 2E 1N from the Origin.
    fn mini() -> Board {
        let mut tiles = Vec::new();
        for y in 0..4 {
            let mut row = Vec::new();
            for x in 0..4 {
                let terrain = if (x, y) == (3, 0) {
                    "Forest"
                } else {
                    "Grassland"
                };
                let mut extras = BTreeSet::new();
                if (x, y) == (3, 0) {
                    extras.insert("Iron".to_string());
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
            width: 4,
            height: 4,
            tiles,
            cities: vec![City {
                x: 1,
                y: 1,
                id: 1,
                name: "Cap".to_string(),
                owner: "Alice".to_string(),
                size: 5,
                improvements: BTreeSet::new(),
            }],
            units: vec![],
            players: vec![Player {
                id: 0,
                name: "Alice".to_string(),
                nation: "Rome".to_string(),
                is_alive: true,
            }],
            known: std::collections::HashMap::new(),
            visibility: None,
            ruleset: None,
            turn: None,
            source: "mini".to_string(),
        }
    }

    #[test]
    fn origin_is_the_capital() {
        let (x, y, label) = egocentric_origin(&mini());
        assert_eq!((x, y), (1, 1));
        assert_eq!(label, "the city \"Cap\"");
    }

    #[test]
    fn board_block_lists_tiles_by_offset() {
        let block = EgocentricEncoder.render(&mini());
        // Header states the origin and its absolute coordinates.
        assert!(
            block.contains("Origin is the city \"Cap\" at absolute (1, 1)"),
            "block:\n{block}"
        );
        // A tile 2 East, 1 North of the Origin renders with the compact offset key and full facts.
        assert!(block.contains("2E 1N: Forest [Iron]"), "block:\n{block}");
        // The origin tile itself is listed (it has a city) as offset 0.
        assert!(
            block.contains("0 (Origin): Grassland {city \"Cap\""),
            "block:\n{block}"
        );
        // Plain default tiles are omitted, so the far SE plain corner never appears.
        assert!(!block.contains("3E 3S"), "block:\n{block}");
    }

    #[test]
    fn referents_render_egocentrically() {
        let b = mini();
        // A tile referent is phrased as an offset from the Origin.
        assert_eq!(
            EgocentricEncoder.render_referent(&b, &Referent::Tile { x: 3, y: 0 }),
            "the tile 2 East and 1 North of the Origin"
        );
        // The origin tile.
        assert_eq!(
            EgocentricEncoder.render_referent(&b, &Referent::Tile { x: 1, y: 1 }),
            "the Origin tile"
        );
        // City referents stay named, with an egocentric parenthetical.
        assert_eq!(
            EgocentricEncoder.render_referent(&b, &Referent::City { id: 1 }),
            "the city \"Cap\" (the Origin)"
        );
    }

    #[test]
    fn list_verbs_enumerate_visible_objects() {
        use crate::model::ToolCall;
        let b = mini();
        let call = |name: &str| {
            Interactive.execute(
                &b,
                &ToolCall {
                    id: String::new(),
                    name: name.to_string(),
                    args_json: "{}".to_string(),
                },
            )
        };
        assert!(
            call("list_cities").contains("\"Cap\" owner Alice at (1, 1) size 5"),
            "{}",
            call("list_cities")
        );
        assert_eq!(call("list_units"), "(no units visible)");
    }

    #[test]
    fn scan_grid_returns_glyph_grid_plus_exact_detail() {
        use crate::model::ToolCall;
        let b = mini();
        let grid = Interactive.execute(
            &b,
            &ToolCall {
                id: String::new(),
                name: "scan_grid".to_string(),
                args_json: "{\"x0\":0,\"y0\":0,\"x1\":3,\"y1\":3}".to_string(),
            },
        );
        // Row 0: three Grassland then Forest at (3,0).
        assert!(grid.contains("  0 gggf"), "grid:\n{grid}");
        // Row 1: the city glyph 'C' overrides terrain at (1,1).
        assert!(grid.contains("  1 gCgg"), "grid:\n{grid}");
        // DETAIL carries the exact facts a glyph can't: the resource and the city contents.
        assert!(grid.contains("(3, 0): Forest [Iron]"), "grid:\n{grid}");
        assert!(
            grid.contains("(1, 1): Grassland {city \"Cap\" size 5 owner Alice}"),
            "grid:\n{grid}"
        );
        // Out-of-bounds box is an error string, never a panic.
        let oob = Interactive.execute(
            &b,
            &ToolCall {
                id: String::new(),
                name: "scan_grid".to_string(),
                args_json: "{\"x0\":0,\"y0\":0,\"x1\":9,\"y1\":9}".to_string(),
            },
        );
        assert!(oob.starts_with("error:"), "{oob}");
    }
}
