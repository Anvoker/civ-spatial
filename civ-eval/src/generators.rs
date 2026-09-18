//! Question generation with free, computed ground truth.
//!
//! Each `QuestionKind` samples parameters deterministically (seeded RNG), computes the answer
//! via the pure `solve` function (so the phrasing and the answer can't drift), and applies
//! admissibility filters. Only *unambiguous* instances are kept — e.g. direction questions
//! target only tiles that lie exactly on a compass ray, so there is one correct answer.

use std::cell::Cell;
use std::collections::{BTreeSet, HashMap, HashSet};

use civ_core::geometry::{self, Dir8, ALL_DIRS};
use civ_core::{Board, Rng};

use crate::encoders::{city_choice_label, most_common_terrain, Referent};
use crate::question::{solve, Answer, EvalItem, Question};
use crate::rules::{self, StrengthAxis};

/// Resource-bearing extras worth asking "nearest" about (excludes infrastructure like Road).
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

/// Whether an extra is a natural resource deposit (vs. infrastructure like Road/Fortress). The
/// single source of truth for "what counts as a resource," shared with the descriptive summaries.
pub(crate) fn is_resource(extra: &str) -> bool {
    RESOURCE_EXTRAS.contains(&extra)
}

/// The generation context threaded to every [`QuestionKind::generate`] — the one seam that carries
/// both the board the MODEL sees and (for hidden-information kinds) the true board the GENERATOR
/// scores on.
///
/// - `board` is the **rendered** board — masked to the fog perspective under `--fog` (what the model
///   perceives), or the real board when unfogged. Every kind renders its referents from this board.
/// - `unmasked` is the **true** board, `Some` only on a fogged run. The hidden-force family (P1/P3/
///   P7f — `reasoning-frontier-questions-v2.md` §v2.3) computes ground truth on it while the model
///   sees only `board`; every other kind ignores it (their answer is a function of the masked board).
/// - `perspective` is the fog player's owner NAME under `--fog`, else `None` — player-relative kinds
///   pin their "you" to it (`fog-three-state-design.md` §9).
pub struct GenCtx<'a> {
    pub board: &'a Board,
    pub unmasked: Option<&'a Board>,
    pub rng: Rng,
    pub n: usize,
    pub perspective: Option<&'a str>,
}

pub trait QuestionKind {
    fn name(&self) -> &'static str;
    /// Generate up to `ctx.n` items for this kind from [`GenCtx`].
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem>;
}

/// Restrict a player-relative kind's candidate owner list to the fog perspective player, if one is
/// set. A coherent fogged run requires that every player-relative item's "you" be the SAME player
/// the board is fogged to (`fog-three-state-design.md` §9): sight is computed around exactly the
/// player whose decision is asked. When `perspective` is `Some(name)`, only that owner survives;
/// when `None` (an unfogged run), the list passes through unchanged (the pre-fog random-owner
/// behavior). If the perspective owns nothing eligible for a kind, the result is empty and that kind
/// yields 0 items — the correct edge behavior (do NOT fall back to another player).
fn restrict_to_perspective(mut owners: Vec<String>, perspective: Option<&str>) -> Vec<String> {
    if let Some(p) = perspective {
        owners.retain(|o| o == p);
    }
    owners
}

/// Difficulty knobs for the parameterised kinds. These are the *reasoning burden* lever
/// (how much computation a question demands over the board), independent of the *perception
/// burden* set by board/crop size. Each field is a `(min, span)` pair sampled as
/// `min + rng.below(span)`, so the value lands in `min ..= min + span - 1`. Only three kinds
/// read these (region-count radius, reachability budget, direction ray length); the rest are
/// difficulty-invariant. Bigger values push off the small-board ceiling (see RESUME.md).
#[derive(Clone, Copy, Debug)]
pub struct Difficulty {
    pub count_radius_min: i32,
    pub count_radius_span: usize,
    pub reach_budget_min: i32,
    pub reach_budget_span: usize,
    pub dir_ray_min: i32,
    pub dir_ray_span: usize,
}

impl Difficulty {
    /// The original tier: small radii/paths — fine on a big board, ceilings on a small crop.
    pub fn easy() -> Self {
        Difficulty {
            count_radius_min: 2,
            count_radius_span: 2, // radius 2..=3
            reach_budget_min: 2,
            reach_budget_span: 4, // budget 2..=5
            dir_ray_min: 1,
            dir_ray_span: 6, // ray 1..=6
        }
    }

    /// The harder tier: larger radii/paths/rays that raise the reasoning burden for real signal.
    pub fn hard() -> Self {
        Difficulty {
            count_radius_min: 3,
            count_radius_span: 3, // radius 3..=5
            reach_budget_min: 3,
            reach_budget_span: 5, // budget 3..=7
            dir_ray_min: 2,
            dir_ray_span: 8, // ray 2..=9
        }
    }

    /// Parse a `--difficulty` value; returns `None` for an unknown name.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "easy" => Some(Self::easy()),
            "hard" => Some(Self::hard()),
            _ => None,
        }
    }
}

/// All question kinds, in a stable order. Difficulty parameterises the three tunable T1 kinds.
/// T2 kinds are appended last so adding them leaves the existing kinds' RNG sub-streams
/// (index-keyed in [`generate_questions`]) unperturbed.
pub fn all_kinds(difficulty: Difficulty) -> Vec<Box<dyn QuestionKind>> {
    vec![
        Box::new(TerrainAtKind),
        Box::new(AdjacentTerrainKind),
        Box::new(DirectionToKind { d: difficulty }),
        Box::new(DistanceKind),
        Box::new(NearestResourceKind),
        Box::new(CountTerrainKind { d: difficulty }),
        Box::new(ReachableKind { d: difficulty }),
        Box::new(UnitStrengthKind),
        Box::new(CityDefenseKind),
        Box::new(BestSiteKind { d: difficulty }),
        Box::new(NearestOwnedResourceKind),
        Box::new(SettleSiteKind),
        Box::new(ReachableNearestKind),
        Box::new(RetreatKind),
        Box::new(CityThreatKind),
        Box::new(CfVacateKind),
        Box::new(AssaultTargetKind),
        Box::new(TriageReinforceKind),
        Box::new(CompareTwoAttacksKind),
        Box::new(ConstraintSiteKind { d: difficulty }),
        Box::new(ForwardPostingKind),
        Box::new(FoggedAssaultKind),
        Box::new(SurpriseStrikeKind),
        Box::new(HiddenForceKind),
    ]
}

/// Generate `per_kind` questions for every kind. Each kind gets an independent sub-stream so
/// adding/removing a kind doesn't perturb the others' questions.
pub fn generate_questions(
    board: &Board,
    seed: u64,
    per_kind: usize,
    difficulty: Difficulty,
    perspective: Option<&str>,
) -> Vec<EvalItem> {
    generate_questions_fogged(board, None, seed, per_kind, difficulty, perspective)
}

/// Full generation entry that also carries the UNMASKED board for the hidden-force family
/// (`reasoning-frontier-questions-v2.md` §v2.3). `board` is what the model sees (masked under fog);
/// `unmasked` is the true board (`Some` only on a fogged run) that those kinds score on. All existing
/// kinds ignore `unmasked` and are byte-for-byte unchanged. The plain [`generate_questions`] forwards
/// here with `unmasked = None`, so the non-fog and unfogged paths are unaffected.
pub fn generate_questions_fogged(
    board: &Board,
    unmasked: Option<&Board>,
    seed: u64,
    per_kind: usize,
    difficulty: Difficulty,
    perspective: Option<&str>,
) -> Vec<EvalItem> {
    if let Some(p) = perspective {
        // A fogged run: every player-relative kind's "you" is pinned to the fog perspective player,
        // so its decision matches whose sight the board is masked to (fog-three-state-design §9). A
        // low yield for these kinds is a real signal that P sees little, not a bug — the per-kind
        // "kept N" lines below make it visible.
        eprintln!(
            "[fog-coherence] fog perspective set: restricting player-relative kinds to player {p:?}\
             {}",
            if unmasked.is_some() {
                " (unmasked board available for hidden-force kinds)"
            } else {
                ""
            }
        );
    }
    let mut out = Vec::new();
    for (i, kind) in all_kinds(difficulty).into_iter().enumerate() {
        let rng = Rng::new(seed ^ (0xA511_E9B3_u64.wrapping_mul(i as u64 + 1)));
        let mut ctx = GenCtx {
            board,
            unmasked,
            rng,
            n: per_kind,
            perspective,
        };
        out.extend(kind.generate(&mut ctx));
    }
    out
}

// --- sampling helpers -----------------------------------------------------
/// Sample a uniformly-random tile, **skipping `Unknown` (unexplored) terrain** so that questions on
/// a fogged board (`--fog`) land on the player's known area. On a board with no `Unknown` tiles the
/// first draw is always accepted, so the RNG stream — and thus generation determinism — is unchanged.
fn rand_tile(board: &Board, rng: &mut Rng) -> (i32, i32) {
    for _ in 0..64 {
        let (x, y) = (
            rng.below(board.width as usize) as i32,
            rng.below(board.height as usize) as i32,
        );
        if board.tile(x, y).terrain != "Unknown" {
            return (x, y);
        }
    }
    (
        rng.below(board.width as usize) as i32,
        rng.below(board.height as usize) as i32,
    )
}

/// Collect up to `n` admissible, de-duplicated items from a candidate closure.
fn collect<F>(board: &Board, rng: &mut Rng, n: usize, mut f: F) -> Vec<EvalItem>
where
    F: FnMut(&Board, &mut Rng) -> Option<EvalItem>,
{
    let mut out = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let budget = n.saturating_mul(200) + 500;
    for _ in 0..budget {
        if out.len() >= n {
            break;
        }
        if let Some(item) = f(board, rng) {
            if seen.insert(item.id.clone()) {
                out.push(item);
            }
        }
    }
    out
}

fn item(board: &Board, q: Question) -> Option<EvalItem> {
    let ans = solve(&q, board)?;
    Some(EvalItem::new(q, ans, board.source.clone()))
}

fn present_terrains(board: &Board) -> Vec<String> {
    let s: BTreeSet<String> = board.iter_tiles().map(|t| t.terrain.clone()).collect();
    s.into_iter().collect()
}

fn present_resources(board: &Board) -> Vec<String> {
    let present: BTreeSet<String> = board
        .iter_tiles()
        .flat_map(|t| t.extras.iter().cloned())
        .collect();
    RESOURCE_EXTRAS
        .iter()
        .filter(|r| present.contains(**r))
        .map(|r| r.to_string())
        .collect()
}

// --- the kinds ------------------------------------------------------------

struct TerrainAtKind;
impl QuestionKind for TerrainAtKind {
    fn name(&self) -> &'static str {
        "terrain"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board;
        let n = ctx.n;
        let rng = &mut ctx.rng;
        collect(board, rng, n, |b, rng| {
            let (x, y) = rand_tile(b, rng);
            item(
                b,
                Question::TerrainAt {
                    tile: Referent::Tile { x, y },
                },
            )
        })
    }
}

struct AdjacentTerrainKind;
impl QuestionKind for AdjacentTerrainKind {
    fn name(&self) -> &'static str {
        "adjacency"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board;
        let n = ctx.n;
        let rng = &mut ctx.rng;
        collect(board, rng, n, |b, rng| {
            let (x, y) = rand_tile(b, rng);
            let dir = *rng.choose(&ALL_DIRS)?;
            let (nx, ny) = geometry::step(x, y, dir);
            if !b.in_bounds(nx, ny) {
                return None;
            }
            item(
                b,
                Question::AdjacentTerrain {
                    origin: Referent::Tile { x, y },
                    dir,
                },
            )
        })
    }
}

struct DirectionToKind {
    d: Difficulty,
}
impl QuestionKind for DirectionToKind {
    fn name(&self) -> &'static str {
        "direction"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board;
        let n = ctx.n;
        let rng = &mut ctx.rng;
        let d = self.d;
        collect(board, rng, n, move |b, rng| {
            // Build a target on an exact compass ray so the direction is unambiguous.
            let (x, y) = rand_tile(b, rng);
            let dir: Dir8 = *rng.choose(&ALL_DIRS)?;
            let k = d.dir_ray_min + rng.below(d.dir_ray_span) as i32; // tiles along the ray
            let (dx, dy) = dir.delta();
            let (tx, ty) = (x + dx * k, y + dy * k);
            if !b.in_bounds(tx, ty) {
                return None;
            }
            item(
                b,
                Question::DirectionTo {
                    origin: Referent::Tile { x, y },
                    target: Referent::Tile { x: tx, y: ty },
                },
            )
        })
    }
}

