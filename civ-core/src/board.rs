//! The neutral, format-agnostic board representation.
//!
//! This is the boundary between the parser (which knows Freeciv) and everything downstream
//! (which does not). Nothing here imports anything Freeciv-specific; a `Board` could equally
//! be produced from another 4X platform's telemetry.
//!
//! Coordinate convention: `x` east (column), `y` south (row, `y == 0` is the north edge),
//! `tiles[y][x]`.

use std::collections::{BTreeSet, HashMap};

/// A single map cell. `terrain` and `extras` use the source's own names as neutral strings
/// (e.g. "Grassland", "River", "Wheat"). `extras` is a sorted set for deterministic output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tile {
    pub x: i32,
    pub y: i32,
    pub terrain: String,
    pub extras: BTreeSet<String>,
    /// Name of the player whose territory this tile is, or `None` if unclaimed.
    pub owner: Option<String>,
}

impl Tile {
    pub fn has(&self, extra: &str) -> bool {
        self.extras.contains(extra)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct City {
    pub x: i32,
    pub y: i32,
    pub id: i32,
    pub name: String,
    pub owner: String,
    pub size: i32,
    /// City improvements/wonders present, by their source name (e.g. "City Walls", "Palace",
    /// "Great Wall"). Decoded from the save's per-city `improvements` bitstring against the
    /// global `improvement_vector`. Carried for the valuation tiers (T2/T3); unused by T0/T1.
    pub improvements: BTreeSet<String>,
}

impl City {
    /// Whether this city has a given improvement (by source name).
    pub fn has(&self, improvement: &str) -> bool {
        self.improvements.contains(improvement)
    }

    /// Whether this city has its own City Walls improvement. NOTE: this is the *per-city* fact
    /// only; the Great Wall wonder grants walls to all of an owner's cities and is a per-player
    /// effect that lives above the single-city level (see `civ-eval`'s city-defense rules).
    pub fn has_walls(&self) -> bool {
        self.has("City Walls")
    }

    /// Whether this city has Coastal Defense (+100% vs sea only; does not affect land defense).
    pub fn has_coastal_defense(&self) -> bool {
        self.has("Coastal Defense")
    }

    /// Whether this city holds the Palace (the true capital marker).
    pub fn has_palace(&self) -> bool {
        self.has("Palace")
    }
}

/// A unit. `veteran` and `hp` are carried for the valuation tiers (T2/T3); they are decoded
/// from the save but not used by T0/T1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    pub x: i32,
    pub y: i32,
    pub id: i32,
    pub kind: String, // Freeciv "type_by_name", e.g. "Armor"
    pub owner: String,
    pub veteran: i32,
    pub hp: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Player {
    pub id: i32,
    pub name: String,
    pub nation: String,
    /// Whether the player is still in the game (`[playerN] is_alive`). A dead slot is never a valid
    /// fog "self" perspective. Absent in older saves → treated as alive.
    pub is_alive: bool,
}

impl Player {
    /// Whether this player is a valid **fog perspective** ("self"): a LIVING, real civilization —
    /// not a dead slot, and not a Freeciv non-civ uprising player. Freeciv's non-civ players carry
    /// nation `"Barbarian"`, `"Pirate"`, or `"Animals"`. Computing sight radii around a barbarian/
    /// dead slot that owns nothing produces a meaningless "self" view, so the CLI rejects it.
    pub fn is_valid_fog_perspective(&self) -> bool {
        self.is_alive && !matches!(self.nation.as_str(), "Barbarian" | "Pirate" | "Animals")
    }
}

/// Per-tile fog-of-war state from a single player's perspective (see [`Board::mask_to_known`]).
///
/// Freeciv models three visibility states; we reproduce them faithfully:
///   - `Unexplored` — the tile was never in the player's `known` map (`'u'` in the save's `map_t`).
///     Rendered as `"Unknown"` terrain with no occupants.
///   - `Fogged` — explored (`known`) but NOT currently within sight of any of the player's own
///     units/cities. Terrain/extras/territory owner are *remembered* (kept); live enemy units are
///     *hidden* (dropped); an enemy city is kept at its last-known state.
///   - `Visible` — currently within sight: full live truth.
///
/// `Visible ⊆ Known` by construction (a tile is Visible only if it is also `known`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Unexplored,
    Fogged,
    Visible,
}

// --- Freeciv `classic` vision model (see `Board::visible_grid`) -------------------------------
//
// Freeciv expresses vision as a per-source `vision_radius_sq` (a SQUARED radius) and tests
// visibility with its map-distance-squared metric `sq_map_distance`. For the non-hex square
// topology that metric is `dx*dx + dy*dy` (see `sq_map_distance` below). A tile is visible from a
// source iff `sq_map_distance(source, tile) <= vision_radius_sq`. Our geometry is non-wrapping, so
// we apply the same squared-Euclidean test over plain (unwrapped) coordinates.
//
// Values are the REAL `classic` ruleset numbers, verified against the locally-installed Freeciv
// 3.2.5 ruleset at `data/classic/` (the save's `revision` is `3.2.90.2-dev`; classic vision is
// stable across 3.2.x):
//   - units:  units.ruleset — `vision_radius_sq` per `[unit_*]` (extracted all 51 unit types).
//   - cities: effects.ruleset `[effect_city_vision]` City_Vision_Radius_Sq = 5, matching
//             game.ruleset `[civstyle] init_vis_radius_sq = 5`.
// They are named constants / a small table so they are easy to correct if the ruleset changes.

/// Base city `vision_radius_sq` for a city with no special buildings. classic = 5 (radius
/// sqrt(5) ≈ 2.24). Source: freeciv S3_2 `data/classic/effects.ruleset` `[effect_city_vision]`
/// (`City_Vision_Radius_Sq = 5`) and `data/classic/game.ruleset` `[civstyle] init_vis_radius_sq = 5`.
pub const CITY_VISION_RADIUS_SQ: i32 = 5;

/// Default unit `vision_radius_sq`. In classic EVERY land unit (Settlers/Workers/Engineers and all
/// land military — Warriors … Armor, plus siege) has `vision_radius_sq = 2` (radius sqrt(2) ≈ 1.41,
/// i.e. the surrounding 3×3 block). Source: freeciv S3_2 `data/classic/units.ruleset`.
pub const UNIT_VISION_RADIUS_SQ_DEFAULT: i32 = 2;

/// The `vision_radius_sq` of a unit given its Freeciv `type_by_name`. Only types whose value differs
/// from [`UNIT_VISION_RADIUS_SQ_DEFAULT`] (2) are listed; everything else — including all land units
/// (and, note, the Explorer, which is `2` in classic, NOT wide-vision) — falls through to the
/// default. Source: `data/classic/units.ruleset` (`vision_radius_sq` per `[unit_*]`): the aircraft
/// (Fighter/Bomber/Helicopter/Stealth*), fast/ocean-going warships (Destroyer/Cruiser/AEGIS
/// Cruiser/Battleship/Submarine/Carrier/Transport), Spy and Leader see `8`; only the AWACS sees `26`.
///
/// NOTE (documented approximation): classic *also* has two conditional `Unit_Vision_Radius_Sq`
/// effects (`data/classic/effects.ruleset`): +8 for a unit on a **Fortress** extra (gated on the
/// **Invention** tech) and 4 for a land unit standing on **Mountains**. We do NOT model either:
/// the Fortress bonus is gated on a tech our `Board` does not decode (applying it unconditionally
/// would over-see), and modelling only the ungated Mountains case would be an inconsistent
/// half-measure for a marginal, on-mountain-only expansion. Base per-source vision is modelled
/// faithfully; these terrain/extra bonuses are a deliberate, noted omission.
pub fn unit_vision_radius_sq(kind: &str) -> i32 {
    match kind {
        "Fighter" | "Bomber" | "Helicopter" | "Stealth Fighter" | "Stealth Bomber"
        | "Destroyer" | "Cruiser" | "AEGIS Cruiser" | "Battleship" | "Submarine" | "Carrier"
        | "Transport" | "Spy" | "Leader" => 8,
        "AWACS" => 26,
        _ => UNIT_VISION_RADIUS_SQ_DEFAULT,
    }
}

/// Freeciv's `sq_map_distance` for the non-hex square topology: the squared map distance
/// `dx*dx + dy*dy` between two tiles. Our geometry is non-wrapping, so no edge wrap is applied.
/// This is the metric visibility is tested against (`<= vision_radius_sq`), matching Freeciv.
pub fn sq_map_distance(a: (i32, i32), b: (i32, i32)) -> i32 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    dx * dx + dy * dy
}

