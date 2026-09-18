//! Offline end-to-end test of the interactive tool loop, driven by a scripted mock `ChatModel`
//! (no API). Proves the loop wiring — request → tool_calls → surface.execute → tool results →
//! final answer → score, with turn/tool-call accounting — before any paid run.

use std::collections::BTreeSet;
use std::sync::Mutex;

use civ_core::{Board, Tile};
use civ_eval::{
    run_interactive, solve, ChatModel, ChatReply, EvalItem, Interactive, InteractiveConfig,
    Message, Question, Referent, ToolCall, ToolDef, Usage,
};

/// A model that replays a fixed script of replies, one per `chat` call.
struct ScriptedChat {
    replies: Mutex<std::collections::VecDeque<ChatReply>>,
}

impl ScriptedChat {
    fn new(replies: Vec<ChatReply>) -> Self {
        ScriptedChat {
            replies: Mutex::new(replies.into_iter().collect()),
        }
    }
}

impl ChatModel for ScriptedChat {
    fn name(&self) -> &str {
        "scripted-mock"
    }
    fn chat(&self, _messages: &[Message], _tools: &[ToolDef]) -> ChatReply {
        self.replies
            .lock()
            .unwrap()
            .pop_front()
            .expect("script exhausted")
    }
}

fn usage(p: u64, c: u64) -> Usage {
    Usage {
        prompt_tokens: p,
        completion_tokens: c,
        cached_tokens: 0,
    }
}

