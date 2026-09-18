//! `civ-core` — the neutral 4X board model, centralized grid geometry, and the Freeciv
//! `.sav` parser. No dependencies; nothing downstream needs to know about Freeciv.

pub mod board;
pub mod geometry;
pub mod json;
pub mod parser;
pub mod rng;

pub use board::{
    sq_map_distance, unit_vision_radius_sq, Board, City, Player, Tile, Unit, Visibility,
    CITY_VISION_RADIUS_SQ, UNIT_VISION_RADIUS_SQ_DEFAULT,
};
pub use geometry::{Dir8, ALL_DIRS};
pub use parser::{parse_save, parse_str, ParseError};
pub use rng::Rng;
