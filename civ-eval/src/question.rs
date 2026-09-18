//! The abstract question, its typed answer, and the pure solver that computes ground truth.
//!
//! Pipeline (see DESIGN.md §6): a `Question` is abstract and carries NO answer; `solve` turns
//! (question, board) into an `Answer`; a generator bundles them into an [`EvalItem`]. Scoring
//! (in `scoring.rs`) is a separate stage that only compares an expected `Answer` to a reply.

use std::collections::BTreeSet;

use civ_core::geometry::{self, chebyshev, direction_between, Dir8};
use civ_core::Board;

use crate::encoders::{city_choice_label, unit_choice_label, Encoder, Referent};
use crate::rules::{self, StrengthAxis};

/// Decisive-margin ratio for T2 comparisons: the winner must be at least this many times the
/// loser on the chosen axis, else the instance is dropped as a (near-)tie (DESIGN.md §6, and
/// `T2-T3-design.md` §3). Tunable (§9 "decisive-margin ε").
const T2_MARGIN: f64 = 1.25;

/// Decisive margin, in PROBABILITY space, for the single-axis `t3-threat` winner (most likely to
/// fall) and the `adv-assault-target` capture axis. The most-threatened city is admitted only when its
/// capture probability (`rules::city_capture_prob`) beats the runner-up's by at least this much; a
/// smaller gap is a near-tie and the instance is dropped. Set to 0.15 — a real but attainable
/// separation in odds space (a "clearly likelier to fall" city, not a coin-flip apart). Replacing the
/// old two-axis unique-dominance (which cleared the 1.25× margin on BOTH the force AND the defense
/// axis simultaneously — satisfied by essentially no real subset, ~2 of ~2800 on T50) with this single
/// odds gap makes decisive instances generate readily while keeping the answer un-litigable.
const THREAT_FALL_MARGIN: f64 = 0.15;

/// Decisive margins for the `forward-posting` (P4) two-axis dominance. PRESSURE (a continuous
/// strength-like sum) reuses the multiplicative [`T2_MARGIN`]; SURVIVAL (a probability) reuses the
/// additive [`THREAT_FALL_MARGIN`]. Shared by the solver and the generator so scored truth and
/// admissibility can never drift.
pub(crate) const POSTING_PRESSURE_MARGIN: f64 = T2_MARGIN;
pub(crate) const POSTING_SURVIVAL_MARGIN: f64 = THREAT_FALL_MARGIN;

/// Numerical zero for strength comparisons.
const EPS: f64 = 1e-9;

/// Difficulty tier (DESIGN.md §7). T0/T1 are built here; T2/T3 are designed separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    T0,
    T1,
    T2,
    T3,
}

impl Tier {
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::T0 => "T0",
            Tier::T1 => "T1",
            Tier::T2 => "T2",
            Tier::T3 => "T3",
        }
    }
}

/// Typed ground truth. The variant determines how the scorer extracts and compares a reply.
/// `Choice` carries its legal option set so the scorer can flag an off-board reply as
/// *invalid* (distinct from *wrong*).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    Int(i64),
    OptionalInt(Option<i64>),
    Direction(Dir8),
    Bool(bool),
    Choice {
        value: String,
        options: Vec<String>,
    },
    Coord {
        x: i32,
        y: i32,
    },
    /// A set-valued answer: any member of `acceptable` scores *correct*, a listed-but-unacceptable
    /// `option` is *wrong* (a blunder), anything not in `options` is *invalid*. Used by T3
    /// dominance scoring — `acceptable` is the non-dominated set, precomputed at generation, so the
    /// scorer stays a pure set-membership comparator (`T2-T3-design.md` §5).
    ChoiceSet {
        acceptable: Vec<String>,
        options: Vec<String>,
    },
    /// A three-way comparison: the first option `a`, the second `b`, or [`INCOMPARABLE`] — the
    /// judgment that the two are a genuine multi-axis trade-off with no dominance either way.
    /// `value` (one of `a`, `b`, or [`INCOMPARABLE`]) is the correct choice. Naming the losing
    /// option, or claiming "incomparable" when one dominates, is *wrong*; anything else is
    /// *invalid*. Used by the comparative kinds, where "incomparable" tests refusal to invent a
    /// weighting (`maximal-calculator-corpus.md` §5).
    Compare3 {
        value: String,
        a: String,
        b: String,
    },
}

/// The canonical token for the "genuine trade-off, no dominance" outcome of an [`Answer::Compare3`].
pub const INCOMPARABLE: &str = "incomparable";

impl Answer {
    /// Canonical surface form — what the Oracle emits and what a model should print.
    pub fn canonical(&self) -> String {
        match self {
            Answer::Int(n) => n.to_string(),
            Answer::OptionalInt(Some(n)) => n.to_string(),
            Answer::OptionalInt(None) => "none".to_string(),
            Answer::Direction(d) => d.short().to_string(),
            Answer::Bool(b) => if *b { "yes" } else { "no" }.to_string(),
            Answer::Choice { value, .. } => value.clone(),
            Answer::Coord { x, y } => format!("({x}, {y})"),
            Answer::ChoiceSet { acceptable, .. } => acceptable.first().cloned().unwrap_or_default(),
            Answer::Compare3 { value, .. } => value.clone(),
        }
    }
}

/// The abstract question. Fully parameterized, no answer. Renders to prompt text via an
/// encoder (which controls how `Referent`s are named).
#[derive(Debug, Clone)]
pub enum Question {
    /// T0 — what terrain is at this tile?
    TerrainAt { tile: Referent },
    /// T0 — what terrain is immediately `dir` of this tile?
    AdjacentTerrain { origin: Referent, dir: Dir8 },
    /// T0 — what single compass direction is `target` from `origin`?
    DirectionTo { origin: Referent, target: Referent },
    /// T1 — Chebyshev distance between two tiles.
    Distance { a: Referent, b: Referent },
    /// T1 — distance from `from` to the nearest tile bearing `resource`.
    NearestResource { from: Referent, resource: String },
    /// T1 — count of `terrain` tiles within Chebyshev `radius` of `center` (inclusive).
    CountTerrainInRadius {
        center: Referent,
        radius: i32,
        terrain: String,
    },
    /// T1 — can a unit step from `from` to `to` within `budget` 8-dir steps, never entering
    /// a tile whose terrain is `avoid`? When `mover` is `Some(player)` the walk is TACTICAL for
    /// that player: it additionally cannot enter an enemy-occupied tile or an enemy city and obeys
    /// classic enemy Zones of Control (no step directly between two enemy-ZOC tiles, unless the
    /// destination is a friendly city or already holds a friendly unit). `None` is the pure-terrain
    /// walk (ownerless "a unit"); the generator sets `mover` to the fog perspective when fogged, so
    /// fogged runs are ZOC-aware by default.
    Reachable {
        from: Referent,
        to: Referent,
        budget: i32,
        avoid: String,
        mover: Option<String>,
    },
    /// T2 — which unit has the greater strength on `axis` (context-free), `a` or `b`?
    UnitStrengthCompare {
        a: Referent,
        b: Referent,
        axis: StrengthAxis,
    },
    /// T2 — which city is better defended (best defender's `def_eff`, incl. terrain + city
    /// center), `a` or `b`?
    CityDefenseCompare { a: Referent, b: Referent },
    /// T1 (siting) — of the candidate tiles, which is the best city site: the one with the most
    /// `terrain` tiles within Chebyshev `radius` of it (inclusive, the work radius)? Answer: that
    /// tile's coordinates (a `Choice` over the candidates). Candidates are listed as absolute
    /// coordinates, so it renders identically under every coordinate encoding — the model must
    /// evaluate each candidate's vicinity, which is where interactive (fetch K small windows) and
    /// static (read the whole board) diverge. Kept only when one candidate is the unique maximizer.
    BestSiteChoice {
        candidates: Vec<Referent>,
        terrain: String,
        radius: i32,
    },
    /// T1 — distance from `player`'s nearest city to the closest UNEXPLOITED `resource` tile
    /// (a resource tile more than 2 tiles / outside the working radius, and within `horizon`
    /// tiles of one of the player's cities), or None if there is no such tile in range.
    NearestOwnedResource {
        player: String,
        resource: String,
        horizon: i32,
    },
    /// T3 (valuation) — of the candidate tiles, which is a SOUND (non-dominated) city site for
    /// `player`? Scored by 4-axis Pareto dominance (food / production / resources / safety) over
    /// each candidate's working radius; the axis *weighting* is withheld, so only a dominated
    /// ("blunder") pick is wrong. The non-dominated set is precomputed at generation (into the
    /// `Answer::ChoiceSet`), so the scorer never sees the board (`T2-T3-design.md` §4/§5).
    SettleSiteChoice {
        player: String,
        candidates: Vec<Referent>,
    },
    /// T3 (valuation) — a threatened friendly `unit` must retreat. Of the candidate destination
    /// tiles (all already reachable by the unit — reachability is an admissibility filter, not a
    /// scored axis), which is a SOUND (non-dominated) tile to retreat to? Scored by 3-axis Pareto
    /// dominance (cover / safety / support) at each destination; the axis *weighting* is withheld,
    /// so only a dominated ("blunder") retreat is wrong. The non-dominated set is precomputed at
    /// generation (into the `Answer::ChoiceSet`), so the scorer never sees the board
    /// (`T2-T3-design.md` §4.3/§5).
    RetreatChoice {
        unit: Referent,
        candidates: Vec<Referent>,
    },
    /// T3 (adversarial + underdetermined) — of the candidate ADVANCE tiles, which is a SOUND forward
    /// posting for `player`'s `unit`? You push the unit forward to pressure the enemy; the enemy then
    /// repositions (1 ply) to punish it. Scored by 2-axis Pareto dominance — PRESSURE created
    /// (Σ enemy `att_eff` + Σ enemy `city_capture_prob` brought into the unit's attack reach from the
    /// posted tile) vs. SURVIVAL (`1 − max` over the enemy's reachable 1-turn repositions of their
    /// `win_probability` against the posted unit) — with the axis *weighting* withheld, so only a
    /// decisively-dominated ("blunder") posting is wrong. The novel bit is the 1-ply
    /// enemy-moves-to-exploit search, which the static `ThreatField` structurally under-reads. The
    /// non-dominated set is precomputed at generation (into the `Answer::ChoiceSet`), so the scorer
    /// never sees the board (`reasoning-frontier-questions-v2.md` §v2.1 P4).
    ForwardPostingChoice {
        player: String,
        unit: Referent,
        candidates: Vec<Referent>,
    },
    /// T3 (valuation) — of `player`'s OWN candidate cities, which single one is under the GREATEST
    /// threat? Scored on ONE axis: the probability the city FALLS to its scariest incoming attacker
    /// (`rules::city_capture_prob` — the `ThreatField`'s scariest reachable attacker vs. the city's
    /// best defender, in odds space). The answer is the city with the MAXIMUM fall probability,
    /// admitted only when it beats the runner-up by [`THREAT_FALL_MARGIN`] (else a near-tie in odds,
    /// dropped). The unique winner is precomputed at generation into an `Answer::Choice`, so the
    /// scorer never sees the board (§5). (Redesigned from the old two-axis force×defense dominance,
    /// which a naive win-probability swap would collapse and which kept almost no real instances.)
    CityThreatChoice {
        player: String,
        candidates: Vec<Referent>,
    },
    /// T1 (real-play, reachability-bounded) — nearest tile bearing `resource` that `unit` can reach
    /// over land within `horizon` turns (at its own move rate), or None. The bounded, egocentric
    /// form of the global `nearest` (`DESIGN.md` §6). Origin is a controlled unit.
    ReachableNearestResource {
        unit: Referent,
        resource: String,
        horizon: i32,
    },
    /// T3 (counterfactual) — can `city` spare its single best defender for THIS turn? The answer
    /// recomputes the city's defense on the counterfactual board with the best defender removed and
    /// compares it to the incoming enemy force (the `ThreatField` at the city tile). `Bool`: yes if
    /// the remaining garrison still decisively holds, no if the vacated city would fall. A blunderer
    /// who reads the city's CURRENT (still-garrisoned) defense answers wrong on the "no" cases.
    /// (`maximal-calculator-corpus.md` §2 `cf-vacate`.)
    CanVacateCity { city: Referent },
    /// T3 (perspective) — of `player`'s OWN candidate cities, which is a SOUND answer to "the
    /// enemy's most attractive assault target"? Scored on TWO orthogonal axes read from the
    /// ATTACKER's point of view: ease of capture (`rules::city_capture_prob` ↑ = softer) and city
    /// value (`size` ↑ = a richer prize). The non-dominated frontier is scored: a city another
    /// candidate decisively dominates on BOTH axes (at least as easy to capture AND at least as big,
    /// a real lead on ≥1) is a wrong pick; every other is sound. `ChoiceSet` over the player's own
    /// cities. (`maximal-calculator-corpus.md` §3 `adv-assault-target`.)
    AssaultTargetChoice {
        player: String,
        candidates: Vec<Referent>,
    },
    /// T3 (allocation) — `player` has ONE spare defender (`reserve`) to garrison one of `candidates`
    /// (their own cities). Which city is a SOUND allocation? Scored on MARGINAL benefit: a city is
    /// acceptable only if the reserve FLIPS it from capturable to held; an already-held city and a
    /// still-doomed city are both wrong (wasted defender), regardless of raw exposure. `ChoiceSet`.
    /// (`maximal-calculator-corpus.md` §4 `triage-reinforce`.)
    TriageReinforceChoice {
        player: String,
        reserve: Referent,
        candidates: Vec<Referent>,
    },
    /// T3 (comparative) — which of two proposed attacks is better, or are they incomparable? Attack
    /// `a` is `attacker_a` striking `target_a`; attack `b` likewise. Scored on two withheld-weighting
    /// axes (exchange favorability ↑, threat removed ↑): the decisively-dominating attack if one
    /// exists, else `incomparable` on a genuine trade-off. `Compare3`; a scalar compare would
    /// collapse. (`maximal-calculator-corpus.md` §5 `compare-two-attacks`.)
    CompareTwoAttacks {
        attacker_a: Referent,
        target_a: Referent,
        attacker_b: Referent,
        target_b: Referent,
    },
    /// T3 (predicate-composition, the "I" survivor — region-search redesign, Variant C). Instead of
    /// a handed shortlist, the model is given a rectangular REGION (every tile within `radius`
    /// Chebyshev of `center`) and must NAME any land tile in it that satisfies the CONJUNCTION of
    /// four constraints for `player` — defensible ∧ within-2-of-water ∧ not-enemy-territory ∧
    /// spaced-≥`spacing`-from-every-city — or certify `"none"` when the region holds no such tile.
    /// The 4th (spacing) constraint is DELIBERATELY not surfaced by the `site_check` tool, so a
    /// per-tile lookup returns only 3/4 of the truth and cannot settle a candidate; the model must
    /// read city coordinates and compute the distance itself. The acceptable set (every satisfying
    /// in-region land tile, or `["none"]`) is precomputed at generation into an `Answer::ChoiceSet`
    /// whose `options` enumerate every in-region land tile plus the literal `"none"`, so the scorer
    /// stays board-free (`analysis/constraint-site-redesign.md` Variant C).
    ConstraintSiteChoice {
        player: String,
        center: Referent,
        radius: i32,
        spacing: i32,
    },
    /// T3 (reasoning-frontier, hidden-information / engine a) — can `player`'s visible `attackers`
    /// CAPTURE the enemy `city` THIS turn, when FOG hides the city's garrison? The city is shown
    /// (last-known) but its defenders are masked out, so a fog-blind reader sees "0 defenders →
    /// trivially takeable" and is systematically WRONG on every defended city. The label is
    /// WORST-CASE-ROBUST and a pure function of OBSERVABLES: the stack is scored against a synthetic
    /// green unit of the city owner's strongest FIELDED defender type ([`rules::worst_case_garrison`],
    /// via [`rules::assault_capture_prob_vs`]) — NOT the true, unobservable garrison, which would make
    /// the yes/no luck. Kept only when that capture probability is decisive (>= high → yes, <= low →
    /// no; the middle band drops). `Answer::Bool`.
    FoggedAssault {
        player: String,
        attackers: Vec<Referent>,
        city: Referent,
    },
    /// T3 (reasoning-frontier, hidden-information / engine a — P1) — which ONE of `player`'s OWN
    /// candidate cities is most exposed to a SURPRISE strike within the next few turns from an enemy
    /// unit the player CANNOT currently see (on a fogged tile)? Ground truth is a DEDUCIBLE, WORST-CASE
    /// function of the MASKED board only (`analysis/surprise-strike-redesign.md`): the scariest VISIBLE
    /// enemy's reach into in-range fog, weighted by closeness ([`rules::surprise_exposure`]) — nothing
    /// hidden is read, so the answer never depends on where an unseen unit actually is. It is computed
    /// in the generator ([`crate::generators::surprise_strike_answer`]) rather than this single-board
    /// `solve` only because it needs the fog perspective. The candidate set always carries a
    /// **provably-safe decoy** — a city with no fogged tile in strike range, which a reader can rule out
    /// WITHOUT seeing the fog (the luck-free floor, §v2.1 P1). `Answer::Choice`.
    SurpriseStrikeExposure {
        player: String,
        candidates: Vec<Referent>,
    },
    /// T3 (reasoning-frontier, hidden-information / engine a — P3) — which of the offered fogged AREAS
    /// (each the Chebyshev-`radius` neighbourhood of a listed center tile) hides the LARGEST massed
    /// enemy force the player cannot see? Scored on summed hidden enemy `att_eff` per region on the
    /// UNMASKED board; a coarse, decisive-margin quantity that tolerates grounding noise (§v2.1 P3,
    /// flagged history-hungry). Computed in the generator from both boards. `Answer::Choice` over the
    /// center coordinates.
    HiddenForceLocalization {
        player: String,
        radius: i32,
        centers: Vec<Referent>,
    },
}

