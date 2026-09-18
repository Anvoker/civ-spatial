//! `viewer-export` — turn a completed run's trace directory into ONE self-contained JSON file the
//! offline replay viewer loads (see `viewer/`). This is an ADDITIVE, read-only command: it never
//! touches the solver/generator/encoder logic and never calls a model. It reads the JSON-per-question
//! traces (schema in `civ_eval::trace`), resolves each referenced board by re-parsing its `.sav`
//! (re-applying any `#crop(...)`/`#fog(pN)` provenance suffix), and recovers each question's
//! referents/candidates.
//!
//! Referent/candidate recovery is FALLBACK-SAFE:
//!   * If a `run.json` manifest is present in the trace dir, regenerate the typed questions
//!     (`generate_questions`) and match by `item_id` for structured referents + full candidate
//!     option lists.
//!   * Otherwise fall back to parsing the `item_id` grammar (`civ_eval::question::Question::id`).
//!
//! `reasoning` is included per interactive turn when the trace carries it (older traces omit it).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use civ_core::{parse_save, Board};
use civ_eval::json::{quote, Json};
use civ_eval::{generate_questions, Answer, Difficulty, EvalItem, Question, Referent};

/// Region-summary sectors are 10x10 (mirrors the private `SECTOR` in `civ_eval::encoders`).
const SECTOR: i32 = 10;

/// Entry point for `civ viewer-export --trace-dir <path> [--out F] [--saves-dir D] [--no-fog]`.
/// When `no_fog` is set, any `#fog(pN)` provenance is ignored so the primary board is full-visibility
/// ground truth (no `perspective`, no `unfogged` companion) — used to build a fog-independent sample.
pub fn run(trace_dir: &str, out: &str, saves_dir: &str, no_fog: bool) -> Result<(), String> {
    let trace_dir = PathBuf::from(trace_dir);
    if !trace_dir.is_dir() {
        return Err(format!(
            "trace dir {} is not a directory",
            trace_dir.display()
        ));
    }
    let saves_dir = PathBuf::from(saves_dir);

    // Collect the per-question trace files (skip the manifest and any results file).
    let mut trace_files: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&trace_dir)
        .map_err(|e| format!("reading {}: {e}", trace_dir.display()))?
    {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if path.extension().and_then(|s| s.to_str()) == Some("json") && name != "run.json" {
            trace_files.push(path);
        }
    }
    trace_files.sort();
    if trace_files.is_empty() {
        return Err(format!(
            "no trace JSON files found in {}",
            trace_dir.display()
        ));
    }

    // Parse every trace up-front so we know the distinct boards and can regenerate per board. Keep
    // each trace's source FILENAME alongside it: it is the natural unique key for a question (one
    // trace file per question), used both to disambiguate identical-parameter duplicates in the
    // viewer's copy-identifier and as provenance.
    let mut traces: Vec<(String, Json)> = Vec::new();
    for path in &trace_files {
        let body = std::fs::read_to_string(path)
            .map_err(|e| format!("reading {}: {e}", path.display()))?;
        let fname = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        match Json::parse(&body) {
            Ok(v) => traces.push((fname, v)),
            Err(e) => eprintln!(
                "warning: skipping unparseable trace {}: {e}",
                path.display()
            ),
        }
    }

    // Resolve each distinct board once (base save + crop/fog provenance).
    let mut board_fields: Vec<String> = traces
        .iter()
        .filter_map(|(_, t)| t.get("board").and_then(Json::as_str).map(str::to_string))
        .collect();
    board_fields.sort();
    board_fields.dedup();

    let mut boards: BTreeMap<String, Board> = BTreeMap::new();
    let mut resolved_extra: BTreeMap<String, (Option<Board>, Option<String>)> = BTreeMap::new();
    for field in &board_fields {
        match resolve_board(field, &saves_dir, no_fog) {
            Ok(r) => {
                resolved_extra.insert(field.clone(), (r.unfogged, r.perspective));
                boards.insert(field.clone(), r.board);
            }
            Err(e) => eprintln!("warning: could not resolve board {field:?}: {e}"),
        }
    }

    // Optional manifest-driven regeneration: item_id -> typed EvalItem, per board.
    let regen = load_manifest(&trace_dir).map(|m| {
        let mut map: BTreeMap<(String, String), EvalItem> = BTreeMap::new();
        for (field, board) in &boards {
            // Regenerate with the SAME fog perspective the board was masked to, so player-relative
            // item ids match the fogged run's traces (fog-three-state-design §9 coherence). The
            // perspective (fogging player's name) was resolved alongside the board in `resolve_board`.
            let perspective = resolved_extra.get(field).and_then(|(_, p)| p.clone());
            for it in generate_questions(
                board,
                m.seed,
                m.per_kind,
                m.difficulty,
                perspective.as_deref(),
            ) {
                map.insert((field.clone(), it.id.clone()), it);
            }
        }
        map
    });
    if regen.is_some() {
        eprintln!(
            "viewer-export: run.json manifest found — using regenerated typed referents/candidates"
        );
    } else {
        eprintln!("viewer-export: no run.json manifest — recovering referents/candidates by parsing item_id");
    }

    // Emit boards.
    let mut out_buf = String::from("{\n\"boards\":{");
    for (i, (field, board)) in boards.iter().enumerate() {
        if i > 0 {
            out_buf.push(',');
        }
        out_buf.push_str(&quote(field));
        out_buf.push(':');
        let (unfogged, perspective) = resolved_extra
            .get(field)
            .map(|(u, p)| (u.as_ref(), p.as_deref()))
            .unwrap_or((None, None));
        out_buf.push_str(&board_json(field, board, unfogged, perspective));
    }
    out_buf.push_str("},\n\"questions\":[");

    // Emit questions.
    let mut model_name = String::new();
    for (i, (trace_file, t)) in traces.iter().enumerate() {
        if i > 0 {
            out_buf.push(',');
        }
        if model_name.is_empty() {
            if let Some(m) = t.get("model").and_then(Json::as_str) {
                model_name = m.to_string();
            }
        }
        let board_field = t.get("board").and_then(Json::as_str).unwrap_or("");
        let board = boards.get(board_field);
        let typed = regen
            .as_ref()
            .zip(t.get("item_id").and_then(Json::as_str))
            .and_then(|(m, id)| m.get(&(board_field.to_string(), id.to_string())));
        out_buf.push_str(&question_json(t, board, typed, trace_file));
    }
    out_buf.push_str("],\n");
    out_buf.push_str(&format!(
        "\"meta\":{{\"trace_dir\":{},\"model\":{},\"n_questions\":{},\"n_boards\":{}}}\n}}",
        quote(&trace_dir.to_string_lossy()),
        quote(&model_name),
        traces.len(),
        boards.len()
    ));

    std::fs::write(out, &out_buf).map_err(|e| format!("writing {out}: {e}"))?;
    println!(
        "viewer-export: wrote {out} — {} questions across {} board(s)",
        traces.len(),
        boards.len()
    );
    Ok(())
}