struct DistanceKind;
impl QuestionKind for DistanceKind {
    fn name(&self) -> &'static str {
        "distance"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board;
        let n = ctx.n;
        let rng = &mut ctx.rng;
        collect(board, rng, n, |b, rng| {
            let a = rand_tile(b, rng);
            let c = rand_tile(b, rng);
            if a == c {
                return None;
            }
            item(
                b,
                Question::Distance {
                    a: Referent::Tile { x: a.0, y: a.1 },
                    b: Referent::Tile { x: c.0, y: c.1 },
                },
            )
        })
    }
}

struct NearestResourceKind;
impl QuestionKind for NearestResourceKind {
    fn name(&self) -> &'static str {
        "nearest"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board;
        let n = ctx.n;
        let rng = &mut ctx.rng;
        let resources = present_resources(board);
        if resources.is_empty() {
            return Vec::new();
        }
        collect(board, rng, n, |b, rng| {
            let (x, y) = rand_tile(b, rng);
            let resource = rng.choose(&resources)?.clone();
            item(
                b,
                Question::NearestResource {
                    from: Referent::Tile { x, y },
                    resource,
                },
            )
        })
    }
}

struct CountTerrainKind {
    d: Difficulty,
}
impl QuestionKind for CountTerrainKind {
    fn name(&self) -> &'static str {
        "region-count"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board;
        let n = ctx.n;
        let rng = &mut ctx.rng;
        // Exclude the board's default (most-common) terrain. The list encoders (raw, adjacency)
        // *omit* default-terrain tiles and only state the default in a header, so counting it is a
        // count-by-exclusion — a genuinely harder, encoding-specific task than tallying a listed
        // terrain. Dropping it keeps region-count measuring the same skill across all encodings.
        let default = most_common_terrain(board);
        let terrains: Vec<String> = present_terrains(board)
            .into_iter()
            .filter(|t| *t != default)
            .collect();
        if terrains.is_empty() {
            return Vec::new();
        }
        let d = self.d;
        collect(board, rng, n, move |b, rng| {
            let (x, y) = rand_tile(b, rng);
            let radius = d.count_radius_min + rng.below(d.count_radius_span) as i32;
            let terrain = rng.choose(&terrains)?.clone();
            item(
                b,
                Question::CountTerrainInRadius {
                    center: Referent::Tile { x, y },
                    radius,
                    terrain,
                },
            )
        })
    }
}

struct ReachableKind {
    d: Difficulty,
}
impl QuestionKind for ReachableKind {
    fn name(&self) -> &'static str {
        "reachability"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board;
        let n = ctx.n;
        // The walk is TACTICAL for the fog perspective when fogged (ZOC-aware by default), else the
        // ownerless pure-terrain walk (§9 coherence: the "you" is the fog perspective).
        let mover = ctx.perspective.map(|s| s.to_string());
        let rng = &mut ctx.rng;
        let avoid = "Mountains".to_string();
        let d = self.d;
        collect(board, rng, n, move |b, rng| {
            let (x, y) = rand_tile(b, rng);
            if b.tile(x, y).terrain == avoid {
                return None;
            }
            let budget = d.reach_budget_min + rng.below(d.reach_budget_span) as i32;
            // Pick a goal within a band that makes both yes/no outcomes plausible.
            let dir: Dir8 = *rng.choose(&ALL_DIRS)?;
            let k = 1 + rng.below((budget + 2) as usize) as i32;
            let (dx, dy) = dir.delta();
            let (gx, gy) = (x + dx * k, y + dy * k);
            if !b.in_bounds(gx, gy) || (gx, gy) == (x, y) {
                return None;
            }
            // Under a tactical (fogged) walk, keep both endpoints off enemy stacks/cities so the
            // "your unit starts here / ends there" framing is coherent (an enemy tile is unenterable
            // and would make the endpoint trivially ill-posed).
            if let Some(p) = &mover {
                let enemy_tile = |tx: i32, ty: i32| {
                    b.units
                        .iter()
                        .any(|u| u.x == tx && u.y == ty && &u.owner != p)
                        || b.cities
                            .iter()
                            .any(|c| c.x == tx && c.y == ty && &c.owner != p)
                };
                if enemy_tile(x, y) || enemy_tile(gx, gy) {
                    return None;
                }
            }
            item(
                b,
                Question::Reachable {
                    from: Referent::Tile { x, y },
                    to: Referent::Tile { x: gx, y: gy },
                    budget,
                    avoid: avoid.clone(),
                    mover: mover.clone(),
                },
            )
        })
    }
}

// --- T2 valuation kinds ---------------------------------------------------

/// T2a — unit strength comparison ("which unit has the greater ATTACK/DEFENSE strength").
/// Samples pairs of *modeled* units and keeps only decisive (>= 1.25x margin) instances; the
/// solver applies that margin, so a `None` from it is a dropped (near-)tie. Drop count logged.
struct UnitStrengthKind;
impl QuestionKind for UnitStrengthKind {
    fn name(&self) -> &'static str {
        "unit-strength"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board;
        let n = ctx.n;
        let rng = &mut ctx.rng;
        // Only units with a modeled stat row can form a scored comparison.
        let ids: Vec<i32> = board
            .units
            .iter()
            .filter(|u| rules::unit_stat(&u.kind).is_some())
            .map(|u| u.id)
            .collect();
        if ids.len() < 2 {
            return Vec::new();
        }
        let drops = Cell::new(0u32);
        let out = collect(board, rng, n, |b, rng| {
            let a = *rng.choose(&ids)?;
            let bb = *rng.choose(&ids)?;
            if a == bb {
                return None;
            }
            let axis = if rng.below(2) == 0 {
                StrengthAxis::Attack
            } else {
                StrengthAxis::Defense
            };
            let q = Question::UnitStrengthCompare {
                a: Referent::Unit { id: a },
                b: Referent::Unit { id: bb },
                axis,
            };
            match item(b, q) {
                some @ Some(_) => some,
                None => {
                    drops.set(drops.get() + 1);
                    None
                }
            }
        });
        log_drops("unit-strength", out.len(), drops.get());
        out
    }
}

/// T2b — city defense comparison ("which city is better defended"). Compares each city's best
/// defender's `def_eff` (terrain + city-center bonus). Decisive-margin + drop-ties/both-undefended
/// enforced by the solver; drop count logged.
struct CityDefenseKind;
impl QuestionKind for CityDefenseKind {
    fn name(&self) -> &'static str {
        "city-defense"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board;
        let n = ctx.n;
        let rng = &mut ctx.rng;
        // Need cleanly-named cities to form a Choice.
        let ids: Vec<i32> = board
            .cities
            .iter()
            .filter(|c| !c.name.is_empty())
            .map(|c| c.id)
            .collect();
        if ids.len() < 2 {
            return Vec::new();
        }
        let drops = Cell::new(0u32);
        let out = collect(board, rng, n, |b, rng| {
            let a = *rng.choose(&ids)?;
            let bb = *rng.choose(&ids)?;
            if a == bb {
                return None;
            }
            let q = Question::CityDefenseCompare {
                a: Referent::City { id: a },
                b: Referent::City { id: bb },
            };
            match item(b, q) {
                some @ Some(_) => some,
                None => {
                    drops.set(drops.get() + 1);
                    None
                }
            }
        });
        log_drops("city-defense", out.len(), drops.get());
        out
    }
}

// --- T1 siting kind -------------------------------------------------------

/// Siting — "which of these K candidate tiles is the best city site for the most {terrain} in the
/// work radius" (`Question::BestSiteChoice`). Candidates are absolute coordinates, so it renders
/// identically under every coordinate encoding; the model must evaluate each candidate's vicinity —
/// where interactive (fetch K small windows) and static (read the whole board) diverge. The solver
/// drops ties for best (ambiguous); count logged. Reuses the `count_radius` work-radius scale.
struct BestSiteKind {
    d: Difficulty,
}

/// Number of candidate sites offered per question.
const BEST_SITE_K: usize = 4;

impl QuestionKind for BestSiteKind {
    fn name(&self) -> &'static str {
        "best-site"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board;
        let n = ctx.n;
        let rng = &mut ctx.rng;
        // Count a present, non-default terrain (the default is the omitted background — usually
        // Ocean); draw candidate SITES from land tiles only (you can't found a city on water).
        let default = most_common_terrain(board);
        let terrains: Vec<String> = present_terrains(board)
            .into_iter()
            .filter(|t| *t != default)
            .collect();
        let land: Vec<(i32, i32)> = board
            .iter_tiles()
            .filter(|t| crate::question::is_land(&t.terrain))
            .map(|t| (t.x, t.y))
            .collect();
        if terrains.is_empty() || land.len() < BEST_SITE_K {
            return Vec::new();
        }
        let d = self.d;
        let drops = Cell::new(0u32);
        let out = collect(board, rng, n, |b, rng| {
            let terrain = rng.choose(&terrains)?.clone();
            let radius = d.count_radius_min + rng.below(d.count_radius_span) as i32;
            // Draw K distinct land tiles as candidates.
            let mut seen: HashSet<(i32, i32)> = HashSet::new();
            let mut candidates: Vec<Referent> = Vec::with_capacity(BEST_SITE_K);
            for _ in 0..(BEST_SITE_K * 20) {
                if candidates.len() >= BEST_SITE_K {
                    break;
                }
                let &(x, y) = rng.choose(&land)?;
                if seen.insert((x, y)) {
                    candidates.push(Referent::Tile { x, y });
                }
            }
            if candidates.len() < BEST_SITE_K {
                return None;
            }
            match item(
                b,
                Question::BestSiteChoice {
                    candidates,
                    terrain,
                    radius,
                },
            ) {
                some @ Some(_) => some,
                None => {
                    drops.set(drops.get() + 1);
                    None
                }
            }
        });
        log_drops("best-site", out.len(), drops.get());
        out
    }
}

// --- T1 empire-relative kind ----------------------------------------------

/// The fixed horizon (hard cap) for `nearest-owned`: resources beyond this many tiles from every
/// city are treated as non-existent (a real player stops asking about far, unexploited resources).
pub(crate) const NEAREST_OWNED_HORIZON: i32 = 6;

/// T1 (empire-relative) — nearest UNEXPLOITED resource of a type within `NEAREST_OWNED_HORIZON`
/// tiles of a player's cities, or "none". Horizon-capped so the search is bounded (unlike the
/// global `nearest`). Logs the numeric/none split for admissibility hygiene.
struct NearestOwnedResourceKind;
impl QuestionKind for NearestOwnedResourceKind {
    fn name(&self) -> &'static str {
        "nearest-owned"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board;
        let n = ctx.n;
        let perspective = ctx.perspective;
        let rng = &mut ctx.rng;
        let resources = present_resources(board);
        // Distinct owners that hold at least one city, restricted to the fog perspective player when
        // fogged (§9 coherence).
        let mut owners: Vec<String> = board.cities.iter().map(|c| c.owner.clone()).collect();
        owners.sort();
        owners.dedup();
        let owners = restrict_to_perspective(owners, perspective);
        if resources.is_empty() || owners.is_empty() {
            return Vec::new();
        }
        let out = collect(board, rng, n, |b, rng| {
            let player = rng.choose(&owners)?.clone();
            let resource = rng.choose(&resources)?.clone();
            item(
                b,
                Question::NearestOwnedResource {
                    player,
                    resource,
                    horizon: NEAREST_OWNED_HORIZON,
                },
            )
        });
        // Count "none" answers among the kept (deduped) items so the tally can never
        // exceed `out.len()` — counting inside the closure would also tally duplicates
        // that `collect` later drops, underflowing the "numeric" subtraction below.
        let none_count = out
            .iter()
            .filter(|it| matches!(it.answer, crate::question::Answer::OptionalInt(None)))
            .count();
        eprintln!(
            "[nearest-owned] kept {} instances ({} answered \"none\", {} numeric)",
            out.len(),
            none_count,
            out.len() - none_count
        );
        out
    }
}

/// Log the decisive-margin drop count for a T2 kind (methodological hygiene — DESIGN.md §6 /
/// `T2-T3-design.md` §3, §9 "instrument drop-rate logs from day one"). Kept to a single stderr
/// line so it never pollutes stdout (the `verify-oracle` PASS line).
fn log_drops(kind: &str, kept: usize, dropped: u32) {
    eprintln!("[{kind}] kept {kept} instances, dropped {dropped} (near-tie / not decisive)");
}

// --- T3 valuation kind: settle-site (dominance) ---------------------------

/// Candidate sites offered per settle-site question.
const SETTLE_K: usize = 4;

/// T3a — best settle site (dominance-scored blunder-avoidance, `T2-T3-design.md` §4.2). Constructs
/// a *judgment-loaded* candidate set: a threatened trap `T` with terrain merit, a **safe twin** `T'`
/// whose terrain axes are IDENTICAL to `T`'s but which is strictly safer (so `T'` dominates `T`
/// purely on safety — a blunder no terrain-counting reasoner would catch), plus distractors that do
/// not terrain-dominate `T`. Admissible only if the solved set has a dominated trap AND >= 2 sound
/// options; drop count logged.
struct SettleSiteKind;

