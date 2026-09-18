//! Centralized grid semantics — the single source of truth for "what does spatial mean".
//!
//! Both the encoders (which *describe* the board) and the question solvers (which compute
//! *ground truth*) use these functions, so the two can never disagree about what "NE of
//! (x,y)" or "distance" means. That consistency is what makes deterministic exact-match
//! scoring valid.
//!
//! Stated semantics (deliberately simpler than Freeciv's native topology — the save is only
//! a source of realistic terrain, not of movement rules):
//!   - 8-connected Moore neighborhood (the 8 compass directions).
//!   - North is toward `y == 0` (the top row). N decreases y, S increases y, E increases x.
//!   - Distance is Chebyshev: `max(|dx|, |dy|)` — 8-direction king steps.
//!   - The board is a bounded, NON-wrapping rectangle.

use crate::board::Board;

/// A compass direction on the 8-neighbor grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dir8 {
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    NW,
}

/// All eight directions, clockwise from north. Stable, human-facing order.
pub const ALL_DIRS: [Dir8; 8] = [
    Dir8::N,
    Dir8::NE,
    Dir8::E,
    Dir8::SE,
    Dir8::S,
    Dir8::SW,
    Dir8::W,
    Dir8::NW,
];

impl Dir8 {
    /// `(dx, dy)` unit step. North is `-y` (toward the top row).
    pub fn delta(self) -> (i32, i32) {
        match self {
            Dir8::N => (0, -1),
            Dir8::NE => (1, -1),
            Dir8::E => (1, 0),
            Dir8::SE => (1, 1),
            Dir8::S => (0, 1),
            Dir8::SW => (-1, 1),
            Dir8::W => (-1, 0),
            Dir8::NW => (-1, -1),
        }
    }

    /// Short token, e.g. `"NE"`. This is the canonical form used in answers/prompts.
    pub fn short(self) -> &'static str {
        match self {
            Dir8::N => "N",
            Dir8::NE => "NE",
            Dir8::E => "E",
            Dir8::SE => "SE",
            Dir8::S => "S",
            Dir8::SW => "SW",
            Dir8::W => "W",
            Dir8::NW => "NW",
        }
    }

    /// Full word, e.g. `"northeast"`.
    pub fn long(self) -> &'static str {
        match self {
            Dir8::N => "north",
            Dir8::NE => "northeast",
            Dir8::E => "east",
            Dir8::SE => "southeast",
            Dir8::S => "south",
            Dir8::SW => "southwest",
            Dir8::W => "west",
            Dir8::NW => "northwest",
        }
    }

    /// Parse a compass token (case-insensitive; accepts short or long form).
    pub fn parse(s: &str) -> Option<Dir8> {
        let t = s.trim().to_ascii_lowercase();
        ALL_DIRS
            .into_iter()
            .find(|d| t == d.short().to_ascii_lowercase() || t == d.long())
    }
}

/// The coordinate one tile in `dir` from `(x, y)` (no bounds check).
pub fn step(x: i32, y: i32, dir: Dir8) -> (i32, i32) {
    let (dx, dy) = dir.delta();
    (x + dx, y + dy)
}

/// King-move distance: the number of 8-direction steps from `a` to `b`.
pub fn chebyshev(a: (i32, i32), b: (i32, i32)) -> i32 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

/// The single compass direction from `origin` toward `target`, or `None` if identical.
///
/// Chosen by the sign of `(dx, dy)`: cardinal when one axis is zero, diagonal otherwise.
/// A coarse bearing — it does not require the target to lie exactly on a diagonal ray.
pub fn direction_between(origin: (i32, i32), target: (i32, i32)) -> Option<Dir8> {
    let dx = (target.0 - origin.0).signum();
    let dy = (target.1 - origin.1).signum();
    if dx == 0 && dy == 0 {
        return None;
    }
    ALL_DIRS.into_iter().find(|d| d.delta() == (dx, dy))
}

/// In-bounds 8-neighbors of `(x, y)`, as `(direction, (nx, ny))`, in `ALL_DIRS` order.
pub fn neighbors(board: &Board, x: i32, y: i32) -> Vec<(Dir8, (i32, i32))> {
    let mut out = Vec::with_capacity(8);
    for d in ALL_DIRS {
        let (nx, ny) = step(x, y, d);
        if board.in_bounds(nx, ny) {
            out.push((d, (nx, ny)));
        }
    }
    out
}

/// Every in-bounds coordinate whose Chebyshev distance from `center` is `<= radius`
/// (including the center). Deterministic row-major order over the bounding square.
pub fn tiles_within(board: &Board, center: (i32, i32), radius: i32) -> Vec<(i32, i32)> {
    let (cx, cy) = center;
    let mut out = Vec::new();
    for y in (cy - radius)..=(cy + radius) {
        for x in (cx - radius)..=(cx + radius) {
            if board.in_bounds(x, y) && chebyshev(center, (x, y)) <= radius {
                out.push((x, y));
            }
        }
    }
    out
}