// --------------------------------------------------------------------------
// board resolution
// --------------------------------------------------------------------------

/// A board resolved from a trace `board` provenance field. `board` is exactly what the model saw
/// (fogged when a `#fog(pN)` suffix is present); `unfogged` is the SAME board with every transform
/// EXCEPT the fog mask applied (the ground truth), populated only when fog was applied so the viewer
/// can offer a fog on/off toggle. `perspective` is the fogging player's display name ("you").
struct Resolved {
    board: Board,
    unfogged: Option<Board>,
    perspective: Option<String>,
}

/// Re-parse the `.sav` named in a trace `board` field and re-apply its `#crop(...)`/`#fog(pN)`
/// provenance suffixes (in order), reproducing the exact board the run saw. When a fog mask is
/// applied, the pre-fog board is snapshotted as [`Resolved::unfogged`] ground truth.
fn resolve_board(field: &str, saves_dir: &Path, no_fog: bool) -> Result<Resolved, String> {
    let mut parts = field.split('#');
    let base = parts.next().ok_or("empty board field")?;
    let save_path = saves_dir.join(base);
    let mut board = parse_save(&save_path).map_err(|e| e.to_string())?;
    let mut unfogged: Option<Board> = None;
    let mut perspective: Option<String> = None;
    for transform in parts {
        if let Some(inner) = transform
            .strip_prefix("crop(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let nums: Vec<i32> = inner
                .split(',')
                .map(|s| s.trim().parse::<i32>())
                .collect::<Result<_, _>>()
                .map_err(|_| format!("bad crop provenance {transform:?}"))?;
            match nums.as_slice() {
                [x0, y0, w, h] => board = board.crop(*x0, *y0, *w, *h)?,
                _ => return Err(format!("crop provenance needs 4 ints: {transform:?}")),
            }
        } else if let Some(pid) = transform
            .strip_prefix("fog(p")
            .and_then(|s| s.strip_suffix(')'))
        {
            // `--no-fog`: ignore the mask entirely so the primary board stays full-visibility.
            if no_fog {
                continue;
            }
            let id: i32 = pid
                .parse()
                .map_err(|_| format!("bad fog provenance {transform:?}"))?;
            // Snapshot the pre-fog (ground-truth) board, and record the fogging player's name.
            perspective = board
                .players
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.name.clone());
            unfogged = Some(board.clone());
            board = board.mask_to_known(id)?;
        } else if !transform.is_empty() {
            return Err(format!("unknown board provenance suffix {transform:?}"));
        }
    }
    Ok(Resolved {
        board,
        unfogged,
        perspective,
    })
}

/// The full grid + cities + units for one board, plus (additively) fog metadata: `perspective`
/// (the fogging player's name, i.e. "you") and an `unfogged` ground-truth board when a fog mask was
/// applied. All fog fields are optional — an export without them is a full-visibility board, and the
/// viewer treats their absence as "no separate ground truth" (backward compatible).
fn board_json(
    field: &str,
    board: &Board,
    unfogged: Option<&Board>,
    perspective: Option<&str>,
) -> String {
    let mut s = format!(
        "{{\"source\":{},\"width\":{},\"height\":{},",
        quote(field),
        board.width,
        board.height
    );
    s.push_str(&board_contents_json(board));
    // Player roster: leader `name` (the owner-identity string on tiles/cities/units) paired with the
    // real `nation`. Freeciv defaults an unnamed AI leader to "Unassigned" (deduped Unassigned2/3/…),
    // so the viewer labels owners by nation to avoid showing those placeholders. Optional/additive.
    if !board.players.is_empty() {
        s.push_str(",\"players\":[");
        for (i, p) in board.players.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!(
                "{{\"name\":{},\"nation\":{}}}",
                quote(&p.name),
                quote(&p.nation)
            ));
        }
        s.push(']');
    }
    if let Some(name) = perspective {
        s.push_str(",\"perspective\":");
        s.push_str(&quote(name));
    }
    if let Some(uf) = unfogged {
        s.push_str(",\"unfogged\":{");
        s.push_str(&board_contents_json(uf));
        s.push('}');
    }
    s.push('}');
    s
}