impl QuestionKind for SettleSiteKind {
    fn name(&self) -> &'static str {
        "settle-site"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board;
        let n = ctx.n;
        let perspective = ctx.perspective;
        let rng = &mut ctx.rng;
        // Owners that hold a city AND face >= 1 enemy modeled LAND unit (so safety is a live axis),
        // restricted to the fog perspective player when fogged (§9 coherence).
        let owners: Vec<String> = {
            let mut s: BTreeSet<String> = board.cities.iter().map(|c| c.owner.clone()).collect();
            s.retain(|p| {
                board.units.iter().any(|u| {
                    &u.owner != p
                        && rules::unit_stat(&u.kind)
                            .map(|st| st.class == rules::UnitClass::Land)
                            .unwrap_or(false)
                })
            });
            s.into_iter().collect()
        };
        let owners = restrict_to_perspective(owners, perspective);
        if owners.is_empty() {
            return Vec::new();
        }
        // Settleable tiles: land, not on or adjacent to an existing city.
        let city_set: HashSet<(i32, i32)> = board.cities.iter().map(|c| (c.x, c.y)).collect();
        let settleable: Vec<(i32, i32)> = board
            .iter_tiles()
            .filter(|t| crate::question::is_land(&t.terrain))
            .map(|t| (t.x, t.y))
            .filter(|&(x, y)| {
                !city_set.contains(&(x, y))
                    && geometry::tiles_within(board, (x, y), 1)
                        .into_iter()
                        .all(|p| !city_set.contains(&p))
            })
            .collect();
        if settleable.len() < SETTLE_K {
            return Vec::new();
        }
        // Precompute each owner's settle axes over every settleable tile (one threat field per owner)
        // so the inner trap/twin search reads cached axes instead of rebuilding the field per tile.
        let mut axes_by_player: HashMap<String, Vec<rules::SiteAxes>> = HashMap::new();
        for p in &owners {
            let field = rules::ThreatField::compute(board, p);
            let ax = settleable
                .iter()
                .map(|&t| rules::site_axes_with(board, t, &field))
                .collect();
            axes_by_player.insert(p.clone(), ax);
        }
        let drops = Cell::new(0u32);
        let out = collect(board, rng, n, |b, rng| {
            let player = rng.choose(&owners)?.clone();
            let axes = &axes_by_player[&player];
            build_settle_item(b, rng, &player, &settleable, axes, &drops)
        });
        log_drops("settle-site", out.len(), drops.get());
        out
    }
}

/// Try to build one judgment-loaded settle-site instance for `player`, using the precomputed
/// per-tile `axes` aligned with `settleable`. Returns None (bumping `drops`) when this draw cannot
/// assemble a strict trap + safe twin + decisive dominated set.
fn build_settle_item(
    board: &Board,
    rng: &mut Rng,
    player: &str,
    settleable: &[(i32, i32)],
    axes: &[rules::SiteAxes],
    drops: &Cell<u32>,
) -> Option<EvalItem> {
    let miss = || {
        drops.set(drops.get() + 1);
        None::<EvalItem>
    };
    // 1. A threatened trap with some terrain merit.
    let ti = rng.below(settleable.len());
    let t_ax = axes[ti];
    if t_ax.safety >= 0.0 || t_ax.food + t_ax.production + t_ax.resources == 0 {
        return miss();
    }
    // 2. A safe twin: identical terrain axes, strictly safer → dominates the trap only via safety.
    let twin = (0..settleable.len()).find(|&j| {
        j != ti
            && axes[j].food == t_ax.food
            && axes[j].production == t_ax.production
            && axes[j].resources == t_ax.resources
            && axes[j].safety > t_ax.safety
    });
    let twin = match twin {
        Some(j) => j,
        None => return miss(),
    };
    // 3. Distractors that do NOT terrain-dominate the trap (keep it a tempting, terrain-sound pick).
    let mut chosen = vec![ti, twin];
    let mut seen: HashSet<usize> = chosen.iter().copied().collect();
    for _ in 0..settleable.len().min(600) {
        if chosen.len() >= SETTLE_K {
            break;
        }
        let j = rng.below(settleable.len());
        if !seen.insert(j) {
            continue;
        }
        if axes[j].dominates_terrain(&t_ax) {
            seen.remove(&j);
            continue;
        }
        chosen.push(j);
    }
    if chosen.len() < SETTLE_K {
        return miss();
    }
    // Shuffle so the trap isn't always the first option.
    for i in (1..chosen.len()).rev() {
        let k = rng.below(i + 1);
        chosen.swap(i, k);
    }
    let candidates: Vec<Referent> = chosen
        .iter()
        .map(|&j| Referent::Tile {
            x: settleable[j].0,
            y: settleable[j].1,
        })
        .collect();
    let it = item(
        board,
        Question::SettleSiteChoice {
            player: player.to_string(),
            candidates,
        },
    )?;
    // 4. Admissibility on the solved item: the trap must be dominated (a blunder exists) AND there
    //    must be >= 2 sound options (a genuine frontier, not one obvious best).
    if let crate::question::Answer::ChoiceSet {
        acceptable,
        options,
    } = &it.answer
    {
        let trap = format!("({}, {})", settleable[ti].0, settleable[ti].1);
        let trap_dominated = !acceptable.iter().any(|a| a == &trap);
        if trap_dominated && acceptable.len() >= 2 && acceptable.len() < options.len() {
            return Some(it);
        }
    }
    miss()
}

// --- T1 real-play kind: reachable-nearest (scout) -------------------------

/// The fixed reach horizon (turns) for the scout task.
const SCOUT_HORIZON: i32 = 6;

/// T1 (real-play, reachability-bounded) — nearest {resource} a player's land UNIT can reach over
/// land within `SCOUT_HORIZON` turns, or "none". The bounded, egocentric form of the global
/// `nearest` (`DESIGN.md` §6). Logs the numeric/none split.
struct ReachableNearestKind;
impl QuestionKind for ReachableNearestKind {
    fn name(&self) -> &'static str {
        "reachable-nearest"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board;
        let n = ctx.n;
        let rng = &mut ctx.rng;
        let resources = present_resources(board);
        let units: Vec<i32> = board
            .units
            .iter()
            .filter(|u| {
                rules::unit_stat(&u.kind)
                    .map(|s| s.class == rules::UnitClass::Land)
                    .unwrap_or(false)
            })
            .map(|u| u.id)
            .collect();
        if resources.is_empty() || units.is_empty() {
            return Vec::new();
        }
        let out = collect(board, rng, n, |b, rng| {
            let id = *rng.choose(&units)?;
            let resource = rng.choose(&resources)?.clone();
            item(
                b,
                Question::ReachableNearestResource {
                    unit: Referent::Unit { id },
                    resource,
                    horizon: SCOUT_HORIZON,
                },
            )
        });
        // Count "none" answers among the kept (deduped) items so the tally can never
        // exceed `out.len()` — counting inside the closure would also tally duplicates
        // that `collect` later drops, underflowing the "numeric" subtraction below.
        let none_count = out
            .iter()
            .filter(|it| matches!(it.answer, crate::question::Answer::OptionalInt(None)))
            .count();
        eprintln!(
            "[reachable-nearest] kept {} instances ({} answered \"none\", {} numeric)",
            out.len(),
            none_count,
            out.len() - none_count
        );
        out
    }
}

// --- T3 valuation kind: t3-retreat (dominance) ----------------------------

/// Candidate retreat tiles offered per question.
const RETREAT_K: usize = 4;

/// How far (in turns, at the unit's own move rate, over land) a candidate retreat tile may be — the
/// retreat's reachability admissibility bound. Kept small so every offered tile is a plausible
/// this-move-or-next retreat; also gives a big enough reachable pool to construct an identical-
/// cover/support safe twin. (`T2-T3-design.md` §4.3 is a one-line sketch here — "reachability this
/// turn" — so this horizon and the axis set are the documented assumptions.)
const RETREAT_HORIZON: i32 = 3;

/// f64 slack for matching two candidates' COVER (which is a sum of exact 0.5 increments, so exact
/// in practice — this is belt-and-suspenders).
const COVER_EPS: f64 = 1e-9;

/// T3 — safest retreat (dominance-scored blunder-avoidance, `T2-T3-design.md` §4.3). Mirrors the
/// settle-site construction: a *threatened trap* retreat `T` with real cover/support merit, a **safe
/// twin** `T'` with IDENTICAL cover + support but strictly safer (so `T'` dominates `T` purely on
/// safety — a blunder no cover/support counter would catch), plus distractors that do not
/// static-dominate `T`. Reachability of every candidate is guaranteed by construction. Admissible
/// only if the solved set has a dominated trap AND >= 2 sound options; drop count logged.
struct RetreatKind;

impl QuestionKind for RetreatKind {
    fn name(&self) -> &'static str {
        "t3-retreat"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board;
        let n = ctx.n;
        let perspective = ctx.perspective;
        let rng = &mut ctx.rng;
        // Owners that face >= 1 enemy modeled LAND unit (so safety is a live axis at all).
        let mut threatened_owners: BTreeSet<String> = {
            let mut s: BTreeSet<String> = board.cities.iter().map(|c| c.owner.clone()).collect();
            s.extend(board.units.iter().map(|u| u.owner.clone()));
            s.retain(|p| {
                board.units.iter().any(|u| {
                    &u.owner != p
                        && rules::unit_stat(&u.kind)
                            .map(|st| st.class == rules::UnitClass::Land)
                            .unwrap_or(false)
                })
            });
            s
        };
        // The retreating unit's owner is the "you" — pin it to the fog perspective when fogged (§9).
        threatened_owners.retain(|p| perspective.is_none_or(|q| p == q));
        if threatened_owners.is_empty() {
            return Vec::new();
        }
        // One threat field per owner (reused across draws), matching settle-site's caching.
        let mut field_by_owner: HashMap<String, rules::ThreatField> = HashMap::new();
        for p in &threatened_owners {
            field_by_owner.insert(p.clone(), rules::ThreatField::compute(board, p));
        }
        // Retreating candidates: owned modeled LAND units standing on a threatened tile (they have a
        // real reason to flee), whose owner faces an enemy land unit.
        let unit_ids: Vec<i32> = board
            .units
            .iter()
            .filter(|u| threatened_owners.contains(&u.owner))
            .filter(|u| {
                rules::unit_stat(&u.kind)
                    .map(|s| s.class == rules::UnitClass::Land)
                    .unwrap_or(false)
            })
            .filter(|u| field_by_owner[&u.owner].at(u.x, u.y) > 0.0)
            .map(|u| u.id)
            .collect();
        if unit_ids.is_empty() {
            return Vec::new();
        }
        let drops = Cell::new(0u32);
        let out = collect(board, rng, n, |b, rng| {
            let id = *rng.choose(&unit_ids)?;
            let owner = &b.units.iter().find(|u| u.id == id)?.owner;
            let field = &field_by_owner[owner];
            build_retreat_item(b, rng, id, field, &drops)
        });
        log_drops("t3-retreat", out.len(), drops.get());
        out
    }
}

