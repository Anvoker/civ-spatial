//! Models behind one trait, so the runner is provider-agnostic (DESIGN.md §10). v1 ships the
//! offline `OracleModel`; the live `OpenAiCompatible` provider (genai/local Qwen/OpenRouter)
//! is added later behind the `remote` feature.

use std::collections::HashMap;

/// Token/cost accounting for one call. First-class from v1 so the accuracy-vs-cost frontier is
/// always measurable, never retrofitted.
#[derive(Debug, Clone, Default)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub cached_tokens: u64,
}

#[derive(Debug, Clone)]
pub struct Reply {
    pub text: String,
    /// Raw `reasoning_content` from a thinking model, captured WHENEVER present — independently of
    /// `text`. `text` still falls back to reasoning when the visible content is empty (so answer
    /// extraction is unchanged); this field is the un-folded raw reasoning for the persist-everything
    /// trace. `None` when the provider returned no reasoning.
    pub reasoning: Option<String>,
    pub usage: Usage,
    pub latency_ms: u64,
}

// --------------------------------------------------------------------------
// tool / chat protocol — the multi-turn, queryable board-access surface
// --------------------------------------------------------------------------

/// The schema of one tool exposed to the model (for the provider's `tools` array). Lives here (not
/// in `encoders`) so both the queryable surface and the chat model share one definition.
#[derive(Debug, Clone)]
pub struct ToolDef {
    pub name: &'static str,
    pub description: String,
    /// The OpenAI-style JSON-Schema `parameters` object, as a string.
    pub params_schema: String,
}

/// One tool invocation: an id (echoed back in the tool-result message), the tool name, and its
/// arguments as a (flat) JSON object string.
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args_json: String,
}

/// A chat role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        }
    }
}

/// One conversation turn. `tool_calls` is set on assistant turns that requested tools;
/// `tool_call_id` is set on a `Tool` turn to say which call it answers.
#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub tool_call_id: Option<String>,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }
    /// An assistant turn that requested tools (content usually empty).
    pub fn assistant_calls(calls: Vec<ToolCall>) -> Self {
        Self {
            role: Role::Assistant,
            content: String::new(),
            tool_calls: calls,
            tool_call_id: None,
        }
    }
    /// A tool-result turn answering `call_id`.
    pub fn tool_result(call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: Some(call_id.into()),
        }
    }
}

/// One assistant response in a tool loop: either final `text`, or `tool_calls` to execute (or both;
/// the loop prefers tool_calls when present). Usage is for this single round-trip.
#[derive(Debug, Clone)]
pub struct ChatReply {
    pub text: Option<String>,
    /// Raw `reasoning_content` for this round-trip, captured WHENEVER present (independently of
    /// `text`). Mirrors [`Reply::reasoning`]: `text` still falls back to reasoning when visible
    /// content is empty, so tool-loop behavior is unchanged; this holds the un-folded raw reasoning
    /// for the trace. `None` when the provider returned no reasoning.
    pub reasoning: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Usage,
    pub latency_ms: u64,
}

/// A multi-turn, tool-using model. Distinct from [`Model`] (one-shot); implemented by live
/// providers for the interactive board-access mode.
pub trait ChatModel {
    fn name(&self) -> &str;
    /// One round-trip: send the running conversation + tool schemas, get back the assistant reply.
    fn chat(&self, messages: &[Message], tools: &[ToolDef]) -> ChatReply;
}

pub trait Model {
    fn name(&self) -> &str;
    /// Produce a reply for `prompt`. Real providers MUST use only `prompt`.
    fn answer(&self, prompt: &str) -> Reply;

    /// Produce a reply for a prompt split at the caching boundary: `cacheable_prefix` is the
    /// board block (stable across a board × encoding), `tail` is the per-question remainder.
    /// The default concatenates and defers to `answer`, so the Oracle and non-caching providers
    /// are unaffected; a caching provider overrides this to mark the prefix as a cache breakpoint.
    fn answer_cached(&self, cacheable_prefix: &str, tail: &str) -> Reply {
        let mut full = String::with_capacity(cacheable_prefix.len() + tail.len());
        full.push_str(cacheable_prefix);
        full.push_str(tail);
        self.answer(&full)
    }
}

/// The perfect oracle: returns the canonical ground-truth answer for each prompt. It powers the
/// harness self-consistency invariant (Oracle must score 100%) and lets the whole pipeline be
/// validated with zero API spend. Token usage is *estimated* from prompt length so the cost
/// columns are populated and already reveal per-encoding token differences.
pub struct OracleModel {
    by_prompt: HashMap<String, String>,
}

impl OracleModel {
    pub fn new(by_prompt: HashMap<String, String>) -> Self {
        OracleModel { by_prompt }
    }
}

/// Cheap, stable token estimate (~4 chars/token). Not exact — just enough to make the cost axis
/// meaningful in the offline sweep.
pub fn estimate_tokens(text: &str) -> u64 {
    (text.chars().count() as u64).div_ceil(4)
}

impl Model for OracleModel {
    fn name(&self) -> &str {
        "oracle"
    }

    fn answer(&self, prompt: &str) -> Reply {
        let canonical = self
            .by_prompt
            .get(prompt)
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        let text = format!("Answer: {canonical}");
        Reply {
            usage: Usage {
                prompt_tokens: estimate_tokens(prompt),
                completion_tokens: estimate_tokens(&text),
                cached_tokens: 0,
            },
            text,
            reasoning: None,
            latency_ms: 0,
        }
    }
}