/// The `"tiles":[…],"cities":[…],"units":[…]` triple for a board (no surrounding braces), shared by
/// the primary board object and the nested `unfogged` ground-truth object.
fn board_contents_json(board: &Board) -> String {
    let mut s = String::from("\"tiles\":[");
    let mut first = true;
    for row in &board.tiles {
        for t in row {
            if !first {
                s.push(',');
            }
            first = false;
            let extras: Vec<String> = t.extras.iter().map(|e| quote(e)).collect();
            let owner = match &t.owner {
                Some(o) => quote(o),
                None => "null".to_string(),
            };
            s.push_str(&format!(
                "{{\"x\":{},\"y\":{},\"terrain\":{},\"extras\":[{}],\"owner\":{}}}",
                t.x,
                t.y,
                quote(&t.terrain),
                extras.join(","),
                owner
            ));
        }
    }
    s.push_str("],\"cities\":[");
    for (i, c) in board.cities.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        // `improvements` (City Walls / Coastal Defense / Great Wall / Palace / …) are load-bearing for
        // the viewer's wall/coastal/Great-Wall/Palace math in city_defense/garrison_defense/
        // city_fall_prob — without them the tool playground would under-compute defense. Emit the set.
        let improvements: Vec<String> = c.improvements.iter().map(|m| quote(m)).collect();
        s.push_str(&format!(
            "{{\"x\":{},\"y\":{},\"id\":{},\"name\":{},\"owner\":{},\"size\":{},\"improvements\":[{}]}}",
            c.x,
            c.y,
            c.id,
            quote(&c.name),
            quote(&c.owner),
            c.size,
            improvements.join(",")
        ));
    }
    s.push_str("],\"units\":[");
    for (i, u) in board.units.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "{{\"x\":{},\"y\":{},\"id\":{},\"kind\":{},\"owner\":{},\"veteran\":{},\"hp\":{}}}",
            u.x,
            u.y,
            u.id,
            quote(&u.kind),
            quote(&u.owner),
            u.veteran,
            u.hp
        ));
    }
    s.push(']');
    s
}

// --------------------------------------------------------------------------
// per-question emission
// --------------------------------------------------------------------------

/// A resolved point the viewer can highlight.
struct Point {
    kind: &'static str, // "tile" | "city" | "unit"
    x: i32,
    y: i32,
    id: Option<i32>,
    label: Option<String>,
}

impl Point {
    fn to_json(&self) -> String {
        let mut s = format!(
            "{{\"kind\":{},\"x\":{},\"y\":{}",
            quote(self.kind),
            self.x,
            self.y
        );
        if let Some(id) = self.id {
            s.push_str(&format!(",\"id\":{id}"));
        }
        if let Some(l) = &self.label {
            s.push_str(&format!(",\"label\":{}", quote(l)));
        }
        s.push('}');
        s
    }
}

/// Aggregate stats summed across a trace's turns, for the viewer's analytics + per-question metrics.
struct TurnStats {
    prompt_tokens: u64,
    completion_tokens: u64,
    cached_tokens: u64,
    latency_ms: u64,
    n_turns: usize,
    n_tool_calls: usize,
    /// tool name -> count, summed over every turn's `response.tool_calls` (ALL tool-using encodings,
    /// not just interactive — e.g. raw-maxops fires count_terrain). Sorted by count desc, then name.
    tool_call_counts: Vec<(String, u64)>,
}

/// Sum per-turn `response.usage`/`latency_ms` and tally `response.tool_calls` names across a trace.
fn turn_stats(t: &Json) -> TurnStats {
    let mut prompt = 0u64;
    let mut completion = 0u64;
    let mut cached = 0u64;
    let mut latency = 0u64;
    let mut n_tool_calls = 0usize;
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let turns = t.get("turns").and_then(Json::as_array).unwrap_or(&[]);
    for turn in turns {
        let resp = turn.get("response");
        if let Some(u) = resp.and_then(|r| r.get("usage")) {
            prompt += u.get("prompt_tokens").and_then(Json::as_u64).unwrap_or(0);
            completion += u
                .get("completion_tokens")
                .and_then(Json::as_u64)
                .unwrap_or(0);
            cached += u.get("cached_tokens").and_then(Json::as_u64).unwrap_or(0);
        }
        latency += resp
            .and_then(|r| r.get("latency_ms"))
            .and_then(Json::as_u64)
            .unwrap_or(0);
        if let Some(calls) = resp
            .and_then(|r| r.get("tool_calls"))
            .and_then(Json::as_array)
        {
            for c in calls {
                if let Some(name) = c.get("name").and_then(Json::as_str) {
                    *counts.entry(name.to_string()).or_insert(0) += 1;
                    n_tool_calls += 1;
                }
            }
        }
    }
    // Most-called first; ties broken by name for determinism.
    let mut tool_call_counts: Vec<(String, u64)> = counts.into_iter().collect();
    tool_call_counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    TurnStats {
        prompt_tokens: prompt,
        completion_tokens: completion,
        cached_tokens: cached,
        latency_ms: latency,
        n_turns: turns.len(),
        n_tool_calls,
        tool_call_counts,
    }
}

