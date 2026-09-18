//! Incremental JSONL result sink with resume/skip semantics.
//!
//! A long live run writes one JSON line per trial and flushes it immediately, so a crash (or an
//! out-of-credit provider) never loses completed trials. A follow-up `--resume` re-run skips the
//! trials already in the file and computes only the missing ones, appending them.
//!
//! The identity of a trial across runs is the 6-tuple **(encoding, item_id, board, model, think,
//! effort)** — every field is persisted per row, so changing the model (or the reasoning toggle /
//! effort) yields no matches and cannot be silently resumed on top of a different run's results.

use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::sync::Mutex;

use crate::json::Json;
use crate::question::EvalItem;
use crate::runner::ResultRow;

/// The resume policy the run was invoked with (`--fresh` / `--resume` / neither).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeMode {
    /// `--fresh`: truncate `--out` and run every trial.
    Fresh,
    /// `--resume`: skip trials already present in `--out`, run the rest, append+flush. Errors when
    /// nothing in the file matches this run (changed model/think/effort, or wrong file).
    Resume,
    /// Neither flag: the ordinary first run — fresh write, but refuse to clobber a file that already
    /// holds matching results (the caller must disambiguate with `--resume` or `--fresh`).
    Default,
}

/// The 6-tuple trial identity: (encoding, item_id, board, model, think, effort).
type ResumeKey = (String, String, String, String, bool, String);

fn make_key(
    enc: &str,
    item_id: &str,
    board: &str,
    model: &str,
    think: bool,
    effort: &str,
) -> ResumeKey {
    (
        enc.to_string(),
        item_id.to_string(),
        board.to_string(),
        model.to_string(),
        think,
        effort.to_string(),
    )
}

/// Read the 6-key of every row already in `out_path`. Missing/unreadable file → empty set; malformed
/// lines are skipped. Rows written before `think`/`effort` existed read as `think=false`,
/// `effort="none"` (they simply won't match a keyed resume, which is the safe outcome).
pub fn parse_rows(out_path: &str) -> HashSet<ResumeKey> {
    let mut keys = HashSet::new();
    let Ok(text) = std::fs::read_to_string(out_path) else {
        return keys;
    };
    // Strip a leading UTF-8 BOM (U+FEFF) if present. A resume file that has been round-tripped
    // through a Windows editor or PowerShell `Out-File`/`Set-Content` acquires a BOM, which would
    // otherwise glue onto the first line and make its JSON unparseable — silently dropping that
    // trial's key from the resume set so it gets needlessly re-run (and duplicated) on `--resume`.
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(v) = Json::parse(line) else { continue };
        let (Some(enc), Some(item), Some(board), Some(model)) = (
            v.get("encoding").and_then(Json::as_str),
            v.get("item_id").and_then(Json::as_str),
            v.get("board").and_then(Json::as_str),
            v.get("model").and_then(Json::as_str),
        ) else {
            continue;
        };
        let think = matches!(v.get("think"), Some(Json::Bool(true)));
        let effort = v.get("effort").and_then(Json::as_str).unwrap_or("none");
        keys.insert(make_key(enc, item, board, model, think, effort));
    }
    keys
}

/// A concurrency-safe, append-and-flush JSONL sink. `record` stamps the run identity's `think`/
/// `effort` onto each row, serializes it, and flushes — so the file is durable after every trial.
pub struct RunSink {
    writer: Mutex<BufWriter<std::fs::File>>,
    /// Trials already present in the file that this run should skip (empty unless resuming).
    done: HashSet<ResumeKey>,
    model: String,
    think: bool,
    effort: String,
}

impl RunSink {
    /// Prepare the sink for `out_path` under `mode`, given the run identity and the trials this run
    /// would produce (`encodings` × `items`). Applies the resume truth table and, for the error
    /// cases, returns a user-facing `Err` **without touching the file**:
    ///
    /// * `Fresh`   → truncate, run everything.
    /// * `Resume`  → ≥1 match: append, skip the matches; 0 matches: `Err` (nothing to resume).
    /// * `Default` → ≥1 match: `Err` (refuse to clobber); 0 matches: fresh write (ordinary run).
    pub fn open(
        out_path: &str,
        mode: ResumeMode,
        encodings: &[String],
        items: &[EvalItem],
        model: &str,
        think: bool,
        effort: &str,
    ) -> Result<RunSink, String> {
        let planned: HashSet<ResumeKey> = encodings
            .iter()
            .flat_map(|e| {
                items
                    .iter()
                    .map(move |it| make_key(e, &it.id, &it.board_source, model, think, effort))
            })
            .collect();
        let existing = parse_rows(out_path);
        let matches: HashSet<ResumeKey> = planned.intersection(&existing).cloned().collect();

        let (truncate, done) = match mode {
            ResumeMode::Fresh => (true, HashSet::new()),
            ResumeMode::Resume => {
                if matches.is_empty() {
                    return Err(format!(
                        "nothing to resume: no matching trials in {out_path} \
                         (changed model/think/effort, or wrong file?); use --fresh to start over"
                    ));
                }
                (false, matches)
            }
            ResumeMode::Default => {
                if !matches.is_empty() {
                    return Err(format!(
                        "refusing to run: {out_path} already has matching results; \
                         pass --resume to continue or --fresh to overwrite"
                    ));
                }
                (true, HashSet::new())
            }
        };

        let mut opts = OpenOptions::new();
        opts.create(true).write(true);
        if truncate {
            opts.truncate(true);
        } else {
            opts.append(true);
        }
        let file = opts
            .open(out_path)
            .map_err(|e| format!("opening {out_path}: {e}"))?;
        Ok(RunSink {
            writer: Mutex::new(BufWriter::new(file)),
            done,
            model: model.to_string(),
            think,
            effort: effort.to_string(),
        })
    }

