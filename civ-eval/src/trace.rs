//! Trajectory tracing: persist the *full* LLM input/output per question to disk as JSON.
//!
//! The result JSONL only keeps aggregate metrics (tokens/turns/calls). This module is the
//! side-channel that saves the actual prompts, tool calls, tool results, and completions so a run
//! can be inspected turn by turn. It lives at the runner level (not inside a provider) so it works
//! for any [`Model`]/[`ChatModel`], and is **not** behind the `remote` feature so it stays testable
//! offline.
//!
//! Design contract: tracing is a pure side-channel. It never changes scoring, token accounting,
//! results JSONL, or determinism, and a disk error only logs a warning — it never aborts a run.
//!
//! One JSON file is written per question, named `<encoding>__<item_id>.json` (both filesystem-
//! sanitized, since item_ids contain `:`/`,`). The sanitizer maps `:` and `,` to *different* safe
//! characters (`:`→`_`, `,`→`-`), so two ids that differ only in `:`/`,` placement — the id
//! separator vs. a coordinate/candidate comma — map to distinct filenames instead of silently
//! overwriting each other. Parallel workers therefore write disjoint files with no lock.
//!
//! File schema (see the tests for a concrete example):
//! ```text
//! { mode, model, encoding, board, item_id, category, tier,
//!   turns: [ { request, response }, ... ],
//!   result: { status, expected, got } }
//! ```
//! For a static (one-shot) trial `request` is `{ cacheable_prefix, tail }` and `response` is
//! `{ text, usage, latency_ms }`. For an interactive trial `request` is `{ messages, tools }`
//! (the full conversation actually sent that turn) and `response` adds `tool_calls`.

use std::path::{Path, PathBuf};

use crate::json::quote;
use crate::model::{ChatReply, Message, Reply, ToolCall, ToolDef, Usage};

/// Metadata identifying one traced question (kept in a struct to keep [`Tracer::begin`] tidy).
pub struct TraceMeta<'a> {
    pub mode: &'a str,
    pub model: &'a str,
    pub encoding: &'a str,
    pub board: &'a str,
    pub item_id: &'a str,
    pub category: &'a str,
    pub tier: &'a str,
}

/// Per-run manifest, written ONCE as `run.json` into the same trace directory the run creates. It
/// captures exactly the parameters needed to REGENERATE the fully-typed question set and match it
/// back to traces by `item_id`: given `run.json` + the `.sav`, a consumer reloads the board (applying
/// `crop`/`fog_player`), calls `generate_questions(board, seed, per_kind, difficulty)`, and recovers
/// the structured referents + candidate option sets. Secrets (API key, base URL) are deliberately
/// NOT recorded. Serialized with the in-house [`crate::json`] writer (no serde).
#[derive(Debug, Clone)]
pub struct RunManifest {
    /// The run id (the trailing segment of the trace dir name for the default layout).
    pub run_id: String,
    /// RNG seed passed to `generate_questions`.
    pub seed: u64,
    /// Questions generated per kind.
    pub per_kind: u64,
    /// Difficulty tier name (`"easy"`/`"hard"`) — reconstruct via `Difficulty::from_name`.
    pub difficulty: String,
    /// `--kinds` filter, if any (absent = all kinds).
    pub kinds: Option<Vec<String>>,
    /// Path to the source `.sav` (the raw board, before transforms).
    pub board_path: String,
    /// The transformed board's `source` string (encodes crop/fog provenance).
    pub board_source: String,
    /// `--crop x0,y0,w,h`, if applied.
    pub crop: Option<(i32, i32, i32, i32)>,
    /// `--fog <player>` mask, if applied.
    pub fog_player: Option<i32>,
    /// Encodings/surfaces exercised this run.
    pub encodings: Vec<String>,
    /// Model label.
    pub model: String,
    /// Whether the reasoning toggle was on.
    pub think: bool,
    /// In-flight concurrency.
    pub concurrency: u64,
}