fn question_json(
    t: &Json,
    board: Option<&Board>,
    typed: Option<&EvalItem>,
    trace_file: &str,
) -> String {
    let get = |k: &str| t.get(k).and_then(Json::as_str).unwrap_or("");
    let item_id = get("item_id");
    let category = get("category");
    let tier = get("tier");
    let encoding = get("encoding");
    let board_ref = get("board");
    let mode = get("mode");

    // Trim the question fragment for clean data (the static tail often starts with "\n\nQUESTION:").
    // The viewer also trims defensively; the full prompt is left intact.
    let question_text = question_text(t).trim().to_string();
    let full_prompt = full_prompt(t);
    let result = t.get("result");
    let status = result
        .and_then(|r| r.get("status"))
        .and_then(Json::as_str)
        .unwrap_or("");
    let expected = result
        .and_then(|r| r.get("expected"))
        .and_then(Json::as_str)
        .unwrap_or("");
    let got = result
        .and_then(|r| r.get("got"))
        .and_then(Json::as_str)
        .unwrap_or("");
    // The model's RAW completion text (and reasoning, if the trace carried it), recovered from the
    // last non-empty turn regardless of mode. `got` is the *parsed* answer and is empty whenever
    // parsing/scoring failed (status=invalid), so surfacing the raw text is what lets the viewer show
    // what the model actually said even when it couldn't be scored.
    let (answer_text, reasoning) = answer_text_and_reasoning(t);

    // Referents / candidates: typed extraction if available, else id-parse.
    let (referents, candidates, radius, options) = match (typed, board) {
        (Some(item), Some(b)) => structured(item, b),
        _ => match board {
            Some(b) => id_parsed(category, item_id, b),
            None => (Vec::new(), Vec::new(), None, Vec::new()),
        },
    };

    let refs_json: Vec<String> = referents.iter().map(Point::to_json).collect();
    let cands_json: Vec<String> = candidates.iter().map(Point::to_json).collect();
    let opts_json: Vec<String> = options.iter().map(|o| quote(o)).collect();

    let mut s = format!(
        "{{\"item_id\":{},\"category\":{},\"tier\":{},\"encoding\":{},\"board_ref\":{},\
         \"question_text\":{},\"model_answer\":{},\"answer_text\":{},\"correct_answer\":{},\"status\":{},\
         \"referents\":[{}],\"candidates\":[{}],\"options\":[{}]",
        quote(item_id),
        quote(category),
        quote(tier),
        quote(encoding),
        quote(board_ref),
        quote(&question_text),
        quote(got),
        quote(&answer_text),
        quote(expected),
        quote(status),
        refs_json.join(","),
        cands_json.join(","),
        opts_json.join(",")
    );
    if !full_prompt.is_empty() {
        s.push_str(&format!(",\"full_prompt\":{}", quote(&full_prompt)));
    }
    if let Some(r) = &reasoning {
        s.push_str(&format!(",\"reasoning\":{}", quote(r)));
    }
    if let Some(r) = radius {
        s.push_str(&format!(",\"radius\":{r}"));
    }
    // Trajectory for interactive modes.
    if mode == "interactive" {
        if let Some(b) = board {
            s.push_str(",\"trajectory\":");
            s.push_str(&trajectory_json(t, b));
        }
    }

    // Aggregate per-question metrics (ALL modes): summed usage/latency, turn/tool-call counts, and the
    // per-tool histogram used by the viewer's tool-call analytics. `cached_tokens` is the cache-hit
    // SUBSET of `prompt_tokens` (NOT additive); there is no total_tokens (total = prompt+completion).
    let st = turn_stats(t);
    s.push_str(&format!(
        ",\"latency_ms\":{},\"prompt_tokens\":{},\"completion_tokens\":{},\"cached_tokens\":{},\
         \"n_turns\":{},\"n_tool_calls\":{}",
        st.latency_ms,
        st.prompt_tokens,
        st.completion_tokens,
        st.cached_tokens,
        st.n_turns,
        st.n_tool_calls
    ));
    s.push_str(",\"tool_call_counts\":{");
    for (i, (name, c)) in st.tool_call_counts.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("{}:{}", quote(name), c));
    }
    s.push('}');
    // The source trace filename — the natural unique key for this question (see the copy-identifier).
    s.push_str(&format!(",\"trace_file\":{}", quote(trace_file)));

    s.push('}');
    s
}

/// Recover the model's raw completion text and (optional) reasoning from a trace, independent of
/// mode. Walks the turns from the end and takes the first non-empty `response.text` / `response
/// .reasoning`. For a static (one-shot) trial there is a single turn; for an interactive trial the
/// final assistant text is the model's answer turn.
fn answer_text_and_reasoning(t: &Json) -> (String, Option<String>) {
    let mut text = String::new();
    let mut reasoning: Option<String> = None;
    if let Some(turns) = t.get("turns").and_then(Json::as_array) {
        for turn in turns.iter().rev() {
            let resp = turn.get("response");
            if text.is_empty() {
                if let Some(tx) = resp.and_then(|r| r.get("text")).and_then(Json::as_str) {
                    if !tx.is_empty() {
                        text = tx.to_string();
                    }
                }
            }
            if reasoning.is_none() {
                if let Some(rx) = resp.and_then(|r| r.get("reasoning")).and_then(Json::as_str) {
                    if !rx.is_empty() {
                        reasoning = Some(rx.to_string());
                    }
                }
            }
            if !text.is_empty() && reasoning.is_some() {
                break;
            }
        }
    }
    (text, reasoning)
}

/// The FULL prompt the model received on turn 0 (not just the question fragment): for a static trial
/// that is `cacheable_prefix` + `tail` (the geometry/instructions block plus the question); for an
/// interactive trial it is every turn-0 message rendered as `[role]\n<content>` (the system
/// instructions + the board-exploration preamble/question). Empty when the trace has no turn 0.
fn full_prompt(t: &Json) -> String {
    let turns = t.get("turns").and_then(Json::as_array);
    let Some(first) = turns.and_then(|ts| ts.first()) else {
        return String::new();
    };
    let req = first.get("request");
    // Static: cacheable_prefix + tail.
    if let Some(prefix) = req
        .and_then(|r| r.get("cacheable_prefix"))
        .and_then(Json::as_str)
    {
        let tail = req
            .and_then(|r| r.get("tail"))
            .and_then(Json::as_str)
            .unwrap_or("");
        return format!("{prefix}{tail}");
    }
    // Interactive: concatenate the turn-0 messages, labeled by role.
    if let Some(msgs) = req.and_then(|r| r.get("messages")).and_then(Json::as_array) {
        let mut parts = Vec::new();
        for m in msgs {
            let role = m.get("role").and_then(Json::as_str).unwrap_or("");
            let content = m.get("content").and_then(Json::as_str).unwrap_or("");
            parts.push(format!("[{role}]\n{content}"));
        }
        return parts.join("\n\n");
    }
    String::new()
}

