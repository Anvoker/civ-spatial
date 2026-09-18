//! Resume/flush semantics, driven entirely by the offline Oracle (zero API spend). Proves the
//! `--resume` / `--fresh` truth table and the 6-tuple resume key (encoding, item_id, board, model,
//! think, effort) end to end: open the sink, run the Oracle through it, inspect the JSONL file.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use civ_core::parse_save;
use civ_eval::{generate_questions, run_oracle, Difficulty, EvalItem, ResumeMode, RunSink};

fn save_path() -> String {
    format!("{}/../data/saves/myagent_T50.sav", env!("CARGO_MANIFEST_DIR"))
}

/// A fresh, unique temp path per call so parallel tests never collide and stale files never leak in.
fn tmp_path(tag: &str) -> String {
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let p = std::env::temp_dir().join(format!("civspatial-resume-{tag}-{nanos}-{n}.jsonl"));
    let p = p.to_string_lossy().into_owned();
    let _ = std::fs::remove_file(&p);
    p
}

fn line_count(path: &str) -> usize {
    std::fs::read_to_string(path)
        .map(|t| t.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0)
}

/// A small, deterministic item set + single static encoding, enough to exercise skip/append.
fn fixture() -> (civ_core::Board, Vec<EvalItem>, Vec<String>) {
    let board = parse_save(save_path()).expect("parse save");
    let items: Vec<EvalItem> = generate_questions(&board, 1, 3, Difficulty::hard(), None)
        .into_iter()
        .take(3)
        .collect();
    assert!(items.len() >= 3, "need at least 3 distinct items");
    (board, items, vec!["raw".to_string()])
}

#[test]
fn resume_skips_matched_and_runs_only_missing() {
    let (board, items, encs) = fixture();
    let path = tmp_path("a");

    // Run 1 (ordinary first run): only the first two items → two rows on disk.
    let s1 = RunSink::open(&path, ResumeMode::Default, &encs, &items[..2], "oracle", true, "none")
        .expect("first run opens");
    let rows1 = run_oracle(&board, &encs, &items[..2], Some(&s1));
    drop(s1);
    assert_eq!(rows1.len(), 2);
    assert_eq!(line_count(&path), 2);

    // Run 2 (resume): all three items. The two already present are skipped; only the third runs and
    // is appended.
    let s2 = RunSink::open(&path, ResumeMode::Resume, &encs, &items, "oracle", true, "none")
        .expect("resume finds matches");
    assert_eq!(s2.skipped(), 2, "the two prior trials are recognized as done");
    let rows2 = run_oracle(&board, &encs, &items, Some(&s2));
    drop(s2);
    assert_eq!(rows2.len(), 1, "only the missing trial ran");
    assert_eq!(line_count(&path), 3, "the missing row was appended, not rewritten");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn resume_with_zero_matches_errors_and_writes_nothing() {
    let (_board, items, encs) = fixture();
    let path = tmp_path("b");

    // No file exists → nothing to resume → error, and no file is created.
    let err = RunSink::open(&path, ResumeMode::Resume, &encs, &items, "oracle", true, "none");
    assert!(err.is_err(), "resume with no matching file must error");
    assert!(
        !Path::new(&path).exists(),
        "the error path must not create/touch the file"
    );
}

#[test]
fn neither_flag_with_matching_file_errors() {
    let (board, items, encs) = fixture();
    let path = tmp_path("c");

    // Seed a file with a matching run.
    let s1 = RunSink::open(&path, ResumeMode::Default, &encs, &items, "oracle", true, "none")
        .expect("seed run opens");
    let _ = run_oracle(&board, &encs, &items, Some(&s1));
    drop(s1);
    let before = line_count(&path);
    assert!(before > 0);

    // Running again with neither flag must refuse (to disambiguate resume vs overwrite) and leave
    // the file untouched.
    let err = RunSink::open(&path, ResumeMode::Default, &encs, &items, "oracle", true, "none");
    assert!(err.is_err(), "a matching file with neither flag must error");
    assert_eq!(line_count(&path), before, "the file must be left untouched");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn fresh_truncates_and_reruns() {
    let (board, items, encs) = fixture();
    let path = tmp_path("d");

    // Seed a file with three rows.
    let s1 = RunSink::open(&path, ResumeMode::Default, &encs, &items, "oracle", true, "none")
        .expect("seed run opens");
    let _ = run_oracle(&board, &encs, &items, Some(&s1));
    drop(s1);
    assert_eq!(line_count(&path), 3);

    // --fresh with only two items truncates the old file and reruns from scratch.
    let s2 = RunSink::open(&path, ResumeMode::Fresh, &encs, &items[..2], "oracle", true, "none")
        .expect("fresh always opens");
    assert_eq!(s2.skipped(), 0, "fresh skips nothing");
    let rows = run_oracle(&board, &encs, &items[..2], Some(&s2));
    drop(s2);
    assert_eq!(rows.len(), 2);
    assert_eq!(line_count(&path), 2, "old three rows were truncated, two rerun");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn six_key_distinguishes_changed_model_think_effort() {
    let (board, items, encs) = fixture();
    let path = tmp_path("e");

    // Seed with model=oracle, think=true, effort=none.
    let s1 = RunSink::open(&path, ResumeMode::Default, &encs, &items, "oracle", true, "none")
        .expect("seed run opens");
    let _ = run_oracle(&board, &encs, &items, Some(&s1));
    drop(s1);

    // Any single change to the identity yields NO matches → resume errors (a changed run, not a
    // continuation of this one).
    assert!(
        RunSink::open(&path, ResumeMode::Resume, &encs, &items, "other-model", true, "none")
            .is_err(),
        "changed model must not match"
    );
    assert!(
        RunSink::open(&path, ResumeMode::Resume, &encs, &items, "oracle", false, "none").is_err(),
        "changed think must not match"
    );
    assert!(
        RunSink::open(&path, ResumeMode::Resume, &encs, &items, "oracle", true, "high").is_err(),
        "changed effort must not match"
    );

    // The identical identity DOES match — everything is already done, so resume skips all of it.
    let same = RunSink::open(&path, ResumeMode::Resume, &encs, &items, "oracle", true, "none")
        .expect("identical identity resumes");
    assert_eq!(same.skipped(), items.len(), "all trials recognized as done");
    let rows = run_oracle(&board, &encs, &items, Some(&same));
    drop(same);
    assert!(rows.is_empty(), "nothing left to run");
    assert_eq!(line_count(&path), items.len(), "file unchanged");

    let _ = std::fs::remove_file(&path);
}