/// Try to build one judgment-loaded retreat instance for unit `unit_id`, using the precomputed
/// owner `field`. Returns None (bumping `drops`) when this draw cannot assemble a threatened trap +
/// identical-cover/support safe twin + decisive dominated set.
fn build_retreat_item(
    board: &Board,
    rng: &mut Rng,
    unit_id: i32,
    field: &rules::ThreatField,
    drops: &Cell<u32>,
) -> Option<EvalItem> {
    let miss = || {
        drops.set(drops.get() + 1);
        None::<EvalItem>
    };
    let u = board.units.iter().find(|u| u.id == unit_id)?;
    let owner = u.owner.clone();
    let (ux, uy) = (u.x, u.y);
    let stat = rules::unit_stat(&u.kind)?;
    // Reachable retreat destinations: land tiles the unit can reach within the horizon, excluding
    // its own tile (a retreat MOVES) and any tile an enemy unit stands on (cannot retreat onto one).
    // TACTICAL for the retreating unit's owner — enemy Zones of Control and enemy-occupied /
    // enemy-city blocking make a retreat path one the unit could actually take.
    let reach = rules::ReachField::from_origins_tactical(
        board,
        &[(ux, uy)],
        stat.move_rate,
        RETREAT_HORIZON,
        &owner,
    );
    let enemy_occ: HashSet<(i32, i32)> = board
        .units
        .iter()
        .filter(|e| e.owner != owner)
        .map(|e| (e.x, e.y))
        .collect();
    let dests: Vec<(i32, i32)> = board
        .iter_tiles()
        .map(|t| (t.x, t.y))
        .filter(|&(x, y)| (x, y) != (ux, uy))
        .filter(|&(x, y)| reach.turns_at(x, y).is_some())
        .filter(|p| !enemy_occ.contains(p))
        .collect();
    if dests.len() < RETREAT_K {
        return miss();
    }
    let axes: Vec<rules::RetreatAxes> = dests
        .iter()
        .map(|&d| rules::retreat_axes_with(board, d, &owner, unit_id, field))
        .collect();
    // 1. A threatened trap with genuine cover or support merit (so it tempts a static reasoner).
    let ti = rng.below(dests.len());
    let t_ax = axes[ti];
    if t_ax.safety >= 0.0 || (t_ax.cover <= 1.0 && t_ax.support == 0) {
        return miss();
    }
    // 2. A safe twin: identical cover + support, strictly safer → dominates the trap only via safety.
    let twin = (0..dests.len()).find(|&j| {
        j != ti
            && (axes[j].cover - t_ax.cover).abs() < COVER_EPS
            && axes[j].support == t_ax.support
            && axes[j].safety > t_ax.safety
    });
    let twin = match twin {
        Some(j) => j,
        None => return miss(),
    };
    // 3. Distractors that do NOT static-dominate the trap (keep it a tempting, cover/support-sound pick).
    let mut chosen = vec![ti, twin];
    let mut seen: HashSet<usize> = chosen.iter().copied().collect();
    for _ in 0..dests.len().min(600) {
        if chosen.len() >= RETREAT_K {
            break;
        }
        let j = rng.below(dests.len());
        if !seen.insert(j) {
            continue;
        }
        if axes[j].dominates_static(&t_ax) {
            seen.remove(&j);
            continue;
        }
        chosen.push(j);
    }
    if chosen.len() < RETREAT_K {
        return miss();
    }
    // Shuffle so the trap isn't always the first option.
    for i in (1..chosen.len()).rev() {
        let k = rng.below(i + 1);
        chosen.swap(i, k);
    }
    let candidates: Vec<Referent> = chosen
        .iter()
        .map(|&j| Referent::Tile {
            x: dests[j].0,
            y: dests[j].1,
        })
        .collect();
    let it = item(
        board,
        Question::RetreatChoice {
            unit: Referent::Unit { id: unit_id },
            candidates,
        },
    )?;
    // 4. Admissibility on the solved item: the trap must be dominated (a blunder exists) AND there
    //    must be >= 2 sound options (a genuine frontier, not one obvious best).
    if let crate::question::Answer::ChoiceSet {
        acceptable,
        options,
    } = &it.answer
    {
        let trap = format!("({}, {})", dests[ti].0, dests[ti].1);
        let trap_dominated = !acceptable.iter().any(|a| a == &trap);
        if trap_dominated && acceptable.len() >= 2 && acceptable.len() < options.len() {
            return Some(it);
        }
    }
    miss()
}

// --- T3 valuation kind: forward-posting (P4 dominance) --------------------

/// Candidate advance tiles offered per forward-posting question.
const POST_K: usize = 4;

/// How far (in turns, at the unit's own move rate, over land) a forward ADVANCE may reach — the
/// posting's reachability admissibility bound. Kept small so every offered tile is a plausible
/// this-move-or-next advance, while giving a big enough reachable pool to find an equal-pressure
/// safer twin. (`reasoning-frontier-questions-v2.md` §v2.1 P4 leaves the horizon a build assumption.)
const POST_HORIZON: i32 = 2;

/// f64 slack for matching two postings' PRESSURE (a sum of att_eff / capture-prob terms; exact in
/// practice when two tiles reach the SAME enemy set, so this is belt-and-suspenders).
const PRESSURE_EPS: f64 = 1e-9;

/// P4 — robust forward posting (dominance-scored blunder-avoidance,
/// `reasoning-frontier-questions-v2.md` §v2.1 P4). Over a unit's reachable ADVANCE tiles it seeds a
/// candidate set around a decisively-DOMINATED trap posting `T` (some other reachable tile `D` is at
/// least as good on BOTH axes — pressure created vs. survival under the enemy's 1-ply reposition
/// reply — and decisively better on one), forcing `D` into the set so `T` is a real blunder, then
/// fills with distractors. The canonical trap is the classic overextension: an equally-safe tile
/// threatens more, or an equal-pressure tile is far safer. Reachability of every candidate is
/// guaranteed by construction. Admissible only if the solved set has a dominated trap AND >= 2 sound
/// options; drop count logged.
struct ForwardPostingKind;

impl QuestionKind for ForwardPostingKind {
    fn name(&self) -> &'static str {
        "forward-posting"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board;
        let n = ctx.n;
        let perspective = ctx.perspective;
        // Reachability is judged on the TRUE board (unmasked when fogged) with enemy-aware tactical
        // movement, so every offered tile is a genuinely legal advance; the pressure/survival axes
        // stay on the model's view (`board`).
        let reach_board = ctx.unmasked.unwrap_or(ctx.board);
        let rng = &mut ctx.rng;
        // Owners that face >= 1 enemy modeled LAND unit — so the enemy has a reposition reply and the
        // survival axis is live at all. The posting unit's owner is the "you" — pin it to the fog
        // perspective when fogged (§9 coherence), matching t3-retreat/cf-vacate.
        let mut owners = threatened_owners(board);
        owners.retain(|p| perspective.is_none_or(|q| p == q));
        if owners.is_empty() {
            return Vec::new();
        }
        // One posting field per owner (reused across draws), matching retreat's caching.
        let mut field_by_owner: HashMap<String, rules::PostingField> = HashMap::new();
        for p in &owners {
            field_by_owner.insert(p.clone(), rules::PostingField::compute(board, p));
        }
        // Posting candidates: owned modeled LAND units whose owner faces an enemy land unit.
        let unit_ids: Vec<i32> = board
            .units
            .iter()
            .filter(|u| owners.contains(&u.owner))
            .filter(|u| {
                rules::unit_stat(&u.kind)
                    .map(|s| s.class == rules::UnitClass::Land)
                    .unwrap_or(false)
            })
            .map(|u| u.id)
            .collect();
        if unit_ids.is_empty() {
            return Vec::new();
        }
        let drops = Cell::new(0u32);
        let out = collect(board, rng, n, |b, rng| {
            let id = *rng.choose(&unit_ids)?;
            let owner = &b.units.iter().find(|u| u.id == id)?.owner;
            let field = &field_by_owner[owner];
            build_posting_item(b, reach_board, rng, id, field, &drops)
        });
        log_drops("forward-posting", out.len(), drops.get());
        out
    }
}

/// Try to build one judgment-loaded forward-posting instance for unit `unit_id`, using the
/// precomputed owner `field`. Seeds the candidate set with a decisively-DOMINATED trap posting (one
/// that creates pressure yet another reachable posting beats on both axes) plus a forced dominator and
/// filler distractors. Returns None (bumping `drops`) when no such trap exists among the unit's
/// reachable advances or the solved set is not decisive (no dominated trap, or < 2 sound options).
fn build_posting_item(
    board: &Board,
    reach_board: &Board,
    rng: &mut Rng,
    unit_id: i32,
    field: &rules::PostingField,
    drops: &Cell<u32>,
) -> Option<EvalItem> {
    let miss = || {
        drops.set(drops.get() + 1);
        None::<EvalItem>
    };
    let u = board.units.iter().find(|u| u.id == unit_id)?;
    let owner = u.owner.clone();
    let (ux, uy) = (u.x, u.y);
    let stat = rules::unit_stat(&u.kind)?;
    // Reachable advance destinations: land tiles the unit can reach within the horizon, judged on the
    // TRUE board with enemy-aware tactical movement (entry-blocking + classic ZOC) so every offered
    // tile is a legal advance. Exclude the unit's own tile (an advance MOVES), any tile an enemy unit
    // stands on, and any enemy-owned city (advancing ONTO one is an assault, not a posting — and under
    // fog its garrison is masked, so it must not slip in as a posting candidate).
    let reach = rules::ReachField::from_origins_tactical(
        reach_board,
        &[(ux, uy)],
        stat.move_rate,
        POST_HORIZON,
        &owner,
    );
    let enemy_occ: HashSet<(i32, i32)> = board
        .units
        .iter()
        .filter(|e| e.owner != owner)
        .map(|e| (e.x, e.y))
        .collect();
    let enemy_city: HashSet<(i32, i32)> = board
        .cities
        .iter()
        .filter(|c| c.owner != owner)
        .map(|c| (c.x, c.y))
        .collect();
    let dests: Vec<(i32, i32)> = board
        .iter_tiles()
        .map(|t| (t.x, t.y))
        .filter(|&(x, y)| (x, y) != (ux, uy))
        .filter(|&(x, y)| reach.turns_at(x, y).is_some())
        .filter(|p| !enemy_occ.contains(p))
        .filter(|p| !enemy_city.contains(p))
        .collect();
    if dests.len() < POST_K {
        return miss();
    }
    let mut axes: Vec<rules::PostingAxes> = Vec::with_capacity(dests.len());
    for &d in &dests {
        match field.axes(board, u, d) {
            Some(a) => axes.push(a),
            None => return miss(),
        }
    }
    let p_margin = crate::question::POSTING_PRESSURE_MARGIN;
    let s_margin = crate::question::POSTING_SURVIVAL_MARGIN;
    let dominates = |j: usize, i: usize| axes[j].decisively_dominates(&axes[i], p_margin, s_margin);
    // Only a posting that creates SOME pressure is an interesting trap (a timid, safe post with zero
    // pressure is trivially a candidate but tests nothing about the aggression/survival trade-off).
    // 1. A trap: a reachable posting `T` that some other reachable posting decisively dominates.
    let dominated: Vec<usize> = (0..dests.len())
        .filter(|&i| axes[i].pressure > PRESSURE_EPS)
        .filter(|&i| (0..dests.len()).any(|j| j != i && dominates(j, i)))
        .collect();
    if dominated.is_empty() {
        return miss();
    }
    let ti = dominated[rng.below(dominated.len())];
    // 2. A dominator of the trap (forced into the set so `T` is genuinely a blunder in the subset).
    let dominators: Vec<usize> = (0..dests.len())
        .filter(|&j| j != ti && dominates(j, ti))
        .collect();
    let dj = dominators[rng.below(dominators.len())];
    // 3. Distractors: any other reachable postings, to fill the candidate set.
    let mut chosen = vec![ti, dj];
    let mut seen: HashSet<usize> = chosen.iter().copied().collect();
    for _ in 0..dests.len().min(600) {
        if chosen.len() >= POST_K {
            break;
        }
        let j = rng.below(dests.len());
        if seen.insert(j) {
            chosen.push(j);
        }
    }
    if chosen.len() < POST_K {
        return miss();
    }
    // Shuffle so the trap isn't always the first option.
    for i in (1..chosen.len()).rev() {
        let k = rng.below(i + 1);
        chosen.swap(i, k);
    }
    let candidates: Vec<Referent> = chosen
        .iter()
        .map(|&j| Referent::Tile {
            x: dests[j].0,
            y: dests[j].1,
        })
        .collect();
    let owner_s = owner.clone();
    let it = item(
        board,
        Question::ForwardPostingChoice {
            player: owner_s,
            unit: Referent::Unit { id: unit_id },
            candidates,
        },
    )?;
    // 4. Admissibility on the solved item: the trap must be dominated (a blunder exists) AND there
    //    must be >= 2 sound options (a genuine frontier, not one obvious best).
    if let crate::question::Answer::ChoiceSet {
        acceptable,
        options,
    } = &it.answer
    {
        let trap = format!("({}, {})", dests[ti].0, dests[ti].1);
        let trap_dominated = !acceptable.iter().any(|a| a == &trap);
        if trap_dominated && acceptable.len() >= 2 && acceptable.len() < options.len() {
            return Some(it);
        }
    }
    miss()
}

// --- T3 valuation kind: t3-threat (city-threat / exposure dominance) ------

/// Candidate cities offered per city-threat question.
const THREAT_K: usize = 4;

/// T3b — most-threatened city, redesigned to a SINGLE axis (`maximal-calculator-corpus.md`): the
/// probability each candidate FALLS to its scariest incoming attacker (`rules::city_capture_prob`).
/// Samples up to [`THREAT_K`] of a player's OWN cities and keeps the instance ONLY when the
/// likeliest-to-fall city clears the runner-up by the decisive probability margin
/// ([`THREAT_FALL_MARGIN`]); the solver applies that filter, so a `None` from it is a dropped
/// (near-tie in odds) draw, logged here. The old two-axis force×defense unique-dominance kept almost
/// no real subsets (~2 of ~2800 on T50); the single odds gap generates decisive instances readily.
struct CityThreatKind;