/// The question text: interactive = first user message; static = the `tail` of turn 0.
fn question_text(t: &Json) -> String {
    let turns = t.get("turns").and_then(Json::as_array);
    let Some(turns) = turns else {
        return String::new();
    };
    let Some(first) = turns.first() else {
        return String::new();
    };
    let req = first.get("request");
    // Static: request has `tail`.
    if let Some(tail) = req.and_then(|r| r.get("tail")).and_then(Json::as_str) {
        return tail.to_string();
    }
    // Interactive: request has `messages`; take the first user message.
    if let Some(msgs) = req.and_then(|r| r.get("messages")).and_then(Json::as_array) {
        for m in msgs {
            if m.get("role").and_then(Json::as_str) == Some("user") {
                return m
                    .get("content")
                    .and_then(Json::as_str)
                    .unwrap_or("")
                    .to_string();
            }
        }
    }
    String::new()
}

// --------------------------------------------------------------------------
// trajectory
// --------------------------------------------------------------------------

/// A fetching tool's region, if the call fetches board tiles.
fn call_region(name: &str, args: &str, board: &Board) -> Option<(i32, i32, i32, i32)> {
    let a = Json::parse(args).ok()?;
    let int = |k: &str| a.get(k).and_then(json_i32);
    match name {
        "scan" | "scan_grid" => Some((int("x0")?, int("y0")?, int("x1")?, int("y1")?)),
        "get_tile" => {
            let x = int("x")?;
            let y = int("y")?;
            Some((x, y, x, y))
        }
        "scan_region" => {
            // The interactive-maxops per-tile terrain window: a center (x, y) → the shared
            // sliding-clamp window (the same bbox region_summary uses).
            Some(civ_eval::region_window(board, int("x")?, int("y")?))
        }
        "region_summary" => {
            // New form: a center (x, y) → the shared sliding-clamp window (exactly what the tool
            // summarizes). Old traces addressed a fixed sector "RrCc" — keep decoding those too.
            if let (Some(x), Some(y)) = (int("x"), int("y")) {
                Some(civ_eval::region_window(board, x, y))
            } else {
                let sector = a.get("sector").and_then(Json::as_str)?;
                sector_bbox(sector, board)
            }
        }
        _ => None,
    }
}

/// Parse a sector id like "R2C4" into its inclusive tile bbox (SECTOR=10, edge-truncated).
fn sector_bbox(sector: &str, board: &Board) -> Option<(i32, i32, i32, i32)> {
    let s = sector.trim();
    let rest = s.strip_prefix('R').or_else(|| s.strip_prefix('r'))?;
    let ci = rest.find(['C', 'c'])?;
    let sy: i32 = rest[..ci].parse().ok()?;
    let sx: i32 = rest[ci + 1..].parse().ok()?;
    let x0 = sx * SECTOR;
    let y0 = sy * SECTOR;
    let x1 = ((sx + 1) * SECTOR).min(board.width) - 1;
    let y1 = ((sy + 1) * SECTOR).min(board.height) - 1;
    if x0 > x1 || y0 > y1 {
        return None;
    }
    Some((x0, y0, x1, y1))
}

fn json_i32(v: &Json) -> Option<i32> {
    match v {
        Json::Num(n) if n.is_finite() => Some(*n as i32),
        _ => None,
    }
}

fn trajectory_json(t: &Json, board: &Board) -> String {
    let Some(turns) = t.get("turns").and_then(Json::as_array) else {
        return "[]".to_string();
    };
    let mut out = String::from("[");
    for (i, turn) in turns.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let resp = turn.get("response");
        let text = resp
            .and_then(|r| r.get("text"))
            .and_then(Json::as_str)
            .unwrap_or("");
        let reasoning = resp.and_then(|r| r.get("reasoning")).and_then(Json::as_str);
        let empty: Vec<Json> = Vec::new();
        let calls = resp
            .and_then(|r| r.get("tool_calls"))
            .and_then(Json::as_array)
            .unwrap_or(&empty);

        let mut calls_json = Vec::new();
        let mut regions_json = Vec::new();
        for c in calls {
            let name = c.get("name").and_then(Json::as_str).unwrap_or("");
            let args = c.get("args").and_then(Json::as_str).unwrap_or("");
            let region = call_region(name, args, board);
            let region_field = match region {
                Some((x0, y0, x1, y1)) => {
                    let bbox = format!("{{\"x0\":{x0},\"y0\":{y0},\"x1\":{x1},\"y1\":{y1}}}");
                    regions_json.push(bbox.clone());
                    bbox
                }
                None => "null".to_string(),
            };
            calls_json.push(format!(
                "{{\"name\":{},\"args\":{},\"region\":{}}}",
                quote(name),
                quote(args),
                region_field
            ));
        }
        out.push_str(&format!(
            "{{\"tool_calls\":[{}],\"fetched_regions\":[{}],\"text\":{}",
            calls_json.join(","),
            regions_json.join(","),
            quote(text)
        ));
        if let Some(r) = reasoning {
            out.push_str(&format!(",\"reasoning\":{}", quote(r)));
        }
        out.push('}');
    }
    out.push(']');
    out
}

// --------------------------------------------------------------------------
// structured extraction from a typed (regenerated) question
// --------------------------------------------------------------------------

