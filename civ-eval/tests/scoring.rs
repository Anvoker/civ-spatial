//! Scorer unit tests — type-directed extraction, and the invalid-vs-wrong distinction.

use civ_core::Dir8;
use civ_eval::{score, Answer, Status};

fn choice(v: &str, opts: &[&str]) -> Answer {
    Answer::Choice {
        value: v.to_string(),
        options: opts.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn int_extraction_requires_an_answer_line() {
    // B2: the integer is read from an explicit `Answer:` line ONLY. Reasoning before it is fine
    // (the last integer on the answer line wins), but a reply with NO answer line is deliberately
    // NOT recovered from prose — even when the prose contains the correct number — because mining
    // surrounding text fabricated `got` values (a prompt radius, a trailing coordinate). Such a
    // reply is no-answer/invalid, not a (possibly fabricated) scored trial.
    let a = Answer::Int(3);
    assert_eq!(score(&a, "I think... Answer: 3").status, Status::Correct);
    // Was Correct under the old prose-recovery path; now invalid (no `Answer:` line). Deliberate.
    assert_eq!(
        score(&a, "The distance is 3 tiles.").status,
        Status::Invalid
    );
    assert_eq!(score(&a, "Answer: 4").status, Status::Wrong);
    assert_eq!(score(&a, "Answer: none").status, Status::Invalid);
}

#[test]
fn direction_parsing() {
    let a = Answer::Direction(Dir8::NE);
    assert_eq!(score(&a, "Answer: NE").status, Status::Correct);
    assert_eq!(
        score(&a, "it lies to the northeast.\nAnswer: northeast").status,
        Status::Correct
    );
    assert_eq!(score(&a, "Answer: SW").status, Status::Wrong);
    assert_eq!(score(&a, "Answer: sideways").status, Status::Invalid);
}

#[test]
fn bool_parsing() {
    let a = Answer::Bool(true);
    assert_eq!(score(&a, "Answer: yes").status, Status::Correct);
    assert_eq!(score(&a, "Answer: no").status, Status::Wrong);
}

#[test]
fn choice_multiword_and_invalid() {
    // "Deep Ocean" must win over "Ocean" (longest-match) and not be marked invalid.
    let a = choice("Deep Ocean", &["Ocean", "Deep Ocean", "Grassland"]);
    assert_eq!(score(&a, "Answer: Deep Ocean").status, Status::Correct);
    assert_eq!(score(&a, "Answer: Ocean").status, Status::Wrong);
    // A terrain not on the board -> invalid, not merely wrong.
    assert_eq!(score(&a, "Answer: Swampland").status, Status::Invalid);
}

#[test]
fn choice_ignores_coordinate_whitespace() {
    // A correct referent reply must not be scored *invalid* over coordinate spacing:
    // model "(29,16)" vs option "(29, 16)". (Regression: T2 unit-strength false negatives.)
    let a = choice(
        "Fighter at (29, 16)",
        &["Fighter at (29, 16)", "Chariot at (36, 27)"],
    );
    assert_eq!(
        score(&a, "Answer: Fighter at (29,16)").status,
        Status::Correct
    );
    assert_eq!(
        score(&a, "Answer: Fighter at (29, 16)").status,
        Status::Correct
    );
    assert_eq!(
        score(&a, "Answer: Chariot at (36,27)").status,
        Status::Wrong
    );
}

#[test]
fn answer_line_precedence() {
    // Reasoning may mention other numbers; the final Answer: line wins.
    let a = Answer::Int(7);
    let reply = "There are 3 forests and 4 hills nearby.\nAnswer: 7";
    assert_eq!(score(&a, reply).status, Status::Correct);
}
