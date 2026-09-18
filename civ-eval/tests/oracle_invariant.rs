//! The harness self-consistency invariant: the Oracle model must score 100% on every generated
//! question under every encoding. If this fails, an AnswerSpec/extractor disagrees with its
//! Answer type — a bug caught with zero API spend (DESIGN.md §6). Also guards determinism.

use civ_core::parse_save;
use civ_eval::{generate_questions, run_oracle, Answer, Difficulty, Status, V1_ENCODERS};

fn save_path() -> String {
    format!(
        "{}/../data/saves/myagent_T50.sav",
        env!("CARGO_MANIFEST_DIR")
    )
}

#[test]
fn oracle_scores_100_percent() {
    let board = parse_save(save_path()).expect("parse save");
    let encodings: Vec<String> = V1_ENCODERS.iter().map(|s| s.to_string()).collect();

    // The invariant must hold at every difficulty — the knobs change question parameters, not
    // the generation → prompt → extraction → scoring contract.
    for difficulty in [Difficulty::easy(), Difficulty::hard()] {
        let items = generate_questions(&board, 1, 12, difficulty, None);
        assert!(!items.is_empty(), "no questions generated");

        let rows = run_oracle(&board, &encodings, &items, None);
        assert_eq!(rows.len(), items.len() * encodings.len());

        let bad: Vec<_> = rows
            .iter()
            .filter(|r| r.status != Status::Correct)
            .collect();
        assert!(
            bad.is_empty(),
            "{} trials not correct at {difficulty:?}; first: {:?}",
            bad.len(),
            bad.first()
                .map(|r| (&r.encoding, &r.item_id, &r.expected, &r.got))
        );
    }
}

#[test]
fn generation_is_deterministic() {
    let board = parse_save(save_path()).expect("parse save");
    let a = generate_questions(&board, 42, 10, Difficulty::hard(), None);
    let b = generate_questions(&board, 42, 10, Difficulty::hard(), None);
    let ids_a: Vec<_> = a.iter().map(|i| i.id.clone()).collect();
    let ids_b: Vec<_> = b.iter().map(|i| i.id.clone()).collect();
    assert_eq!(ids_a, ids_b, "same seed must produce the same question set");
}

#[test]
fn every_category_present() {
    let board = parse_save(save_path()).expect("parse save");
    let items = generate_questions(&board, 1, 12, Difficulty::hard(), None);
    let cats: std::collections::BTreeSet<_> = items.iter().map(|i| i.category).collect();
    for expected in [
        "terrain",
        "adjacency",
        "direction",
        "distance",
        "nearest",
        "region-count",
        "reachability",
        "unit-strength", // T2a
        "city-defense",  // T2b
        "best-site",     // T1 siting
    ] {
        assert!(cats.contains(expected), "missing category {expected}");
    }
}

/// The region-search constraint-site kind (Variant C) must reliably produce BOTH a satisfiable
/// region (some in-region tile qualifies) AND a "none" region (the region holds no valid site) at
/// each difficulty — the some/none split the kind is built to guarantee. If it degenerated to
/// all-none or all-some the item would test nothing. (Oracle correctness on these is covered by
/// `oracle_scores_100_percent`, which generates every kind.)
#[test]
fn constraint_site_produces_both_some_and_none() {
    let board = parse_save(save_path()).expect("parse save");
    for difficulty in [Difficulty::easy(), Difficulty::hard()] {
        let items = generate_questions(&board, 7, 24, difficulty, None);
        let cs: Vec<_> = items
            .iter()
            .filter(|i| i.category == "constraint-site")
            .collect();
        assert!(
            !cs.is_empty(),
            "constraint-site produced no items at {difficulty:?}"
        );
        let none_n = cs
            .iter()
            .filter(|it| {
                matches!(&it.answer, Answer::ChoiceSet { acceptable, .. }
                    if acceptable.len() == 1 && acceptable[0] == "none")
            })
            .count();
        let some_n = cs.len() - none_n;
        assert!(
            none_n > 0 && some_n > 0,
            "constraint-site must mix some/none at {difficulty:?}: {some_n} some, {none_n} none"
        );
        // Every kept item is a real region search: its options enumerate several in-region land
        // tiles plus the "none" escape (not a 4-tile shortlist).
        for it in &cs {
            if let Answer::ChoiceSet { options, .. } = &it.answer {
                assert!(
                    options.contains(&"none".to_string()) && options.len() >= 5,
                    "a region item should enumerate the region + none, got {} options",
                    options.len()
                );
            }
        }
    }
}

