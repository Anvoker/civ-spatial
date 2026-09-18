//! Scoring — a separate stage that compares an expected [`Answer`] to a model reply. It never
//! re-runs the solver; it only extracts and compares (DESIGN.md §6). Type-directed by the
//! answer variant. Distinguishes *invalid* (unparseable / off the legal option set) from
//! *wrong*, which is a meaningful methodological difference.

use civ_core::geometry::Dir8;

use crate::question::{Answer, INCOMPARABLE};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Correct,
    Wrong,
    /// No parseable answer of the expected type (or a Choice not among the legal options).
    Invalid,
    /// The provider returned nothing (empty / 0-token completion) even after a retry — a *failed
    /// call*, not an answered-wrong. Distinct from `Invalid` so it can be excluded from the scored
    /// denominator instead of silently deflating accuracy (B4).
    Error,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Correct => "correct",
            Status::Wrong => "wrong",
            Status::Invalid => "invalid",
            Status::Error => "error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Score {
    pub status: Status,
    /// The normalized value the scorer extracted, for debugging (may be empty).
    pub extracted: String,
}

impl Score {
    pub fn correct(&self) -> bool {
        self.status == Status::Correct
    }
}

/// Score a reply against the expected answer.
pub fn score(expected: &Answer, reply: &str) -> Score {
    // S1: EVERY answer type requires an explicit `Answer:` line. We read ONLY the text after the
    // last `Answer:` marker (`answer_marker_field`) — never the last line, the whole reply, or an
    // entire-reply token scan — so reasoning prose can't be mined into an answer. `None` here means
    // there is no `Answer:` line at all → invalid (a genuine no-answer), not a fabricated score.
    let field = answer_marker_field(reply);
    match expected {
        // B2: the integer answer is the LAST integer on the `Answer:` line; no whole-reply fallback.
        Answer::Int(want) => match field.as_deref().and_then(last_int) {
            Some(got) => cmp(got == *want, got.to_string()),
            None => invalid(""),
        },
        Answer::OptionalInt(want) => {
            // S2 (value-first): if the `Answer:` line carries an explicit integer, score on THAT —
            // a `none`/`na` token elsewhere on the line (e.g. `Answer: 3 (none of the others
            // qualify)`) must not beat the real value. Only when the line has no integer do we fall
            // to the none verdict, and that token search is confined to the answer line (never the
            // surrounding prose). Matters most for `nearest-owned` / `reachable-nearest`.
            let Some(field) = field.as_deref() else {
                return invalid("");
            };
            match last_int(field) {
                Some(g) => cmp(*want == Some(g), g.to_string()),
                None => {
                    if mentions_none(field) {
                        cmp(want.is_none(), "none".to_string())
                    } else {
                        invalid("")
                    }
                }
            }
        }
        Answer::Direction(want) => match field.as_deref().and_then(parse_direction) {
            Some(got) => cmp(got == *want, got.short().to_string()),
            None => invalid(""),
        },
        Answer::Bool(want) => match field.as_deref().and_then(parse_bool) {
            Some(got) => cmp(got == *want, if got { "yes" } else { "no" }.to_string()),
            None => invalid(""),
        },
        Answer::Choice { value, options } => {
            match field.as_deref().and_then(|f| match_option(f, options)) {
                Some(opt) => cmp(opt.eq_ignore_ascii_case(value), opt),
                None => invalid(field.as_deref().unwrap_or("")),
            }
        }
        Answer::ChoiceSet {
            acceptable,
            options,
        } => match field.as_deref().and_then(|f| match_option(f, options)) {
            // In `options` → correct iff also in `acceptable` (else it's a dominated blunder =
            // wrong); not in `options` → invalid (named something not offered).
            Some(opt) => cmp(acceptable.iter().any(|a| a.eq_ignore_ascii_case(&opt)), opt),
            None => invalid(field.as_deref().unwrap_or("")),
        },
        // B2: the coordinate pair is read from the `Answer:` line ONLY (no whole-reply fallback),
        // so a cap-out ending in trailing coordinates with no answer line scores no-answer/invalid
        // instead of fabricating a pair from that trailing text.
        Answer::Coord { x, y } => match answer_line_two_ints(reply) {
            Some((gx, gy)) => cmp(gx == *x as i64 && gy == *y as i64, format!("({gx}, {gy})")),
            None => invalid(""),
        },
        Answer::Compare3 { value, a, b } => {
            // S2 (value-first), mirroring OptionalInt: match an explicit option on the `Answer:`
            // line FIRST; only when the line names no option do we fall to the incomparable/neither
            // verdict, and that token search is confined to the answer line. So `Answer: (12, 5)
            // (neither of the far ones is close)` scores on the coordinate, not "neither". Claiming a
            // dominant attack when it is actually a trade-off is a *wrong* judgment (not invalid).
            let Some(field) = field.as_deref() else {
                return invalid("");
            };
            let opts = [a.clone(), b.clone()];
            if let Some(opt) = match_option(field, &opts) {
                return cmp(opt.eq_ignore_ascii_case(value), opt);
            }
            if mentions_incomparable(field) {
                return cmp(value == INCOMPARABLE, INCOMPARABLE.to_string());
            }
            invalid(field)
        }
    }
}

