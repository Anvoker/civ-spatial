//! Fog perspective ↔ question-`player` COHERENCE (`fog-three-state-design.md` §9).
//!
//! When the board is fogged to player P (`--fog P`), every *player-relative* question kind must
//! generate for exactly P, so the item's decision-maker ("you are player X") matches whose sight the
//! board is masked to. Sight is computed around P; a question about a different player M would reason
//! over a map masked to N's vision (M's own units could even be fog-hidden). This proves the
//! generation-time restriction that enforces it, and that the perspective-agnostic terrain/geometry
//! kinds are untouched.
//!
//! The three properties share ONE unfogged + ONE pinned generation (T677 is a large, threat-dense
//! board; a full generation is costly), so they live in a single test. The masked+perspective path
//! (fogging then generating) is exercised end-to-end by `oracle_invariant.rs`'s fogged gates, which
//! now pass the perspective — so it is not re-run here.

use std::collections::BTreeSet;

use civ_core::{parse_save, Board};
use civ_eval::{generate_questions, Difficulty, EvalItem, Question, Referent};

fn t677() -> Board {
    let path = format!(
        "{}/../data/saves/testcontroller_T677.sav",
        env!("CARGO_MANIFEST_DIR")
    );
    parse_save(path).expect("parse T677")
}

fn player_name(board: &Board, id: i32) -> String {
    board
        .players
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| panic!("no player id {id}"))
}

/// The owner-of-record ("you") of a *player-relative* item, or `None` for a perspective-agnostic
/// kind. The seven player-relative kinds are exactly those whose "you" §9 pins to the fog player:
/// five carry the owner NAME directly; `t3-retreat` and `cf-vacate` carry a unit/city whose owner is
/// the "you". `constraint-site` is deliberately EXCLUDED — §9 classifies it perspective-agnostic
/// (fog is a perception mask over terrain, with no player semantics), so it is left alone.
fn player_relative_owner(item: &EvalItem, board: &Board) -> Option<String> {
    match &item.question {
        Question::SettleSiteChoice { player, .. }
        | Question::CityThreatChoice { player, .. }
        | Question::AssaultTargetChoice { player, .. }
        | Question::TriageReinforceChoice { player, .. }
        | Question::NearestOwnedResource { player, .. } => Some(player.clone()),
        Question::RetreatChoice {
            unit: Referent::Unit { id },
            ..
        } => board
            .units
            .iter()
            .find(|u| u.id == *id)
            .map(|u| u.owner.clone()),
        Question::CanVacateCity {
            city: Referent::City { id },
        } => board
            .cities
            .iter()
            .find(|c| c.id == *id)
            .map(|c| c.owner.clone()),
        _ => None,
    }
}

/// Proves, on the contested T677 board from a live-civ perspective (player 0, Testcontroller):
///   (a) under a fog perspective P, EVERY generated player-relative item's "you" is P;
///   (b) the unfogged path (`None`) still spans MULTIPLE owners — no regression;
///   (c) a perspective-agnostic terrain kind is unaffected by the perspective.
/// The restriction is a generation-time filter, so it is validated on the unmasked board (which
/// guarantees P has player-relative content, making (a) non-vacuous). Small `per_kind` keeps the
/// two full generations affordable while still yielding many player-relative items.
#[test]
fn fog_perspective_coherence() {
    let board = t677();
    let p = player_name(&board, 0);
    let per_kind = 5;

    let unfogged = generate_questions(&board, 1, per_kind, Difficulty::hard(), None);
    let pinned = generate_questions(&board, 1, per_kind, Difficulty::hard(), Some(&p));

    // (a) every player-relative item under perspective P is about P (and there is at least one).
    let pinned_owners: Vec<String> = pinned
        .iter()
        .filter_map(|it| player_relative_owner(it, &board))
        .collect();
    assert!(
        !pinned_owners.is_empty(),
        "expected some player-relative items for perspective {p:?}; got none (would be vacuous)"
    );
    for (it, owner) in pinned
        .iter()
        .filter_map(|it| player_relative_owner(it, &board).map(|o| (it, o)))
    {
        assert_eq!(
            owner, p,
            "player-relative item {} is about {owner:?}, not the fog perspective {p:?}",
            it.id
        );
    }

    // (b) no-regression: unfogged generation spans multiple distinct owners.
    let unfogged_owners: BTreeSet<String> = unfogged
        .iter()
        .filter_map(|it| player_relative_owner(it, &board))
        .collect();
    assert!(
        unfogged_owners.len() >= 2,
        "unfogged generation should span multiple owners (found {}): {unfogged_owners:?}",
        unfogged_owners.len()
    );

    // (c) a perspective-agnostic terrain kind is byte-identical with vs. without a perspective.
    let terrain_ids = |items: &[EvalItem]| -> Vec<String> {
        items
            .iter()
            .filter(|it| it.category == "terrain")
            .map(|it| it.id.clone())
            .collect()
    };
    let none_terrain = terrain_ids(&unfogged);
    assert!(!none_terrain.is_empty(), "expected some terrain items");
    assert_eq!(
        none_terrain,
        terrain_ids(&pinned),
        "terrain (perspective-agnostic) items must not change under a fog perspective"
    );
}