/// floor(sqrt(n)) for n >= 0, by integer bisection — the bounding-box half-width for a
/// `vision_radius_sq` disk (no floating point, so no off-by-one at perfect squares).
fn isqrt(n: i32) -> i32 {
    if n <= 0 {
        return 0;
    }
    let mut r = 0;
    while (r + 1) * (r + 1) <= n {
        r += 1;
    }
    r
}

/// Facts keyed by coordinate, plus object lists for cities/units/players.
/// `tiles` is a dense `height` x `width` grid (row-major: `tiles[y][x]`).
#[derive(Debug, Clone)]
pub struct Board {
    pub width: i32,
    pub height: i32,
    pub tiles: Vec<Vec<Tile>>,
    pub cities: Vec<City>,
    pub units: Vec<Unit>,
    pub players: Vec<Player>,
    /// Per-player fog-of-war: `known[player_id][y][x]` is true iff that player has explored (ever
    /// seen) the tile. Decoded from each player's `map_t` grid in the save (`'u'` = unexplored).
    /// Empty when a save carries no per-player maps, or on a board that is already a masked view.
    /// Used by [`Board::mask_to_known`] to build a realistic per-player view; unused by T0/T1
    /// unless a `--fog` run masks the board first.
    pub known: HashMap<i32, Vec<Vec<bool>>>,
    /// Per-tile [`Visibility`] for a masked, single-perspective board (produced by
    /// [`Board::mask_to_known`]). `None` on an omniscient (non-fog) board — treated as all-`Visible`,
    /// so every existing board and code path is unchanged. Row-major, same dims as `tiles`.
    pub visibility: Option<Vec<Vec<Visibility>>>,
    /// The save's ruleset directory (`[savefile] rulesetdir`, e.g. `"classic"`), or `None` if the
    /// save carried none. Used by the CLI to gate runs to the ruleset our solver constants target.
    pub ruleset: Option<String>,
    /// The game turn (`[game] turn`), or `None` if absent. Carried for provenance/corpus metadata.
    pub turn: Option<i32>,
    /// Provenance (e.g. the save filename) for traceability in results.
    pub source: String,
}