/// A 3×3 board with a known terrain at (1,1).
fn tiny_board() -> Board {
    let mut tiles = Vec::new();
    for y in 0..3 {
        let mut row = Vec::new();
        for x in 0..3 {
            let terrain = if (x, y) == (1, 1) {
                "Hills"
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
        width: 3,
        height: 3,
        tiles,
        cities: vec![],
        units: vec![],
        players: vec![],
        known: std::collections::HashMap::new(),
        visibility: None,
        ruleset: None,
        turn: None,
        source: "tiny".into(),
    }
}

#[test]
fn loop_executes_a_tool_then_answers_and_scores() {
    let board = tiny_board();
    let q = Question::TerrainAt {
        tile: Referent::Tile { x: 1, y: 1 },
    };
    let ans = solve(&q, &board).expect("solve");
    let canonical = ans.canonical(); // "Hills"
    let item = EvalItem::new(q, ans, "tiny".into());

    // Turn 1: the model asks to look at the tile. Turn 2: it answers.
    let script = vec![
        ChatReply {
            text: None,
            reasoning: None,
            tool_calls: vec![ToolCall {
                id: "c1".into(),
                name: "get_tile".into(),
                args_json: "{\"x\":1,\"y\":1}".into(),
            }],
            usage: usage(500, 10),
            latency_ms: 5,
        },
        ChatReply {
            text: Some(format!("I looked at the tile.\nAnswer: {canonical}")),
            reasoning: None,
            tool_calls: vec![],
            usage: usage(520, 8),
            latency_ms: 6,
        },
    ];
    let model = ScriptedChat::new(script);

    let rows = run_interactive(
        &board,
        &Interactive,
        &model,
        std::slice::from_ref(&item),
        InteractiveConfig::default(),
        1,
        None,
        None,
    );

    assert_eq!(rows.len(), 1);
    let r = &rows[0];
    assert_eq!(r.encoding, "interactive");
    assert_eq!(
        r.status.as_str(),
        "correct",
        "expected correct, got {:?} ({})",
        r.status,
        r.got
    );
    assert_eq!(r.n_turns, 2, "two round-trips");
    assert_eq!(r.n_tool_calls, 1, "one tool call executed");
    // Tokens are summed over the trajectory (500+520 prompt, 10+8 completion).
    assert_eq!(r.prompt_tokens, 1020);
    assert_eq!(r.completion_tokens, 18);
}

/// A model that records every `messages` slice it is handed (so a test can inspect the system prompt
/// and tool-result stamps the loop actually sends) while replaying a fixed script.
struct RecordingChat {
    replies: Mutex<std::collections::VecDeque<ChatReply>>,
    seen: Mutex<Vec<Vec<Message>>>,
}

impl ChatModel for RecordingChat {
    fn name(&self) -> &str {
        "recording-mock"
    }
    fn chat(&self, messages: &[Message], _tools: &[ToolDef]) -> ChatReply {
        self.seen.lock().unwrap().push(messages.to_vec());
        self.replies.lock().unwrap().pop_front().expect("script exhausted")
    }
}

#[test]
fn system_prompt_states_budget_and_tool_results_are_turn_stamped() {
    let board = tiny_board();
    let q = Question::TerrainAt {
        tile: Referent::Tile { x: 1, y: 1 },
    };
    let ans = solve(&q, &board).expect("solve");
    let canonical = ans.canonical();
    let item = EvalItem::new(q, ans, "tiny".into());

    let script = vec![
        ChatReply {
            text: None,
            reasoning: None,
            tool_calls: vec![ToolCall {
                id: "c1".into(),
                name: "get_tile".into(),
                args_json: "{\"x\":1,\"y\":1}".into(),
            }],
            usage: usage(500, 10),
            latency_ms: 5,
        },
        ChatReply {
            text: Some(format!("Answer: {canonical}")),
            reasoning: None,
            tool_calls: vec![],
            usage: usage(520, 8),
            latency_ms: 6,
        },
    ];
    let model = RecordingChat {
        replies: Mutex::new(script.into_iter().collect()),
        seen: Mutex::new(Vec::new()),
    };

    let _ = run_interactive(
        &board,
        &Interactive,
        &model,
        std::slice::from_ref(&item),
        InteractiveConfig::default(),
        1,
        None,
        None,
    );

    let seen = model.seen.lock().unwrap();
    // The system message (first message of the first request) states the hard turn limit and the
    // priority order (don't get forced into a rushed answer > correctness > efficiency).
    let system = &seen[0][0];
    assert_eq!(system.role, civ_eval::Role::System, "first message is the system prompt");
    assert!(
        system.content.contains("HARD LIMIT of 16 tool-use turns"),
        "budget stated: {}",
        system.content
    );
    assert!(
        system.content.contains("keep at least one turn in reserve"),
        "priority stated"
    );
    // The 2nd request carries the tool result, stamped with the running turn count `[turn 1/16]`.
    let second = &seen[1];
    let stamped = second
        .iter()
        .any(|m| m.role == civ_eval::Role::Tool && m.content.contains("[turn 1/16]"));
    assert!(stamped, "tool result should carry a [turn n/N] stamp: {second:?}");
}

#[test]
fn budget_zero_forces_immediate_answer() {
    let board = tiny_board();
    let q = Question::TerrainAt {
        tile: Referent::Tile { x: 1, y: 1 },
    };
    let ans = solve(&q, &board).expect("solve");
    let item = EvalItem::new(q, ans, "tiny".into());

    // With max_turns = 0 the loop must force an answer on the very first call (no tools).
    let script = vec![ChatReply {
        text: Some("Answer: Hills".into()),
        reasoning: None,
        tool_calls: vec![],
        usage: usage(100, 5),
        latency_ms: 1,
    }];
    let model = ScriptedChat::new(script);
    let cfg = InteractiveConfig {
        max_turns: 0,
        token_budget: 200_000,
        ..InteractiveConfig::default()
    };
    let rows = run_interactive(
        &board,
        &Interactive,
        &model,
        std::slice::from_ref(&item),
        cfg,
        1,
        None,
        None,
    );

    assert_eq!(rows[0].n_turns, 1, "forced single turn");
    assert_eq!(rows[0].n_tool_calls, 0, "no tools on the forced turn");
    assert_eq!(rows[0].status.as_str(), "correct");
}

/// A model that keeps calling a real tool (heavy, fully-cached prompt) until the loop stops handing
/// it tools (the forced-answer turn, `tools == []`), then answers. Exercises C1: the giant cached
/// prefix must NOT trip the token budget.
struct CachedHeavyChat {
    prompt: u64,
    cached: u64,
}
impl ChatModel for CachedHeavyChat {
    fn name(&self) -> &str {
        "cached-heavy-mock"
    }
    fn chat(&self, _messages: &[Message], tools: &[ToolDef]) -> ChatReply {
        if tools.is_empty() {
            // Forced-answer turn: no tools available.
            ChatReply {
                text: Some("Answer: Hills".into()),
                reasoning: None,
                tool_calls: vec![],
                usage: Usage {
                    prompt_tokens: self.prompt,
                    completion_tokens: 5,
                    cached_tokens: self.cached,
                },
                latency_ms: 1,
            }
        } else {
            ChatReply {
                text: None,
                reasoning: None,
                tool_calls: vec![ToolCall {
                    id: "c".into(),
                    name: "get_tile".into(),
                    args_json: "{\"x\":1,\"y\":1}".into(),
                }],
                usage: Usage {
                    prompt_tokens: self.prompt,
                    completion_tokens: 5,
                    cached_tokens: self.cached,
                },
                latency_ms: 1,
            }
        }
    }
}

#[test]
fn large_cached_prefix_does_not_force_early_answer() {
    // C1: prompt_tokens is enormous every turn but fully cached, so the cache-deducted spend stays
    // tiny and the SMALL token_budget must NOT force an early answer — max_turns governs instead.
    // Without the fix (charging the whole prompt) the budget would trip after turn 1.
    let board = tiny_board();
    let q = Question::TerrainAt {
        tile: Referent::Tile { x: 1, y: 1 },
    };
    let ans = solve(&q, &board).expect("solve");
    let item = EvalItem::new(q, ans, "tiny".into());

    let model = CachedHeavyChat {
        prompt: 100_000,
        cached: 100_000,
    };
    let cfg = InteractiveConfig {
        max_turns: 5,
        token_budget: 500, // far below prompt_tokens; only survivable because it's all cached
        ..InteractiveConfig::default()
    };
    let rows = run_interactive(
        &board,
        &Interactive,
        &model,
        std::slice::from_ref(&item),
        cfg,
        1,
        None,
        None,
    );
    // All five tool turns ran before the forced answer: the budget never tripped early.
    assert_eq!(rows[0].n_tool_calls, 5, "ran the full turn allowance, not force-stopped");
    assert_eq!(rows[0].n_turns, 6, "5 tool turns + 1 forced-answer turn");
    assert_eq!(rows[0].status.as_str(), "correct");
}

#[test]
fn off_menu_tool_call_is_rejected_not_executed() {
    // C2: a tool the model names that is NOT on the advertised menu must get an error result, not a
    // real tool answer (only a strict provider would otherwise block it).
    let board = tiny_board();
    let q = Question::TerrainAt {
        tile: Referent::Tile { x: 1, y: 1 },
    };
    let ans = solve(&q, &board).expect("solve");
    let canonical = ans.canonical();
    let item = EvalItem::new(q, ans, "tiny".into());

    let script = vec![
        ChatReply {
            text: None,
            reasoning: None,
            tool_calls: vec![ToolCall {
                id: "c1".into(),
                name: "definitely_not_a_tool".into(),
                args_json: "{}".into(),
            }],
            usage: usage(500, 10),
            latency_ms: 5,
        },
        ChatReply {
            text: Some(format!("Answer: {canonical}")),
            reasoning: None,
            tool_calls: vec![],
            usage: usage(520, 8),
            latency_ms: 6,
        },
    ];
    let model = RecordingChat {
        replies: Mutex::new(script.into_iter().collect()),
        seen: Mutex::new(Vec::new()),
    };

    let _ = run_interactive(
        &board,
        &Interactive,
        &model,
        std::slice::from_ref(&item),
        InteractiveConfig::default(),
        1,
        None,
        None,
    );

    let seen = model.seen.lock().unwrap();
    // The 2nd request carries the tool result for the off-menu call: an error, never a tile answer.
    let second = &seen[1];
    let tool_msg = second
        .iter()
        .find(|m| m.role == civ_eval::Role::Tool)
        .expect("a tool result was pushed");
    assert!(
        tool_msg.content.contains("is not available on this surface"),
        "off-menu call must return the error: {}",
        tool_msg.content
    );
    assert!(
        tool_msg.content.contains("definitely_not_a_tool"),
        "error names the offending tool: {}",
        tool_msg.content
    );
}