/// The T2 valuation categories must be present *and* carry the T2 tier end-to-end (so the runner
/// wires their `rules_block` and the Oracle still scores them 100%, exercised by the invariant
/// above). Guards the tier plumbing the rules block hangs off of.
#[test]
fn t2_categories_are_tier_t2() {
    use civ_eval::Tier;
    let board = parse_save(save_path()).expect("parse save");
    let items = generate_questions(&board, 1, 12, Difficulty::hard(), None);
    for it in items
        .iter()
        .filter(|i| matches!(i.category, "unit-strength" | "city-defense"))
    {
        assert_eq!(it.tier, Tier::T2, "{} should be T2", it.category);
    }
}

/// The Oracle-100% invariant must also hold on a three-state FOGGED board (`--fog`): masking to a
/// player's perspective introduces `Unknown` terrain, `(fogged)` remembered tiles, and hidden enemy
/// units — all of which must flow cleanly through generation → prompt → extraction → scoring, on the
/// SAME masked board the solvers read. Uses player 1 (more explored → more questions at T50).
#[test]
fn oracle_100_on_fogged_board() {
    let board = parse_save(save_path())
        .expect("parse save")
        .mask_to_known(1)
        .expect("fog player 1");
    // The board really is a three-state fog view (guards against silently testing an omniscient one).
    assert!(
        board.visibility.is_some(),
        "masked board carries visibility"
    );
    assert!(
        board
            .tiles
            .iter()
            .enumerate()
            .flat_map(|(y, r)| (0..r.len()).map(move |x| (x as i32, y as i32)))
            .any(|(x, y)| board.is_fogged(x, y)),
        "some tiles are fogged"
    );
    let perspective = perspective_name(&board, 1);
    assert_fogged_oracle_100(&board, 8, &perspective);
}

/// The fogged invariant on the DENSE, contested T677 board, from a LIVE-CIV perspective (player 0,
/// Testcontroller) — where thousands of enemy units become fog-hidden. This is the board with real
/// fog-sensitive combat/threat/assault content, so it is the load-bearing re-lock.
#[test]
fn oracle_100_on_fogged_t677() {
    let path = format!(
        "{}/../data/saves/testcontroller_T677.sav",
        env!("CARGO_MANIFEST_DIR")
    );
    let board = parse_save(path)
        .expect("parse T677")
        .mask_to_known(0)
        .expect("fog player 0 (Testcontroller, a live civ)");
    assert!(board.visibility.is_some(), "masked T677 carries visibility");
    let perspective = perspective_name(&board, 0);
    assert_fogged_oracle_100(&board, 6, &perspective);
}

/// Resolve a masked board's perspective player NAME from its id (the fog "self"). Mirrors the CLI's
/// `fog_perspective_name`, so the fogged oracle gate exercises the §9-coherent generation path
/// (player-relative kinds pinned to the fog player).
fn perspective_name(board: &civ_core::Board, id: i32) -> String {
    board
        .players
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| panic!("no player id {id} on board"))
}

/// Shared assertion: the Oracle scores 100% across the V1 encoders on a masked board at both tiers,
/// generating with the fog perspective so player-relative items are coherent (§9).
fn assert_fogged_oracle_100(board: &civ_core::Board, per_kind: usize, perspective: &str) {
    let encodings: Vec<String> = V1_ENCODERS.iter().map(|s| s.to_string()).collect();
    for difficulty in [Difficulty::easy(), Difficulty::hard()] {
        let items = generate_questions(board, 1, per_kind, difficulty, Some(perspective));
        assert!(
            !items.is_empty(),
            "no questions generated on the fogged board at {difficulty:?}"
        );
        let rows = run_oracle(board, &encodings, &items, None);
        let bad: Vec<_> = rows
            .iter()
            .filter(|r| r.status != Status::Correct)
            .collect();
        assert!(
            bad.is_empty(),
            "{} trials not correct on fogged board at {difficulty:?}; first: {:?}",
            bad.len(),
            bad.first()
                .map(|r| (&r.encoding, &r.item_id, &r.expected, &r.got))
        );
    }
}