impl QuestionKind for CityThreatKind {
    fn name(&self) -> &'static str {
        "t3-threat"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board;
        let n = ctx.n;
        let perspective = ctx.perspective;
        let rng = &mut ctx.rng;
        // Owners that face >= 1 enemy modeled LAND unit (so incoming force can be non-zero at all).
        let threatened: HashSet<String> = {
            let mut s: HashSet<String> = HashSet::new();
            for c in &board.cities {
                if board.units.iter().any(|u| {
                    u.owner != c.owner
                        && rules::unit_stat(&u.kind)
                            .map(|st| st.class == rules::UnitClass::Land)
                            .unwrap_or(false)
                }) {
                    s.insert(c.owner.clone());
                }
            }
            s
        };
        // Per-owner list of that owner's distinctly-named city ids; keep only owners with >= 2 such
        // cities who are threatened. Sorted owner list for deterministic sampling.
        let mut cities_by_owner: HashMap<String, Vec<i32>> = HashMap::new();
        for c in &board.cities {
            if !c.name.is_empty() && threatened.contains(&c.owner) {
                cities_by_owner
                    .entry(c.owner.clone())
                    .or_default()
                    .push(c.id);
            }
        }
        let mut owners: Vec<String> = cities_by_owner
            .iter()
            .filter(|(_, ids)| ids.len() >= 2)
            .map(|(p, _)| p.clone())
            .collect();
        owners.sort();
        // Pin the "you" to the fog perspective player when fogged (§9 coherence).
        let owners = restrict_to_perspective(owners, perspective);
        if owners.is_empty() {
            return Vec::new();
        }
        let drops = Cell::new(0u32);
        let out = collect(board, rng, n, |b, rng| {
            let player = rng.choose(&owners)?.clone();
            let ids = &cities_by_owner[&player];
            // Sample a variable-size subset (2..=min(THREAT_K, #cities)) of this player's cities.
            // Varying the size (vs. a fixed K) diversifies the candidate sets so that on sparse
            // boards a *monotone* subset (one clear dominator) can still be drawn — otherwise a few
            // undefended cities poison every fixed-size-4 draw and yield collapses to zero.
            let cap = THREAT_K.min(ids.len());
            let k = 2 + rng.below(cap - 1); // 2..=cap
            let mut seen: HashSet<i32> = HashSet::new();
            let mut chosen: Vec<i32> = Vec::with_capacity(k);
            for _ in 0..(k * 20) {
                if chosen.len() >= k {
                    break;
                }
                let id = *rng.choose(ids)?;
                if seen.insert(id) {
                    chosen.push(id);
                }
            }
            if chosen.len() < 2 {
                return None;
            }
            let candidates: Vec<Referent> =
                chosen.into_iter().map(|id| Referent::City { id }).collect();
            match item(b, Question::CityThreatChoice { player, candidates }) {
                some @ Some(_) => some,
                None => {
                    drops.set(drops.get() + 1);
                    None
                }
            }
        });
        log_drops("t3-threat", out.len(), drops.get());
        out
    }
}

// --- shared helper: owners facing an enemy land unit --------------------------

/// The set of players who face at least one enemy modeled LAND unit — the precondition for any
/// threat-driven kind (settle/retreat/threat all compute this locally; the maximal-calculator kinds
/// share this one helper). Owners are taken from both cities and units.
fn threatened_owners(board: &Board) -> BTreeSet<String> {
    let mut s: BTreeSet<String> = board.cities.iter().map(|c| c.owner.clone()).collect();
    s.extend(board.units.iter().map(|u| u.owner.clone()));
    s.retain(|p| {
        board.units.iter().any(|u| {
            &u.owner != p
                && rules::unit_stat(&u.kind)
                    .map(|st| st.class == rules::UnitClass::Land)
                    .unwrap_or(false)
        })
    });
    s
}

// --- maximal-calculator kind: cf-vacate (counterfactual, Bool) ----------------

/// cf-vacate (C) — "can this city spare its best defender for a turn?" (`maximal-calculator-corpus.md`
/// §2, the cleanest counterfactual). A *sampling* kind: draw the player's own garrisoned, threatened
/// cities and let the solver keep only decisive instances (current garrison secure, and the vacated
/// board decisively holds OR decisively falls — the near-tie band is dropped).
///
/// BALANCE (2026-08-18 validity fix): the yes/no split is a validity requirement, not incidental. An
/// all-"no" set (the original degeneracy: every threatened frontier city was single-defender, so
/// pulling its lone defender left it undefended → decisive fall → "no") makes accuracy indistinguish-
/// able from a constant-"no" prior. This sampler now *buckets every candidate by its oracle verdict*
/// and draws the most even yes/no mix the board+perspective can support, so a "reasoned no" is
/// separable from a "no" prior. The "yes" cases are a genuine second strong defender (or walls/
/// terrain carrying a real second unit). If a given board+perspective has candidates of only one
/// verdict, that is a corpus-content fact — emitted (single-valued) and logged, not fabricated. The
/// oracle and the `Answer::Bool` contract are untouched; only *which* cities are asked changes.
struct CfVacateKind;

impl QuestionKind for CfVacateKind {
    fn name(&self) -> &'static str {
        "cf-vacate"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board;
        let n = ctx.n;
        let perspective = ctx.perspective;
        let rng = &mut ctx.rng;
        // The city being (counterfactually) vacated is the perspective player's own — pin its owner
        // to the fog perspective when fogged (§9 coherence).
        let mut threatened = threatened_owners(board);
        threatened.retain(|p| perspective.is_none_or(|q| p == q));
        // Candidate cities: owned by a threatened player, distinctly named, and currently holding at
        // least one modeled own defender (there is a defender to pull out). Iterated in board order
        // (deterministic); each distinct city yields at most one item, so no dedup pass is needed.
        let ids: Vec<i32> = board
            .cities
            .iter()
            .filter(|c| !c.name.is_empty() && threatened.contains(&c.owner))
            .filter(|c| {
                board.units.iter().any(|u| {
                    u.owner == c.owner
                        && u.x == c.x
                        && u.y == c.y
                        && rules::unit_stat(&u.kind).is_some()
                })
            })
            .map(|c| c.id)
            .collect();
        if ids.is_empty() {
            return Vec::new();
        }
        // Bucket every candidate by its (decisive) oracle verdict. Non-decisive candidates
        // (presupposition fails / no live threat / near-tie band) return None and are counted as
        // drops, exactly as before — only the *selection* among decisive items changes.
        let mut yes: Vec<EvalItem> = Vec::new();
        let mut no: Vec<EvalItem> = Vec::new();
        let mut drops = 0u32;
        for &id in &ids {
            match item(
                board,
                Question::CanVacateCity {
                    city: Referent::City { id },
                },
            ) {
                Some(it) => match it.answer {
                    Answer::Bool(true) => yes.push(it),
                    Answer::Bool(false) => no.push(it),
                    _ => drops += 1, // defensive: cf-vacate is Bool-only
                },
                None => drops += 1,
            }
        }
        let (avail_yes, avail_no) = (yes.len(), no.len());
        // Deterministic shuffle within each bucket (Fisher–Yates via the seeded rng), so which
        // subset is emitted when a bucket exceeds its share is reproducible but not board-order-biased.
        for bucket in [&mut yes, &mut no] {
            for i in (1..bucket.len()).rev() {
                let k = rng.below(i + 1);
                bucket.swap(i, k);
            }
        }
        // Interleave yes/no to draw up to n with the most even split the buckets allow; when one
        // bucket empties, fill the rest from the other.
        let mut yes_it = yes.into_iter();
        let mut no_it = no.into_iter();
        let mut out: Vec<EvalItem> = Vec::new();
        let mut take_yes = true;
        while out.len() < n {
            let next = if take_yes {
                yes_it.next().or_else(|| no_it.next())
            } else {
                no_it.next().or_else(|| yes_it.next())
            };
            match next {
                Some(it) => out.push(it),
                None => break,
            }
            take_yes = !take_yes;
        }
        let emit_yes = out
            .iter()
            .filter(|it| matches!(it.answer, Answer::Bool(true)))
            .count();
        eprintln!(
            "[cf-vacate] balance: available yes={avail_yes} no={avail_no}; emitted yes={emit_yes} no={}",
            out.len() - emit_yes
        );
        log_drops("cf-vacate", out.len(), drops);
        out
    }
}

// --- maximal-calculator kind: adv-assault-target (perspective, ChoiceSet) ------

/// Candidate cities offered per assault-target question.
const ASSAULT_K: usize = 4;

/// adv-assault-target (P) — "which of YOUR cities is the enemy's most attractive assault target?"
/// (`maximal-calculator-corpus.md` §3). Scores the attractiveness FRONTIER over two orthogonal axes —
/// ease of capture (`rules::city_capture_prob` ↑) and city value = size (↑) — as a ChoiceSet: any
/// non-dominated target is sound, a decisively-dominated one is a blunder. Candidate subsets are
/// required to VARY IN SIZE (else the value axis is degenerate and the frontier collapses to the
/// single capture axis). A *sampling* kind mirroring `t3-threat`'s subset draws; drop count logged.
struct AssaultTargetKind;

impl QuestionKind for AssaultTargetKind {
    fn name(&self) -> &'static str {
        "adv-assault-target"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board;
        let n = ctx.n;
        let perspective = ctx.perspective;
        let rng = &mut ctx.rng;
        let threatened = threatened_owners(board);
        let mut cities_by_owner: HashMap<String, Vec<i32>> = HashMap::new();
        for c in &board.cities {
            if !c.name.is_empty() && threatened.contains(&c.owner) {
                cities_by_owner
                    .entry(c.owner.clone())
                    .or_default()
                    .push(c.id);
            }
        }
        let mut owners: Vec<String> = cities_by_owner
            .iter()
            .filter(|(_, ids)| ids.len() >= 2)
            .map(|(p, _)| p.clone())
            .collect();
        owners.sort();
        // Pin the "you" to the fog perspective player when fogged (§9 coherence).
        let owners = restrict_to_perspective(owners, perspective);
        if owners.is_empty() {
            return Vec::new();
        }
        let drops = Cell::new(0u32);
        let out = collect(board, rng, n, |b, rng| {
            let player = rng.choose(&owners)?.clone();
            let ids = &cities_by_owner[&player];
            // Variable subset (2..=min(K, #cities)), like t3-threat, to diversify the frontier shape.
            let cap = ASSAULT_K.min(ids.len());
            let k = 2 + rng.below(cap - 1);
            let mut seen: HashSet<i32> = HashSet::new();
            let mut chosen: Vec<i32> = Vec::with_capacity(k);
            for _ in 0..(k * 20) {
                if chosen.len() >= k {
                    break;
                }
                let id = *rng.choose(ids)?;
                if seen.insert(id) {
                    chosen.push(id);
                }
            }
            if chosen.len() < 2 {
                return None;
            }
            // The VALUE axis is city size; if every chosen candidate is the same size that axis is
            // degenerate and the frontier collapses to the single capture axis. Require the sizes to
            // vary (resample otherwise), so the size×capture trade-off is actually exercised.
            let sizes: Vec<i32> = chosen
                .iter()
                .filter_map(|id| b.cities.iter().find(|c| c.id == *id).map(|c| c.size))
                .collect();
            if sizes.iter().all(|s| *s == sizes[0]) {
                return None; // all-equal size → degenerate value axis, resample
            }
            let candidates: Vec<Referent> =
                chosen.into_iter().map(|id| Referent::City { id }).collect();
            match item(b, Question::AssaultTargetChoice { player, candidates }) {
                some @ Some(_) => some,
                None => {
                    drops.set(drops.get() + 1);
                    None
                }
            }
        });
        log_drops("adv-assault-target", out.len(), drops.get());
        out
    }
}

// --- maximal-calculator kind: triage-reinforce (allocation, ChoiceSet) --------

/// Candidate cities offered per triage-reinforce question.
const TRIAGE_K: usize = 4;

/// triage-reinforce (S, allocation) — "which city gets the one spare defender?"
/// (`maximal-calculator-corpus.md` §4). The scored axis is MARGINAL benefit: the reserve is sound only
/// where it FLIPS a city from capturable to held. A most-threatened-but-doomed city and a least-
/// threatened-but-already-safe city are both wrong, so a naive exposure ranker (either polarity) fails.
/// A *sampling* kind: per threatened owner, fix the strongest spare land defender (a unit not
/// garrisoning one of that owner's cities) and draw city subsets; the solver drops non-decisive sets.
struct TriageReinforceKind;

