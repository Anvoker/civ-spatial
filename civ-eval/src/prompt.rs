//! Prompt assembly: geometry-rules preamble + board block + question + strict answer-line
//! instruction. Held IDENTICAL across all encodings so the elicitation format is a constant,
//! not a confound (DESIGN.md §9). `rules_block` is the reserved T2/T3 seam (empty for T0/T1).

use civ_core::Board;

use crate::encoders::Encoder;
use crate::question::EvalItem;

const RULES: &str = "\
You are reasoning about a FIXED game board laid out on a square grid.
Conventions:
- Coordinates are (x, y). x increases to the east; y increases to the south; y = 0 is the north edge.
- The 8 compass directions are N, NE, E, SE, S, SW, W, NW. North is toward smaller y.
- Distance is the number of steps moving one tile at a time in any of the 8 directions, with a
  diagonal counting as a single step (Chebyshev distance = max(|dx|, |dy|)).
- The board does NOT wrap around at its edges.";

/// A prompt split at the prompt-caching boundary. `cacheable_prefix` (geometry rules + the
/// board block, plus any T2/T3 rules) is identical across every question for a given
/// (board × encoding), so a caching provider pays those tokens once; `tail` (the question and
/// answer-line instruction) varies per item. `full()` reproduces the flat prompt exactly, so
/// providers without caching — and the offline Oracle — see the same bytes as before.
pub struct Prompt {
    pub cacheable_prefix: String,
    pub tail: String,
}

impl Prompt {
    pub fn full(&self) -> String {
        let mut s = String::with_capacity(self.cacheable_prefix.len() + self.tail.len());
        s.push_str(&self.cacheable_prefix);
        s.push_str(&self.tail);
        s
    }
}

/// Assemble the prompt for one item under one encoding, split at the cache boundary.
///
/// `rules_block` carries the (optional) combat/strength rules for T2/T3; pass `None` for T0/T1.
pub fn assemble(
    board_block: &str,
    item: &EvalItem,
    enc: &dyn Encoder,
    board: &Board,
    rules_block: Option<&str>,
) -> Prompt {
    // Stable prefix: rules preamble + (optional T2/T3 rules) + board block.
    let mut prefix = String::new();
    prefix.push_str(RULES);
    prefix.push_str("\n\n");
    if let Some(rules) = rules_block {
        prefix.push_str(rules);
        prefix.push_str("\n\n");
    }
    prefix.push_str(board_block);

    // Variable tail: the question and the strict answer-line instruction.
    let mut tail = String::new();
    tail.push_str("\n\nQUESTION: ");
    tail.push_str(&item.question.render(enc, board));
    tail.push_str("\n\nThink briefly if needed, then end your reply with a line formatted EXACTLY as:\nAnswer: <X>\nwhere <X> is ");
    tail.push_str(&item.question.answer_hint());
    tail.push('.');

    Prompt {
        cacheable_prefix: prefix,
        tail,
    }
}