impl RunManifest {
    /// Serialize to a compact JSON object with the in-house writer (no serde).
    pub fn to_json(&self) -> String {
        let arr = |items: &[String]| -> String {
            let parts: Vec<String> = items.iter().map(|s| quote(s)).collect();
            format!("[{}]", parts.join(","))
        };
        let kinds = match &self.kinds {
            Some(k) => arr(k),
            None => "null".to_string(),
        };
        let crop = match self.crop {
            Some((x0, y0, w, h)) => {
                format!("{{\"x0\":{x0},\"y0\":{y0},\"w\":{w},\"h\":{h}}}")
            }
            None => "null".to_string(),
        };
        let fog = match self.fog_player {
            Some(p) => p.to_string(),
            None => "null".to_string(),
        };
        format!(
            "{{\"run_id\":{},\"seed\":{},\"per_kind\":{},\"difficulty\":{},\"kinds\":{},\
             \"board_path\":{},\"board_source\":{},\"crop\":{},\"fog_player\":{},\
             \"encodings\":{},\"model\":{},\"think\":{},\"concurrency\":{}}}",
            quote(&self.run_id),
            self.seed,
            self.per_kind,
            quote(&self.difficulty),
            kinds,
            quote(&self.board_path),
            quote(&self.board_source),
            crop,
            fog,
            arr(&self.encodings),
            quote(&self.model),
            self.think,
            self.concurrency,
        )
    }

    /// Write `run.json` into `dir`. Pure side-channel: a disk error only warns, never aborts.
    pub fn write_to(&self, dir: &Path) {
        let path = dir.join("run.json");
        if let Err(e) = std::fs::write(&path, self.to_json()) {
            eprintln!("warning: writing run manifest {}: {e}", path.display());
        }
    }
}

/// Owns the trace output directory. Cheap to share across threads (`&Tracer` is `Sync`): each
/// question writes its own file, so there is no shared mutable state and no lock.
#[derive(Debug, Clone)]
pub struct Tracer {
    dir: PathBuf,
}

impl Tracer {
    /// Create a tracer rooted at `dir`, creating the directory tree. Returns `None` (after logging
    /// a warning) if the directory can't be created, so a disk problem disables tracing rather than
    /// crashing the run.
    pub fn new(dir: impl Into<PathBuf>) -> Option<Tracer> {
        let dir = dir.into();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            eprintln!(
                "warning: trace dir {} could not be created: {e}; tracing disabled",
                dir.display()
            );
            return None;
        }
        Some(Tracer { dir })
    }

    /// The output directory (for logging).
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Begin capturing one question's trajectory. Record turns on the returned builder, then call
    /// [`TraceBuilder::finish`] to write the file.
    pub fn begin<'a>(&'a self, meta: &TraceMeta) -> TraceBuilder<'a> {
        TraceBuilder {
            tracer: self,
            mode: meta.mode.to_string(),
            model: meta.model.to_string(),
            encoding: meta.encoding.to_string(),
            board: meta.board.to_string(),
            item_id: meta.item_id.to_string(),
            category: meta.category.to_string(),
            tier: meta.tier.to_string(),
            turns: Vec::new(),
        }
    }

    /// Write one trace file. Failures log a warning and are otherwise ignored (side-channel).
    fn write(&self, encoding: &str, item_id: &str, body: &str) {
        let name = format!("{}__{}.json", sanitize(encoding), sanitize(item_id));
        let path = self.dir.join(name);
        if let Err(e) = std::fs::write(&path, body) {
            eprintln!("warning: writing trace {}: {e}", path.display());
        }
    }
}

/// Accumulates the ordered turns of one question, then serializes them to a file on [`Self::finish`].
pub struct TraceBuilder<'a> {
    tracer: &'a Tracer,
    mode: String,
    model: String,
    encoding: String,
    board: String,
    item_id: String,
    category: String,
    tier: String,
    /// Pre-serialized `{ "request": ..., "response": ... }` objects, in send order.
    turns: Vec<String>,
}