fn referent_point(r: &Referent, board: &Board, kind_hint: &'static str) -> Option<Point> {
    let (x, y) = r.resolve(board)?;
    let (kind, id, label) = match r {
        Referent::Tile { .. } => (kind_hint, None, None),
        Referent::City { id } => (
            "city",
            Some(*id),
            board
                .cities
                .iter()
                .find(|c| c.id == *id)
                .map(|c| c.name.clone()),
        ),
        Referent::Unit { id } => (
            "unit",
            Some(*id),
            board
                .units
                .iter()
                .find(|u| u.id == *id)
                .map(|u| u.kind.clone()),
        ),
    };
    Some(Point {
        kind,
        x,
        y,
        id,
        label,
    })
}

fn structured(
    item: &EvalItem,
    board: &Board,
) -> (Vec<Point>, Vec<Point>, Option<i32>, Vec<String>) {
    let mut refs = Vec::new();
    let mut cands = Vec::new();
    let mut radius = None;
    let push_ref = |v: &mut Vec<Point>, r: &Referent| {
        if let Some(p) = referent_point(r, board, "tile") {
            v.push(p);
        }
    };
    let push_cands = |v: &mut Vec<Point>, cs: &[Referent]| {
        for c in cs {
            if let Some(p) = referent_point(c, board, "tile") {
                v.push(p);
            }
        }
    };
    match &item.question {
        Question::TerrainAt { tile } => push_ref(&mut refs, tile),
        Question::AdjacentTerrain { origin, .. } => push_ref(&mut refs, origin),
        Question::DirectionTo { origin, target } => {
            push_ref(&mut refs, origin);
            push_ref(&mut refs, target);
        }
        Question::Distance { a, b } => {
            push_ref(&mut refs, a);
            push_ref(&mut refs, b);
        }
        Question::NearestResource { from, .. } => push_ref(&mut refs, from),
        Question::CountTerrainInRadius {
            center, radius: r, ..
        } => {
            push_ref(&mut refs, center);
            radius = Some(*r);
        }
        Question::Reachable { from, to, .. } => {
            push_ref(&mut refs, from);
            push_ref(&mut refs, to);
        }
        Question::UnitStrengthCompare { a, b, .. } => {
            push_ref(&mut refs, a);
            push_ref(&mut refs, b);
        }
        Question::CityDefenseCompare { a, b } => {
            push_ref(&mut refs, a);
            push_ref(&mut refs, b);
        }
        Question::BestSiteChoice {
            candidates,
            radius: r,
            ..
        } => {
            push_cands(&mut cands, candidates);
            radius = Some(*r);
        }
        Question::NearestOwnedResource { .. } => {}
        Question::SettleSiteChoice { candidates, .. } => push_cands(&mut cands, candidates),
        Question::RetreatChoice { unit, candidates } => {
            push_ref(&mut refs, unit);
            push_cands(&mut cands, candidates);
        }
        Question::ForwardPostingChoice {
            unit, candidates, ..
        } => {
            push_ref(&mut refs, unit);
            push_cands(&mut cands, candidates);
        }
        Question::CityThreatChoice { candidates, .. } => push_cands(&mut cands, candidates),
        Question::ReachableNearestResource { unit, .. } => push_ref(&mut refs, unit),
        Question::CanVacateCity { city } => push_ref(&mut refs, city),
        Question::AssaultTargetChoice { candidates, .. } => push_cands(&mut cands, candidates),
        Question::TriageReinforceChoice {
            reserve,
            candidates,
            ..
        } => {
            push_ref(&mut refs, reserve);
            push_cands(&mut cands, candidates);
        }
        Question::CompareTwoAttacks {
            attacker_a,
            target_a,
            attacker_b,
            target_b,
        } => {
            push_ref(&mut refs, attacker_a);
            push_ref(&mut refs, target_a);
            push_ref(&mut refs, attacker_b);
            push_ref(&mut refs, target_b);
        }
        Question::ConstraintSiteChoice {
            center, radius: r, ..
        } => {
            push_ref(&mut refs, center);
            radius = Some(*r);
        }
        Question::FoggedAssault {
            attackers, city, ..
        } => {
            push_ref(&mut refs, city);
            push_cands(&mut cands, attackers);
        }
        Question::SurpriseStrikeExposure { candidates, .. } => push_cands(&mut cands, candidates),
        Question::HiddenForceLocalization {
            centers, radius: r, ..
        } => {
            push_cands(&mut cands, centers);
            radius = Some(*r);
        }
    }
    let options = answer_options(&item.answer);
    (refs, cands, radius, options)
}

fn answer_options(a: &Answer) -> Vec<String> {
    match a {
        Answer::Choice { options, .. } => options.clone(),
        Answer::ChoiceSet { options, .. } => options.clone(),
        Answer::Compare3 { a, b, .. } => vec![a.clone(), b.clone(), "incomparable".to_string()],
        _ => Vec::new(),
    }
}

// --------------------------------------------------------------------------
// fallback: recover referents/candidates by parsing the item_id grammar
// --------------------------------------------------------------------------

/// Parse a single referent key: `X,Y` (tile), `cN` (city), `uN` (unit). Resolves to coords.
fn parse_key(tok: &str, board: &Board) -> Option<Point> {
    let tok = tok.trim();
    if let Some(rest) = tok.strip_prefix('c') {
        if let Ok(id) = rest.parse::<i32>() {
            let c = board.cities.iter().find(|c| c.id == id)?;
            return Some(Point {
                kind: "city",
                x: c.x,
                y: c.y,
                id: Some(id),
                label: Some(c.name.clone()),
            });
        }
    }
    if let Some(rest) = tok.strip_prefix('u') {
        if let Ok(id) = rest.parse::<i32>() {
            let u = board.units.iter().find(|u| u.id == id)?;
            return Some(Point {
                kind: "unit",
                x: u.x,
                y: u.y,
                id: Some(id),
                label: Some(u.kind.clone()),
            });
        }
    }
    // A "X,Y" tile key (the caller passes the already-joined pair).
    let mut it = tok.split(',');
    let x = it.next()?.trim().parse::<i32>().ok()?;
    let y = it.next()?.trim().parse::<i32>().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some(Point {
        kind: "tile",
        x,
        y,
        id: None,
        label: None,
    })
}