/// Whether the field names the "incomparable" (genuine trade-off) verdict as a standalone token.
fn mentions_incomparable(s: &str) -> bool {
    s.split(|c: char| !c.is_ascii_alphabetic()).any(|t| {
        let t = t.to_ascii_lowercase();
        t == "incomparable" || t == "neither" || t == "tradeoff"
    })
}

/// Whether the field names the "no such thing" outcome as a standalone token.
fn mentions_none(s: &str) -> bool {
    s.split(|c: char| !c.is_ascii_alphabetic()).any(|t| {
        let t = t.to_ascii_lowercase();
        t == "none" || t == "na"
    })
}

fn cmp(ok: bool, extracted: String) -> Score {
    Score {
        status: if ok { Status::Correct } else { Status::Wrong },
        extracted,
    }
}

fn invalid(extracted: &str) -> Score {
    Score {
        status: Status::Invalid,
        extracted: extracted.trim().to_string(),
    }
}

/// Case-insensitive substring search returning the byte offset of the match.
fn find_ci(haystack: &str, needle: &str) -> Option<usize> {
    let h = haystack.to_ascii_lowercase();
    h.find(&needle.to_ascii_lowercase())
}

/// The text following the last `Answer:` marker, or `None` when the reply has no such marker. This
/// never falls back to the last line or the whole reply — every answer type (S1) relies on that so
/// an unformatted reply scores no-answer rather than being mined from surrounding prose.
fn answer_marker_field(reply: &str) -> Option<String> {
    let mut best: Option<String> = None;
    for line in reply.lines() {
        if let Some(pos) = find_ci(line, "answer:") {
            best = Some(line[pos + "answer:".len()..].trim().to_string());
        }
    }
    best
}

/// The coordinate answer: the first two integers on an explicit `Answer:` line (e.g.
/// `Answer: (12, 5)` → `(12, 5)`), or `None` if there is no answer line / no pair on it. Answer-line
/// only (via [`answer_marker_field`]), no whole-reply fallback, so a cap-out's trailing coordinates
/// can never be fabricated into an answer (B2).
fn answer_line_two_ints(reply: &str) -> Option<(i64, i64)> {
    answer_marker_field(reply)
        .as_deref()
        .and_then(first_two_ints)
}

/// The LAST integer appearing in `s`. Prefers the last integer so `Answer: nearest is 3` → 3 (the
/// number is the tail of the phrase). Signed; scans left-to-right and keeps the final match.
fn last_int(s: &str) -> Option<i64> {
    let chars: Vec<char> = s.chars().collect();
    let mut found: Option<i64> = None;
    let mut i = 0;
    while i < chars.len() {
        let neg = chars[i] == '-' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit();
        if chars[i].is_ascii_digit() || neg {
            let start = i;
            if neg {
                i += 1;
            }
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            if let Ok(v) = chars[start..i].iter().collect::<String>().parse() {
                found = Some(v);
            }
        } else {
            i += 1;
        }
    }
    found
}