    /// Whether this trial is already in the file (and so should be skipped) under the run identity.
    pub fn is_done(&self, encoding: &str, item_id: &str, board: &str) -> bool {
        self.done.contains(&make_key(
            encoding,
            item_id,
            board,
            &self.model,
            self.think,
            &self.effort,
        ))
    }

    /// Stamp the run's `think`/`effort` onto `row`, then append it as one JSON line and flush.
    pub fn record(&self, row: &mut ResultRow) {
        row.think = self.think;
        row.effort = self.effort.clone();
        let line = row.to_json();
        if let Ok(mut w) = self.writer.lock() {
            let _ = writeln!(w, "{line}");
            let _ = w.flush();
        }
    }

    /// How many trials were skipped as already-done (0 unless resuming).
    pub fn skipped(&self) -> usize {
        self.done.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::ResultRow;
    use crate::scoring::Status;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// A fresh, unique temp path per call so parallel tests never collide.
    fn tmp_path(tag: &str) -> String {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let p = std::env::temp_dir().join(format!("civspatial-sink-{tag}-{nanos}-{n}.jsonl"));
        let p = p.to_string_lossy().into_owned();
        let _ = std::fs::remove_file(&p);
        p
    }

    fn row_with_item_id(item_id: &str) -> ResultRow {
        ResultRow {
            model: "oracle".into(),
            think: false,
            effort: "none".into(),
            encoding: "raw".into(),
            board: "corpus_large#fog(p4)".into(),
            item_id: item_id.into(),
            category: "forward-posting".into(),
            tier: "T3".into(),
            status: Status::Correct,
            expected: "x".into(),
            got: "x".into(),
            prompt_tokens: 0,
            completion_tokens: 0,
            cached_tokens: 0,
            latency_ms: 0,
            n_turns: 1,
            n_tool_calls: 0,
        }
    }

    /// The core resume-key round-trip for a NON-ASCII `item_id` (the late-board fog player
    /// "François Mitterrand", `ç`): serialize the row exactly as the sink writes it, read it back
    /// through `parse_rows`, and assert the recovered 6-key byte-matches the freshly-generated key —
    /// i.e. the item is correctly recognized as done and skipped on `--resume`. Regression guard for
    /// the Windows non-ASCII resume claim: the write (`json::quote`) and read (`json::Json::parse`)
    /// paths must round-trip UTF-8 losslessly, with no `ç` → U+FFFD replacement on either side.
    #[test]
    fn nonascii_item_id_resume_key_round_trips() {
        let item_id = "forward-posting:François Mitterrand:u784:86,27,87,23,91,26,89,26";
        let path = tmp_path("nonascii");
        std::fs::write(&path, format!("{}\n", row_with_item_id(item_id).to_json())).unwrap();

        // The stored bytes must be the real UTF-8 for `ç` (C3 A7), never the U+FFFD replacement.
        let bytes = std::fs::read(&path).unwrap();
        assert!(
            bytes.windows(2).any(|w| w == [0xC3, 0xA7]),
            "the written item_id must carry real UTF-8 ç bytes"
        );
        assert!(
            !String::from_utf8_lossy(&bytes).contains('\u{fffd}'),
            "no U+FFFD replacement char may appear in the written row"
        );

        let keys = parse_rows(&path);
        let fresh = make_key("raw", item_id, "corpus_large#fog(p4)", "oracle", false, "none");
        assert!(
            keys.contains(&fresh),
            "the freshly-generated 6-key must match the parsed key for a non-ASCII item_id"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// A resume file carrying a leading UTF-8 BOM (as a Windows editor or PowerShell
    /// `Out-File`/`Set-Content` leaves behind) must still yield the first row's resume key — the BOM
    /// must not glue onto line 1 and drop that trial from the done set (→ needless re-run + duplicate).
    #[test]
    fn leading_bom_does_not_drop_the_first_resume_key() {
        let item_id = "forward-posting:François Mitterrand:u784:1,2,3,4";
        let path = tmp_path("bom");
        let mut bytes = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
        bytes.extend_from_slice(format!("{}\n", row_with_item_id(item_id).to_json()).as_bytes());
        std::fs::write(&path, &bytes).unwrap();

        let keys = parse_rows(&path);
        let fresh = make_key("raw", item_id, "corpus_large#fog(p4)", "oracle", false, "none");
        assert!(
            keys.contains(&fresh),
            "a BOM-prefixed resume file must still recover the first row's 6-key"
        );
        let _ = std::fs::remove_file(&path);
    }
}