impl QuestionKind for TriageReinforceKind {
    fn name(&self) -> &'static str {
        "triage-reinforce"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board;
        let n = ctx.n;
        let perspective = ctx.perspective;
        let rng = &mut ctx.rng;
        let threatened = threatened_owners(board);
        let mut cities_by_owner: HashMap<String, Vec<i32>> = HashMap::new();
        for c in &board.cities {
            if !c.name.is_empty() && threatened.contains(&c.owner) {
                cities_by_owner
                    .entry(c.owner.clone())
                    .or_default()
                    .push(c.id);
            }
        }
        // The one spare defender per owner: the strongest (by context-free def_eff) modeled LAND unit
        // NOT standing on one of that owner's own cities (a mobile reserve, not a committed garrison).
        let own_city_tiles: HashSet<(String, i32, i32)> = board
            .cities
            .iter()
            .map(|c| (c.owner.clone(), c.x, c.y))
            .collect();
        let mut reserve_by_owner: HashMap<String, i32> = HashMap::new();
        for u in &board.units {
            if !cities_by_owner.contains_key(&u.owner) {
                continue;
            }
            let is_land = rules::unit_stat(&u.kind)
                .map(|s| s.class == rules::UnitClass::Land)
                .unwrap_or(false);
            if !is_land || own_city_tiles.contains(&(u.owner.clone(), u.x, u.y)) {
                continue;
            }
            let Some(def) = rules::def_eff_base(u) else {
                continue;
            };
            if def <= 0.0 {
                continue;
            }
            let better = reserve_by_owner.get(&u.owner).is_none_or(|&cur| {
                let cur_def = board
                    .units
                    .iter()
                    .find(|x| x.id == cur)
                    .and_then(rules::def_eff_base)
                    .unwrap_or(0.0);
                def > cur_def
            });
            if better {
                reserve_by_owner.insert(u.owner.clone(), u.id);
            }
        }
        let mut owners: Vec<String> = cities_by_owner
            .iter()
            .filter(|(p, ids)| ids.len() >= 2 && reserve_by_owner.contains_key(*p))
            .map(|(p, _)| p.clone())
            .collect();
        owners.sort();
        // Pin the "you" to the fog perspective player when fogged (§9 coherence).
        let owners = restrict_to_perspective(owners, perspective);
        if owners.is_empty() {
            return Vec::new();
        }
        let drops = Cell::new(0u32);
        let out = collect(board, rng, n, |b, rng| {
            let player = rng.choose(&owners)?.clone();
            let ids = &cities_by_owner[&player];
            let reserve = Referent::Unit {
                id: reserve_by_owner[&player],
            };
            let cap = TRIAGE_K.min(ids.len());
            let k = 2 + rng.below(cap - 1);
            let mut seen: HashSet<i32> = HashSet::new();
            let mut chosen: Vec<i32> = Vec::with_capacity(k);
            for _ in 0..(k * 20) {
                if chosen.len() >= k {
                    break;
                }
                let id = *rng.choose(ids)?;
                if seen.insert(id) {
                    chosen.push(id);
                }
            }
            if chosen.len() < 2 {
                return None;
            }
            let candidates: Vec<Referent> =
                chosen.into_iter().map(|id| Referent::City { id }).collect();
            match item(
                b,
                Question::TriageReinforceChoice {
                    player,
                    reserve,
                    candidates,
                },
            ) {
                some @ Some(_) => some,
                None => {
                    drops.set(drops.get() + 1);
                    None
                }
            }
        });
        log_drops("triage-reinforce", out.len(), drops.get());
        out
    }
}

// --- maximal-calculator kind: compare-two-attacks (comparative, Compare3) ------

/// compare-two-attacks (S, comparative) — "which attack is better, or are they incomparable?"
/// (`maximal-calculator-corpus.md` §5). Two withheld-weighting axes (exchange favorability ↑, threat
/// removed ↑); the correct answer is the decisive dominator, or "incomparable" on a genuine trade-off
/// — testing refusal to invent a weighting. A *sampling* kind: pick an attacking owner, two of its
/// land attackers, and two distinct enemy targets; the solver keeps decisive dominances and genuine
/// trade-offs and drops near-ties. Drop count logged.
struct CompareTwoAttacksKind;

impl QuestionKind for CompareTwoAttacksKind {
    fn name(&self) -> &'static str {
        "compare-two-attacks"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board;
        let n = ctx.n;
        let rng = &mut ctx.rng;
        // Attackers per owner: modeled LAND units with positive attack strength.
        let mut attackers_by_owner: HashMap<String, Vec<i32>> = HashMap::new();
        for u in &board.units {
            let is_land = rules::unit_stat(&u.kind)
                .map(|s| s.class == rules::UnitClass::Land)
                .unwrap_or(false);
            if is_land && rules::att_eff(u).map(|a| a > 0.0).unwrap_or(false) {
                attackers_by_owner
                    .entry(u.owner.clone())
                    .or_default()
                    .push(u.id);
            }
        }
        // Attackable enemy targets for an owner: any other player's modeled unit with positive
        // defense on its tile (the favorability divisor must be non-zero).
        let owners: Vec<String> = {
            let mut v: Vec<String> = attackers_by_owner.keys().cloned().collect();
            v.sort();
            v
        };
        let targets_by_owner: HashMap<String, Vec<i32>> = owners
            .iter()
            .map(|p| {
                let ts: Vec<i32> = board
                    .units
                    .iter()
                    .filter(|u| &u.owner != p)
                    .filter(|u| {
                        rules::unit_def_on_own_tile(board, u)
                            .map(|d| d > 0.0)
                            .unwrap_or(false)
                    })
                    .map(|u| u.id)
                    .collect();
                (p.clone(), ts)
            })
            .collect();
        // Owners that can pose a two-target comparison at all.
        let eligible: Vec<String> = owners
            .into_iter()
            .filter(|p| !attackers_by_owner[p].is_empty() && targets_by_owner[p].len() >= 2)
            .collect();
        if eligible.is_empty() {
            return Vec::new();
        }
        let drops = Cell::new(0u32);
        let out = collect(board, rng, n, |b, rng| {
            let player = rng.choose(&eligible)?.clone();
            let attackers = &attackers_by_owner[&player];
            let targets = &targets_by_owner[&player];
            let aa = *rng.choose(attackers)?;
            let ab = *rng.choose(attackers)?;
            let ta = *rng.choose(targets)?;
            let tb = *rng.choose(targets)?;
            if ta == tb {
                return None; // distinct targets give distinct answer labels
            }
            let q = Question::CompareTwoAttacks {
                attacker_a: Referent::Unit { id: aa },
                target_a: Referent::Unit { id: ta },
                attacker_b: Referent::Unit { id: ab },
                target_b: Referent::Unit { id: tb },
            };
            match item(b, q) {
                some @ Some(_) => some,
                None => {
                    drops.set(drops.get() + 1);
                    None
                }
            }
        });
        log_drops("compare-two-attacks", out.len(), drops.get());
        out
    }
}

// --- maximal-calculator kind: constraint-site (region search + non-bundled spacing, Variant C) ---

/// A region must hold at least this many settleable land tiles to be a real SEARCH (not a trivial
/// one-or-two-tile check). Below this the item is dropped.
const CONSTRAINT_MIN_REGION_LAND: usize = 4;
/// Cap on a region's land-tile option set. Keeps exhaustive `site_check` probing expensive-but-
/// bounded and the `ChoiceSet` option list sane; larger regions (open, land-heavy hard radii) are
/// dropped rather than posed.
const CONSTRAINT_MAX_REGION_LAND: usize = 100;

/// constraint-site (I) — region-search redesign (Variant C = A + B,
/// `analysis/constraint-site-redesign.md`). The model is handed a rectangular REGION (a
/// `count_radius`-scaled Chebyshev window around a center), NOT a shortlist, and must NAME any land
/// tile in it that satisfies FOUR constraints — defensible ∧ within-2-of-water ∧ not-enemy-territory
/// ∧ spaced-≥`CITY_MIN_SPACING`-from-every-city — or certify `"none"`. The spacing constraint is the
/// non-bundled seam (B): `site_check` never reports it, so exhaustively probing the tool still leaves
/// the answer wrong on spacing. Generation alternates SOME-cases (center drawn near a precomputed
/// survivor so the region is guaranteed to hold one) and NONE-cases (a region with no survivor), and
/// logs the some/none/dropped tally. Region size scales with `Difficulty` (via `count_radius`).
struct ConstraintSiteKind {
    d: Difficulty,
}

impl QuestionKind for ConstraintSiteKind {
    fn name(&self) -> &'static str {
        "constraint-site"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board;
        let n = ctx.n;
        let d = self.d;
        let rng = &mut ctx.rng;
        // Perspectives: players that own at least one city (a real "enemy territory" frame).
        let mut players: Vec<String> = board.cities.iter().map(|c| c.owner.clone()).collect();
        players.sort();
        players.dedup();
        if players.is_empty() {
            return Vec::new();
        }
        let spacing = rules::CITY_MIN_SPACING;
        let city_set: HashSet<(i32, i32)> = board.cities.iter().map(|c| (c.x, c.y)).collect();
        // Settleable land tiles (not on an existing city) — the pool of region centers.
        let land: Vec<(i32, i32)> = board
            .iter_tiles()
            .filter(|t| crate::question::is_land(&t.terrain))
            .map(|t| (t.x, t.y))
            .filter(|p| !city_set.contains(p))
            .collect();
        if land.is_empty() {
            return Vec::new();
        }
        // Per player, the SURVIVORS: land tiles meeting ALL FOUR constraints. Centering a SOME-case
        // near a survivor guarantees the region actually contains a satisfying tile (the same role
        // the `all3` classification played in the old shortlist generator).
        let mut survivors_by_player: HashMap<String, Vec<(i32, i32)>> = HashMap::new();
        for p in &players {
            let survivors: Vec<(i32, i32)> = land
                .iter()
                .copied()
                .filter(|&t| rules::constraint_site_ok_spaced(board, p, t, spacing))
                .collect();
            survivors_by_player.insert(p.clone(), survivors);
        }
        let drops = Cell::new(0u32);
        let attempt = Cell::new(0usize);
        let out = collect(board, rng, n, |b, rng| {
            let miss = || {
                drops.set(drops.get() + 1);
                None::<EvalItem>
            };
            let player = rng.choose(&players)?.clone();
            let survivors = &survivors_by_player[&player];
            let radius = d.count_radius_min + rng.below(d.count_radius_span) as i32;
            // Alternate SOME/NONE so both are produced; fall back to NONE when this player has no
            // survivor to seed a SOME-case region.
            let want_some = attempt.get().is_multiple_of(2) && !survivors.is_empty();
            attempt.set(attempt.get() + 1);
            // Pick the region center. SOME: a random offset (<= radius on each axis) from a survivor,
            // so the survivor lands SOMEWHERE in the window (not always its center). NONE: a random
            // land tile; the solve+intent check below rejects it if it happens to hold a survivor.
            let (cx, cy) = if want_some {
                let &(sx, sy) = rng.choose(survivors)?;
                let dx = rng.below((2 * radius + 1) as usize) as i32 - radius;
                let dy = rng.below((2 * radius + 1) as usize) as i32 - radius;
                (sx + dx, sy + dy)
            } else {
                *rng.choose(&land)?
            };
            if !b.in_bounds(cx, cy) {
                return miss();
            }
            let it = item(
                b,
                Question::ConstraintSiteChoice {
                    player: player.clone(),
                    center: Referent::Tile { x: cx, y: cy },
                    radius,
                    spacing,
                },
            );
            let Some(it) = it else { return miss() };
            // The solver is the source of truth for the some/none split and the region size. Keep the
            // item only if its split matches the intent AND its option set is a real, bounded search.
            if let crate::question::Answer::ChoiceSet {
                acceptable,
                options,
            } = &it.answer
            {
                let land_opts = options.len().saturating_sub(1); // minus the "none" token
                let is_none = acceptable.len() == 1 && acceptable[0] == "none";
                let split_ok = if want_some { !is_none } else { is_none };
                let size_ok = (CONSTRAINT_MIN_REGION_LAND..=CONSTRAINT_MAX_REGION_LAND)
                    .contains(&land_opts);
                if split_ok && size_ok {
                    return Some(it);
                }
            }
            miss()
        });
        let none_count = out
            .iter()
            .filter(|it| {
                matches!(&it.answer, crate::question::Answer::ChoiceSet { acceptable, .. }
                    if acceptable.len() == 1 && acceptable[0] == "none")
            })
            .count();
        eprintln!(
            "[constraint-site] kept {} instances ({} none-answer, {} some-answer), dropped {}",
            out.len(),
            none_count,
            out.len() - none_count,
            drops.get()
        );
        out
    }
}

// --- reasoning-frontier kind: fogged-garrison assault (P7f, hidden-information) --------------
// The one hidden-information kind in this slice (`reasoning-frontier-questions-v2.md` §v2.1 P7f):
// can the perspective player's VISIBLE adjacent stack capture an enemy city whose garrison is
// FOG-HIDDEN? Ground truth is a DEDUCIBLE, worst-case function of the MASKED board only: the stack is
// scored against a synthetic green unit of the city owner's strongest VISIBLE defender type
// (`rules::worst_case_garrison` on the masked board — the OBSERVED repertoire), NOT the true hidden
// garrison. A fog-blind reader sees "0 defenders → trivially takeable" and is systematically wrong on
// every worst-case-defended city — that gap is the signal. Nothing hidden is read, so moving/adding a
// defender on a fogged tile cannot change the yes/no (the deducibility invariant), consistent with the
// surprise-strike redesign (`analysis/surprise-strike-redesign.md`).
struct FoggedAssaultKind;