/// Parse a candidate list segment (comma-joined keys) into points. Numeric tokens pair up into
/// tile coords; `cN`/`uN` tokens are id keys.
fn parse_key_list(seg: &str, board: &Board) -> Vec<Point> {
    let toks: Vec<&str> = seg
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let tok = toks[i];
        if tok.starts_with('c') || tok.starts_with('u') {
            if let Some(p) = parse_key(tok, board) {
                out.push(p);
            }
            i += 1;
        } else {
            // Expect a numeric x,y pair.
            if i + 1 < toks.len() {
                let pair = format!("{},{}", toks[i], toks[i + 1]);
                if let Some(p) = parse_key(&pair, board) {
                    out.push(p);
                }
            }
            i += 2;
        }
    }
    out
}

fn id_parsed(
    category: &str,
    item_id: &str,
    board: &Board,
) -> (Vec<Point>, Vec<Point>, Option<i32>, Vec<String>) {
    let segs: Vec<&str> = item_id.split(':').collect();
    let mut refs = Vec::new();
    let mut cands = Vec::new();
    let mut radius = None;
    let seg = |i: usize| segs.get(i).copied().unwrap_or("");
    let push_ref = |k: &str, refs: &mut Vec<Point>| {
        if let Some(p) = parse_key(k, board) {
            refs.push(p);
        }
    };
    match category {
        "terrain" | "adjacency" | "nearest" => push_ref(seg(1), &mut refs),
        "direction" | "distance" | "reachability" | "unit-strength" | "city-defense" => {
            push_ref(seg(1), &mut refs);
            push_ref(seg(2), &mut refs);
        }
        "region-count" => {
            push_ref(seg(1), &mut refs);
            radius = seg(2).parse::<i32>().ok();
        }
        "best-site" => {
            // best-site:terrain:radius:cands
            radius = seg(2).parse::<i32>().ok();
            cands = parse_key_list(seg(3), board);
        }
        "settle-site" | "t3-threat" | "adv-assault-target" | "constraint-site" => {
            // <cat>:player:cands
            cands = parse_key_list(seg(2), board);
        }
        "t3-retreat" => {
            // t3-retreat:unit:cands
            push_ref(seg(1), &mut refs);
            cands = parse_key_list(seg(2), board);
        }
        "triage-reinforce" => {
            // triage-reinforce:player:reserve:cands
            push_ref(seg(2), &mut refs);
            cands = parse_key_list(seg(3), board);
        }
        "reachable-nearest" => push_ref(seg(1), &mut refs),
        "cf-vacate" => push_ref(seg(1), &mut refs),
        "compare-two-attacks" => {
            for i in 1..=4 {
                push_ref(seg(i), &mut refs);
            }
        }
        "nearest-owned" => {}
        _ => {}
    }
    (refs, cands, radius, Vec::new())
}

// --------------------------------------------------------------------------
// manifest
// --------------------------------------------------------------------------

struct Manifest {
    seed: u64,
    per_kind: usize,
    difficulty: Difficulty,
}