impl TraceBuilder<'_> {
    /// Record one static (one-shot) turn: the prompt split at the cache boundary and its reply.
    pub fn static_turn(&mut self, cacheable_prefix: &str, tail: &str, reply: &Reply) {
        let request = format!(
            "{{\"cacheable_prefix\":{},\"tail\":{}}}",
            quote(cacheable_prefix),
            quote(tail)
        );
        let response = format!(
            "{{\"text\":{},\"reasoning\":{},\"usage\":{},\"latency_ms\":{}}}",
            quote(&reply.text),
            opt_str_json(&reply.reasoning),
            usage_json(&reply.usage),
            reply.latency_ms
        );
        self.turns
            .push(format!("{{\"request\":{request},\"response\":{response}}}"));
    }

    /// Record one interactive turn: the exact conversation sent (`messages` + available `tools`) and
    /// the assistant reply (text and/or tool calls, with usage). Call this right after `chat`, before
    /// appending the assistant/tool-result messages, so the request reflects what was actually sent.
    pub fn chat_turn(&mut self, messages: &[Message], tools: &[ToolDef], reply: &ChatReply) {
        let msgs: Vec<String> = messages.iter().map(message_json).collect();
        let tool_names: Vec<String> = tools.iter().map(|t| quote(t.name)).collect();
        let request = format!(
            "{{\"messages\":[{}],\"tools\":[{}]}}",
            msgs.join(","),
            tool_names.join(",")
        );
        let text = match &reply.text {
            Some(t) => quote(t),
            None => "null".to_string(),
        };
        let response = format!(
            "{{\"text\":{},\"reasoning\":{},\"tool_calls\":{},\"usage\":{},\"latency_ms\":{}}}",
            text,
            opt_str_json(&reply.reasoning),
            tool_calls_json(&reply.tool_calls),
            usage_json(&reply.usage),
            reply.latency_ms
        );
        self.turns
            .push(format!("{{\"request\":{request},\"response\":{response}}}"));
    }

    /// Finish the trace: append the scored outcome and write the file.
    pub fn finish(self, status: &str, expected: &str, got: &str) {
        let result = format!(
            "{{\"status\":{},\"expected\":{},\"got\":{}}}",
            quote(status),
            quote(expected),
            quote(got)
        );
        let body = format!(
            "{{\"mode\":{},\"model\":{},\"encoding\":{},\"board\":{},\"item_id\":{},\
             \"category\":{},\"tier\":{},\"turns\":[{}],\"result\":{}}}",
            quote(&self.mode),
            quote(&self.model),
            quote(&self.encoding),
            quote(&self.board),
            quote(&self.item_id),
            quote(&self.category),
            quote(&self.tier),
            self.turns.join(","),
            result
        );
        self.tracer.write(&self.encoding, &self.item_id, &body);
    }
}

fn usage_json(u: &Usage) -> String {
    format!(
        "{{\"prompt_tokens\":{},\"completion_tokens\":{},\"cached_tokens\":{}}}",
        u.prompt_tokens, u.completion_tokens, u.cached_tokens
    )
}

/// Serialize an optional string as a JSON string or `null`.
fn opt_str_json(s: &Option<String>) -> String {
    match s {
        Some(v) => quote(v),
        None => "null".to_string(),
    }
}

/// Serialize a tool call. `args` is the model's raw arguments JSON, embedded as a *string* (not
/// inlined) so a malformed/partial argument blob can never corrupt the trace file.
fn tool_call_json(tc: &ToolCall) -> String {
    format!(
        "{{\"id\":{},\"name\":{},\"args\":{}}}",
        quote(&tc.id),
        quote(&tc.name),
        quote(&tc.args_json)
    )
}

fn tool_calls_json(calls: &[ToolCall]) -> String {
    let items: Vec<String> = calls.iter().map(tool_call_json).collect();
    format!("[{}]", items.join(","))
}

fn message_json(m: &Message) -> String {
    let mut s = format!(
        "{{\"role\":{},\"content\":{}",
        quote(m.role.as_str()),
        quote(&m.content)
    );
    if !m.tool_calls.is_empty() {
        s.push_str(",\"tool_calls\":");
        s.push_str(&tool_calls_json(&m.tool_calls));
    }
    if let Some(id) = &m.tool_call_id {
        s.push_str(",\"tool_call_id\":");
        s.push_str(&quote(id));
    }
    s.push('}');
    s
}