impl QuestionKind for FoggedAssaultKind {
    fn name(&self) -> &'static str {
        "fogged-assault"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board; // the MASKED board the model sees — the ONLY board this kind reads now
        let n = ctx.n;
        // Hidden-information kind pinned to the fog perspective (the "you"). The unmasked board is
        // intentionally NOT used: the worst-case garrison is derived from the VISIBLE repertoire, so
        // ground truth is a pure function of the masked board.
        let Some(me) = ctx.perspective else {
            return Vec::new();
        };
        let rng = &mut ctx.rng;
        // My visible land units (mine are never masked out).
        let my_land: Vec<&civ_core::Unit> = board
            .units
            .iter()
            .filter(|u| u.owner == me)
            .filter(|u| {
                rules::unit_stat(&u.kind)
                    .map(|s| s.class == rules::UnitClass::Land)
                    .unwrap_or(false)
            })
            .collect();
        if my_land.is_empty() {
            return Vec::new();
        }
        // Candidate targets: enemy cities visible on the masked board (last-known under fog), each
        // with >= 1 of my land units adjacent (able to strike THIS turn). One entry per city, using
        // ALL its adjacent attackers as the storming stack.
        let candidates: Vec<(i32, Vec<i32>)> = board
            .cities
            .iter()
            .filter(|c| c.owner != me)
            // The garrison must be GENUINELY hidden: the city tile is fogged (last-known city shown,
            // live defenders masked) on the board the model sees. Otherwise the model can just read
            // the defenders and there is no hidden information to reason about.
            .filter(|c| board.is_fogged(c.x, c.y))
            .filter_map(|c| {
                // The storming stack: my land units POISED NEAR the fogged city (Chebyshev 2..=3) —
                // close enough to commit an assault this turn, but NOT adjacent, since an adjacent
                // unit would see into the tile and un-hide the garrison. Reachability of the strike
                // itself is granted (magic placement, as in compare-two-attacks / triage-reinforce);
                // the judged question is capturability vs the UNSEEN garrison, not the approach.
                let atk: Vec<i32> = my_land
                    .iter()
                    .filter(|u| (2..=3).contains(&geometry::chebyshev((u.x, u.y), (c.x, c.y))))
                    .map(|u| u.id)
                    .collect();
                (!atk.is_empty()).then_some((c.id, atk))
            })
            .collect();
        if candidates.is_empty() {
            return Vec::new();
        }
        let drops = Cell::new(0u32);
        let out = collect(board, rng, n, |b, rng| {
            let (cid, atk) = rng.choose(&candidates)?.clone();
            let q = Question::FoggedAssault {
                player: me.to_string(),
                attackers: atk.into_iter().map(|id| Referent::Unit { id }).collect(),
                city: Referent::City { id: cid },
            };
            // Solve ground truth on the MASKED board `b` (what the model sees): the last-known enemy
            // city, my visible attackers, and — crucially — `worst_case_garrison`'s scan restricted to
            // VISIBLE enemy units (the observed repertoire). The model never sees the defenders, and the
            // label never depends on a type it cannot observe.
            match solve(&q, b) {
                Some(ans) => Some(EvalItem::new(q, ans, board.source.clone())),
                None => {
                    drops.set(drops.get() + 1);
                    None
                }
            }
        });
        log_drops("fogged-assault", out.len(), drops.get());
        out
    }
}

// --- reasoning-frontier kind: surprise-strike exposure (P1, hidden-information) --------------
// Which of the perspective's own cities is most exposed to a strike from an enemy it CANNOT see
// (`reasoning-frontier-questions-v2.md` §v2.1 P1, redesigned per `analysis/surprise-strike-redesign.md`).
// Ground truth is now a DEDUCIBLE, worst-case function of the MASKED board only: the scariest VISIBLE
// enemy's reach into in-range fog, weighted by closeness ([`rules::surprise_exposure`]). Nothing hidden
// is read, so the answer never depends on where an unseen unit actually is. The candidate set still
// carries a provably-safe decoy (no fogged tile in strike range) as the luck-free credit floor.