impl Board {
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && x < self.width && y >= 0 && y < self.height
    }

    /// Panics if out of bounds — callers on the geometry paths always check first.
    pub fn tile(&self, x: i32, y: i32) -> &Tile {
        assert!(
            self.in_bounds(x, y),
            "tile ({x},{y}) out of bounds for {}x{}",
            self.width,
            self.height
        );
        &self.tiles[y as usize][x as usize]
    }

    pub fn iter_tiles(&self) -> impl Iterator<Item = &Tile> {
        self.tiles.iter().flat_map(|row| row.iter())
    }

    pub fn player(&self, name: &str) -> Option<&Player> {
        self.players.iter().find(|p| p.name == name)
    }

    pub fn cities_of(&self, owner: &str) -> Vec<&City> {
        self.cities.iter().filter(|c| c.owner == owner).collect()
    }

    pub fn units_of(&self, owner: &str) -> Vec<&Unit> {
        self.units.iter().filter(|u| u.owner == owner).collect()
    }

    /// A rectangular sub-board with coordinates re-based to `(0, 0)`. Keeps only the tiles,
    /// cities, and units inside `[x0, x0+w) x [y0, y0+h)`, remapping their coordinates. Used to
    /// shrink a board so denser encodings fit a smaller context window. Players are retained.
    pub fn crop(&self, x0: i32, y0: i32, w: i32, h: i32) -> Result<Board, String> {
        if w <= 0 || h <= 0 {
            return Err(format!("crop size must be positive (got {w}x{h})"));
        }
        if x0 < 0 || y0 < 0 || x0 + w > self.width || y0 + h > self.height {
            return Err(format!(
                "crop ({x0},{y0},{w},{h}) out of bounds for {}x{} board",
                self.width, self.height
            ));
        }
        let mut tiles = Vec::with_capacity(h as usize);
        for ny in 0..h {
            let mut row = Vec::with_capacity(w as usize);
            for nx in 0..w {
                let src = &self.tiles[(y0 + ny) as usize][(x0 + nx) as usize];
                row.push(Tile {
                    x: nx,
                    y: ny,
                    terrain: src.terrain.clone(),
                    extras: src.extras.clone(),
                    owner: src.owner.clone(),
                });
            }
            tiles.push(row);
        }
        let in_rect = |x: i32, y: i32| x >= x0 && x < x0 + w && y >= y0 && y < y0 + h;
        let cities = self
            .cities
            .iter()
            .filter(|c| in_rect(c.x, c.y))
            .map(|c| City {
                x: c.x - x0,
                y: c.y - y0,
                ..c.clone()
            })
            .collect();
        let units = self
            .units
            .iter()
            .filter(|u| in_rect(u.x, u.y))
            .map(|u| Unit {
                x: u.x - x0,
                y: u.y - y0,
                ..u.clone()
            })
            .collect();
        Ok(Board {
            width: w,
            height: h,
            tiles,
            cities,
            units,
            players: self.players.clone(),
            known: HashMap::new(),
            visibility: None,
            ruleset: self.ruleset.clone(),
            turn: self.turn,
            source: format!("{}#crop({x0},{y0},{w},{h})", self.source),
        })
    }

    /// The set of tiles CURRENTLY VISIBLE to `player_id` under Freeciv `classic` vision: any tile
    /// whose squared map-distance ([`sq_map_distance`]) to one of the player's OWN units or cities
    /// is within that source's `vision_radius_sq` ([`CITY_VISION_RADIUS_SQ`] for cities,
    /// [`unit_vision_radius_sq`] for units). Own sources only — no shared/allied vision. Returns a
    /// row-major `height × width` bool grid (`true` = visible). Errors if `player_id` has no player.
    ///
    /// This is raw line-of-sight (bounded by the vision radii); [`Board::mask_to_known`] additionally
    /// intersects it with the save's `known` bit, so we never "see" a tile the save says was never
    /// explored.
    pub fn visible_grid(&self, player_id: i32) -> Result<Vec<Vec<bool>>, String> {
        let name = self
            .players
            .iter()
            .find(|p| p.id == player_id)
            .map(|p| p.name.clone())
            .ok_or_else(|| format!("no player with id {player_id}"))?;
        let mut vis = vec![vec![false; self.width as usize]; self.height as usize];
        let mut mark = |cx: i32, cy: i32, rsq: i32| {
            let r = isqrt(rsq);
            for y in (cy - r)..=(cy + r) {
                for x in (cx - r)..=(cx + r) {
                    if self.in_bounds(x, y) && sq_map_distance((cx, cy), (x, y)) <= rsq {
                        vis[y as usize][x as usize] = true;
                    }
                }
            }
        };
        for c in &self.cities {
            if c.owner == name {
                mark(c.x, c.y, CITY_VISION_RADIUS_SQ);
            }
        }
        for u in &self.units {
            if u.owner == name {
                mark(u.x, u.y, unit_vision_radius_sq(&u.kind));
            }
        }
        Ok(vis)
    }

    /// The view of this board as seen by `player_id` — real Freeciv **three-state fog of war**
    /// ([`Visibility`]). Per tile:
    ///   - **Unexplored** (never in the player's `known` map): `"Unknown"` terrain, no extras/owner,
    ///     all occupants hidden.
    ///   - **Fogged** (`known` but not currently in sight, see [`Board::visible_grid`]): terrain,
    ///     extras and territory owner are *remembered* (kept); live **enemy units are hidden**
    ///     (dropped); an enemy **city is kept** at its last-known state.
    ///   - **Visible** (`known` and in sight): full live truth.
    ///
    /// Own units/cities always survive (they sit on their own tile, which is in sight). The result
    /// carries a per-tile [`visibility`](Board::visibility) grid so encoders can *tell* fogged from
    /// visible-empty. Because the masked `Board` is the single value fed to BOTH the solvers and the
    /// encoders, dropping hidden enemies here makes every downstream computation fog-consistent with
    /// no per-solver plumbing (the "mask upstream, nothing reaches around it" seam).
    ///
    /// Requires per-player [`known`](Board::known) data; errors if the save had none for the player,
    /// or if `player_id` has no player entry.
    pub fn mask_to_known(&self, player_id: i32) -> Result<Board, String> {
        let grid = self
            .known
            .get(&player_id)
            .ok_or_else(|| format!("no known-map (fog) data for player {player_id}"))?;
        let name = self
            .players
            .iter()
            .find(|p| p.id == player_id)
            .map(|p| p.name.clone())
            .ok_or_else(|| format!("no player with id {player_id}"))?;
        let is_known = |x: i32, y: i32| -> bool {
            grid.get(y as usize)
                .and_then(|r| r.get(x as usize))
                .copied()
                .unwrap_or(false)
        };
        // Raw sight, then intersected with `known` (a Visible tile must also be explored).
        let sight = self.visible_grid(player_id)?;
        let is_visible = |x: i32, y: i32| -> bool {
            is_known(x, y)
                && sight
                    .get(y as usize)
                    .and_then(|r| r.get(x as usize))
                    .copied()
                    .unwrap_or(false)
        };
        let state = |x: i32, y: i32| -> Visibility {
            if !is_known(x, y) {
                Visibility::Unexplored
            } else if is_visible(x, y) {
                Visibility::Visible
            } else {
                Visibility::Fogged
            }
        };

        let mut tiles = Vec::with_capacity(self.height as usize);
        let mut visibility = Vec::with_capacity(self.height as usize);
        for y in 0..self.height {
            let mut row = Vec::with_capacity(self.width as usize);
            let mut vrow = Vec::with_capacity(self.width as usize);
            for x in 0..self.width {
                // Terrain/extras/owner are retained on any KNOWN tile (fogged OR visible); only
                // never-explored tiles collapse to Unknown.
                if is_known(x, y) {
                    row.push(self.tiles[y as usize][x as usize].clone());
                } else {
                    row.push(Tile {
                        x,
                        y,
                        terrain: "Unknown".to_string(),
                        extras: BTreeSet::new(),
                        owner: None,
                    });
                }
                vrow.push(state(x, y));
            }
            tiles.push(row);
            visibility.push(vrow);
        }
        // Cities: kept on any known tile (an enemy city on a fogged tile stays "last-known"; its
        // garrison — the units — is still hidden by the unit rule below, so a fogged enemy city
        // reads as undefended-as-far-as-you-can-see, which is fog-correct).
        let cities = self
            .cities
            .iter()
            .filter(|c| is_known(c.x, c.y))
            .cloned()
            .collect();
        // Units: own units survive on any known tile (always in sight by construction, but kept
        // explicitly for robustness); enemy units survive ONLY on currently-visible tiles.
        let units = self
            .units
            .iter()
            .filter(|u| {
                if u.owner == name {
                    is_known(u.x, u.y)
                } else {
                    is_visible(u.x, u.y)
                }
            })
            .cloned()
            .collect();
        Ok(Board {
            width: self.width,
            height: self.height,
            tiles,
            cities,
            units,
            players: self.players.clone(),
            known: HashMap::new(),
            visibility: Some(visibility),
            ruleset: self.ruleset.clone(),
            turn: self.turn,
            source: format!("{}#fog(p{player_id})", self.source),
        })
    }

    /// The [`Visibility`] of `(x, y)` on a masked board; `Visible` everywhere on an omniscient
    /// (`visibility == None`) board, and for any out-of-bounds coordinate.
    pub fn visibility_at(&self, x: i32, y: i32) -> Visibility {
        match &self.visibility {
            None => Visibility::Visible,
            Some(g) => g
                .get(y as usize)
                .and_then(|r| r.get(x as usize))
                .copied()
                .unwrap_or(Visibility::Visible),
        }
    }

    /// Whether `(x, y)` is currently **fogged** (explored but not in sight) on a masked board.
    /// Always `false` on an omniscient board — the honesty marker encoders use.
    pub fn is_fogged(&self, x: i32, y: i32) -> bool {
        matches!(self.visibility_at(x, y), Visibility::Fogged)
    }

    /// Build a `Board` from the offline replay viewer's per-board export JSON (the shape emitted by
    /// `civ-cli`'s `viewer_export` and documented in `viewer/README.md`). This is the seam that lets
    /// the browser tool-playground run the REAL Rust tool dispatch (`civ_eval::run_tool`) against
    /// whatever board it is currently displaying, so the viewer never re-implements a tool in JS.
    ///
    /// The export board is ALREADY the exact view the tools should see (the viewer serializes the
    /// current fogged-or-unfogged view), so this treats it as fully-visible ground truth of that view:
    /// [`visibility`](Board::visibility) is `None` and [`known`](Board::known) is empty. `ruleset`/
    /// `turn` are not carried by the export and are `None`. Every field is `pub`, so this hand-builds
    /// the struct directly.
    ///
    /// Expected object fields (unknown fields are ignored, missing optional fields default):
    ///   - `width`, `height`: integers (required).
    ///   - `tiles`: array of `{x,y,terrain,extras:[string],owner:string|null}` (row-major; every
    ///     in-bounds tile). Placed into the grid by its own `(x,y)`.
    ///   - `cities`: array of `{x,y,id,name,owner,size,improvements:[string]?}`.
    ///   - `units`: array of `{x,y,id,kind,owner,veteran,hp}`.
    ///   - `players` (optional): array of `{name,nation}`; ids are assigned by array index and every
    ///     entry is treated as alive (the export carries no `is_alive`).
    ///   - `source` (optional): provenance string; defaults to `"viewer"`.
    pub fn from_viewer_json(json: &str) -> Result<Board, String> {
        use crate::json::Json;

        let v = Json::parse(json).map_err(|e| format!("bad board json: {e}"))?;
        let req_i32 = |key: &str| -> Result<i32, String> {
            v.get(key)
                .and_then(Json::as_i64)
                .map(|n| n as i32)
                .ok_or_else(|| format!("board json missing integer {key:?}"))
        };
        let width = req_i32("width")?;
        let height = req_i32("height")?;
        if width <= 0 || height <= 0 {
            return Err(format!(
                "board json has non-positive dimensions {width}x{height}"
            ));
        }

        // String set (extras / improvements): an array of strings -> a sorted set.
        let str_set = |v: Option<&Json>| -> BTreeSet<String> {
            v.and_then(Json::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|e| e.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };
        // A `string | null` field -> Option<String>.
        let opt_str = |o: Option<&Json>| -> Option<String> {
            match o {
                Some(j) if !j.is_null() => j.as_str().map(str::to_string),
                _ => None,
            }
        };
        let obj_i32 = |o: &Json, key: &str| -> i32 {
            o.get(key)
                .and_then(Json::as_i64)
                .map(|n| n as i32)
                .unwrap_or(0)
        };
        let obj_str = |o: &Json, key: &str| -> String {
            o.get(key).and_then(Json::as_str).unwrap_or("").to_string()
        };

        // Tiles: allocate the full grid up front (defaulting to "Unknown", as a fog export does for
        // never-explored tiles) and place each listed tile by its own (x, y), so we are robust to
        // ordering or an incomplete list.
        let mut tiles: Vec<Vec<Tile>> = (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| Tile {
                        x,
                        y,
                        terrain: "Unknown".to_string(),
                        extras: BTreeSet::new(),
                        owner: None,
                    })
                    .collect()
            })
            .collect();
        if let Some(arr) = v.get("tiles").and_then(Json::as_array) {
            for t in arr {
                let x = obj_i32(t, "x");
                let y = obj_i32(t, "y");
                if x < 0 || x >= width || y < 0 || y >= height {
                    continue; // out-of-bounds tile entry — skip defensively
                }
                tiles[y as usize][x as usize] = Tile {
                    x,
                    y,
                    terrain: obj_str(t, "terrain"),
                    extras: str_set(t.get("extras")),
                    owner: opt_str(t.get("owner")),
                };
            }
        }

        let cities: Vec<City> = v
            .get("cities")
            .and_then(Json::as_array)
            .map(|arr| {
                arr.iter()
                    .map(|c| City {
                        x: obj_i32(c, "x"),
                        y: obj_i32(c, "y"),
                        id: obj_i32(c, "id"),
                        name: obj_str(c, "name"),
                        owner: obj_str(c, "owner"),
                        size: obj_i32(c, "size"),
                        improvements: str_set(c.get("improvements")),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let units: Vec<Unit> = v
            .get("units")
            .and_then(Json::as_array)
            .map(|arr| {
                arr.iter()
                    .map(|u| Unit {
                        x: obj_i32(u, "x"),
                        y: obj_i32(u, "y"),
                        id: obj_i32(u, "id"),
                        kind: obj_str(u, "kind"),
                        owner: obj_str(u, "owner"),
                        veteran: obj_i32(u, "veteran"),
                        hp: obj_i32(u, "hp"),
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Players: optional roster of {name, nation}. The export has no player id or is_alive, so ids
        // are the array index and everyone is treated as alive. Tools key on owner NAME strings, so
        // this roster is only for completeness (e.g. `player`/`capital_of` name lookups).
        let players: Vec<Player> = v
            .get("players")
            .and_then(Json::as_array)
            .map(|arr| {
                arr.iter()
                    .enumerate()
                    .map(|(i, p)| Player {
                        id: i as i32,
                        name: obj_str(p, "name"),
                        nation: obj_str(p, "nation"),
                        is_alive: true,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let source = v
            .get("source")
            .and_then(Json::as_str)
            .unwrap_or("viewer")
            .to_string();

        Ok(Board {
            width,
            height,
            tiles,
            cities,
            units,
            players,
            known: HashMap::new(),
            visibility: None,
            ruleset: None,
            turn: None,
            source,
        })
    }

    /// The owner's capital: the city holding the Palace improvement (the true capital marker) if
    /// one is decoded, else a deterministic stand-in — the owner's largest city (ties broken by
    /// lowest id). Used as the default egocentric origin.
    pub fn capital_of(&self, owner: &str) -> Option<&City> {
        let cities = self.cities_of(owner);
        cities
            .iter()
            .filter(|c| c.has_palace())
            .max_by_key(|c| (c.size, -c.id))
            .or_else(|| cities.iter().max_by_key(|c| (c.size, -c.id)))
            .copied()
    }
}