/// Whether a terrain name is land (a possible city site). Freeciv `classic` water terrain names.
pub(crate) fn is_land(terrain: &str) -> bool {
    // "Unknown" (unexplored, fog of war) counts as non-land: you can't settle on, or path a land
    // unit through, territory you have never seen.
    !matches!(terrain, "Ocean" | "Deep Ocean" | "Lake" | "Unknown")
}

/// The coordinate label for a tile referent, matching how candidates are listed in the question.
fn coord_label(r: &Referent, board: &Board) -> String {
    match r.resolve(board) {
        Some((x, y)) => format!("({x}, {y})"),
        None => "(?)".to_string(),
    }
}

impl Question {
    pub fn category(&self) -> &'static str {
        match self {
            Question::TerrainAt { .. } => "terrain",
            Question::AdjacentTerrain { .. } => "adjacency",
            Question::DirectionTo { .. } => "direction",
            Question::Distance { .. } => "distance",
            Question::NearestResource { .. } => "nearest",
            Question::CountTerrainInRadius { .. } => "region-count",
            Question::Reachable { .. } => "reachability",
            Question::UnitStrengthCompare { .. } => "unit-strength",
            Question::CityDefenseCompare { .. } => "city-defense",
            Question::BestSiteChoice { .. } => "best-site",
            Question::NearestOwnedResource { .. } => "nearest-owned",
            Question::SettleSiteChoice { .. } => "settle-site",
            Question::RetreatChoice { .. } => "t3-retreat",
            Question::ForwardPostingChoice { .. } => "forward-posting",
            Question::CityThreatChoice { .. } => "t3-threat",
            Question::ReachableNearestResource { .. } => "reachable-nearest",
            Question::CanVacateCity { .. } => "cf-vacate",
            Question::AssaultTargetChoice { .. } => "adv-assault-target",
            Question::TriageReinforceChoice { .. } => "triage-reinforce",
            Question::CompareTwoAttacks { .. } => "compare-two-attacks",
            Question::ConstraintSiteChoice { .. } => "constraint-site",
            Question::FoggedAssault { .. } => "fogged-assault",
            Question::SurpriseStrikeExposure { .. } => "surprise-strike",
            Question::HiddenForceLocalization { .. } => "hidden-force",
        }
    }

    pub fn tier(&self) -> Tier {
        match self {
            Question::TerrainAt { .. }
            | Question::AdjacentTerrain { .. }
            | Question::DirectionTo { .. } => Tier::T0,
            Question::UnitStrengthCompare { .. } | Question::CityDefenseCompare { .. } => Tier::T2,
            Question::SettleSiteChoice { .. }
            | Question::RetreatChoice { .. }
            | Question::ForwardPostingChoice { .. }
            | Question::CityThreatChoice { .. }
            | Question::CanVacateCity { .. }
            | Question::AssaultTargetChoice { .. }
            | Question::TriageReinforceChoice { .. }
            | Question::CompareTwoAttacks { .. }
            | Question::ConstraintSiteChoice { .. }
            | Question::FoggedAssault { .. }
            | Question::SurpriseStrikeExposure { .. }
            | Question::HiddenForceLocalization { .. } => Tier::T3,
            _ => Tier::T1,
        }
    }

    /// A stable id from the category and parameters (deterministic given the board+seed).
    pub fn id(&self) -> String {
        match self {
            Question::TerrainAt { tile } => format!("terrain:{}", tile.key()),
            Question::AdjacentTerrain { origin, dir } => {
                format!("adjacency:{}:{}", origin.key(), dir.short())
            }
            Question::DirectionTo { origin, target } => {
                format!("direction:{}:{}", origin.key(), target.key())
            }
            Question::Distance { a, b } => format!("distance:{}:{}", a.key(), b.key()),
            Question::NearestResource { from, resource } => {
                format!("nearest:{}:{}", from.key(), resource)
            }
            Question::CountTerrainInRadius {
                center,
                radius,
                terrain,
            } => {
                format!("region-count:{}:{radius}:{terrain}", center.key())
            }
            Question::Reachable {
                from,
                to,
                budget,
                avoid,
                mover,
            } => {
                let m = mover.as_deref().unwrap_or("-");
                format!(
                    "reachability:{}:{}:{budget}:{avoid}:{m}",
                    from.key(),
                    to.key()
                )
            }
            Question::UnitStrengthCompare { a, b, axis } => {
                format!("unit-strength:{}:{}:{}", a.key(), b.key(), axis.key())
            }
            Question::CityDefenseCompare { a, b } => {
                format!("city-defense:{}:{}", a.key(), b.key())
            }
            Question::BestSiteChoice {
                candidates,
                terrain,
                radius,
            } => {
                let cs: Vec<String> = candidates.iter().map(|c| c.key()).collect();
                format!("best-site:{terrain}:{radius}:{}", cs.join(","))
            }
            Question::NearestOwnedResource {
                player,
                resource,
                horizon,
            } => {
                format!("nearest-owned:{player}:{resource}:{horizon}")
            }
            Question::SettleSiteChoice { player, candidates } => {
                let cs: Vec<String> = candidates.iter().map(|c| c.key()).collect();
                format!("settle-site:{player}:{}", cs.join(","))
            }
            Question::RetreatChoice { unit, candidates } => {
                let cs: Vec<String> = candidates.iter().map(|c| c.key()).collect();
                format!("t3-retreat:{}:{}", unit.key(), cs.join(","))
            }
            Question::ForwardPostingChoice {
                player,
                unit,
                candidates,
            } => {
                let cs: Vec<String> = candidates.iter().map(|c| c.key()).collect();
                format!("forward-posting:{player}:{}:{}", unit.key(), cs.join(","))
            }
            Question::CityThreatChoice { player, candidates } => {
                let cs: Vec<String> = candidates.iter().map(|c| c.key()).collect();
                format!("t3-threat:{player}:{}", cs.join(","))
            }
            Question::ReachableNearestResource {
                unit,
                resource,
                horizon,
            } => {
                format!("reachable-nearest:{}:{resource}:{horizon}", unit.key())
            }
            Question::CanVacateCity { city } => format!("cf-vacate:{}", city.key()),
            Question::AssaultTargetChoice { player, candidates } => {
                let cs: Vec<String> = candidates.iter().map(|c| c.key()).collect();
                format!("adv-assault-target:{player}:{}", cs.join(","))
            }
            Question::TriageReinforceChoice {
                player,
                reserve,
                candidates,
            } => {
                let cs: Vec<String> = candidates.iter().map(|c| c.key()).collect();
                format!(
                    "triage-reinforce:{player}:{}:{}",
                    reserve.key(),
                    cs.join(",")
                )
            }
            Question::CompareTwoAttacks {
                attacker_a,
                target_a,
                attacker_b,
                target_b,
            } => {
                format!(
                    "compare-two-attacks:{}:{}:{}:{}",
                    attacker_a.key(),
                    target_a.key(),
                    attacker_b.key(),
                    target_b.key()
                )
            }
            Question::ConstraintSiteChoice {
                player,
                center,
                radius,
                spacing,
            } => {
                format!("constraint-site:{player}:{}:{radius}:{spacing}", center.key())
            }
            Question::FoggedAssault {
                player,
                attackers,
                city,
            } => {
                let a: Vec<String> = attackers.iter().map(|c| c.key()).collect();
                format!("fogged-assault:{player}:{}:{}", city.key(), a.join(","))
            }
            Question::SurpriseStrikeExposure { player, candidates } => {
                let cs: Vec<String> = candidates.iter().map(|c| c.key()).collect();
                format!("surprise-strike:{player}:{}", cs.join(","))
            }
            Question::HiddenForceLocalization {
                player,
                radius,
                centers,
            } => {
                let cs: Vec<String> = centers.iter().map(|c| c.key()).collect();
                format!("hidden-force:{player}:{radius}:{}", cs.join(","))
            }
        }
    }

    /// The natural-language question, with referents named by the encoder.
    pub fn render(&self, enc: &dyn Encoder, board: &Board) -> String {
        let r = |x: &Referent| enc.render_referent(board, x);
        match self {
            Question::TerrainAt { tile } => {
                format!("What is the terrain of {}?", r(tile))
            }
            Question::AdjacentTerrain { origin, dir } => format!(
                "What is the terrain of the tile immediately to the {} ({}) of {}?",
                dir.long(),
                dir.short(),
                r(origin)
            ),
            Question::DirectionTo { origin, target } => format!(
                "In which single compass direction does {} lie from {}?",
                r(target),
                r(origin)
            ),
            Question::Distance { a, b } => format!(
                "How many tiles apart are {} and {}, counting a diagonal step as one move?",
                r(a),
                r(b)
            ),
            Question::NearestResource { from, resource } => format!(
                "How many tiles is it from {} to the nearest tile containing {}, \
                 counting a diagonal step as one move?",
                r(from),
                resource
            ),
            Question::CountTerrainInRadius { center, radius, terrain } => format!(
                "How many {} tiles are within {} tiles (Chebyshev distance) of {}, \
                 including that tile itself?",
                terrain,
                radius,
                r(center)
            ),
            Question::Reachable { from, to, budget, avoid, mover } => match mover {
                None => format!(
                    "Starting on {}, can a unit reach {} in at most {} steps (moving one tile at a \
                     time in any of the 8 directions) without ever entering a {} tile?",
                    r(from),
                    r(to),
                    budget,
                    avoid
                ),
                Some(p) => format!(
                    "Starting on {}, can a unit belonging to {} reach {} in at most {} steps \
                     (moving one tile at a time in any of the 8 directions) without ever entering a \
                     {} tile, a tile occupied by an enemy unit, or an enemy city, and respecting \
                     enemy zones of control (it cannot step directly between two tiles that are \
                     both next to an enemy military unit, unless the tile it steps onto is one of \
                     {}'s own cities or already holds one of {}'s units)?",
                    r(from),
                    p,
                    r(to),
                    budget,
                    avoid,
                    p,
                    p
                ),
            },
            Question::UnitStrengthCompare { a, b, axis } => format!(
                "Which unit has the greater {} strength, {} or {}?",
                axis.as_str(),
                r(a),
                r(b)
            ),
            Question::CityDefenseCompare { a, b } => {
                format!("Which city is better defended, {} or {}?", r(a), r(b))
            }
            Question::BestSiteChoice { candidates, terrain, radius } => {
                let list =
                    candidates.iter().map(|c| coord_label(c, board)).collect::<Vec<_>>().join(", ");
                format!(
                    "Which of these candidate tiles would make the best city site — the one with \
                     the most {} tiles within {} tiles (Chebyshev distance) of it, counting the \
                     tile itself: {}? Answer with that tile's coordinates.",
                    terrain, radius, list
                )
            }
            Question::NearestOwnedResource { player, resource, horizon } => format!(
                "You are player {player}. Considering only tiles bearing {resource} that are NOT \
                 already within 2 tiles (Chebyshev distance) of one of your cities (i.e. resources \
                 you do not yet work), what is the fewest number of TURNS a unit would need to \
                 travel over land (moving one tile per turn, never crossing ocean) from your \
                 nearest city to reach the closest such {resource} tile? Consider only tiles \
                 reachable within {horizon} turns; answer with a single whole number of turns, or \
                 \"none\" if there is no such {resource} in reach."
            ),
            Question::SettleSiteChoice { player, candidates } => {
                let list =
                    candidates.iter().map(|c| coord_label(c, board)).collect::<Vec<_>>().join(", ");
                format!(
                    "You are founding a new city for player {player}. Following the SETTLE-SITE \
                     RULES above, which of these candidate tiles is a SOUND (non-dominated) city \
                     site: {list}? Answer with the coordinates of any sound candidate."
                )
            }
            Question::RetreatChoice { unit, candidates } => {
                let list =
                    candidates.iter().map(|c| coord_label(c, board)).collect::<Vec<_>>().join(", ");
                format!(
                    "One of your units, {}, is under enemy threat and must retreat. Following the \
                     RETREAT RULES above, which of these tiles is a SOUND (non-dominated) tile to \
                     retreat to: {list}? Answer with the coordinates of any sound candidate.",
                    r(unit)
                )
            }
            Question::ForwardPostingChoice { player, unit, candidates } => {
                let list =
                    candidates.iter().map(|c| coord_label(c, board)).collect::<Vec<_>>().join(", ");
                format!(
                    "You are player {player}. To pressure the enemy you will advance {} to one of \
                     these tiles. Assume the enemy then repositions to punish you. Following the \
                     POSTING RULES above, which advance is SOUND (not clearly worse than another when \
                     you weigh pressure created vs risk of being destroyed on the enemy's reply): \
                     {list}? Answer with the coordinates of any sound advance.",
                    r(unit)
                )
            }
            Question::CityThreatChoice { player, candidates } => {
                let list = candidates.iter().map(r).collect::<Vec<_>>().join(", ");
                format!(
                    "You are player {player}. Following the CITY-THREAT RULES above, which single \
                     one of your cities is under the GREATEST threat — the one MOST LIKELY TO FALL \
                     this turn to its scariest incoming attacker: {list}? Answer with that city's \
                     name."
                )
            }
            Question::ReachableNearestResource { unit, resource, horizon } => format!(
                "You control {}. Moving over land only (never crossing ocean), what is the fewest \
                 number of turns it needs to travel to reach the nearest tile containing {}? A unit \
                 moves up to its own move rate in tiles per turn. Consider only tiles reachable \
                 within {} turns; answer with a single whole number of turns, or \"none\" if no \
                 such tile is in reach.",
                r(unit),
                resource,
                horizon
            ),
            Question::CanVacateCity { city } => format!(
                "Following the VACATE RULES above, can {} spare its single best defender for THIS \
                 turn — if you pulled that defender out now, would the city still hold against the \
                 enemy force that can reach it? Answer yes or no.",
                r(city)
            ),
            Question::AssaultTargetChoice { player, candidates } => {
                let list = candidates.iter().map(r).collect::<Vec<_>>().join(", ");
                format!(
                    "You are player {player}. Following the ASSAULT-TARGET RULES above, reasoning \
                     from the enemy's point of view, which of your cities is a MOST ATTRACTIVE \
                     assault target (a sound answer — one not clearly a worse target than another): \
                     {list}? Answer with that city's name."
                )
            }
            Question::TriageReinforceChoice { player, reserve, candidates } => {
                let list = candidates.iter().map(r).collect::<Vec<_>>().join(", ");
                format!(
                    "You are player {player}. You have one spare defender ({}); ASSUME you can \
                     deploy it to ANY of these cities this turn (ignore travel distance): {list}. \
                     Following the TRIAGE-REINFORCE RULES above, which city does it best protect — \
                     the one whose odds of holding it most improves (a sound allocation, one the \
                     defender actually saves)? Answer with that city's name.",
                    r(reserve)
                )
            }
            Question::CompareTwoAttacks { attacker_a, target_a, attacker_b, target_b } => format!(
                "Following the ATTACK-COMPARISON RULES above, compare two proposed attacks purely on \
                 VALUE — assume EITHER attack could be made (ignore whether a unit can reach its \
                 target this turn). Attack A: {} strikes the enemy {}. Attack B: {} strikes the \
                 enemy {}. If you could make either attack, which is the better one, or are they \
                 incomparable? Answer with the TARGET coordinates of the better attack ({} or {}), \
                 or the word \"incomparable\".",
                r(attacker_a),
                r(target_a),
                r(attacker_b),
                r(target_b),
                coord_label(target_a, board),
                coord_label(target_b, board),
            ),
            Question::ConstraintSiteChoice {
                player,
                center,
                radius,
                spacing,
            } => {
                let (cx, cy) = center.resolve(board).unwrap_or((0, 0));
                let (x0, y0) = (cx - radius, cy - radius);
                let (x1, y1) = (cx + radius, cy + radius);
                format!(
                    "You are choosing a fortified city site for player {player}. Following the \
                     CONSTRAINT-SITE RULES above, search the square REGION of tiles within {radius} \
                     tiles (Chebyshev distance) of {} — i.e. every tile from ({x0}, {y0}) to \
                     ({x1}, {y1}) that lies on the map — and name any LAND tile in it that meets ALL \
                     of the requirements at once: defensible AND within 2 tiles of water AND not in \
                     enemy territory AND at least {spacing} tiles (Chebyshev) from every existing \
                     city. Answer with the coordinates of any qualifying tile in the region, or the \
                     word \"none\" if the region contains no such tile.",
                    coord_label(center, board)
                )
            }
            Question::FoggedAssault { player, attackers, city } => {
                let list = attackers.iter().map(r).collect::<Vec<_>>().join(", ");
                format!(
                    "You are player {player}. You are considering committing your nearby units — \
                     {list} — to storm the enemy city {} THIS turn; ASSUME they can reach it and \
                     strike (ignore the approach). The city lies under fog, so you cannot see its \
                     garrison. Following the CITY-ASSAULT RULES above, would the assault CAPTURE the \
                     city this turn? Answer yes or no.",
                    r(city)
                )
            }
            Question::SurpriseStrikeExposure { player, candidates } => {
                let list = candidates.iter().map(r).collect::<Vec<_>>().join(", ");
                format!(
                    "You are player {player}. Following the SURPRISE-STRIKE RULES above, which single \
                     one of your cities is most likely to come under attack within the next few turns \
                     from an enemy unit you CANNOT currently see (one hidden in the fog nearby): \
                     {list}? Answer with that city's name."
                )
            }
            Question::HiddenForceLocalization { player, radius, centers } => {
                let list = centers.iter().map(|c| coord_label(c, board)).collect::<Vec<_>>().join(", ");
                format!(
                    "You are player {player}. Enemy forces may be massing out of your sight. Each of \
                     these areas is the square of tiles within {radius} tiles (Chebyshev) of a center \
                     tile. Following the HIDDEN-FORCE RULES above, which area hides the LARGEST massed \
                     enemy force you cannot currently see — identify it by its CENTER: {list}? Answer \
                     with that center's coordinates."
                )
            }
        }
    }

    /// Type-specific hint for the strict answer line (composed by the prompt assembler).
    pub fn answer_hint(&self) -> String {
        match self {
            Question::TerrainAt { .. } | Question::AdjacentTerrain { .. } => {
                "a single terrain name".to_string()
            }
            Question::DirectionTo { .. } => {
                "one compass direction (N, NE, E, SE, S, SW, W, NW)".to_string()
            }
            Question::Distance { .. }
            | Question::NearestResource { .. }
            | Question::CountTerrainInRadius { .. } => "a single whole number".to_string(),
            Question::Reachable { .. } => "yes or no".to_string(),
            Question::UnitStrengthCompare { .. } => {
                "the stronger unit, named by its type and coordinates (e.g. \"Armor at (24, 25)\")"
                    .to_string()
            }
            Question::CityDefenseCompare { .. } => "the better-defended city's name".to_string(),
            Question::BestSiteChoice { .. } => {
                "the best candidate tile's coordinates, e.g. (34, 12)".to_string()
            }
            Question::NearestOwnedResource { .. } => {
                "a single whole number of tiles, or the word \"none\"".to_string()
            }
            Question::SettleSiteChoice { .. } | Question::RetreatChoice { .. } => {
                "the coordinates of a sound candidate tile, e.g. (34, 12)".to_string()
            }
            Question::ForwardPostingChoice { .. } => {
                "the coordinates of a sound advance, e.g. (34, 12)".to_string()
            }
            Question::CityThreatChoice { .. } => "the most-threatened city's name".to_string(),
            Question::ReachableNearestResource { .. } => {
                "a single whole number of turns, or the word \"none\"".to_string()
            }
            Question::CanVacateCity { .. } => "yes or no".to_string(),
            Question::AssaultTargetChoice { .. } => {
                "a sound target city's name".to_string()
            }
            Question::TriageReinforceChoice { .. } => {
                "the name of the city to reinforce".to_string()
            }
            Question::CompareTwoAttacks { .. } => {
                "the better attack's target coordinates, e.g. (34, 12), or the word \"incomparable\""
                    .to_string()
            }
            Question::ConstraintSiteChoice { .. } => {
                "the coordinates of a qualifying tile in the region, e.g. (34, 12), or the word \
                 \"none\""
                    .to_string()
            }
            Question::FoggedAssault { .. } => "yes or no".to_string(),
            Question::SurpriseStrikeExposure { .. } => {
                "the most-exposed city's name".to_string()
            }
            Question::HiddenForceLocalization { .. } => {
                "the center coordinates of the area hiding the largest force, e.g. (34, 12)"
                    .to_string()
            }
        }
    }
}