/// Read an optional `run.json` manifest `{seed, per_kind, difficulty}` from the trace dir.
/// Fallback-safe: any read/parse failure returns `None` (the caller then uses id-parsing).
fn load_manifest(trace_dir: &Path) -> Option<Manifest> {
    let path = trace_dir.join("run.json");
    let body = std::fs::read_to_string(&path).ok()?;
    let v = Json::parse(&body).ok()?;
    let seed = v.get("seed").and_then(Json::as_u64).unwrap_or(1);
    let per_kind = v.get("per_kind").and_then(Json::as_u64).unwrap_or(12) as usize;
    let difficulty = v
        .get("difficulty")
        .and_then(Json::as_str)
        .and_then(Difficulty::from_name)
        .unwrap_or_else(Difficulty::hard);
    Some(Manifest {
        seed,
        per_kind,
        difficulty,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use civ_core::{City, Tile, Unit};
    use std::collections::{BTreeSet, HashMap};

    /// A tiny 5x5 all-Grassland board with a city (id 7) at (2,2) and a unit (id 3) at (1,1).
    fn fixture_board() -> Board {
        let mut tiles = Vec::new();
        for y in 0..5 {
            let mut row = Vec::new();
            for x in 0..5 {
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
            width: 5,
            height: 5,
            tiles,
            cities: vec![City {
                x: 2,
                y: 2,
                id: 7,
                name: "Testville".to_string(),
                owner: "Alice".to_string(),
                size: 4,
                improvements: BTreeSet::new(),
            }],
            units: vec![Unit {
                x: 1,
                y: 1,
                id: 3,
                kind: "Warriors".to_string(),
                owner: "Alice".to_string(),
                veteran: 0,
                hp: 10,
            }],
            players: Vec::new(),
            known: HashMap::new(),
            visibility: None,
            ruleset: None,
            turn: None,
            source: "fixture.sav".to_string(),
        }
    }

    #[test]
    fn id_parse_recovers_tile_referent() {
        let b = fixture_board();
        let (refs, cands, radius, _) = id_parsed("region-count", "region-count:3,4:2:Forest", &b);
        assert_eq!(refs.len(), 1);
        assert_eq!((refs[0].x, refs[0].y), (3, 4));
        assert_eq!(radius, Some(2));
        assert!(cands.is_empty());
    }

    #[test]
    fn id_parse_recovers_city_and_unit_keys() {
        let b = fixture_board();
        let (refs, _, _, _) = id_parsed("cf-vacate", "cf-vacate:c7", &b);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].kind, "city");
        assert_eq!((refs[0].x, refs[0].y), (2, 2));

        let (refs, cands, _, _) = id_parsed("t3-retreat", "t3-retreat:u3:0,0,4,4", &b);
        assert_eq!(refs[0].kind, "unit");
        assert_eq!((refs[0].x, refs[0].y), (1, 1));
        // Candidate list "0,0,4,4" -> two tiles (0,0) and (4,4).
        assert_eq!(cands.len(), 2);
        assert_eq!((cands[0].x, cands[0].y), (0, 0));
        assert_eq!((cands[1].x, cands[1].y), (4, 4));
    }

    #[test]
    fn sector_bbox_is_edge_truncated() {
        let b = fixture_board();
        // R0C0 on a 5x5 board is the whole board (SECTOR=10 truncated to 4,4).
        assert_eq!(sector_bbox("R0C0", &b), Some((0, 0, 4, 4)));
        // A sector fully off the board yields None.
        assert_eq!(sector_bbox("R5C5", &b), None);
    }

    #[test]
    fn answer_text_recovers_raw_completion_when_got_is_empty() {
        // A trace whose parse failed (got="") but whose model DID emit text: the viewer must still
        // be able to show what the model said. We recover it from the last non-empty turn.
        let trace = Json::parse(
            "{\"turns\":[{\"response\":{\"text\":\"I think the answer is (3, 4) maybe.\",\
             \"reasoning\":\"let me count tiles\"}}],\"result\":{\"status\":\"invalid\",\"got\":\"\"}}",
        )
        .unwrap();
        let (text, reasoning) = answer_text_and_reasoning(&trace);
        assert_eq!(text, "I think the answer is (3, 4) maybe.");
        assert_eq!(reasoning.as_deref(), Some("let me count tiles"));
    }

    #[test]
    fn answer_text_takes_last_nonempty_turn_and_omits_absent_reasoning() {
        let trace = Json::parse(
            "{\"turns\":[{\"response\":{\"text\":\"scanning...\"}},\
             {\"response\":{\"text\":\"Answer: Forest\"}}]}",
        )
        .unwrap();
        let (text, reasoning) = answer_text_and_reasoning(&trace);
        assert_eq!(text, "Answer: Forest");
        assert!(reasoning.is_none());
    }

    #[test]
    fn turn_stats_sums_usage_latency_and_tallies_tool_calls_across_turns() {
        // Two turns: the first fires two tools (count_terrain twice via separate calls), the second
        // fires one; usage/latency sum; cached is the prompt subset (summed, not additive to total).
        let t = Json::parse(
            "{\"turns\":[\
               {\"response\":{\"usage\":{\"prompt_tokens\":100,\"completion_tokens\":5,\"cached_tokens\":80},\
                 \"latency_ms\":120,\"tool_calls\":[{\"name\":\"count_terrain\",\"args\":\"{}\"},\
                 {\"name\":\"distance\",\"args\":\"{}\"}]}},\
               {\"response\":{\"usage\":{\"prompt_tokens\":150,\"completion_tokens\":7,\"cached_tokens\":120},\
                 \"latency_ms\":80,\"tool_calls\":[{\"name\":\"count_terrain\",\"args\":\"{}\"}]}}\
             ]}",
        )
        .unwrap();
        let st = turn_stats(&t);
        assert_eq!(st.prompt_tokens, 250);
        assert_eq!(st.completion_tokens, 12);
        assert_eq!(st.cached_tokens, 200);
        assert_eq!(st.latency_ms, 200);
        assert_eq!(st.n_turns, 2);
        assert_eq!(st.n_tool_calls, 3);
        // Most-called first: count_terrain (2) before distance (1).
        assert_eq!(
            st.tool_call_counts,
            vec![
                ("count_terrain".to_string(), 2),
                ("distance".to_string(), 1)
            ]
        );
    }

    #[test]
    fn turn_stats_handles_a_static_no_tool_trace() {
        let t = Json::parse(
            "{\"turns\":[{\"response\":{\"usage\":{\"prompt_tokens\":40,\"completion_tokens\":2,\"cached_tokens\":0},\"latency_ms\":10,\"text\":\"Answer: X\"}}]}",
        )
        .unwrap();
        let st = turn_stats(&t);
        assert_eq!(
            (
                st.prompt_tokens,
                st.completion_tokens,
                st.n_tool_calls,
                st.n_turns
            ),
            (40, 2, 0, 1)
        );
        assert!(st.tool_call_counts.is_empty());
    }

    #[test]
    fn call_region_parses_fetching_tools() {
        let b = fixture_board();
        assert_eq!(
            call_region("scan", "{\"x0\":1,\"y0\":1,\"x1\":3,\"y1\":3}", &b),
            Some((1, 1, 3, 3))
        );
        assert_eq!(
            call_region("get_tile", "{\"x\":2,\"y\":4}", &b),
            Some((2, 4, 2, 4))
        );
        // scan_region decodes to the same sliding-clamp window as region_summary's center form.
        assert_eq!(
            call_region("scan_region", "{\"x\":2,\"y\":2}", &b),
            Some(civ_eval::region_window(&b, 2, 2))
        );
        // A non-fetching op has no region.
        assert_eq!(call_region("travel_turns", "{\"a\":1}", &b), None);
    }
}