/// Score a surprise-strike instance from an already-computed per-city `exposure` map (keyed by city
/// id, [`rules::surprise_exposure`] on the masked board). Returns the `Answer::Choice` (most-exposed
/// city) or `None` when not decisive / no distinct names / no provably-safe decoy. The generator and
/// the pub [`surprise_strike_answer`] both funnel through this one core, so scored truth cannot drift.
fn surprise_strike_choice(
    cities: &[&civ_core::City],
    exposure: &HashMap<i32, f64>,
    masked: &Board,
) -> Option<Answer> {
    if cities.len() < 2 {
        return None;
    }
    let labels: Vec<String> = cities.iter().map(|c| city_choice_label(c)).collect();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for l in &labels {
        if l.is_empty() || !seen.insert(l.clone()) {
            return None; // need distinctly-named cities
        }
    }
    let exp: Vec<f64> = cities
        .iter()
        .map(|c| exposure.get(&c.id).copied().unwrap_or(0.0))
        .collect();
    let mut order: Vec<usize> = (0..cities.len()).collect();
    order.sort_by(|&a, &b| {
        exp[b]
            .partial_cmp(&exp[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let (top, runner) = (order[0], order[1]);
    if exp[top] <= 0.0 {
        return None; // no observable exposure to anyone -> nothing to assess
    }
    // Decisive: the top exposure clears the runner-up by the ratio margin (a ~0 runner-up clears).
    if exp[top] < crate::question::POSTING_PRESSURE_MARGIN * exp[runner] {
        return None;
    }
    // Luck-free floor: at least one PROVABLY-SAFE decoy — a candidate with no fogged tile in strike
    // range (so it cannot be the answer, and exposure 0 confirms it). A blunderer picking it is
    // wrong by reasoning, not by luck. Provable from the masked board's fog alone.
    let has_safe_decoy = cities.iter().enumerate().any(|(i, c)| {
        i != top
            && exp[i] <= 0.0
            && !rules::any_fogged_within(masked, (c.x, c.y), rules::SURPRISE_REACH)
    });
    if !has_safe_decoy {
        return None;
    }
    Some(Answer::Choice {
        value: labels[top].clone(),
        options: labels,
    })
}

/// Score a surprise-strike instance over `cities` for `player`, reading ONLY the `masked` board (the
/// P1 redesign — deducible, worst-case exposure; `analysis/surprise-strike-redesign.md`). Builds the
/// masked-board [`rules::ThreatField`] once, scores each candidate's [`rules::surprise_exposure_with`],
/// then defers to [`surprise_strike_choice`]. `pub` so the synthetic tests can exercise the scored
/// truth directly (no RNG). The generator uses a precomputed exposure map instead, but both share
/// [`surprise_strike_choice`], so they cannot diverge.
pub fn surprise_strike_answer(
    masked: &Board,
    player: &str,
    cities: &[&civ_core::City],
) -> Option<Answer> {
    let field = rules::ThreatField::compute(masked, player);
    let exposure: HashMap<i32, f64> = cities
        .iter()
        .map(|c| {
            (
                c.id,
                rules::surprise_exposure_with(masked, &field, (c.x, c.y), rules::SURPRISE_REACH),
            )
        })
        .collect();
    surprise_strike_choice(cities, &exposure, masked)
}

struct SurpriseStrikeKind;

impl QuestionKind for SurpriseStrikeKind {
    fn name(&self) -> &'static str {
        "surprise-strike"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board; // masked (what the model sees) — the ONLY board this kind reads now
        let n = ctx.n;
        // A fogged run pins the "you" to the perspective player; without a fog perspective there is no
        // masked board and nothing to assess. The unmasked board is intentionally NOT used (the P1
        // redesign makes ground truth a pure function of the masked board).
        let Some(me) = ctx.perspective else {
            return Vec::new();
        };
        let rng = &mut ctx.rng;
        let own: Vec<&civ_core::City> = board
            .cities
            .iter()
            .filter(|c| c.owner == me && !c.name.is_empty())
            .collect();
        if own.len() < 2 {
            return Vec::new();
        }
        // Deducible exposure from the MASKED board: the scariest VISIBLE-enemy reach into in-range fog,
        // weighted by closeness. Build the threat field once, then score every own city.
        let field = rules::ThreatField::compute(board, me);
        let mut exposure: HashMap<i32, f64> = HashMap::new();
        let mut exposed: Vec<i32> = Vec::new();
        let mut safe: Vec<i32> = Vec::new();
        for c in &own {
            let e =
                rules::surprise_exposure_with(board, &field, (c.x, c.y), rules::SURPRISE_REACH);
            exposure.insert(c.id, e);
            if e > 0.0 {
                exposed.push(c.id);
            }
            // Provably-SAFE decoy: no fogged tile within SURPRISE_REACH — nowhere for an unseen enemy
            // to be, so exposure is 0 and a reader can prove it WITHOUT seeing the fog.
            if !rules::any_fogged_within(board, (c.x, c.y), rules::SURPRISE_REACH) {
                safe.push(c.id);
            }
        }
        // A usable instance needs at least one of each — the deducible answer and the luck-free decoy.
        if exposed.is_empty() || safe.is_empty() {
            return Vec::new();
        }
        let all_ids: Vec<i32> = own.iter().map(|c| c.id).collect();
        let drops = Cell::new(0u32);
        let out = collect(board, rng, n, |b, rng| {
            // Seed the set with one exposed + one safe city, then fill from the rest (2..=3 total).
            let mut chosen: Vec<i32> = vec![*rng.choose(&exposed)?, *rng.choose(&safe)?];
            if chosen[0] == chosen[1] {
                return None;
            }
            let extra = rng.below(2); // 0..=1 filler
            for _ in 0..(extra * 20 + 1) {
                if chosen.len() >= 2 + extra {
                    break;
                }
                let id = *rng.choose(&all_ids)?;
                if !chosen.contains(&id) {
                    chosen.push(id);
                }
            }
            let cities: Vec<&civ_core::City> = chosen
                .iter()
                .filter_map(|&id| b.cities.iter().find(|c| c.id == id))
                .collect();
            if cities.len() != chosen.len() {
                return None;
            }
            match surprise_strike_choice(&cities, &exposure, b) {
                Some(ans) => {
                    let q = Question::SurpriseStrikeExposure {
                        player: me.to_string(),
                        candidates: chosen.iter().map(|&id| Referent::City { id }).collect(),
                    };
                    Some(EvalItem::new(q, ans, b.source.clone()))
                }
                None => {
                    drops.set(drops.get() + 1);
                    None
                }
            }
        });
        log_drops("surprise-strike", out.len(), drops.get());
        out
    }
}

// --- reasoning-frontier kind: hidden-force localization (P3, hidden-information) --------------
// Which fogged AREA hides the largest massed enemy force (`reasoning-frontier-questions-v2.md`
// §v2.1 P3). Ground truth is summed hidden enemy att_eff per region on the unmasked board; the set
// carries a provably-empty decoy (a region with no fogged tile at all).

/// Chebyshev radius of a hidden-force region (the area around a listed center).
const HF_RADIUS: i32 = 2;

/// Score a hidden-force instance: pick the region center with the most summed hidden enemy `att_eff`,
/// requiring a decisive margin and at least one provably-empty decoy (a region with no fogged tile).
fn hidden_force_answer(
    unmasked: &Board,
    masked: &Board,
    player: &str,
    radius: i32,
    centers: &[(i32, i32)],
) -> Option<Answer> {
    if centers.len() < 2 {
        return None;
    }
    let labels: Vec<String> = centers
        .iter()
        .map(|&(x, y)| format!("({x}, {y})"))
        .collect();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for l in &labels {
        if !seen.insert(l.clone()) {
            return None; // distinct centers
        }
    }
    let force: Vec<f64> = centers
        .iter()
        .map(|&c| rules::hidden_force_in_region(unmasked, masked, player, c, radius))
        .collect();
    let mut order: Vec<usize> = (0..centers.len()).collect();
    order.sort_by(|&a, &b| {
        force[b]
            .partial_cmp(&force[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let (top, runner) = (order[0], order[1]);
    if force[top] <= 0.0 {
        return None;
    }
    if force[top] < crate::question::POSTING_PRESSURE_MARGIN * force[runner] {
        return None; // not a decisive margin
    }
    // Provably-empty decoy: a region with no fogged tile at all -> nothing can hide there (force 0).
    let has_empty_decoy = centers
        .iter()
        .enumerate()
        .any(|(i, &c)| i != top && force[i] <= 0.0 && !rules::any_fogged_within(masked, c, radius));
    if !has_empty_decoy {
        return None;
    }
    Some(Answer::Choice {
        value: labels[top].clone(),
        options: labels,
    })
}

struct HiddenForceKind;

impl QuestionKind for HiddenForceKind {
    fn name(&self) -> &'static str {
        "hidden-force"
    }
    fn generate(&self, ctx: &mut GenCtx) -> Vec<EvalItem> {
        let board = ctx.board; // masked
        let n = ctx.n;
        let (Some(me), Some(unmasked)) = (ctx.perspective, ctx.unmasked) else {
            return Vec::new();
        };
        let rng = &mut ctx.rng;
        // HOT centers: the tiles of enemy land units the perspective cannot see (each anchors a region
        // that genuinely hides force). COLD centers: land tiles with NO fogged tile in radius (a
        // provably-empty decoy region). An instance = one hot + cold decoys.
        let hot: Vec<(i32, i32)> = unmasked
            .units
            .iter()
            .filter(|u| u.owner != me)
            .filter(|u| board.is_fogged(u.x, u.y))
            .filter(|u| {
                rules::unit_stat(&u.kind)
                    .map(|s| s.class == rules::UnitClass::Land)
                    .unwrap_or(false)
            })
            .map(|u| (u.x, u.y))
            .collect();
        let cold: Vec<(i32, i32)> = board
            .iter_tiles()
            .filter(|t| crate::question::is_land(&t.terrain))
            .map(|t| (t.x, t.y))
            .filter(|&c| !rules::any_fogged_within(board, c, HF_RADIUS))
            .collect();
        if hot.is_empty() || cold.len() < 2 {
            return Vec::new();
        }
        let drops = Cell::new(0u32);
        let out = collect(board, rng, n, |b, rng| {
            let mut centers: Vec<(i32, i32)> = vec![*rng.choose(&hot)?];
            let extra = 2 + rng.below(2); // 2..=3 cold decoys -> 3..=4 total
            let mut seen: HashSet<(i32, i32)> = centers.iter().copied().collect();
            for _ in 0..(extra * 20) {
                if centers.len() > extra {
                    break;
                }
                let c = *rng.choose(&cold)?;
                if seen.insert(c) {
                    centers.push(c);
                }
            }
            if centers.len() < 2 {
                return None;
            }
            match hidden_force_answer(unmasked, b, me, HF_RADIUS, &centers) {
                Some(ans) => {
                    let q = Question::HiddenForceLocalization {
                        player: me.to_string(),
                        radius: HF_RADIUS,
                        centers: centers
                            .iter()
                            .map(|&(x, y)| Referent::Tile { x, y })
                            .collect(),
                    };
                    Some(EvalItem::new(q, ans, b.source.clone()))
                }
                None => {
                    drops.set(drops.get() + 1);
                    None
                }
            }
        });
        log_drops("hidden-force", out.len(), drops.get());
        out
    }
}

// --- AUDIT: hidden-force (P3) is information-bound (luck) ------------------------------------
// Evidence for `analysis/hidden-force-audit.md`. `hidden_force_answer` -> `hidden_force_in_region`
// scores each region by the att_eff of the enemy units ACTUALLY STANDING on its fogged tiles, read
// from the UNMASKED board. These tests demonstrate that the ground-truth answer is a function of the
// hidden placement the model cannot see: moving the hidden stack between two equally-fogged regions
// flips the answer while the MASKED (observable) board stays byte-for-byte identical.
#[cfg(test)]
mod hidden_force_audit {
    use super::*;
    use civ_core::{Player, Tile, Unit, Visibility};

    /// A 1-row grassland board `width` wide; tiles with `x >= fog_from` are Fogged on the masked
    /// board, the rest Visible. `enemy` = enemy ("En") units placed at the given (x, kind). The
    /// perspective player is "Me". The masked/unmasked distinction is carried entirely by the
    /// `visibility` grid, so a masked clone (same visibility, units stripped or not) is observationally
    /// identical regardless of where the fogged enemy units sit.
    fn board(width: i32, fog_from: i32, enemy: &[(i32, &str)]) -> Board {
        let row: Vec<Tile> = (0..width)
            .map(|x| Tile {
                x,
                y: 0,
                terrain: "Grassland".to_string(),
                extras: BTreeSet::new(),
                owner: None,
            })
            .collect();
        let vrow: Vec<Visibility> = (0..width)
            .map(|x| {
                if x >= fog_from {
                    Visibility::Fogged
                } else {
                    Visibility::Visible
                }
            })
            .collect();
        let units: Vec<Unit> = enemy
            .iter()
            .enumerate()
            .map(|(i, &(x, kind))| Unit {
                x,
                y: 0,
                id: 100 + i as i32,
                kind: kind.to_string(),
                owner: "En".to_string(),
                veteran: 0,
                hp: 10,
            })
            .collect();
        Board {
            width,
            height: 1,
            tiles: vec![row],
            cities: vec![],
            units,
            players: vec![
                Player {
                    id: 0,
                    name: "Me".to_string(),
                    nation: String::new(),
                    is_alive: true,
                },
                Player {
                    id: 1,
                    name: "En".to_string(),
                    nation: String::new(),
                    is_alive: true,
                },
            ],
            known: std::collections::HashMap::new(),
            visibility: Some(vec![vrow]),
            ruleset: None,
            turn: None,
            source: "hf-audit".to_string(),
        }
    }

    fn choice(a: &Answer) -> &str {
        match a {
            Answer::Choice { value, .. } => value.as_str(),
            _ => panic!("expected a Choice answer"),
        }
    }

    /// DECISIVE: the masked board is identical across both scenarios (same width, same fog band, no
    /// visible enemy at all), yet moving the hidden stack from region A (center 9) to region B
    /// (center 6) flips the ground-truth answer from "(9, 0)" to "(6, 0)". The answer is therefore a
    /// function of hidden placement, not of anything observable => information-bound (luck).
    #[test]
    fn answer_flips_when_hidden_stack_moves_masked_board_unchanged() {
        // width 12; x >= 6 fogged. Candidate centers: (9,0) hot-A, (6,0) hot-B, (2,0) provably-empty
        // decoy (tiles x=0..4 all visible). radius = HF_RADIUS = 2.
        let centers = [(9, 0), (6, 0), (2, 0)];

        // Masked board the model sees: same in both worlds. No visible enemy (all enemy units live on
        // fogged tiles, so they never appear). This is the ONLY thing the model gets.
        let masked = board(12, 6, &[]);

        // World A: the hidden 3-Legion stack is massed at (9, 0).
        let unmasked_a = board(12, 6, &[(9, "Legion"), (9, "Legion"), (9, "Legion")]);
        let ans_a = hidden_force_answer(&unmasked_a, &masked, "Me", HF_RADIUS, &centers)
            .expect("world A yields an instance");
        assert_eq!(choice(&ans_a), "(9, 0)");

        // World B: the SAME stack is massed at (6, 0) instead. Nothing else changes; the masked board
        // is byte-for-byte the one above.
        let unmasked_b = board(12, 6, &[(6, "Legion"), (6, "Legion"), (6, "Legion")]);
        let ans_b = hidden_force_answer(&unmasked_b, &masked, "Me", HF_RADIUS, &centers)
            .expect("world B yields an instance");
        assert_eq!(choice(&ans_b), "(6, 0)");

        // The observable input was identical; the answer moved with the invisible units.
        assert_ne!(choice(&ans_a), choice(&ans_b), "answer is set by hidden placement");
    }

    /// The force magnitude the answer keys on is read straight off the UNMASKED units: with the stack
    /// present the region scores > 0; remove the (still-fogged, still-invisible) units and it scores 0.
    /// Nothing on the masked board distinguishes these two worlds.
    #[test]
    fn region_force_is_read_from_unmasked_units() {
        let masked = board(12, 6, &[]);
        let occupied = board(12, 6, &[(9, "Legion"), (9, "Legion")]);
        let empty = board(12, 6, &[]);
        let f_occ = rules::hidden_force_in_region(&occupied, &masked, "Me", (9, 0), HF_RADIUS);
        let f_emp = rules::hidden_force_in_region(&empty, &masked, "Me", (9, 0), HF_RADIUS);
        assert!(f_occ > 0.0, "hidden stack contributes force: {f_occ}");
        assert_eq!(f_emp, 0.0, "no hidden unit => no force, same masked view");
    }
}

// --- cf-vacate balance: the sampler must emit BOTH yes and no when the board supports both --------
// Guards the 2026-08-18 validity fix (see the `CfVacateKind` doc-comment): the original sampler was
// verdict-blind and, on single-defender boards, produced an all-"no" set that made accuracy
// indistinguishable from a constant-"no" prior. The sampler now buckets candidates by oracle verdict
// and draws an even mix; here we build a board that supports both verdicts and assert the emitted set
// is NOT constant.
#[cfg(test)]
mod cf_vacate_balance {
    use super::*;
    use civ_core::{City, Player, Tile, Unit};

    /// A 1-row grassland strip with two perspective-player ("A") cities under a live enemy ("B")
    /// land threat. `Solo` (x=3) holds a single strong defender; `Twin` (x=8) holds two. Both are
    /// currently secure (the best defender clearly wins), so pulling the best leaves Solo UNDEFENDED
    /// (decisive fall → oracle "no") but leaves Twin with a still-decisive second defender (→ "yes").
    /// Enemy Legions sit adjacent to each city so both have a live incoming threat.
    fn two_city_board() -> Board {
        let width = 14;
        let row: Vec<Tile> = (0..width)
            .map(|x| Tile {
                x,
                y: 0,
                terrain: "Grassland".to_string(),
                extras: BTreeSet::new(),
                owner: None,
            })
            .collect();
        let city = |id, x, name: &str| City {
            x,
            y: 0,
            id,
            name: name.to_string(),
            owner: "A".to_string(),
            size: 1,
            improvements: BTreeSet::new(),
        };
        let def = |id, x| Unit {
            x,
            y: 0,
            id,
            kind: "Alpine Troops".to_string(),
            owner: "A".to_string(),
            veteran: 0,
            hp: 10,
        };
        // A weak enemy land unit (Warriors, att 1): a live threat that a strong Alpine garrison
        // clearly beats (so the city is "currently secure" — the oracle presupposition holds) yet
        // that trivially takes the city once it is left UNDEFENDED (the "no" side).
        let threat = |id, x| Unit {
            x,
            y: 0,
            id,
            kind: "Warriors".to_string(),
            owner: "B".to_string(),
            veteran: 0,
            hp: 10,
        };
        Board {
            width,
            height: 1,
            tiles: vec![row],
            cities: vec![city(1, 3, "Solo"), city(2, 8, "Twin")],
            units: vec![
                def(10, 3),      // Solo: single defender
                def(20, 8),      // Twin: first defender
                def(21, 8),      // Twin: second defender (the payoff that makes vacating safe)
                threat(90, 2),   // threatens Solo (adjacent)
                threat(91, 9),   // threatens Twin (adjacent)
            ],
            players: vec![
                Player { id: 0, name: "A".to_string(), nation: String::new(), is_alive: true },
                Player { id: 1, name: "B".to_string(), nation: String::new(), is_alive: true },
            ],
            known: std::collections::HashMap::new(),
            visibility: None,
            ruleset: None,
            turn: None,
            source: "cf-vacate-balance".to_string(),
        }
    }

    fn bool_of(it: &EvalItem) -> bool {
        match it.answer {
            Answer::Bool(b) => b,
            _ => panic!("cf-vacate must be a Bool answer"),
        }
    }

    /// The board supports both verdicts, so the generated set MUST contain both — never the
    /// degenerate constant-"no" (or constant-"yes") set the original sampler produced.
    #[test]
    fn emits_both_yes_and_no_when_board_supports_both() {
        let board = two_city_board();
        let kind = CfVacateKind;
        // Try several seeds: determinism aside, none may collapse to a single value.
        for seed in 0..8u64 {
            let mut ctx = GenCtx {
                board: &board,
                unmasked: None,
                rng: Rng::new(seed),
                n: 8,
                perspective: Some("A"),
            };
            let items = kind.generate(&mut ctx);
            assert_eq!(items.len(), 2, "both cities are decisive => 2 items (seed {seed})");
            let any_yes = items.iter().any(bool_of);
            let any_no = items.iter().any(|it| !bool_of(it));
            assert!(any_yes, "expected a 'yes' (Twin holds after vacating) at seed {seed}");
            assert!(any_no, "expected a 'no' (Solo falls after vacating) at seed {seed}");
        }
    }

    /// Same (board, seed) always yields the same set — the frozen-dataset determinism contract.
    #[test]
    fn generation_is_deterministic() {
        let board = two_city_board();
        let kind = CfVacateKind;
        let gen = |seed| {
            let mut ctx = GenCtx {
                board: &board,
                unmasked: None,
                rng: Rng::new(seed),
                n: 8,
                perspective: Some("A"),
            };
            kind.generate(&mut ctx)
                .iter()
                .map(|it| (it.id.clone(), bool_of(it)))
                .collect::<Vec<_>>()
        };
        assert_eq!(gen(3), gen(3), "same seed => identical set");
    }
}