fn first_two_ints(s: &str) -> Option<(i64, i64)> {
    let mut nums = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() && nums.len() < 2 {
        let neg = chars[i] == '-' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit();
        if chars[i].is_ascii_digit() || neg {
            let start = i;
            if neg {
                i += 1;
            }
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            if let Ok(v) = chars[start..i].iter().collect::<String>().parse() {
                nums.push(v);
            }
        } else {
            i += 1;
        }
    }
    if nums.len() == 2 {
        Some((nums[0], nums[1]))
    } else {
        None
    }
}

fn parse_direction(s: &str) -> Option<Dir8> {
    // Prefer a standalone token; scan tokens split on non-alphabetic characters.
    for tok in s.split(|c: char| !c.is_ascii_alphabetic()) {
        if let Some(d) = Dir8::parse(tok) {
            return Some(d);
        }
    }
    None
}

fn parse_bool(s: &str) -> Option<bool> {
    for tok in s.split(|c: char| !c.is_ascii_alphabetic()) {
        match tok.to_ascii_lowercase().as_str() {
            "yes" | "true" | "y" => return Some(true),
            "no" | "false" | "n" => return Some(false),
            _ => {}
        }
    }
    None
}

/// Lowercase and strip ALL whitespace. This keeps closed-set matching insensitive to formatting
/// differences that don't change the answer — notably coordinate spacing like `(29, 16)` vs
/// `(29,16)`, which otherwise scored a correct referent reply (e.g. a T2 unit choice) as *invalid*.
/// Safe for multi-word options too: "Deep Ocean" → "deepocean" on both sides still matches, and the
/// longest-option-first tiebreak below keeps "Deep Ocean" from being captured by "Ocean".
fn squash(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Markdown emphasis / code characters to drop before matching, so `**Perm**` == `Perm`,
/// `` `(83, 39)` `` == `(83, 39)`, and a bolded `**Answer: …**` block behaves like the plain one
/// (B-coord). Options never legitimately contain these, so stripping them from both sides is safe.
fn strip_markdown(s: &str) -> String {
    s.chars().filter(|c| !matches!(c, '*' | '`' | '_' | '#')).collect()
}

/// Canonicalize every coordinate-shaped pair — `x, y`, `x,y`, or `(x, y)`, with or without
/// surrounding parens and any interior spacing — to the paren-wrapped, space-free form `(x,y)`
/// (B-coord). Run before [`squash`] so a coord answer written `83, 39`, `(83,39)`, or (after
/// [`strip_markdown`]) `**83, 39**` all compare equal to the option `(83, 39)`, while KEEPING the
/// parens that make substring matching safe: `(3,4)` is not a substring of `(13,4)`, whereas the
/// bare `3,4` IS a substring of `13,4`. Non-coordinate text (city names, prose, decimals like
/// `21.875`) is left untouched — a number must be followed by `,` then another integer to match.
fn canonicalize_coords(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if let Some((next, x, y)) = try_coord(&chars, i) {
            out.push('(');
            out.push_str(&x);
            out.push(',');
            out.push_str(&y);
            out.push(')');
            i = next;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Try to read a coordinate pair `(? int (,) int )?` starting at `i`. On success returns the index
/// just past it and the two integer texts; on failure `None` (the caller then advances one char).
fn try_coord(chars: &[char], mut i: usize) -> Option<(usize, String, String)> {
    let ws = |c: &[char], j: &mut usize| {
        while c.get(*j).is_some_and(|ch| ch.is_whitespace()) {
            *j += 1;
        }
    };
    let digits = |c: &[char], j: &mut usize| -> Option<String> {
        let start = *j;
        while c.get(*j).is_some_and(|ch| ch.is_ascii_digit()) {
            *j += 1;
        }
        (*j > start).then(|| c[start..*j].iter().collect())
    };
    let had_paren = chars.get(i) == Some(&'(');
    if had_paren {
        i += 1;
        ws(chars, &mut i);
    }
    let x = digits(chars, &mut i)?;
    ws(chars, &mut i);
    if chars.get(i) != Some(&',') {
        return None;
    }
    i += 1;
    ws(chars, &mut i);
    let y = digits(chars, &mut i)?;
    if had_paren {
        ws(chars, &mut i);
        if chars.get(i) == Some(&')') {
            i += 1;
        }
    }
    Some((i, x, y))
}

/// Full comparison normalization: strip markdown, canonicalize coordinate formatting, then
/// [`squash`] (lowercase + drop whitespace). Applied to BOTH the reply field and each option so the
/// two are compared on the same footing.
fn norm(s: &str) -> String {
    squash(&canonicalize_coords(&strip_markdown(s)))
}

/// Positive comparative/superlative keywords (space-free, lowercased) that mark the entity a model
/// DECLARES the winner in a verbose two-entity answer ("… is better defended …"). Used only to
/// disambiguate which of several named options was actually selected — never to detect an answer.
/// Kept high-precision (no `most`/`more`, which appear inside unrelated words) so it can't misfire.
const WINNER_KEYWORDS: &[&str] = &[
    "better",
    "best",
    "stronger",
    "strongest",
    "greater",
    "greatest",
    "safer",
    "safest",
    "sturdier",
    "sturdiest",
    "tougher",
    "toughest",
    "harder",
    "hardest",
    "higher",
    "highest",
    "superior",
    "soundest",
];

/// Byte offset of the earliest winner keyword in the already-normalized field, if any.
fn first_winner_keyword(norm_field: &str) -> Option<usize> {
    WINNER_KEYWORDS
        .iter()
        .filter_map(|k| norm_field.find(k))
        .min()
}

/// Match the reply against a closed option set. Prefers an exact (format-insensitive) match on the
/// whole field; otherwise an option appearing as a phrase in the field. When TWO OR MORE options
/// appear (a verbose comparison sentence naming both entities, e.g. `Answer: Caen … is better
/// defended … versus Perm …`), it returns the option the model actually SELECTED — the one occurring
/// closest BEFORE its positive-claim keyword — rather than whichever happens to come first in the
/// option list (the two-entity extraction bug, which recorded the fogged/loser entity). If no winner
/// keyword is present it falls back to the legacy longest-option-first order, so a currently-correct
/// terse reply is never disturbed. Returns the matched option (canonical casing), or `None`.
fn match_option(field: &str, options: &[String]) -> Option<String> {
    let f = norm(field);
    if let Some(o) = options.iter().find(|o| norm(o) == f) {
        return Some(o.clone());
    }
    // Every option that appears as a phrase, with its position in the normalized field.
    let mut hits: Vec<(usize, &String)> = options
        .iter()
        .filter_map(|o| {
            let os = norm(o);
            if os.is_empty() {
                return None;
            }
            f.find(&os).map(|pos| (pos, o))
        })
        .collect();
    match hits.len() {
        0 => return None,
        1 => return Some(hits[0].1.clone()),
        _ => {}
    }
    // Two-plus options named: prefer the one declared the winner (closest option before the first
    // positive keyword). Only overrides the legacy order when that leading claim is unambiguous.
    if let Some(kw) = first_winner_keyword(&f) {
        if let Some((_, o)) = hits.iter().filter(|(pos, _)| *pos < kw).max_by_key(|(pos, _)| *pos) {
            return Some((*o).clone());
        }
    }
    // Legacy fallback: longest option first (so "Deep Ocean" wins over "Ocean"), stable within a
    // length so the original option order is the tiebreak — byte-identical to the pre-fix behavior.
    hits.sort_by_key(|(_, o)| std::cmp::Reverse(o.len()));
    Some(hits[0].1.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_int_none_matches_none() {
        assert_eq!(
            score(&Answer::OptionalInt(None), "Answer: none").status,
            Status::Correct
        );
    }

    #[test]
    fn optional_int_some_matches_number() {
        assert_eq!(
            score(&Answer::OptionalInt(Some(4)), "Answer: 4").status,
            Status::Correct
        );
    }

    #[test]
    fn optional_int_none_but_number_is_wrong() {
        assert_eq!(
            score(&Answer::OptionalInt(None), "Answer: 4").status,
            Status::Wrong
        );
    }

    #[test]
    fn optional_int_none_said_over_a_number_is_wrong() {
        // S2 value-first: an explicit integer on the answer line is scored even when a "none" token
        // is also present, so "none within 6" reads the 6; 6 ≠ 4 → wrong either way.
        assert_eq!(
            score(&Answer::OptionalInt(Some(4)), "Answer: none within 6").status,
            Status::Wrong
        );
    }

    #[test]
    fn optional_int_value_beats_none_token_in_explanation() {
        // S2: `Answer: 3 (none of the others qualify)` must score on 3 (the real value), NOT let the
        // parenthetical "none" veto it into a wrong "none" verdict.
        let sc = score(&Answer::OptionalInt(Some(3)), "Answer: 3 (none of the others qualify)");
        assert_eq!(sc.status, Status::Correct);
        assert_eq!(sc.extracted, "3");
        // The same reply when the truth IS 3 but expected differs stays a value comparison, not none.
        assert_eq!(
            score(&Answer::OptionalInt(Some(5)), "Answer: 3 (none of the others qualify)").status,
            Status::Wrong
        );
    }

    #[test]
    fn optional_int_genuine_none_still_scores_none() {
        // A bare `Answer: none` (no integer) still falls to the none verdict.
        assert_eq!(
            score(&Answer::OptionalInt(None), "Answer: none of them qualify").status,
            Status::Correct
        );
        // …and claiming none when a number is expected is wrong.
        assert_eq!(
            score(&Answer::OptionalInt(Some(2)), "Answer: none of them qualify").status,
            Status::Wrong
        );
    }

    // --- S1: every answer type requires an explicit `Answer:` line (no prose mining) ---

    #[test]
    fn direction_requires_answer_line() {
        let a = Answer::Direction(Dir8::NE);
        assert_eq!(score(&a, "Answer: NE").status, Status::Correct);
        // The direction word is in the reasoning but there is no `Answer:` line → invalid, never
        // mined from prose.
        assert_eq!(
            score(&a, "The city lies to the northeast of the unit.").status,
            Status::Invalid
        );
    }

    #[test]
    fn bool_requires_answer_line() {
        let a = Answer::Bool(true);
        assert_eq!(score(&a, "Answer: yes").status, Status::Correct);
        assert_eq!(
            score(&a, "Yes, it can reach the tile in time.").status,
            Status::Invalid
        );
    }

    #[test]
    fn compare3_value_beats_incomparable_token_in_explanation() {
        // S2: an explicit option on the answer line wins over a stray "neither"/"tradeoff" token.
        let a = Answer::Compare3 {
            value: "(1, 2)".to_string(),
            a: "(1, 2)".to_string(),
            b: "(3, 4)".to_string(),
        };
        let sc = score(&a, "Answer: (1, 2) (neither of the far tiles is close)");
        assert_eq!(sc.status, Status::Correct);
        // A genuine trade-off verdict (no option named) still scores incomparable.
        let inc = Answer::Compare3 {
            value: INCOMPARABLE.to_string(),
            a: "(1, 2)".to_string(),
            b: "(3, 4)".to_string(),
        };
        assert_eq!(
            score(&inc, "Answer: neither — it is a genuine tradeoff").status,
            Status::Correct
        );
        // Naming a concrete option when the truth is incomparable is a wrong judgment.
        assert_eq!(
            score(&inc, "Answer: (1, 2) dominates").status,
            Status::Wrong
        );
    }

    fn choice_set(acceptable: &[&str], options: &[&str]) -> Answer {
        Answer::ChoiceSet {
            acceptable: acceptable.iter().map(|s| s.to_string()).collect(),
            options: options.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn choice_set_accepts_any_acceptable_member() {
        let a = choice_set(&["(1, 1)", "(2, 2)"], &["(1, 1)", "(2, 2)", "(3, 3)"]);
        assert_eq!(score(&a, "Answer: (2, 2)").status, Status::Correct);
    }

    #[test]
    fn choice_set_dominated_pick_is_wrong() {
        // (3, 3) is offered but NOT acceptable → a dominated blunder → wrong (the negative case
        // that guards the T3 dominance scorer, `T2-T3-design.md` §5).
        let a = choice_set(&["(1, 1)"], &["(1, 1)", "(3, 3)"]);
        assert_eq!(score(&a, "Answer: (3, 3)").status, Status::Wrong);
        assert_eq!(score(&a, "Answer: (1, 1)").status, Status::Correct);
    }

    #[test]
    fn choice_set_off_option_is_invalid() {
        let a = choice_set(&["(1, 1)"], &["(1, 1)", "(3, 3)"]);
        assert_eq!(score(&a, "Answer: (9, 9)").status, Status::Invalid);
    }

    // --- B2: integer extraction is answer-line-only, never fabricated from surrounding text ---

    #[test]
    fn int_extracts_from_answer_line() {
        let sc = score(&Answer::Int(19), "Answer: 19");
        assert_eq!(sc.status, Status::Correct);
        assert_eq!(sc.extracted, "19");
    }

    #[test]
    fn int_takes_last_integer_on_answer_line() {
        // `Answer: nearest is 3` → 3 (the trailing number), not a stray earlier token.
        assert_eq!(
            score(&Answer::Int(3), "Answer: nearest is 3").status,
            Status::Correct
        );
    }

    #[test]
    fn int_no_answer_line_is_invalid_not_fabricated() {
        // The model wrote the correct count (19) but omitted the `Answer:` line, and the prompt's
        // Chebyshev radius (5) is in the text. We deliberately DO NOT recover the unformatted 19
        // (that risks fabrication) → invalid; and we must never grab the stray 5.
        let reply = "Scanning radius 5 around the tile, there are 19 Grassland tiles.";
        let sc = score(&Answer::Int(19), reply);
        assert_eq!(sc.status, Status::Invalid);
        assert_ne!(
            sc.extracted, "5",
            "must not fabricate the prompt radius as the answer"
        );
        assert_ne!(
            sc.extracted, "19",
            "unformatted answers are deliberately not recovered"
        );
    }

    #[test]
    fn int_capout_trailing_coords_is_invalid_not_fabricated() {
        // A truncated cap-out with no answer line, ending in a coordinate: must be invalid, NOT the
        // coordinate component 29 (the ~28 cap-out rows this reclassifies `wrong`→`invalid`).
        let reply = "Let me check the units near the capital at ...(29, 11)";
        let sc = score(&Answer::Int(29), reply);
        assert_eq!(sc.status, Status::Invalid);
        assert_ne!(
            sc.extracted, "29",
            "must not fabricate a trailing coordinate as the answer"
        );
    }

    // --- B2 (OptionalInt): same answer-line-only rule; `none` precedence preserved exactly. This
    // is the `nearest-owned` / `reachable-nearest` answer type (the dispersed ablation kinds). ---

    #[test]
    fn optional_int_answer_line_number_extracts() {
        // Well-formed numeric answer still extracts (no regression).
        let sc = score(&Answer::OptionalInt(Some(3)), "Answer: 3");
        assert_eq!(sc.status, Status::Correct);
        assert_eq!(sc.extracted, "3");
    }

    #[test]
    fn optional_int_none_answer_line_still_wins() {
        // `none` detection + precedence preserved: correct when expected none, wrong when a number.
        assert_eq!(
            score(&Answer::OptionalInt(None), "Answer: none").status,
            Status::Correct
        );
        let wrong = score(&Answer::OptionalInt(Some(4)), "Answer: none");
        assert_eq!(wrong.status, Status::Wrong);
        assert_eq!(wrong.extracted, "none");
    }

    #[test]
    fn optional_int_capout_trailing_coords_is_invalid_not_fabricated() {
        // Cap-out with no answer line and not "none": invalid, NOT a stray coordinate component.
        let reply = "Searching outward from the capital... reached (29, 11) before the cap.";
        let sc = score(&Answer::OptionalInt(Some(7)), reply);
        assert_eq!(sc.status, Status::Invalid);
        assert_ne!(
            sc.extracted, "29",
            "must not fabricate the coordinate x as the answer"
        );
        assert_ne!(
            sc.extracted, "11",
            "must not fabricate the coordinate y as the answer"
        );
        // Same for the expected-none case: no answer line and not "none" → invalid, not fabricated.
        let sc_none = score(&Answer::OptionalInt(None), reply);
        assert_eq!(sc_none.status, Status::Invalid);
    }

    // --- B2 (Coord): answer-line-only coordinate pair; no whole-reply fabrication. ---

    #[test]
    fn coord_answer_line_pair_extracts() {
        let sc = score(&Answer::Coord { x: 12, y: 5 }, "Answer: (12, 5)");
        assert_eq!(sc.status, Status::Correct);
        assert_eq!(sc.extracted, "(12, 5)");
    }

    #[test]
    fn coord_capout_trailing_coords_is_invalid_not_fabricated() {
        // A truncated cap-out ending in coordinates but with no answer line → invalid, NOT (29, 11).
        let reply = "Scanning the frontier tiles near ...(29, 11)";
        let sc = score(&Answer::Coord { x: 29, y: 11 }, reply);
        assert_eq!(sc.status, Status::Invalid);
        assert_ne!(
            sc.extracted, "(29, 11)",
            "must not fabricate a trailing coordinate pair"
        );
    }

    // --- Bug 1: verbose two-entity "which-X" answers extract the SELECTED entity, not the first-
    // named / first-in-options / fogged one. These are the exact patterns from the recorded traces
    // (Haiku subscription arm) that mis-scored a correct pick as wrong. ---

    fn choice(value: &str, options: &[&str]) -> Answer {
        Answer::Choice {
            value: value.to_string(),
            options: options.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn choice_verbose_comparison_picks_declared_winner_not_fogged_loser() {
        // The canonical mis-score: options are listed [loser, winner] and BOTH names appear in the
        // answer sentence; the model declares the FIRST-named one "better defended". Legacy code
        // returned the first-in-options-order name (Perm); the fix returns the model's real pick.
        let a = choice("Caen", &["Perm", "Caen"]);
        let sc = score(
            &a,
            "Answer: Caen (91, 45) is better defended with a defense of 21.875, \
             versus Perm (87, 44) whose defense is hidden under fog.",
        );
        assert_eq!(sc.status, Status::Correct, "extracted {:?}", sc.extracted);
        assert_eq!(sc.extracted, "Caen");
    }

    #[test]
    fn choice_verbose_winner_named_after_the_loser_still_recovered() {
        // Recovery must not depend on the winner being FIRST in the sentence: "Unlike Perm, Caen is
        // better defended" names the loser first but the winner is the subject of "is better".
        let a = choice("Caen", &["Perm", "Caen"]);
        let sc = score(&a, "Answer: Unlike Perm, Caen is the better-defended city.");
        assert_eq!(sc.status, Status::Correct, "extracted {:?}", sc.extracted);
        assert_eq!(sc.extracted, "Caen");
    }

    #[test]
    fn choice_verbose_best_keyword_two_cities() {
        // The `best`-phrased variant (triage-reinforce style), options listed [winner, loser].
        let a = choice("Samara", &["Samara", "Kaminaljuyu"]);
        let sc = score(
            &a,
            "Answer: Samara at (85, 24) is better defended. Kaminaljuyu is under fog. \
             Based on the visible measurement, Samara is the best-defended city.",
        );
        assert_eq!(sc.status, Status::Correct, "extracted {:?}", sc.extracted);
        assert_eq!(sc.extracted, "Samara");
    }

    #[test]
    fn choice_terse_answers_unchanged_by_the_fix() {
        // Non-regression: a terse single-name answer is unchanged regardless of option order, and a
        // terse pick of the SECOND-listed option still scores that option (not the first).
        assert_eq!(score(&choice("Caen", &["Perm", "Caen"]), "Answer: Caen").status, Status::Correct);
        assert_eq!(score(&choice("Perm", &["Perm", "Caen"]), "Answer: Perm").status, Status::Correct);
        // Terse pick of the loser is still WRONG (must not be "helpfully" flipped).
        let sc = score(&choice("Caen", &["Perm", "Caen"]), "Answer: Perm");
        assert_eq!(sc.status, Status::Wrong);
        assert_eq!(sc.extracted, "Perm");
    }

    #[test]
    fn choice_no_winner_keyword_falls_back_to_legacy_order() {
        // When both names appear but there is NO positive keyword, behavior is the legacy longest-
        // first / first-in-options order — unchanged from before the fix (conservative fallback).
        let a = choice("Perm", &["Perm", "Caen"]);
        let sc = score(&a, "Answer: between Perm and Caen, I choose the former.");
        // Legacy: equal length, first-in-options = Perm.
        assert_eq!(sc.extracted, "Perm");
    }

    #[test]
    fn compare3_verbose_picks_declared_better_attack() {
        // Compare3 over coordinate targets: "Answer: (12, 5) is the better attack … than (30, 8)".
        let a = Answer::Compare3 {
            value: "(12, 5)".to_string(),
            a: "(12, 5)".to_string(),
            b: "(30, 8)".to_string(),
        };
        let sc = score(&a, "Answer: (12, 5) is the better attack, far superior to (30, 8).");
        assert_eq!(sc.status, Status::Correct, "extracted {:?}", sc.extracted);
    }

    // --- Bug 2: markdown emphasis + paren/format coordinate normalization for coord-valued option
    // sets (settle-site / t3-retreat / forward-posting / constraint-site — all `ChoiceSet` over
    // coordinate strings). A correctly-chosen coordinate must match regardless of `**`/backticks or
    // whether the model wrote parens. ---

    #[test]
    fn choiceset_coord_markdown_bold_matches() {
        let a = choice_set(&["(83, 39)"], &["(83, 39)", "(12, 7)"]);
        let sc = score(&a, "Answer: **83, 39**");
        assert_eq!(sc.status, Status::Correct, "extracted {:?}", sc.extracted);
    }

    #[test]
    fn choiceset_coord_bare_no_parens_matches() {
        let a = choice_set(&["(83, 39)"], &["(83, 39)", "(12, 7)"]);
        assert_eq!(score(&a, "Answer: 83, 39").status, Status::Correct);
        assert_eq!(score(&a, "Answer: (83,39)").status, Status::Correct);
        assert_eq!(score(&a, "Answer: `(83, 39)`").status, Status::Correct);
    }

    #[test]
    fn choiceset_coord_substring_safety_preserved() {
        // Normalization must NOT let (3, 4) spuriously match a field naming (13, 4): the parens are
        // re-added canonically so `(3,4)` is not a substring of `(13,4)`.
        let a = choice_set(&["(13, 4)"], &["(3, 4)", "(13, 4)"]);
        let sc = score(&a, "Answer: **13, 4**");
        assert_eq!(sc.status, Status::Correct, "extracted {:?}", sc.extracted);
        assert_eq!(sc.extracted, "(13, 4)");
        // And picking the dominated (3,4) is still a distinct, wrong extraction.
        let dominated = choice_set(&["(13, 4)"], &["(3, 4)", "(13, 4)"]);
        assert_eq!(score(&dominated, "Answer: 3, 4").extracted, "(3, 4)");
    }

    #[test]
    fn choiceset_coord_plain_parens_still_correct() {
        // Non-regression: the already-working `(x, y)` form is unaffected.
        let a = choice_set(&["(83, 39)"], &["(83, 39)", "(12, 7)"]);
        assert_eq!(score(&a, "Answer: (83, 39)").status, Status::Correct);
    }

    #[test]
    fn decimal_on_answer_line_not_misread_as_coord() {
        // A decimal defense value must not be canonicalized into a coordinate that matches an option.
        let a = choice_set(&["(2, 8)"], &["(2, 8)", "(9, 1)"]);
        // "21.875" contains "1.8" etc. but no `int , int`; the only real coord is (2, 8).
        assert_eq!(score(&a, "Answer: (2, 8), measured defense 21.875").status, Status::Correct);
    }
}