/// Distinct terrain names present on the board (sorted). The option set for terrain answers.
pub fn terrain_names(board: &Board) -> Vec<String> {
    let set: BTreeSet<String> = board.iter_tiles().map(|t| t.terrain.clone()).collect();
    set.into_iter().collect()
}

/// The pure solver: compute ground truth for a question over a board. Returns `None` when the
/// question is ill-posed on this board (e.g. a resource that appears nowhere), so generators
/// can reject it.
pub fn solve(q: &Question, board: &Board) -> Option<Answer> {
    match q {
        Question::TerrainAt { tile } => {
            let (x, y) = tile.resolve(board)?;
            Some(Answer::Choice {
                value: board.tile(x, y).terrain.clone(),
                options: terrain_names(board),
            })
        }
        Question::AdjacentTerrain { origin, dir } => {
            let (x, y) = origin.resolve(board)?;
            let (nx, ny) = geometry::step(x, y, *dir);
            if !board.in_bounds(nx, ny) {
                return None;
            }
            Some(Answer::Choice {
                value: board.tile(nx, ny).terrain.clone(),
                options: terrain_names(board),
            })
        }
        Question::DirectionTo { origin, target } => {
            let o = origin.resolve(board)?;
            let t = target.resolve(board)?;
            direction_between(o, t).map(Answer::Direction)
        }
        Question::Distance { a, b } => Some(Answer::Int(chebyshev(
            a.resolve(board)?,
            b.resolve(board)?,
        ) as i64)),
        Question::NearestResource { from, resource } => {
            let o = from.resolve(board)?;
            let best = board
                .iter_tiles()
                .filter(|t| t.has(resource))
                .map(|t| chebyshev(o, (t.x, t.y)))
                .min()?;
            Some(Answer::Int(best as i64))
        }
        Question::CountTerrainInRadius {
            center,
            radius,
            terrain,
        } => {
            let c = center.resolve(board)?;
            let n = geometry::tiles_within(board, c, *radius)
                .into_iter()
                .filter(|&(x, y)| board.tile(x, y).terrain == *terrain)
                .count();
            Some(Answer::Int(n as i64))
        }
        Question::Reachable {
            from,
            to,
            budget,
            avoid,
            mover,
        } => {
            let s = from.resolve(board)?;
            let g = to.resolve(board)?;
            Some(Answer::Bool(reachable(
                board,
                s,
                g,
                *budget,
                avoid,
                mover.as_deref(),
            )))
        }
        Question::UnitStrengthCompare { a, b, axis } => {
            let ua = a.unit(board)?;
            let ub = b.unit(board)?;
            let la = unit_choice_label(ua);
            let lb = unit_choice_label(ub);
            if la == lb {
                return None; // indistinguishable labels (same type + coords) — ambiguous
            }
            let sa = rules::strength(ua, *axis)?;
            let sb = rules::strength(ub, *axis)?;
            let winner = decisive_winner(sa, sb, &la, &lb)?;
            Some(Answer::Choice {
                value: winner,
                options: vec![la, lb],
            })
        }
        Question::CityDefenseCompare { a, b } => {
            let ca = a.city(board)?;
            let cb = b.city(board)?;
            let la = city_choice_label(ca);
            let lb = city_choice_label(cb);
            if la.is_empty() || lb.is_empty() || la == lb {
                return None; // need two distinctly-named cities for a clean Choice
            }
            let sa = rules::city_defense(board, ca);
            let sb = rules::city_defense(board, cb);
            let winner = decisive_winner(sa, sb, &la, &lb)?;
            Some(Answer::Choice {
                value: winner,
                options: vec![la, lb],
            })
        }
        Question::BestSiteChoice {
            candidates,
            terrain,
            radius,
        } => {
            if candidates.len() < 2 {
                return None;
            }
            // Score each candidate = count of `terrain` in its Chebyshev-`radius` vicinity.
            let mut scored: Vec<(usize, String)> = Vec::with_capacity(candidates.len());
            for c in candidates {
                let (x, y) = c.resolve(board)?;
                let cnt = geometry::tiles_within(board, (x, y), *radius)
                    .into_iter()
                    .filter(|&(tx, ty)| board.tile(tx, ty).terrain == *terrain)
                    .count();
                scored.push((cnt, format!("({x}, {y})")));
            }
            let max = scored.iter().map(|(c, _)| *c).max().unwrap_or(0);
            if max == 0 {
                return None; // no candidate has any of the terrain nearby — vacuous
            }
            let winners: Vec<&String> = scored
                .iter()
                .filter(|(c, _)| *c == max)
                .map(|(_, l)| l)
                .collect();
            if winners.len() != 1 {
                return None; // tie for best → ambiguous, drop
            }
            let value = winners[0].clone();
            let options: Vec<String> = scored.into_iter().map(|(_, l)| l).collect();
            Some(Answer::Choice { value, options })
        }
        Question::NearestOwnedResource {
            player,
            resource,
            horizon,
        } => {
            let cities: Vec<(i32, i32)> = board
                .cities
                .iter()
                .filter(|c| &c.owner == player)
                .map(|c| (c.x, c.y))
                .collect();
            if cities.is_empty() {
                return None; // ill-posed: player has no cities
            }
            // Travel-cost bound (DESIGN.md §6): land reach in turns (settler-like move 1) from the
            // nearest city, within `horizon` turns. TACTICAL for `player` — enemy Zones of Control
            // and enemy-occupied / enemy-city blocking apply (parity with `travel_turns_from_owned`).
            // Unexploited = outside the Chebyshev-2 working radius of every own city (the work radius
            // stays a Chebyshev area, not a travel cost).
            let reach =
                rules::ReachField::from_origins_tactical(board, &cities, 1, *horizon, player);
            let best = board
                .iter_tiles()
                .filter(|t| t.has(resource))
                .filter(|t| {
                    cities
                        .iter()
                        .map(|&c| chebyshev((t.x, t.y), c))
                        .min()
                        .unwrap()
                        > 2
                })
                .filter_map(|t| reach.turns_at(t.x, t.y))
                .min();
            Some(Answer::OptionalInt(best.map(|d| d as i64)))
        }
        Question::SettleSiteChoice { player, candidates } => {
            if candidates.len() < 2 {
                return None;
            }
            let mut coords: Vec<(i32, i32)> = Vec::with_capacity(candidates.len());
            for c in candidates {
                coords.push(c.resolve(board)?);
            }
            // The graded threat field is board-level; build it once, then score each candidate.
            let field = rules::ThreatField::compute(board, player);
            let axes: Vec<rules::SiteAxes> = coords
                .iter()
                .map(|&p| rules::site_axes_with(board, p, &field))
                .collect();
            let options: Vec<String> = coords.iter().map(|&(x, y)| format!("({x}, {y})")).collect();
            // Acceptable = the non-dominated set (no other candidate Pareto-dominates it).
            let mut acceptable = Vec::new();
            for i in 0..coords.len() {
                let dominated = (0..coords.len()).any(|j| j != i && axes[j].dominates(&axes[i]));
                if !dominated {
                    acceptable.push(options[i].clone());
                }
            }
            // A useful instance needs at least one dominated (blunder) option; otherwise every pick
            // is sound and the question tests nothing.
            if acceptable.len() == options.len() {
                return None;
            }
            Some(Answer::ChoiceSet {
                acceptable,
                options,
            })
        }
        Question::RetreatChoice { unit, candidates } => {
            if candidates.len() < 2 {
                return None;
            }
            let u = unit.unit(board)?;
            let owner = u.owner.clone();
            let uid = u.id;
            let mut coords: Vec<(i32, i32)> = Vec::with_capacity(candidates.len());
            for c in candidates {
                coords.push(c.resolve(board)?);
            }
            // Threat field is board+owner-level; build once, then score each destination.
            let field = rules::ThreatField::compute(board, &owner);
            let axes: Vec<rules::RetreatAxes> = coords
                .iter()
                .map(|&p| rules::retreat_axes_with(board, p, &owner, uid, &field))
                .collect();
            let options: Vec<String> = coords.iter().map(|&(x, y)| format!("({x}, {y})")).collect();
            // Acceptable = the non-dominated set (no other candidate Pareto-dominates it).
            let mut acceptable = Vec::new();
            for i in 0..coords.len() {
                let dominated = (0..coords.len()).any(|j| j != i && axes[j].dominates(&axes[i]));
                if !dominated {
                    acceptable.push(options[i].clone());
                }
            }
            // A useful instance needs at least one dominated (blunder) retreat; otherwise every pick
            // is sound and the question tests nothing.
            if acceptable.len() == options.len() {
                return None;
            }
            Some(Answer::ChoiceSet {
                acceptable,
                options,
            })
        }
        Question::ForwardPostingChoice {
            player,
            unit,
            candidates,
        } => {
            if candidates.len() < 2 {
                return None;
            }
            let u = unit.unit(board)?;
            if &u.owner != player {
                return None; // the posting is the player's OWN unit
            }
            let stat = rules::unit_stat(&u.kind)?;
            if stat.class != rules::UnitClass::Land {
                return None; // a land advance (air/sea postings are out of model)
            }
            let mut coords: Vec<(i32, i32)> = Vec::with_capacity(candidates.len());
            for c in candidates {
                coords.push(c.resolve(board)?);
            }
            // Posting field is board+player-level; build once, then score each candidate posting.
            let field = rules::PostingField::compute(board, player);
            let mut axes: Vec<rules::PostingAxes> = Vec::with_capacity(coords.len());
            for &p in &coords {
                axes.push(field.axes(board, u, p)?);
            }
            let options: Vec<String> = coords.iter().map(|&(x, y)| format!("({x}, {y})")).collect();
            // Acceptable = the non-dominated set: a posting no other candidate DECISIVELY dominates on
            // BOTH axes (≥ on both, a decisive lead on ≥1). A decisively-dominated posting is a blunder.
            let mut acceptable = Vec::new();
            for i in 0..coords.len() {
                let dominated = (0..coords.len()).any(|j| {
                    j != i
                        && axes[j].decisively_dominates(
                            &axes[i],
                            POSTING_PRESSURE_MARGIN,
                            POSTING_SURVIVAL_MARGIN,
                        )
                });
                if !dominated {
                    acceptable.push(options[i].clone());
                }
            }
            // A useful instance needs at least one dominated (blunder) posting AND at least one sound
            // answer; otherwise every pick is sound and the question tests nothing.
            if acceptable.is_empty() || acceptable.len() == options.len() {
                return None;
            }
            Some(Answer::ChoiceSet {
                acceptable,
                options,
            })
        }
        Question::CityThreatChoice { player, candidates } => {
            if candidates.len() < 2 {
                return None;
            }
            // Resolve each candidate to one of the PLAYER's own cities with a distinct, non-empty
            // name (needed for a clean `Choice`/`ChoiceSet` over city names).
            let mut cities: Vec<&civ_core::City> = Vec::with_capacity(candidates.len());
            for c in candidates {
                let city = c.city(board)?;
                if &city.owner != player {
                    return None; // a threatened-city question is over the player's OWN cities
                }
                cities.push(city);
            }
            let labels: Vec<String> = cities.iter().map(|c| city_choice_label(c)).collect();
            let mut seen: BTreeSet<String> = BTreeSet::new();
            for l in &labels {
                if l.is_empty() || !seen.insert(l.clone()) {
                    return None; // need distinctly-named cities
                }
            }
            // SINGLE axis: the probability the city FALLS to its scariest incoming attacker
            // (`rules::city_capture_prob` — the ThreatField's scariest attacker vs. the city's best
            // defender). Build the field once (for the owner: enemies of `player` are the attackers),
            // then score each candidate's fall odds.
            let field = rules::ThreatField::compute(board, player);
            let fall: Vec<f64> = cities
                .iter()
                .map(|c| rules::city_capture_prob(board, c, &field))
                .collect();
            // The answer is the city MOST LIKELY to fall — admitted only if it clears the runner-up by
            // the decisive probability margin (else a near-tie in odds → drop, the honest cost of a
            // single un-litigable answer). Find the top-two fall probabilities.
            let mut order: Vec<usize> = (0..fall.len()).collect();
            order.sort_by(|&a, &b| {
                fall[b]
                    .partial_cmp(&fall[a])
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let (top, runner_up) = (order[0], order[1]);
            if fall[top] - fall[runner_up] < THREAT_FALL_MARGIN {
                return None; // no decisively most-threatened city
            }
            Some(Answer::Choice {
                value: labels[top].clone(),
                options: labels,
            })
        }
        Question::ReachableNearestResource {
            unit,
            resource,
            horizon,
        } => {
            let u = unit.unit(board)?;
            let stat = rules::unit_stat(&u.kind)?;
            if stat.class != rules::UnitClass::Land {
                return None; // a land-travel task; non-land units are ill-posed here
            }
            // TACTICAL for the unit's owner: enemy Zones of Control and enemy-occupied / enemy-city
            // blocking apply (parity with the `reach_turns` tool op).
            let reach = rules::ReachField::from_origins_tactical(
                board,
                &[(u.x, u.y)],
                stat.move_rate,
                *horizon,
                &u.owner,
            );
            let best = board
                .iter_tiles()
                .filter(|t| t.has(resource))
                .filter_map(|t| reach.turns_at(t.x, t.y))
                .min();
            Some(Answer::OptionalInt(best.map(|d| d as i64)))
        }
        Question::CanVacateCity { city } => {
            let c = city.city(board)?;
            // The counterfactual reads the CURRENT garrison (best defender first), then judges the
            // odds with the best defender REMOVED.
            let ranked = rules::defenders_ranked(board, c);
            let (best_u, best_def) = *ranked.first()?; // no garrison → ill-posed
            let field = rules::ThreatField::compute(board, &c.owner);
            field.attacker_at(c.x, c.y)?; // no live threat this turn → trivially safe to vacate, drop
                                          // Presuppose the city is currently secure with its full garrison (the scariest attacker
                                          // clearly LOSES to the best defender), so the question is purely the counterfactual of
                                          // removing that defender — not "is it already lost".
            let fall_with_best = rules::city_fall_prob(&field, c.x, c.y, best_u, best_def);
            if fall_with_best > rules::WIN_PROB_LOW {
                return None; // not decisively secure now → presupposition fails, drop
            }
            // With the best defender pulled, the second-best defends (or the city is undefended).
            let fall_without = match ranked.get(1) {
                Some(&(u2, d2)) => rules::city_fall_prob(&field, c.x, c.y, u2, d2),
                None => rules::city_fall_prob_undefended(&field, c.x, c.y),
            };
            if fall_without <= rules::WIN_PROB_LOW {
                Some(Answer::Bool(true)) // remaining garrison decisively holds → can vacate
            } else if fall_without >= rules::WIN_PROB_HIGH {
                Some(Answer::Bool(false)) // vacated city decisively falls → cannot
            } else {
                None // near-tie band → non-decisive, drop
            }
        }
        Question::AssaultTargetChoice { player, candidates } => {
            if candidates.len() < 2 {
                return None;
            }
            let mut cities: Vec<&civ_core::City> = Vec::with_capacity(candidates.len());
            for c in candidates {
                let city = c.city(board)?;
                if &city.owner != player {
                    return None; // targets are the player's OWN cities
                }
                cities.push(city);
            }
            let labels: Vec<String> = cities.iter().map(|c| city_choice_label(c)).collect();
            let mut seen: BTreeSet<String> = BTreeSet::new();
            for l in &labels {
                if l.is_empty() || !seen.insert(l.clone()) {
                    return None; // need distinctly-named cities
                }
            }
            // Two orthogonal attractiveness axes: capture-win probability (↑ = softer) and city value
            // = size (↑ = a richer prize). Acceptable = the attractiveness frontier: a city NOT
            // decisively dominated on BOTH axes by any other. A city another candidate decisively
            // dominates (≥ on both, a real lead on ≥1) is a blunder pick.
            let field = rules::ThreatField::compute(board, player);
            let axes: Vec<rules::AssaultAxes> = cities
                .iter()
                .map(|c| rules::assault_axes(board, c, &field))
                .collect();
            let mut acceptable = Vec::new();
            for i in 0..axes.len() {
                let dominated_by_some = (0..axes.len())
                    .any(|j| j != i && axes[j].decisively_dominates(&axes[i], THREAT_FALL_MARGIN));
                if !dominated_by_some {
                    acceptable.push(labels[i].clone());
                }
            }
            // Need at least one blunder (a decisively-worse target) AND at least one sound answer.
            if acceptable.is_empty() || acceptable.len() == labels.len() {
                return None;
            }
            Some(Answer::ChoiceSet {
                acceptable,
                options: labels,
            })
        }
        Question::TriageReinforceChoice {
            player,
            reserve,
            candidates,
        } => {
            if candidates.len() < 2 {
                return None;
            }
            let d = reserve.unit(board)?;
            if &d.owner != player {
                return None; // the spare defender is the player's own
            }
            let dstat = rules::unit_stat(&d.kind)?;
            if dstat.class != rules::UnitClass::Land {
                return None; // a garrison is a land defender
            }
            let mut cities: Vec<&civ_core::City> = Vec::with_capacity(candidates.len());
            for c in candidates {
                let city = c.city(board)?;
                if &city.owner != player {
                    return None;
                }
                cities.push(city);
            }
            let labels: Vec<String> = cities.iter().map(|c| city_choice_label(c)).collect();
            let mut seen: BTreeSet<String> = BTreeSet::new();
            for l in &labels {
                if l.is_empty() || !seen.insert(l.clone()) {
                    return None;
                }
            }
            let field = rules::ThreatField::compute(board, player);
            // A city is a SOUND allocation only if the reserve FLIPS it from capturable to held.
            // Any candidate whose state (before OR after) is a near-tie makes the whole instance
            // non-decisive → drop, so every kept instance scores cleanly.
            let mut acceptable = Vec::new();
            for (i, c) in cities.iter().enumerate() {
                // BEFORE: the odds the scariest attacker beats the city's current best defender.
                let before_fall = match rules::best_defender(board, c) {
                    Some((u, def)) => rules::city_fall_prob(&field, c.x, c.y, u, def),
                    None => rules::city_fall_prob_undefended(&field, c.x, c.y),
                };
                // AFTER: garrisoning the reserve makes the defender whichever is stronger, the
                // current best defender or the reserve on this city tile.
                let reserve_def = rules::def_eff_if_garrisoned(d, board, c)?;
                let cur = rules::best_defender(board, c);
                let cur_def = cur.map(|(_, def)| def).unwrap_or(0.0);
                let after_fall = if reserve_def >= cur_def {
                    rules::city_fall_prob(&field, c.x, c.y, d, reserve_def)
                } else {
                    // Safe: reserve_def < cur_def implies a current defender exists.
                    let (u, def) = cur.unwrap();
                    rules::city_fall_prob(&field, c.x, c.y, u, def)
                };
                let before = capture_state(before_fall);
                let after = capture_state(after_fall);
                match (before, after) {
                    (CaptureState::Capturable, CaptureState::Held) => {
                        acceptable.push(labels[i].clone())
                    }
                    (CaptureState::Capturable, CaptureState::Capturable) => {} // doomed → wrong
                    (CaptureState::Held, _) => {} // already safe → wrong
                    _ => return None,             // any Contested (near-tie) → non-decisive, drop
                }
            }
            if acceptable.is_empty() || acceptable.len() == labels.len() {
                return None;
            }
            Some(Answer::ChoiceSet {
                acceptable,
                options: labels,
            })
        }
        Question::CompareTwoAttacks {
            attacker_a,
            target_a,
            attacker_b,
            target_b,
        } => {
            let aa = attacker_a.unit(board)?;
            let ta = target_a.unit(board)?;
            let ab = attacker_b.unit(board)?;
            let tb = target_b.unit(board)?;
            // Both attacks belong to the same attacking player; both targets are enemy units.
            if aa.owner != ab.owner || ta.owner == aa.owner || tb.owner == ab.owner {
                return None;
            }
            let la = format!("({}, {})", ta.x, ta.y);
            let lb = format!("({}, {})", tb.x, tb.y);
            if la == lb {
                return None; // need distinct target labels
            }
            let ax = rules::attack_axes(board, aa, ta)?;
            let bx = rules::attack_axes(board, ab, tb)?;
            let value = if ax.decisively_dominates(&bx, T2_MARGIN) {
                la.clone()
            } else if bx.decisively_dominates(&ax, T2_MARGIN) {
                lb.clone()
            } else if ax.genuine_tradeoff(&bx, T2_MARGIN) {
                INCOMPARABLE.to_string()
            } else {
                return None; // neither a decisive dominance nor a genuine trade-off → near-tie, drop
            };
            Some(Answer::Compare3 {
                value,
                a: la,
                b: lb,
            })
        }
        Question::ConstraintSiteChoice {
            player,
            center,
            radius,
            spacing,
        } => {
            let c = center.resolve(board)?;
            // The search space is the region: every LAND tile within `radius` (Chebyshev) of the
            // center that is not an existing city (you cannot found on one). These are the offered
            // options — the model must PERCEIVE them off the board, not iterate a handed shortlist.
            let city_set: std::collections::HashSet<(i32, i32)> =
                board.cities.iter().map(|c| (c.x, c.y)).collect();
            let region_land: Vec<(i32, i32)> = civ_core::geometry::tiles_within(board, c, *radius)
                .into_iter()
                .filter(|&p| is_land(&board.tile(p.0, p.1).terrain) && !city_set.contains(&p))
                .collect();
            // A region with too few settleable tiles is not a real search; drop it.
            if region_land.len() < 2 {
                return None;
            }
            // The acceptable set = every in-region land tile meeting ALL FOUR constraints (the three
            // tool-visible predicates AND the non-bundled spacing rule). If empty, the sole
            // acceptable answer is the literal "none". Scorer is board-free, so precompute both.
            let satisfying: Vec<String> = region_land
                .iter()
                .filter(|&&p| rules::constraint_site_ok_spaced(board, player, p, *spacing))
                .map(|&(x, y)| format!("({x}, {y})"))
                .collect();
            // Decisive only if at least one in-region tile FAILS the conjunction — a real wrong pick.
            // (If every tile satisfies, any pick is correct and the item tests nothing; drop it.)
            if satisfying.len() == region_land.len() {
                return None;
            }
            // Options are every in-region land tile PLUS the real "none" escape, so that picking
            // "none" when a tile qualifies (and picking a tile when the answer is "none") are both
            // scoreable; a tile outside the region (or water) is not offered → invalid.
            let mut options: Vec<String> = region_land
                .iter()
                .map(|&(x, y)| format!("({x}, {y})"))
                .collect();
            options.push("none".to_string());
            let acceptable = if satisfying.is_empty() {
                vec!["none".to_string()]
            } else {
                satisfying
            };
            Some(Answer::ChoiceSet {
                acceptable,
                options,
            })
        }
        Question::FoggedAssault {
            player,
            attackers,
            city,
        } => {
            // NOTE: `board` here is the perspective's MASKED board (the generator solves P7-fogged on
            // what the model sees), so the label is a pure function of OBSERVABLES. We do NOT score
            // against the true hidden garrison (that would make the yes/no luck: a city that happens to
            // hold only a Caravan would score "yes" though a rational fog-reasoner correctly says
            // "no"). Instead we score against a WORST-CASE-ROBUST synthetic garrison: one green unit of
            // the owner's strongest VISIBLE defender type ([`rules::worst_case_garrison`], scanning the
            // masked board → the OBSERVED repertoire, so a type that exists only on fogged tiles is
            // never assumed), sitting in the city with its last-known terrain/center/walls context.
            let c = city.city(board)?;
            if &c.owner == player {
                return None; // the target is an ENEMY city
            }
            // The attacking stack: the player's OWN land units, all currently able to strike the
            // city (poised at Chebyshev 2..=3 — the snapshot-honest "this turn" storm).
            let mut stack: Vec<&civ_core::Unit> = Vec::with_capacity(attackers.len());
            for a in attackers {
                let u = a.unit(board)?;
                if &u.owner != player {
                    return None;
                }
                let st = rules::unit_stat(&u.kind)?;
                if st.class != rules::UnitClass::Land {
                    return None; // a land storm
                }
                // Poised near the fogged city (not adjacent — adjacency would reveal the garrison).
                // The strike's reachability is granted (magic placement); only capturability is judged.
                if !(2..=3).contains(&chebyshev((u.x, u.y), (c.x, c.y))) {
                    return None;
                }
                stack.push(u);
            }
            if stack.is_empty() {
                return None;
            }
            // Capture probability against the synthetic worst-case garrison. If the owner fields no
            // modeled combat unit anywhere, the worst case is an EMPTY city — undefended, so the
            // assault captures for certain (p = 1). With a single defender the capture prob is
            // attacker-order-independent, so no ordering bracket is needed.
            let p = match rules::worst_case_garrison(board, c) {
                Some((synthetic, def_power)) => {
                    rules::assault_capture_prob_vs(&stack, &[(&synthetic, def_power)])
                }
                None => rules::assault_capture_prob_vs(&stack, &[]),
            };
            if p >= rules::WIN_PROB_HIGH {
                Some(Answer::Bool(true)) // beats the worst-case defender → robustly takeable
            } else if p <= rules::WIN_PROB_LOW {
                Some(Answer::Bool(false)) // loses to the worst-case defender → robustly holds
            } else {
                None // middle band → non-decisive, drop
            }
        }
        // P1 / P3: the answer depends on masked-out occupants AND the perspective's fog, i.e. on BOTH
        // the unmasked and masked boards — it cannot be computed from a single board here, so it is
        // solved in the generator (see `surprise_strike_answer` / `hidden_force_answer`). A
        // single-board `solve` is ill-posed for them and returns None.
        Question::SurpriseStrikeExposure { .. } | Question::HiddenForceLocalization { .. } => None,
    }
}

/// The decisive capture state of a city this turn from the COMBAT ODDS, applying the
/// [`rules::WIN_PROB_HIGH`]/[`rules::WIN_PROB_LOW`] band the same way the 1.25× margin was applied to
/// the old strength ratio: `Capturable` when the scariest attacker's win probability is high (the
/// city falls, undefended cities under live threat included), `Held` when it is low (the defense
/// holds), and `Contested` for the near-tie band in between. Used by `triage-reinforce` to judge
/// whether a reinforcement FLIPS the outcome; a `Contested` classification makes the instance
/// non-decisive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureState {
    Held,
    Capturable,
    Contested,
}

fn capture_state(fall_prob: f64) -> CaptureState {
    if fall_prob >= rules::WIN_PROB_HIGH {
        CaptureState::Capturable // the attacker decisively wins → the city falls
    } else if fall_prob <= rules::WIN_PROB_LOW {
        CaptureState::Held // the attacker decisively loses (or no threat) → the city holds
    } else {
        CaptureState::Contested
    }
}

/// The decisive winner of two strengths, or `None` if the instance must be dropped: both zero,
/// or within the `T2_MARGIN` ratio (a near-tie). On success the strengths are strictly ordered,
/// so the returned label is unambiguous.
fn decisive_winner(sa: f64, sb: f64, la: &str, lb: &str) -> Option<String> {
    let (hi, lo, hi_label) = if sa >= sb { (sa, sb, la) } else { (sb, sa, lb) };
    if hi <= EPS {
        return None; // both effectively zero
    }
    if hi < T2_MARGIN * lo {
        return None; // too close to call decisively (also covers exact ties)
    }
    Some(hi_label.to_string())
}

/// Enemy occupancy, friendly units/cities and enemy Zone-of-Control tiles for `mover`, used to make
/// the [`reachable`] walk TACTICAL (same rules as [`rules::ReachField::from_origins_tactical`]).
struct TacticalSets {
    enemy_block: std::collections::HashSet<(i32, i32)>, // enemy unit tiles OR enemy cities (no entry)
    friendly_exempt: std::collections::HashSet<(i32, i32)>, // friendly city / friendly unit (ZOC exception)
    zoc: std::collections::HashSet<(i32, i32)>,             // tiles under enemy military ZOC
}

impl TacticalSets {
    fn build(board: &Board, mover: &str) -> TacticalSets {
        use std::collections::HashSet;
        let mut enemy_block: HashSet<(i32, i32)> = HashSet::new();
        let mut friendly_exempt: HashSet<(i32, i32)> = HashSet::new();
        let mut zoc: HashSet<(i32, i32)> = HashSet::new();
        for u in &board.units {
            if u.owner == mover {
                friendly_exempt.insert((u.x, u.y));
                continue;
            }
            enemy_block.insert((u.x, u.y));
            // Only enemy LAND MILITARY units (attack > 0) exert a zone of control.
            if rules::unit_stat(&u.kind)
                .is_some_and(|s| s.class == rules::UnitClass::Land && s.attack > 0.0)
            {
                for (_, (nx, ny)) in geometry::neighbors(board, u.x, u.y) {
                    zoc.insert((nx, ny));
                }
            }
        }
        for c in &board.cities {
            if c.owner == mover {
                friendly_exempt.insert((c.x, c.y));
            } else {
                enemy_block.insert((c.x, c.y));
            }
        }
        TacticalSets {
            enemy_block,
            friendly_exempt,
            zoc,
        }
    }
}

/// Breadth-first reachability on the 8-neighbor grid, forbidding entry into `avoid` terrain and
/// into any non-land (water/unexplored) tile — a land unit cannot cross Ocean/Lake. When
/// `mover` is `Some(player)` the walk is TACTICAL for that player: it also cannot enter an
/// enemy-occupied tile or an enemy city, and it obeys classic enemy Zones of Control — a step from
/// `A` to `B` is forbidden when BOTH lie in some enemy ZOC, unless `B` is a friendly city / holds a
/// friendly unit. `None` keeps the pure-terrain walk.
fn reachable(
    board: &Board,
    start: (i32, i32),
    goal: (i32, i32),
    budget: i32,
    avoid: &str,
    mover: Option<&str>,
) -> bool {
    if board.tile(goal.0, goal.1).terrain == avoid {
        return false; // cannot end on a forbidden tile
    }
    if !is_land(&board.tile(goal.0, goal.1).terrain) {
        return false; // a land unit cannot end on water (Ocean/Lake/unexplored)
    }
    let tac = mover.map(|p| TacticalSets::build(board, p));
    // A tactical walk cannot end ON an enemy unit or enemy city either.
    if let Some(t) = &tac {
        if t.enemy_block.contains(&goal) {
            return false;
        }
    }
    if start == goal {
        return true;
    }
    let w = board.width as usize;
    let mut dist = vec![-1i32; w * board.height as usize];
    let idx = |x: i32, y: i32| (y as usize) * w + x as usize;
    let mut queue = std::collections::VecDeque::new();
    dist[idx(start.0, start.1)] = 0;
    queue.push_back(start);
    while let Some((x, y)) = queue.pop_front() {
        let d = dist[idx(x, y)];
        if d >= budget {
            continue;
        }
        let a_in_zoc = tac.as_ref().is_some_and(|t| t.zoc.contains(&(x, y)));
        for (_, (nx, ny)) in geometry::neighbors(board, x, y) {
            if board.tile(nx, ny).terrain == avoid {
                continue;
            }
            // Water is impassable to a land unit regardless of the tactical/`mover` setting: the
            // walk can never ENTER an Ocean/Lake (or unexplored) tile. This mirrors every other
            // movement construct in the codebase, all of which restrict to `is_land`.
            if !is_land(&board.tile(nx, ny).terrain) {
                continue;
            }
            if let Some(t) = &tac {
                // Cannot step onto an enemy unit / enemy city.
                if t.enemy_block.contains(&(nx, ny)) {
                    continue;
                }
                // Classic ZOC: no step between two enemy-controlled tiles, unless the destination is
                // a friendly city / already holds a friendly unit.
                if a_in_zoc && t.zoc.contains(&(nx, ny)) && !t.friendly_exempt.contains(&(nx, ny)) {
                    continue;
                }
            }
            let i = idx(nx, ny);
            if dist[i] == -1 {
                dist[i] = d + 1;
                if (nx, ny) == goal {
                    return true;
                }
                queue.push_back((nx, ny));
            }
        }
    }
    false
}

/// A question bundled with its computed ground truth — a frozen dataset row, ready to pose
/// and grade. Category/tier are denormalized for convenient result output.
#[derive(Debug, Clone)]
pub struct EvalItem {
    pub id: String,
    pub category: &'static str,
    pub tier: Tier,
    pub question: Question,
    pub answer: Answer,
    pub board_source: String,
}

impl EvalItem {
    pub fn new(question: Question, answer: Answer, board_source: String) -> Self {
        EvalItem {
            id: question.id(),
            category: question.category(),
            tier: question.tier(),
            question,
            answer,
            board_source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use civ_core::board::{City, Player, Tile, Unit};

    /// A 14x14 all-Grassland board with one city "Home" (owner "p") at (5, 5) and the given
    /// resource tiles placed at their coordinates.
    fn board_with(resources: &[(i32, i32, &str)]) -> Board {
        let mut tiles = Vec::new();
        for y in 0..14 {
            let mut row = Vec::new();
            for x in 0..14 {
                let mut extras = BTreeSet::new();
                for &(rx, ry, name) in resources {
                    if (rx, ry) == (x, y) {
                        extras.insert(name.to_string());
                    }
                }
                row.push(Tile {
                    x,
                    y,
                    terrain: "Grassland".to_string(),
                    extras,
                    owner: None,
                });
            }
            tiles.push(row);
        }
        Board {
            width: 14,
            height: 14,
            tiles,
            cities: vec![City {
                x: 5,
                y: 5,
                id: 1,
                name: "Home".to_string(),
                owner: "p".to_string(),
                size: 3,
                improvements: BTreeSet::new(),
            }],
            units: vec![],
            players: vec![Player {
                id: 0,
                name: "p".to_string(),
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
    fn nearest_owned_picks_closest_unexploited_in_range() {
        // City at (5,5). Gold at (9,5) is Chebyshev 4 (unexploited, in range); Gold at (7,5)
        // is Chebyshev 2 (exploited → excluded). Answer must be the distance-4 tile.
        let b = board_with(&[(9, 5, "Gold"), (7, 5, "Gold")]);
        let q = Question::NearestOwnedResource {
            player: "p".to_string(),
            resource: "Gold".to_string(),
            horizon: 6,
        };
        assert_eq!(solve(&q, &b), Some(Answer::OptionalInt(Some(4))));
    }

    #[test]
    fn nearest_owned_only_exploited_is_none() {
        // The single Gold tile is within the 2-tile work radius (exploited) → no unexploited
        // resource in range → None.
        let b = board_with(&[(7, 5, "Gold")]);
        let q = Question::NearestOwnedResource {
            player: "p".to_string(),
            resource: "Gold".to_string(),
            horizon: 6,
        };
        assert_eq!(solve(&q, &b), Some(Answer::OptionalInt(None)));
    }

    #[test]
    fn nearest_owned_beyond_horizon_is_none() {
        // Silk at (13,5) is Chebyshev 8 from the city — outside the horizon of 6 → None.
        let b = board_with(&[(13, 5, "Silk")]);
        let q = Question::NearestOwnedResource {
            player: "p".to_string(),
            resource: "Silk".to_string(),
            horizon: 6,
        };
        assert_eq!(solve(&q, &b), Some(Answer::OptionalInt(None)));
    }

    #[test]
    fn nearest_owned_absent_resource_is_none() {
        // No Iron anywhere → None (in range, not ill-posed: the player has a city).
        let b = board_with(&[(9, 5, "Gold")]);
        let q = Question::NearestOwnedResource {
            player: "p".to_string(),
            resource: "Iron".to_string(),
            horizon: 6,
        };
        assert_eq!(solve(&q, &b), Some(Answer::OptionalInt(None)));
    }

    /// The `board_with` grid but with the city removed and one `unit_kind` unit (id 7, owner "p")
    /// placed at `(ux, uy)` — for the reachable-nearest (scout) solver.
    fn board_with_unit(unit_kind: &str, ux: i32, uy: i32, resources: &[(i32, i32, &str)]) -> Board {
        let mut b = board_with(resources);
        b.cities.clear();
        b.units.push(Unit {
            x: ux,
            y: uy,
            id: 7,
            kind: unit_kind.to_string(),
            owner: "p".to_string(),
            veteran: 0,
            hp: 10,
        });
        b
    }

    /// A small board where every tile is Grassland except a set of `ocean` tiles (marked "Ocean").
    /// No cities, no units — a pure-terrain walk board for the [`reachable`] oracle.
    fn ocean_board(w: i32, h: i32, ocean: &[(i32, i32)]) -> Board {
        let mut tiles = Vec::new();
        for y in 0..h {
            let mut row = Vec::new();
            for x in 0..w {
                let terrain = if ocean.contains(&(x, y)) {
                    "Ocean"
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
        Board {
            width: w,
            height: h,
            tiles,
            cities: vec![],
            units: vec![],
            players: vec![],
            known: std::collections::HashMap::new(),
            visibility: None,
            ruleset: None,
            turn: None,
            source: "ocean-mini".to_string(),
        }
    }

    #[test]
    fn reachable_cannot_cross_water_channel() {
        // A 5x5 board split by a full-height 1-tile-wide OCEAN channel at column x=2. Origin (1,2)
        // and goal (3,2) sit on opposite shores of the strait, same row, no Mountains. A land unit
        // cannot cross ocean, so no path exists at ANY budget → the oracle must return false. (Under
        // the pre-fix oracle the BFS waded straight across and returned true — the validity bug.)
        let channel: Vec<(i32, i32)> = (0..5).map(|y| (2, y)).collect();
        let b = ocean_board(5, 5, &channel);
        assert!(
            !reachable(&b, (1, 2), (3, 2), 10, "Mountains", None),
            "a land unit must not be able to cross a full water channel"
        );
        // The goal itself being water is likewise unreachable (goal guard).
        assert!(
            !reachable(&b, (1, 2), (2, 2), 10, "Mountains", None),
            "a land goal on an ocean tile is unreachable"
        );
    }

    #[test]
    fn reachable_land_detour_around_water_succeeds() {
        // The same strait, but with a single land bridge at (2,0): now a land detour exists. Path
        // (1,2)->(1,1)->(2,0)->(3,1)->(3,2) is 4 diagonal-and-orthogonal steps → reachable within a
        // budget of 6, but NOT within a budget of 3 (the detour is longer than the blocked 2-step
        // straight line). Control that the water guard doesn't over-block a genuinely land-connected goal.
        let channel: Vec<(i32, i32)> = (1..5).map(|y| (2, y)).collect(); // ocean at (2,1..4); (2,0) is land
        let b = ocean_board(5, 5, &channel);
        assert!(
            reachable(&b, (1, 2), (3, 2), 6, "Mountains", None),
            "a land detour over the (2,0) bridge should reach the far shore within budget"
        );
        assert!(
            !reachable(&b, (1, 2), (3, 2), 3, "Mountains", None),
            "the detour is too long for a budget of 3"
        );
    }

    #[test]
    fn reachable_nearest_scales_with_move_rate() {
        // Warriors (move 1) at (5,5), Gold at (9,5) [Chebyshev 4, all land] → 4 turns.
        let b = board_with_unit("Warriors", 5, 5, &[(9, 5, "Gold")]);
        let q = Question::ReachableNearestResource {
            unit: Referent::Unit { id: 7 },
            resource: "Gold".to_string(),
            horizon: 6,
        };
        assert_eq!(solve(&q, &b), Some(Answer::OptionalInt(Some(4))));
        // Chariot (move 2) reaches the same tile in ceil(4/2) = 2 turns.
        let b2 = board_with_unit("Chariot", 5, 5, &[(9, 5, "Gold")]);
        assert_eq!(solve(&q, &b2), Some(Answer::OptionalInt(Some(2))));
        // Silk at (13,5) is 8 turns for Warriors → beyond the horizon of 6 → none.
        let b3 = board_with_unit("Warriors", 5, 5, &[(13, 5, "Silk")]);
        let q3 = Question::ReachableNearestResource {
            unit: Referent::Unit { id: 7 },
            resource: "Silk".to_string(),
            horizon: 6,
        };
        assert_eq!(solve(&q3, &b3), Some(Answer::OptionalInt(None)));
    }

    /// A 14x1 Grassland strip with Hills at the given x's, player "A"'s Warriors (id 7) at
    /// `unit_x`, and an enemy "B" Legion (id 9) at `legion_x`. For the t3-retreat solver/scorer.
    fn retreat_board(hills: &[i32], unit_x: i32, legion_x: i32) -> Board {
        let mut row = Vec::new();
        for x in 0..14 {
            let terrain = if hills.contains(&x) {
                "Hills"
            } else {
                "Grassland"
            };
            row.push(Tile {
                x,
                y: 0,
                terrain: terrain.to_string(),
                extras: BTreeSet::new(),
                owner: None,
            });
        }
        Board {
            width: 14,
            height: 1,
            tiles: vec![row],
            cities: vec![],
            units: vec![
                Unit {
                    x: unit_x,
                    y: 0,
                    id: 7,
                    kind: "Warriors".to_string(),
                    owner: "A".to_string(),
                    veteran: 0,
                    hp: 10,
                },
                Unit {
                    x: legion_x,
                    y: 0,
                    id: 9,
                    kind: "Legion".to_string(),
                    owner: "B".to_string(),
                    veteran: 0,
                    hp: 10,
                },
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
            source: "retreat".to_string(),
        }
    }

    #[test]
    fn retreat_scores_sound_correct_and_dominated_wrong() {
        use crate::scoring::{score, Status};
        // Warriors at x=6, Legion (attack 4, move 1) at x=1. Hills at x=3 (threatened, but the
        // UNIQUE max-cover tile among candidates -> sound) and x=10 (unreachable). The graded threat
        // field fades with distance; x=9 is beyond the 6-turn horizon -> threat 0 (safe).
        let b = retreat_board(&[3, 10], 6, 1);
        let candidates = vec![
            Referent::Tile { x: 3, y: 0 }, // Hills, threatened, unique cover -> sound
            Referent::Tile { x: 4, y: 0 }, // Grassland, threatened -> dominated blunder
            Referent::Tile { x: 5, y: 0 }, // Grassland, threatened -> dominated blunder
            Referent::Tile { x: 9, y: 0 }, // Grassland, safe -> sound
        ];
        let q = Question::RetreatChoice {
            unit: Referent::Unit { id: 7 },
            candidates,
        };
        let ans = solve(&q, &b).expect("retreat instance should be admissible");
        let Answer::ChoiceSet {
            acceptable,
            options,
        } = &ans
        else {
            panic!("expected a ChoiceSet answer");
        };
        assert_eq!(options.len(), 4);
        assert!(
            acceptable.contains(&"(9, 0)".to_string()),
            "safe tile must be sound: {acceptable:?}"
        );
        assert!(
            acceptable.contains(&"(3, 0)".to_string()),
            "unique-cover tile must be sound: {acceptable:?}"
        );
        assert!(
            !acceptable.contains(&"(4, 0)".to_string()),
            "threatened Grassland must be dominated"
        );
        // POSITIVE: a sound retreat scores correct.
        assert_eq!(score(&ans, "Answer: (9, 0)").status, Status::Correct);
        // NEGATIVE (mandatory): a dominated retreat scores wrong.
        assert_eq!(score(&ans, "Answer: (4, 0)").status, Status::Wrong);
        // A tile that was never offered -> invalid (not wrong).
        assert_eq!(score(&ans, "Answer: (0, 0)").status, Status::Invalid);
    }

    /// A 13x13 all-Grassland board for the forward-posting (P4) solver/scorer. Player "A" has one
    /// Warriors (id 7) at (6, 6). Enemy "B" holds two UNDEFENDED cities — "West1" at (3, 6) and
    /// "West2" at (3, 4) — that give pressure, and one punisher Armor (att 10, move 3, id 9) at
    /// (4, 2) that can reposition to strike some postings but not others.
    fn posting_board() -> Board {
        let mut tiles = Vec::new();
        for y in 0..13 {
            let mut row = Vec::new();
            for x in 0..13 {
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
        Board {
            width: 13,
            height: 13,
            tiles,
            cities: vec![
                City {
                    x: 3,
                    y: 6,
                    id: 30,
                    name: "West1".to_string(),
                    owner: "B".to_string(),
                    size: 2,
                    improvements: BTreeSet::new(),
                },
                City {
                    x: 3,
                    y: 4,
                    id: 31,
                    name: "West2".to_string(),
                    owner: "B".to_string(),
                    size: 2,
                    improvements: BTreeSet::new(),
                },
            ],
            units: vec![
                Unit {
                    x: 6,
                    y: 6,
                    id: 7,
                    kind: "Warriors".to_string(),
                    owner: "A".to_string(),
                    veteran: 0,
                    hp: 10,
                },
                Unit {
                    x: 4,
                    y: 2,
                    id: 9,
                    kind: "Armor".to_string(),
                    owner: "B".to_string(),
                    veteran: 0,
                    hp: 30,
                },
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
            source: "posting".to_string(),
        }
    }

    #[test]
    fn forward_posting_scores_sound_correct_and_dominated_wrong() {
        use crate::scoring::{score, Status};
        // Warriors (id 7) at (6, 6) can advance up to 2 tiles. Three candidate postings:
        //  - (4, 6): adjacent to West1 only  → pressure 1.0; the Armor can reposition to (4, 5) and
        //            cave it in → survival ~0. A PRESSURE TRAP.
        //  - (4, 7): adjacent to West1 only  → pressure 1.0 (TIES the trap); no enemy can reach a tile
        //            next to it in one turn → survival 1.0. The SAFE TWIN — dominates the trap purely
        //            on survival.
        //  - (4, 5): adjacent to BOTH West1 and West2 → pressure 2.0; the Armor punishes it too →
        //            survival ~0. A genuine trade-off vs the twin (more pressure, far less survival).
        let b = posting_board();
        let candidates = vec![
            Referent::Tile { x: 4, y: 6 }, // trap → dominated blunder
            Referent::Tile { x: 4, y: 7 }, // safe twin → sound
            Referent::Tile { x: 4, y: 5 }, // aggressive frontier point → sound
        ];
        let q = Question::ForwardPostingChoice {
            player: "A".to_string(),
            unit: Referent::Unit { id: 7 },
            candidates,
        };
        let ans = solve(&q, &b).expect("forward-posting instance should be admissible");
        let Answer::ChoiceSet {
            acceptable,
            options,
        } = &ans
        else {
            panic!("expected a ChoiceSet answer");
        };
        assert_eq!(options.len(), 3);
        assert!(
            acceptable.contains(&"(4, 7)".to_string()),
            "equal-pressure safer twin must be sound: {acceptable:?}"
        );
        assert!(
            acceptable.contains(&"(4, 5)".to_string()),
            "higher-pressure trade-off must be sound: {acceptable:?}"
        );
        assert!(
            !acceptable.contains(&"(4, 6)".to_string()),
            "the punishable equal-pressure trap must be dominated: {acceptable:?}"
        );
        // POSITIVE: a sound advance scores correct.
        assert_eq!(score(&ans, "Answer: (4, 7)").status, Status::Correct);
        assert_eq!(score(&ans, "Answer: (4, 5)").status, Status::Correct);
        // NEGATIVE (mandatory): the decisively-dominated posting scores wrong.
        assert_eq!(score(&ans, "Answer: (4, 6)").status, Status::Wrong);
        // A tile that was never offered → invalid (not wrong).
        assert_eq!(score(&ans, "Answer: (0, 0)").status, Status::Invalid);
    }

    #[test]
    fn forward_posting_all_sound_is_dropped() {
        // Offer only the two non-dominated postings (the safe twin and the aggressive trade-off):
        // neither dominates the other, so every pick is sound → the instance tests nothing → dropped.
        let b = posting_board();
        let q = Question::ForwardPostingChoice {
            player: "A".to_string(),
            unit: Referent::Unit { id: 7 },
            candidates: vec![Referent::Tile { x: 4, y: 7 }, Referent::Tile { x: 4, y: 5 }],
        };
        assert!(
            solve(&q, &b).is_none(),
            "no dominated blunder among the candidates → dropped"
        );
    }

    /// A 20x1 Grassland strip for the t3-threat solver/scorer: player "A" owns three cities and
    /// enemy "B" has one Legion (attack 4, move 1) at `legion_x`. Cities: `(x, name, garrison?)`.
    fn threat_board(cities: &[(i32, &str, bool)], legion_x: i32) -> Board {
        let mut row = Vec::new();
        for x in 0..20 {
            row.push(Tile {
                x,
                y: 0,
                terrain: "Grassland".to_string(),
                extras: BTreeSet::new(),
                owner: None,
            });
        }
        let mut city_vec = Vec::new();
        let mut unit_vec = vec![Unit {
            x: legion_x,
            y: 0,
            id: 99,
            kind: "Legion".to_string(),
            owner: "B".to_string(),
            veteran: 0,
            hp: 10,
        }];
        for (i, &(cx, name, garrison)) in cities.iter().enumerate() {
            let id = 10 + i as i32;
            city_vec.push(City {
                x: cx,
                y: 0,
                id,
                name: name.to_string(),
                owner: "A".to_string(),
                size: 2,
                improvements: BTreeSet::new(),
            });
            if garrison {
                unit_vec.push(Unit {
                    x: cx,
                    y: 0,
                    id: 20 + i as i32,
                    kind: "Warriors".to_string(),
                    owner: "A".to_string(),
                    veteran: 0,
                    hp: 10,
                });
            }
        }
        Board {
            width: 20,
            height: 1,
            tiles: vec![row],
            cities: city_vec,
            units: unit_vec,
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
            source: "threat".to_string(),
        }
    }

    #[test]
    fn city_threat_picks_decisive_dominator_and_scores() {
        use crate::scoring::{score, Status};
        // "Exposed" at x=2 is undefended (def 0) and sits next to the Legion (full incoming 4).
        // "Safe1"/"Safe2" are garrisoned (def 1.5) and far beyond the Legion's 6-turn reach
        // (incoming 0). Exposed is thus decisively more threatened on BOTH axes than each of them.
        let b = threat_board(
            &[
                (2, "Exposed", false),
                (15, "Safe1", true),
                (18, "Safe2", true),
            ],
            4,
        );
        let candidates = vec![
            Referent::City { id: 10 }, // Exposed  → the unique dominator (soft + attacked)
            Referent::City { id: 11 }, // Safe1
            Referent::City { id: 12 }, // Safe2
        ];
        let q = Question::CityThreatChoice {
            player: "A".to_string(),
            candidates,
        };
        let ans = solve(&q, &b).expect("threat instance should be admissible");
        let Answer::Choice { value, options } = &ans else {
            panic!("expected a Choice answer");
        };
        assert_eq!(options.len(), 3);
        assert_eq!(value, "Exposed", "most-likely-to-fall must be Exposed");
        // POSITIVE: the most-threatened city scores correct.
        assert_eq!(score(&ans, "Answer: Exposed").status, Status::Correct);
        // NEGATIVE (mandatory): a not-most-threatened city scores wrong.
        assert_eq!(score(&ans, "Answer: Safe1").status, Status::Wrong);
        // A city that was never offered → invalid (not wrong).
        assert_eq!(score(&ans, "Answer: Nowhere").status, Status::Invalid);
    }

    #[test]
    fn city_threat_no_decisive_dominator_is_dropped() {
        // Two equally-exposed, equally-undefended cities (both incoming 4, both def 0 → both fall
        // probability 1.0) plus a safe one: the top two fall probabilities tie, so no city clears the
        // runner-up by THREAT_FALL_MARGIN and the instance is dropped — the honest cost of a single
        // un-litigable answer.
        let b = threat_board(
            &[(2, "SoftA", false), (6, "SoftB", false), (18, "Safe", true)],
            4,
        );
        let candidates = vec![
            Referent::City { id: 10 },
            Referent::City { id: 11 },
            Referent::City { id: 12 },
        ];
        let q = Question::CityThreatChoice {
            player: "A".to_string(),
            candidates,
        };
        assert!(solve(&q, &b).is_none());
    }

    #[test]
    fn nearest_owned_no_cities_is_ill_posed() {
        // A player with no cities → ill-posed → None (rejected by the generator).
        let b = board_with(&[(9, 5, "Gold")]);
        let q = Question::NearestOwnedResource {
            player: "ghost".to_string(),
            resource: "Gold".to_string(),
            horizon: 6,
        };
        assert_eq!(solve(&q, &b), None);
    }

    // --- maximal-calculator kinds --------------------------------------------

    /// A `width`x1 all-Grassland strip for the maximal-calculator kinds. Cities are
    /// `(x, name, owner)`; units are `(id, x, kind, owner)` placed at full (max-hitpoint) health so
    /// `health(u) = 1.0`. Players are derived from the distinct owners.
    fn mc_strip(
        width: i32,
        cities: &[(i32, &str, &str)],
        units: &[(i32, i32, &str, &str)],
    ) -> Board {
        let mut row = Vec::new();
        for x in 0..width {
            row.push(Tile {
                x,
                y: 0,
                terrain: "Grassland".to_string(),
                extras: BTreeSet::new(),
                owner: None,
            });
        }
        let city_vec: Vec<City> = cities
            .iter()
            .enumerate()
            .map(|(i, &(x, name, owner))| City {
                x,
                y: 0,
                id: 100 + i as i32,
                name: name.to_string(),
                owner: owner.to_string(),
                size: 2,
                improvements: BTreeSet::new(),
            })
            .collect();
        let unit_vec: Vec<Unit> = units
            .iter()
            .map(|&(id, x, kind, owner)| {
                let hp = rules::unit_stat(kind)
                    .map(|s| s.hitpoints as i32)
                    .unwrap_or(10);
                Unit {
                    x,
                    y: 0,
                    id,
                    kind: kind.to_string(),
                    owner: owner.to_string(),
                    veteran: 0,
                    hp,
                }
            })
            .collect();
        let mut owners: BTreeSet<String> = BTreeSet::new();
        owners.extend(city_vec.iter().map(|c| c.owner.clone()));
        owners.extend(unit_vec.iter().map(|u| u.owner.clone()));
        let players = owners
            .into_iter()
            .enumerate()
            .map(|(i, name)| Player {
                id: i as i32,
                name,
                nation: String::new(),
                is_alive: true,
            })
            .collect();
        Board {
            width,
            height: 1,
            tiles: vec![row],
            cities: city_vec,
            units: unit_vec,
            players,
            known: std::collections::HashMap::new(),
            visibility: None,
            ruleset: None,
            turn: None,
            source: "mc".to_string(),
        }
    }

    #[test]
    fn cf_vacate_scores_yes_and_no_with_negative() {
        use crate::scoring::{score, Status};
        // City "Fort" (A) at x=2 next to an enemy Legion (attack 4, move 1) at x=3 → incoming 4.
        // NO case: a lone Alpine Troops (def 5) garrison → def_eff 5 x 1.5 (city center) = 7.5 holds
        // now (>= 1.25 x 4), but with it pulled the city has 0 defense → it falls this turn.
        let b = mc_strip(
            6,
            &[(2, "Fort", "A")],
            &[(9, 3, "Legion", "B"), (7, 2, "Alpine Troops", "A")],
        );
        let q = Question::CanVacateCity {
            city: Referent::City { id: 100 },
        };
        assert_eq!(
            solve(&q, &b),
            Some(Answer::Bool(false)),
            "one defender → cannot vacate"
        );
        let ans = solve(&q, &b).unwrap();
        // POSITIVE: the correct "no" scores correct.
        assert_eq!(score(&ans, "Answer: no").status, Status::Correct);
        // NEGATIVE (mandatory): the wrong "yes" scores wrong (not merely invalid).
        assert_eq!(score(&ans, "Answer: yes").status, Status::Wrong);

        // YES case: a SECOND Alpine Troops → the remaining defender (7.5) still holds when the best
        // is pulled, so the city can spare one for a turn.
        let b2 = mc_strip(
            6,
            &[(2, "Fort", "A")],
            &[
                (9, 3, "Legion", "B"),
                (7, 2, "Alpine Troops", "A"),
                (8, 2, "Alpine Troops", "A"),
            ],
        );
        let q2 = Question::CanVacateCity {
            city: Referent::City { id: 100 },
        };
        assert_eq!(
            solve(&q2, &b2),
            Some(Answer::Bool(true)),
            "two defenders → can vacate"
        );
    }

    #[test]
    fn compare_two_attacks_dominance_incomparable_and_negative() {
        use crate::scoring::{score, Status};
        // Favorability is now the WIN PROBABILITY (the HP/firepower odds), not a raw att/def ratio,
        // so the axes are built from real kill chances. A owns an Armor (att 10) at x=0 and a weak
        // Warriors (att 1) at x=1; enemy B has Warriors (def 1, att 1) at x=5, a Cannon (def 1, att
        // 8) at x=6, and an Armor (def 5, att 10, 30 hp) at x=7, all on open ground.
        let b = mc_strip(
            9,
            &[],
            &[
                (1, 0, "Armor", "A"),
                (3, 1, "Warriors", "A"),
                (10, 5, "Warriors", "B"),
                (11, 6, "Cannon", "B"),
                (12, 7, "Armor", "B"),
            ],
        );
        // DOMINANCE: Armor→Warriors (win-prob ≈1, threat 1) vs Armor→Cannon (win-prob ≈1, threat 8).
        // Both kills are near-certain (favorability ties), but the Cannon strike removes far more
        // threat → it decisively dominates.
        let q = Question::CompareTwoAttacks {
            attacker_a: Referent::Unit { id: 1 },
            target_a: Referent::Unit { id: 10 },
            attacker_b: Referent::Unit { id: 1 },
            target_b: Referent::Unit { id: 11 },
        };
        let ans = solve(&q, &b).expect("decisive dominance instance");
        assert_eq!(
            ans,
            Answer::Compare3 {
                value: "(6, 0)".into(),
                a: "(5, 0)".into(),
                b: "(6, 0)".into()
            }
        );
        assert_eq!(score(&ans, "Answer: (6, 0)").status, Status::Correct);
        // NEGATIVE (mandatory): naming the dominated attack is wrong; so is calling it incomparable.
        assert_eq!(score(&ans, "Answer: (5, 0)").status, Status::Wrong);
        assert_eq!(score(&ans, "Answer: incomparable").status, Status::Wrong);

        // INCOMPARABLE: Armor→Warriors (win-prob ≈1, threat 1) vs Warriors→Armor (win-prob ≈0 — a
        // weak unit swinging at a tough, high-HP Armor — but threat 10). Each attack decisively wins
        // a DIFFERENT axis (A the exchange, B the scalp) → a genuine trade-off, no invented weighting.
        let q2 = Question::CompareTwoAttacks {
            attacker_a: Referent::Unit { id: 1 },
            target_a: Referent::Unit { id: 10 },
            attacker_b: Referent::Unit { id: 3 },
            target_b: Referent::Unit { id: 12 },
        };
        let ans2 = solve(&q2, &b).expect("genuine trade-off instance");
        assert_eq!(
            ans2,
            Answer::Compare3 {
                value: INCOMPARABLE.into(),
                a: "(5, 0)".into(),
                b: "(7, 0)".into()
            }
        );
        assert_eq!(score(&ans2, "Answer: incomparable").status, Status::Correct);
        // NEGATIVE: picking a side on a genuine trade-off is wrong.
        assert_eq!(score(&ans2, "Answer: (5, 0)").status, Status::Wrong);
    }

    #[test]
    fn triage_reinforce_flips_marginal_city_only() {
        use crate::scoring::{score, Status};
        // A owns three cities and a spare Alpine Troops (def 5, id 50) at x=20. Enemies: an Armor
        // (att 10, move 3) at x=1 and a Legion (att 4, move 1) at x=13.
        //  - "Doomed"  at x=2  (undefended): incoming 10; even +Alpine (def_eff 7.5) stays capturable.
        //  - "Savable" at x=14 (undefended): incoming 4; +Alpine (7.5 >= 1.25x4) FLIPS it to held.
        //  - "Safe"    at x=28 (Warriors):   beyond both enemies' reach → incoming 0, already held.
        let b = mc_strip(
            30,
            &[(2, "Doomed", "A"), (14, "Savable", "A"), (28, "Safe", "A")],
            &[
                (1, 1, "Armor", "B"),
                (2, 13, "Legion", "B"),
                (50, 20, "Alpine Troops", "A"),
                (60, 28, "Warriors", "A"),
            ],
        );
        let q = Question::TriageReinforceChoice {
            player: "A".to_string(),
            reserve: Referent::Unit { id: 50 },
            candidates: vec![
                Referent::City { id: 100 }, // Doomed
                Referent::City { id: 101 }, // Savable
                Referent::City { id: 102 }, // Safe
            ],
        };
        let ans = solve(&q, &b).expect("triage instance should be admissible");
        let Answer::ChoiceSet {
            acceptable,
            options,
        } = &ans
        else {
            panic!("expected ChoiceSet")
        };
        assert_eq!(options.len(), 3);
        assert_eq!(
            acceptable,
            &vec!["Savable".to_string()],
            "only the flip-able city is sound"
        );
        assert_eq!(score(&ans, "Answer: Savable").status, Status::Correct);
        // The MOST-threatened city is a blunder (doomed anyway), and so is the least (already safe).
        assert_eq!(score(&ans, "Answer: Doomed").status, Status::Wrong);
        assert_eq!(score(&ans, "Answer: Safe").status, Status::Wrong);
    }

    #[test]
    fn assault_target_frontier_and_negative() {
        use crate::scoring::{score, Status};
        // A owns three cities; two enemy Legions sit next to two of them.
        //  - "Soft"     at x=2  next to Legion1 (x=1): incoming 4, undefended.
        //  - "AlsoSoft" at x=14 next to Legion2 (x=13): incoming 4, undefended — ties Soft.
        //  - "Tough"    at x=28 (Warriors garrison): incoming 0, defended → a decisively worse target.
        let b = mc_strip(
            30,
            &[(2, "Soft", "A"), (14, "AlsoSoft", "A"), (28, "Tough", "A")],
            &[
                (1, 1, "Legion", "B"),
                (2, 13, "Legion", "B"),
                (60, 28, "Warriors", "A"),
            ],
        );
        let q = Question::AssaultTargetChoice {
            player: "A".to_string(),
            candidates: vec![
                Referent::City { id: 100 }, // Soft
                Referent::City { id: 101 }, // AlsoSoft
                Referent::City { id: 102 }, // Tough
            ],
        };
        let ans = solve(&q, &b).expect("assault-target instance should be admissible");
        let Answer::ChoiceSet { acceptable, .. } = &ans else {
            panic!("expected ChoiceSet")
        };
        assert!(acceptable.contains(&"Soft".to_string()));
        assert!(acceptable.contains(&"AlsoSoft".to_string()));
        assert!(
            !acceptable.contains(&"Tough".to_string()),
            "the defended, quiet city is a poor target"
        );
        // POSITIVE: a frontier target scores correct.
        assert_eq!(score(&ans, "Answer: Soft").status, Status::Correct);
        // NEGATIVE: the decisively-worse target scores wrong.
        assert_eq!(score(&ans, "Answer: Tough").status, Status::Wrong);
    }

    #[test]
    fn assault_target_size_axis_creates_frontier() {
        use crate::scoring::{score, Status};
        // The value axis (city size) makes a big-but-safe city and a small-but-soft city a genuine
        // trade-off, both on the frontier — where a single capture axis would have ranked one clearly
        // above the other. Only the enemy "B" Legion at (1,0) threatens anything.
        //  - "SmallSoft" (x=2,  size 2): undefended next to the Legion → capture 1.0.
        //  - "BigSafe"   (x=28, size 8): far out of reach → capture 0.0, but the biggest prize.
        //    → neither dominates the other: both SOUND.
        //  - "SmallSafe" (x=40, size 2): capture 0.0 AND small → decisively dominated by BOTH → wrong.
        let mut b = mc_strip(
            50,
            &[
                (2, "SmallSoft", "A"),
                (28, "BigSafe", "A"),
                (40, "SmallSafe", "A"),
            ],
            &[(1, 1, "Legion", "B")],
        );
        for c in &mut b.cities {
            c.size = if c.name == "BigSafe" { 8 } else { 2 };
        }
        let q = Question::AssaultTargetChoice {
            player: "A".to_string(),
            candidates: vec![
                Referent::City { id: 100 }, // SmallSoft
                Referent::City { id: 101 }, // BigSafe
                Referent::City { id: 102 }, // SmallSafe
            ],
        };
        let ans = solve(&q, &b).expect("size-frontier instance should be admissible");
        let Answer::ChoiceSet { acceptable, .. } = &ans else {
            panic!("expected ChoiceSet")
        };
        assert!(acceptable.contains(&"SmallSoft".to_string()));
        assert!(
            acceptable.contains(&"BigSafe".to_string()),
            "the big prize is on the frontier even though it is safer"
        );
        assert!(
            !acceptable.contains(&"SmallSafe".to_string()),
            "small AND safe is dominated on both axes"
        );
        // POSITIVE: the big-but-safe prize is a sound target (a single capture axis would reject it).
        assert_eq!(score(&ans, "Answer: BigSafe").status, Status::Correct);
        // NEGATIVE: the small, safe city is decisively worse on both axes.
        assert_eq!(score(&ans, "Answer: SmallSafe").status, Status::Wrong);
    }

    // --- constraint-site (predicate-composition / intersection survivor) -----

    /// An 8x8 Grassland grid for the constraint-site solver/scorer. Column x=0 is Ocean (so a tile
    /// with x <= 2 is "within 2 of water"); the listed tiles are Hills (defensible); tile (2, 5) is
    /// inside enemy ("B") territory. Player "A" owns a capital at (5, 0). The resulting predicate
    /// classes for player "A":
    ///   (2, 1) Hills, near water, unowned  → satisfies ALL THREE
    ///   (1, 6) Hills, near water, unowned  → satisfies ALL THREE
    ///   (1, 3) Grassland, near water       → near-miss: NOT defensible
    ///   (6, 6) Hills, inland               → near-miss: NOT near water
    ///   (6, 2) Hills, inland               → near-miss: NOT near water
    ///   (2, 5) Hills, near water, ENEMY    → near-miss: in enemy territory
    fn constraint_board() -> Board {
        let hills: &[(i32, i32)] = &[(2, 1), (1, 6), (6, 6), (6, 2), (2, 5)];
        let mut tiles = Vec::new();
        for y in 0..8 {
            let mut row = Vec::new();
            for x in 0..8 {
                let terrain = if x == 0 {
                    "Ocean"
                } else if hills.contains(&(x, y)) {
                    "Hills"
                } else {
                    "Grassland"
                };
                let owner = if (x, y) == (2, 5) {
                    Some("B".to_string())
                } else {
                    None
                };
                row.push(Tile {
                    x,
                    y,
                    terrain: terrain.to_string(),
                    extras: BTreeSet::new(),
                    owner,
                });
            }
            tiles.push(row);
        }
        Board {
            width: 8,
            height: 8,
            tiles,
            cities: vec![City {
                x: 5,
                y: 0,
                id: 1,
                name: "Cap".to_string(),
                owner: "A".to_string(),
                size: 2,
                improvements: BTreeSet::new(),
            }],
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

    /// A 4-wide (x=0 Ocean, so x<=2 is "within 2 of water"; x=3 is inland) x 6-tall all-Hills grid,
    /// every tile unowned, with a single city "Cap" at (2, 1) owned by "A". Used for the spacing
    /// tests: a coastal Hills tile close to that city passes the three tool-visible predicates but
    /// FAILS the min-spacing rule, so spacing (and only spacing) changes the answer.
    fn spacing_board() -> Board {
        let mut tiles = Vec::new();
        for y in 0..6 {
            let mut row = Vec::new();
            for x in 0..4 {
                let terrain = if x == 0 { "Ocean" } else { "Hills" };
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
        Board {
            width: 4,
            height: 6,
            tiles,
            cities: vec![City {
                x: 2,
                y: 1,
                id: 1,
                name: "Cap".to_string(),
                owner: "A".to_string(),
                size: 2,
                improvements: BTreeSet::new(),
            }],
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
            source: "spacing".to_string(),
        }
    }

    #[test]
    fn constraint_site_some_case_scores() {
        use crate::scoring::{score, Status};
        let b = constraint_board();
        // A region (center (2, 2), radius 2) whose ONLY all-four survivor is the Hills tile (2, 1);
        // every other in-region land tile fails at least one requirement.
        let q = Question::ConstraintSiteChoice {
            player: "A".to_string(),
            center: Referent::Tile { x: 2, y: 2 },
            radius: 2,
            spacing: 3,
        };
        let ans = solve(&q, &b).expect("some-case should be admissible");
        let Answer::ChoiceSet {
            acceptable,
            options,
        } = &ans
        else {
            panic!("expected ChoiceSet")
        };
        assert_eq!(
            acceptable,
            &vec!["(2, 1)".to_string()],
            "only the all-satisfying tile is a valid site"
        );
        assert!(
            options.contains(&"none".to_string()),
            "the none escape must be offered"
        );
        assert!(
            options.contains(&"(2, 1)".to_string()) && options.contains(&"(1, 3)".to_string()),
            "options enumerate the region's land tiles"
        );
        // POSITIVE: the survivor scores correct.
        assert_eq!(score(&ans, "Answer: (2, 1)").status, Status::Correct);
        // NEGATIVE (mandatory): an in-region tile that fails a requirement is a wrong pick, not
        // merely invalid. (1, 3) is Grassland → not defensible.
        assert_eq!(score(&ans, "Answer: (1, 3)").status, Status::Wrong);
        // NEGATIVE (mandatory): saying "none" when a tile qualifies is wrong.
        assert_eq!(score(&ans, "Answer: none").status, Status::Wrong);
        // A tile OUTSIDE the region → invalid (never offered).
        assert_eq!(score(&ans, "Answer: (7, 7)").status, Status::Invalid);
    }

    #[test]
    fn constraint_site_none_case_scores() {
        use crate::scoring::{score, Status};
        let b = constraint_board();
        // A region (center (6, 4), radius 2) of inland Hills + open Grassland: no tile is BOTH
        // defensible AND within 2 of water, so no tile satisfies all four → "none" is correct.
        let q = Question::ConstraintSiteChoice {
            player: "A".to_string(),
            center: Referent::Tile { x: 6, y: 4 },
            radius: 2,
            spacing: 3,
        };
        let ans = solve(&q, &b).expect("none-case should be admissible");
        let Answer::ChoiceSet { acceptable, .. } = &ans else {
            panic!("expected ChoiceSet")
        };
        assert_eq!(
            acceptable,
            &vec!["none".to_string()],
            "the region holds no satisfying tile"
        );
        // POSITIVE: certifying the empty region scores correct.
        assert_eq!(score(&ans, "Answer: none").status, Status::Correct);
        // NEGATIVE (mandatory): picking an in-region tile when the answer is "none" is wrong.
        assert_eq!(score(&ans, "Answer: (6, 6)").status, Status::Wrong); // inland Hills, fails water
        assert_eq!(score(&ans, "Answer: (6, 2)").status, Status::Wrong);
        // A tile outside the region → invalid.
        assert_eq!(score(&ans, "Answer: (0, 0)").status, Status::Invalid);
    }

    #[test]
    fn constraint_site_all_satisfy_is_dropped() {
        // On the all-Hills coastal `spacing_board`, a region far from the city whose every land
        // tile meets all four requirements has no wrong pick → not decisive → dropped.
        let b = spacing_board();
        let q = Question::ConstraintSiteChoice {
            player: "A".to_string(),
            center: Referent::Tile { x: 1, y: 5 },
            radius: 1, // tiles (1,4),(2,4),(1,5),(2,5) — all Hills, coastal, unowned, spaced >= 3
            spacing: 3,
        };
        assert!(
            solve(&q, &b).is_none(),
            "every in-region tile is a valid site → no clear wrong pick → dropped"
        );
    }

    #[test]
    fn constraint_site_spacing_predicate() {
        // The pure spacing predicate: distance from the nearest city, Chebyshev.
        let b = constraint_board(); // sole city at (5, 0)
        assert!(!rules::is_spaced_from_cities(&b, (5, 0), 3), "on a city → distance 0");
        assert!(!rules::is_spaced_from_cities(&b, (3, 1), 3), "Chebyshev 2 < 3 → too close");
        assert!(rules::is_spaced_from_cities(&b, (2, 1), 3), "Chebyshev 3 >= 3 → spaced");
        assert!(!rules::is_spaced_from_cities(&b, (2, 1), 4), "Chebyshev 3 < 4 → too close");
    }

    #[test]
    fn constraint_site_spacing_changes_the_answer() {
        use crate::scoring::{score, Status};
        // On `spacing_board`, the coastal Hills tile (2, 3) passes ALL THREE tool-visible predicates
        // but sits only Chebyshev 2 from the city at (2, 1) → the un-tooled spacing rule disqualifies
        // it. This is the whole point of B: `site_check` would report ok=yes, yet the site is invalid.
        let b = spacing_board();
        assert!(
            rules::constraint_site_ok(&b, "A", (2, 3)),
            "(2, 3) passes defensible + water + not-enemy (what site_check reports)"
        );
        assert!(
            !rules::constraint_site_ok_spaced(&b, "A", (2, 3), 3),
            "...but FAILS spacing (Chebyshev 2 from the city at (2, 1))"
        );
        // A region that contains BOTH the disqualified-by-spacing tile (2, 3) and properly spaced
        // survivors like (1, 4): the survivors are acceptable, (2, 3) is a WRONG pick, not acceptable.
        let q = Question::ConstraintSiteChoice {
            player: "A".to_string(),
            center: Referent::Tile { x: 1, y: 3 },
            radius: 2,
            spacing: 3,
        };
        let ans = solve(&q, &b).expect("mixed region should be admissible");
        let Answer::ChoiceSet { acceptable, .. } = &ans else {
            panic!("expected ChoiceSet")
        };
        assert!(
            !acceptable.contains(&"(2, 3)".to_string()),
            "spacing removes (2, 3) from the acceptable set"
        );
        assert!(
            acceptable.contains(&"(1, 4)".to_string()),
            "a properly spaced coastal Hills tile is acceptable"
        );
        // (2, 3) is offered (in-region land) but not acceptable → a wrong pick, not invalid.
        assert_eq!(score(&ans, "Answer: (2, 3)").status, Status::Wrong);
        assert_eq!(score(&ans, "Answer: (1, 4)").status, Status::Correct);
    }
}