/// Map a string to a safe filename fragment: keep `[A-Za-z0-9._-]`, and map the two id punctuation
/// characters to DISTINCT safe replacements — `:`→`_`, `,`→`-` — so ids differing only in `:`/`,`
/// placement (B6) don't collapse to the same name and silently overwrite each other. Any other
/// unsafe character falls back to `_`.
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ':' => '_',
            ',' => '-',
            c if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') => c,
            _ => '_',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::Json;
    use crate::model::{ChatReply, Message, Reply, ToolCall, ToolDef, Usage};

    fn tmp_dir(tag: &str) -> PathBuf {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("civ-trace-test-{tag}-{ts}"))
    }

    fn only_file(dir: &Path) -> PathBuf {
        let files: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(files.len(), 1, "expected exactly one trace file");
        files[0].path()
    }

    #[test]
    fn sanitize_distinguishes_colon_from_comma() {
        // B6: two ids differing ONLY in `:` vs `,` placement must not sanitize to the same name.
        // Previously both mapped to `_`, so `region-count:3,4:...` and a `,`/`:`-swapped sibling
        // silently overwrote each other.
        // Swapping the two separators must yield distinct, Windows-safe names.
        let a = sanitize("a:b,c");
        let b = sanitize("a,b:c");
        assert_ne!(
            a, b,
            "ids differing only in :/, placement must yield distinct filenames"
        );
        assert_eq!(a, "a_b-c");
        assert_eq!(b, "a-b_c");
        assert!(!a.contains(':') && !a.contains(','));
        assert!(!b.contains(':') && !b.contains(','));
        // A realistic id pair (the id separator `:` vs. a coordinate comma `,`).
        assert_ne!(sanitize("terrain:1,2"), sanitize("terrain,1:2"));
    }

    #[test]
    fn writes_interactive_trace_with_expected_structure() {
        let dir = tmp_dir("interactive");
        let tracer = Tracer::new(&dir).expect("tracer");
        let meta = TraceMeta {
            mode: "interactive",
            model: "stub-model",
            encoding: "interactive",
            board: "board.sav",
            item_id: "terrain:x=1,y=2",
            category: "terrain",
            tier: "T0",
        };
        let mut tb = tracer.begin(&meta);

        // Turn 1: model asks a tool.
        let messages = vec![
            Message::system("system prompt"),
            Message::user("what terrain at (1,2)?"),
        ];
        let tools = vec![ToolDef {
            name: "get_tile",
            description: "d".into(),
            params_schema: "{}".into(),
        }];
        let call = ToolCall {
            id: "call_1".into(),
            name: "get_tile".into(),
            args_json: "{\"x\":1,\"y\":2}".into(),
        };
        let reply = ChatReply {
            text: None,
            reasoning: None,
            tool_calls: vec![call.clone()],
            usage: Usage {
                prompt_tokens: 100,
                completion_tokens: 5,
                cached_tokens: 90,
            },
            latency_ms: 12,
        };
        tb.chat_turn(&messages, &tools, &reply);

        // Turn 2: after the tool result, the model answers.
        let mut msgs2 = messages.clone();
        msgs2.push(Message::assistant_calls(vec![call.clone()]));
        msgs2.push(Message::tool_result("call_1", "Forest"));
        let final_reply = ChatReply {
            text: Some("Answer: Forest".into()),
            // A model that returns BOTH visible content and reasoning — the reasoning must survive.
            reasoning: Some("The tile at (1,2) reads Forest.".into()),
            tool_calls: vec![],
            usage: Usage {
                prompt_tokens: 120,
                completion_tokens: 4,
                cached_tokens: 100,
            },
            latency_ms: 8,
        };
        tb.chat_turn(&msgs2, &tools, &final_reply);
        tb.finish("correct", "Forest", "Forest");

        let body = std::fs::read_to_string(only_file(&dir)).unwrap();
        let v = Json::parse(&body).expect("valid trace json");
        assert_eq!(v.get("mode").and_then(Json::as_str), Some("interactive"));
        assert_eq!(v.get("model").and_then(Json::as_str), Some("stub-model"));

        let turns = v.get("turns").and_then(Json::as_array).unwrap();
        assert_eq!(turns.len(), 2);

        // Turn 0: request carries the 2-message conversation + tool names; response has usage + a call.
        let req0 = turns[0].get("request").unwrap();
        assert_eq!(
            req0.get("messages")
                .and_then(Json::as_array)
                .map(|m| m.len()),
            Some(2)
        );
        assert_eq!(
            req0.get("tools").and_then(Json::as_array).map(|t| t.len()),
            Some(1)
        );
        let resp0 = turns[0].get("response").unwrap();
        assert_eq!(
            resp0
                .get("usage")
                .and_then(|u| u.get("cached_tokens"))
                .and_then(Json::as_u64),
            Some(90)
        );
        let tc = resp0.get("tool_calls").and_then(Json::as_array).unwrap();
        assert_eq!(tc[0].get("name").and_then(Json::as_str), Some("get_tile"));

        // Turn 1: request grew to 4 messages (assistant call + tool result appended).
        let req1 = turns[1].get("request").unwrap();
        assert_eq!(
            req1.get("messages")
                .and_then(Json::as_array)
                .map(|m| m.len()),
            Some(4)
        );
        // Turn 1 response retains BOTH the visible answer and the raw reasoning (persist-everything):
        // a model returning both must not have its reasoning dropped.
        let resp1 = turns[1].get("response").unwrap();
        assert_eq!(
            resp1.get("text").and_then(Json::as_str),
            Some("Answer: Forest")
        );
        assert_eq!(
            resp1.get("reasoning").and_then(Json::as_str),
            Some("The tile at (1,2) reads Forest.")
        );
        // Turn 0 had no reasoning → serialized as JSON null.
        assert!(turns[0]
            .get("response")
            .and_then(|r| r.get("reasoning"))
            .map(Json::is_null)
            .unwrap_or(false));

        let result = v.get("result").unwrap();
        assert_eq!(result.get("status").and_then(Json::as_str), Some("correct"));
        assert_eq!(result.get("got").and_then(Json::as_str), Some("Forest"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn writes_static_trace_and_sanitizes_filename() {
        let dir = tmp_dir("static");
        let tracer = Tracer::new(&dir).unwrap();
        let meta = TraceMeta {
            mode: "static",
            model: "oracle",
            encoding: "raw",
            board: "b.sav",
            item_id: "terrain:a,b",
            category: "terrain",
            tier: "T0",
        };
        let mut tb = tracer.begin(&meta);
        let reply = Reply {
            text: "Answer: Ocean".into(),
            reasoning: None,
            usage: Usage {
                prompt_tokens: 50,
                completion_tokens: 3,
                cached_tokens: 0,
            },
            latency_ms: 0,
        };
        tb.static_turn("PREFIX", "TAIL", &reply);
        tb.finish("correct", "Ocean", "Ocean");

        let path = only_file(&dir);
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            !name.contains(':') && !name.contains(','),
            "filename not sanitized: {name}"
        );

        let body = std::fs::read_to_string(&path).unwrap();
        let v = Json::parse(&body).unwrap();
        let turns = v.get("turns").and_then(Json::as_array).unwrap();
        assert_eq!(turns.len(), 1);
        let req = turns[0].get("request").unwrap();
        assert_eq!(
            req.get("cacheable_prefix").and_then(Json::as_str),
            Some("PREFIX")
        );
        assert_eq!(req.get("tail").and_then(Json::as_str), Some("TAIL"));
        assert_eq!(
            turns[0]
                .get("response")
                .and_then(|r| r.get("text"))
                .and_then(Json::as_str),
            Some("Answer: Ocean")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn static_turn_retains_both_content_and_reasoning() {
        // A one-shot reply that carries BOTH a visible answer and raw reasoning must persist both.
        let dir = tmp_dir("static-reasoning");
        let tracer = Tracer::new(&dir).unwrap();
        let meta = TraceMeta {
            mode: "static",
            model: "thinker",
            encoding: "raw",
            board: "b.sav",
            item_id: "terrain:0,0",
            category: "terrain",
            tier: "T0",
        };
        let mut tb = tracer.begin(&meta);
        let reply = Reply {
            text: "Answer: Ocean".into(),
            reasoning: Some("Row 0 col 0 is water.".into()),
            usage: Usage::default(),
            latency_ms: 0,
        };
        tb.static_turn("PREFIX", "TAIL", &reply);
        tb.finish("correct", "Ocean", "Ocean");

        let body = std::fs::read_to_string(only_file(&dir)).unwrap();
        let v = Json::parse(&body).unwrap();
        let resp = v
            .get("turns")
            .and_then(Json::as_array)
            .and_then(|t| t.first())
            .and_then(|t| t.get("response"))
            .unwrap();
        assert_eq!(
            resp.get("text").and_then(Json::as_str),
            Some("Answer: Ocean")
        );
        assert_eq!(
            resp.get("reasoning").and_then(Json::as_str),
            Some("Row 0 col 0 is water.")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_manifest_round_trips_through_json() {
        let manifest = RunManifest {
            run_id: "1717171717171".into(),
            seed: 7,
            per_kind: 12,
            difficulty: "hard".into(),
            kinds: Some(vec!["terrain".into(), "nearest".into()]),
            board_path: "data/saves/myagent_T50.sav".into(),
            board_source: "data/saves/myagent_T50.sav#crop(2,3,10,10)#fog(p0)".into(),
            crop: Some((2, 3, 10, 10)),
            fog_player: Some(0),
            encodings: vec!["raw".into(), "interactive".into()],
            model: "some-model".into(),
            think: true,
            concurrency: 4,
        };
        let v = Json::parse(&manifest.to_json()).expect("manifest is valid JSON");

        assert_eq!(
            v.get("run_id").and_then(Json::as_str),
            Some("1717171717171")
        );
        assert_eq!(v.get("seed").and_then(Json::as_u64), Some(7));
        assert_eq!(v.get("per_kind").and_then(Json::as_u64), Some(12));
        assert_eq!(v.get("difficulty").and_then(Json::as_str), Some("hard"));
        let kinds = v.get("kinds").and_then(Json::as_array).unwrap();
        assert_eq!(kinds.len(), 2);
        assert_eq!(kinds[0].as_str(), Some("terrain"));
        assert_eq!(
            v.get("board_path").and_then(Json::as_str),
            Some("data/saves/myagent_T50.sav")
        );
        let crop = v.get("crop").unwrap();
        assert_eq!(crop.get("x0").and_then(Json::as_u64), Some(2));
        assert_eq!(crop.get("h").and_then(Json::as_u64), Some(10));
        assert_eq!(v.get("fog_player").and_then(Json::as_u64), Some(0));
        let encs = v.get("encodings").and_then(Json::as_array).unwrap();
        assert_eq!(encs.len(), 2);
        assert_eq!(encs[1].as_str(), Some("interactive"));
        assert_eq!(v.get("model").and_then(Json::as_str), Some("some-model"));
        assert_eq!(v.get("think"), Some(&Json::Bool(true)));
        assert_eq!(v.get("concurrency").and_then(Json::as_u64), Some(4));

        // `None` kinds/crop/fog serialize as JSON null; write_to drops a `run.json`.
        let bare = RunManifest {
            kinds: None,
            crop: None,
            fog_player: None,
            ..manifest
        };
        let bv = Json::parse(&bare.to_json()).unwrap();
        assert!(bv.get("kinds").map(Json::is_null).unwrap_or(false));
        assert!(bv.get("crop").map(Json::is_null).unwrap_or(false));
        assert!(bv.get("fog_player").map(Json::is_null).unwrap_or(false));

        let dir = tmp_dir("manifest");
        std::fs::create_dir_all(&dir).unwrap();
        bare.write_to(&dir);
        let written = std::fs::read_to_string(dir.join("run.json")).unwrap();
        assert_eq!(Json::parse(&written).unwrap(), bv);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
