//! An OpenAI-compatible chat client (behind the `remote` feature).
//!
//! One `Model` impl covers every OpenAI-compatible endpoint via its base URL:
//! - a *local* server (LM Studio, Ollama, vLLM, llama.cpp) over `http://host:port/v1`, and
//! - a *cloud* provider (OpenRouter, OpenAI, …) over `https://host/v1`.
//!
//! Transport is `ureq` (blocking, so it matches the sync `Model` trait) with pure-Rust `rustls`
//! TLS — no OpenSSL/system dependency on Windows. It handles TLS, chunked encoding, and redirects,
//! which the previous hand-rolled `std::net` client could not, so the same code path now reaches
//! HTTPS cloud models. `ureq` is confined to this file behind the `remote` feature; the offline
//! core stays dependency-free.

use std::time::{Duration, Instant};

use crate::json::{quote, Json};
use crate::model::{ChatModel, ChatReply, Message, Model, Reply, Role, ToolCall, ToolDef, Usage};

/// Graded reasoning effort for `--reasoning-effort`. On OpenRouter it maps to
/// `"reasoning":{"effort":"..."}`; on other hosts to `"reasoning_effort":"..."`. When set it
/// overrides the on/off default derived from `--think`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Effort {
    Low,
    Medium,
    High,
}

impl Effort {
    /// Parse a CLI value (`low`/`medium`/`high`); `None` for anything else.
    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "low" => Some(Effort::Low),
            "medium" => Some(Effort::Medium),
            "high" => Some(Effort::High),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
        }
    }
}

/// Which reasoning-control dialect an endpoint speaks. Chosen once from the base URL at
/// construction, so the rest of the request builder is a clean match on this instead of a pile of
/// per-call host string checks.
///
/// The `--think` toggle was *not* portable before this split: emitting only `reasoning_effort` made
/// the switch inert on cloud providers that don't understand it (Gemini stayed reasoning-on, Claude
/// stayed off), so `--think` was meaningless there.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ReasoningDialect {
    /// OpenRouter's unified field: `"reasoning":{"enabled":true|false}` (or `{"effort":"..."}`).
    /// This is honored across upstreams (DeepSeek, Gemini, Anthropic, …), so `--think` toggles
    /// them all the same way.
    OpenRouterUnified,
    /// LM Studio / Ollama / other OpenAI-compatible hosts: `"reasoning_effort":"none"` is the
    /// switch the local Qwen build honors; nothing on think-on (provider default). These do NOT
    /// understand the unified `reasoning` object.
    ReasoningEffort,
}

impl ReasoningDialect {
    /// Pick the dialect from the base URL host: OpenRouter gets the unified `reasoning` field,
    /// everything else keeps the `reasoning_effort` path.
    fn from_base_url(base_url: &str) -> Self {
        if base_url.contains("openrouter.ai") {
            ReasoningDialect::OpenRouterUnified
        } else {
            ReasoningDialect::ReasoningEffort
        }
    }
}

pub struct OpenAiCompatible {
    label: String,
    /// Full request URL, e.g. `https://openrouter.ai/api/v1/chat/completions`.
    url: String,
    model_id: String,
    api_key: Option<String>,
    read_timeout: Duration,
    /// When false, ask the provider to suppress the reasoning trace (see `request_body`).
    think: bool,
    /// Reasoning-control dialect for this endpoint, chosen from the base URL (see the enum).
    dialect: ReasoningDialect,
    /// Optional graded reasoning effort (`--reasoning-effort`); overrides the `think` on/off default.
    effort: Option<Effort>,
    /// Optional OpenRouter provider pin — routing an encoding comparison across providers at
    /// different quantizations would be an uncontrolled variable, so we can hard-pin one.
    provider: Option<String>,
    /// When true, send the board block as a separate content part marked with `cache_control`
    /// so caching providers (Anthropic; passed through by OpenRouter) charge it once per
    /// board × encoding. Off by default, so the local/LM-Studio request bytes are unchanged.
    cache: bool,
}

/// The recorded model label, `"<id> [think|nothink]"`. This IS what [`OpenAiCompatible::name`]
/// returns and what a result row persists as its `model` field, so a resume key MUST be built from
/// this — never the bare `--model` flag — or a live run's planned keys never match its own written
/// rows and both `--resume` and the refuse-clobber guard silently break. (The Oracle, whose name
/// equals its flag, was unaffected, which is exactly why an Oracle-only test missed it.)
pub fn model_label(model_id: &str, think: bool) -> String {
    format!("{model_id} [{}]", if think { "think" } else { "nothink" })
}

impl OpenAiCompatible {
    /// Build from a base URL like `http://localhost:1234/v1` or `https://openrouter.ai/api/v1`
    /// plus a model id. `think` controls whether the reasoning trace is enabled; it is folded into
    /// the recorded label so results from the two modes are distinguishable.
    pub fn from_base_url(
        base_url: &str,
        model_id: &str,
        api_key: Option<String>,
        think: bool,
    ) -> Result<Self, String> {
        if !(base_url.starts_with("http://") || base_url.starts_with("https://")) {
            return Err(format!(
                "base URL must start with http:// or https:// (got {base_url:?})"
            ));
        }
        let base = base_url.trim_end_matches('/');
        Ok(OpenAiCompatible {
            label: model_label(model_id, think),
            url: format!("{base}/chat/completions"),
            model_id: model_id.to_string(),
            api_key,
            read_timeout: Duration::from_secs(600),
            think,
            dialect: ReasoningDialect::from_base_url(base_url),
            effort: None,
            provider: None,
            cache: false,
        })
    }

    /// Set a graded reasoning effort (`--reasoning-effort low|medium|high`), overriding the
    /// `--think` on/off default. No-op when `None`.
    pub fn with_reasoning_effort(mut self, effort: Option<Effort>) -> Self {
        self.effort = effort;
        self
    }

    /// Pin an OpenRouter upstream provider by name (e.g. "Fireworks") and disable fallbacks, so a
    /// comparison run can't silently drift across providers/quantizations. No-op for other hosts.
    pub fn with_provider(mut self, provider: Option<String>) -> Self {
        self.provider = provider.filter(|p| !p.is_empty());
        self
    }

    /// Enable prompt caching of the board prefix (see the `cache` field). Off by default.
    pub fn with_cache(mut self, cache: bool) -> Self {
        self.cache = cache;
        self
    }

    /// The shared request options (temperature/stream + reasoning + provider pin), as JSON
    /// fragments to splice after `"model"`.
    fn options_fragment(&self) -> String {
        let reasoning = self.reasoning_fragment();
        let provider = match &self.provider {
            // OpenRouter provider pin (ignored by other OpenAI-compatible hosts).
            Some(p) => format!(
                ",\"provider\":{{\"order\":[{}],\"allow_fallbacks\":false}}",
                quote(p)
            ),
            None => String::new(),
        };
        format!("{reasoning}{provider}")
    }

    /// The reasoning-control JSON fragment (leading comma included, or empty), in the dialect
    /// chosen at construction. This is what makes `--think` mean the same thing everywhere:
    ///
    /// - **OpenRouter** speaks the unified `reasoning` object, honored across upstreams, so
    ///   `--think off`/`on` reliably toggle DeepSeek, Gemini, and Claude alike. A `--reasoning-effort`
    ///   value, if set, overrides the on/off default with `{"effort":"..."}`.
    /// - **Local (LM Studio/Ollama) and other hosts** keep the `reasoning_effort` path: `"none"` on
    ///   think-off (the switch the local Qwen build honors — `/no_think` and `enable_thinking` were
    ///   ignored), nothing on think-on. A graded effort maps straight onto `reasoning_effort`.
    fn reasoning_fragment(&self) -> String {
        match self.dialect {
            ReasoningDialect::OpenRouterUnified => match self.effort {
                Some(e) => format!(",\"reasoning\":{{\"effort\":\"{}\"}}", e.as_str()),
                None => format!(",\"reasoning\":{{\"enabled\":{}}}", self.think),
            },
            ReasoningDialect::ReasoningEffort => match self.effort {
                Some(e) => format!(",\"reasoning_effort\":\"{}\"", e.as_str()),
                None if self.think => String::new(),
                None => ",\"reasoning_effort\":\"none\"".to_string(),
            },
        }
    }

    /// Flat single-string content — the original request shape, byte-for-byte.
    fn request_body(&self, prompt: &str) -> String {
        format!(
            "{{\"model\":{},\"temperature\":0,\"stream\":false{},\
             \"messages\":[{{\"role\":\"user\",\"content\":{}}}]}}",
            quote(&self.model_id),
            self.options_fragment(),
            quote(prompt)
        )
    }

    /// Content split into a cacheable prefix part (`cache_control: ephemeral`) and a variable
    /// tail part. Anthropic caches on the marker; OpenRouter forwards it; providers that don't
    /// cache still receive valid text parts and simply ignore the extra field.
    fn request_body_parts(&self, prefix: &str, tail: &str) -> String {
        format!(
            "{{\"model\":{},\"temperature\":0,\"stream\":false{},\
             \"messages\":[{{\"role\":\"user\",\"content\":[\
             {{\"type\":\"text\",\"text\":{},\"cache_control\":{{\"type\":\"ephemeral\"}}}},\
             {{\"type\":\"text\",\"text\":{}}}]}}]}}",
            quote(&self.model_id),
            self.options_fragment(),
            quote(prefix),
            quote(tail)
        )
    }

    /// POST a prebuilt request body and parse the reply (timing, content extraction, usage).
    fn complete(&self, body: &str) -> Reply {
        let started = Instant::now();
        let result = self.post(body);
        let latency_ms = started.elapsed().as_millis() as u64;
        match result {
            Ok(body) => {
                // Capture raw reasoning WHENEVER present (persist-everything), regardless of
                // whether visible content exists.
                let reasoning = json_string(&body, "reasoning_content").filter(|s| !s.is_empty());
                // Prefer visible content; fall back to reasoning content for thinking models.
                let text = json_string(&body, "content")
                    .filter(|s| !s.trim().is_empty())
                    .or_else(|| reasoning.clone())
                    .unwrap_or_default();
                Reply {
                    usage: Usage {
                        prompt_tokens: json_uint(&body, "prompt_tokens").unwrap_or(0),
                        completion_tokens: json_uint(&body, "completion_tokens").unwrap_or(0),
                        // Cloud providers report cache hits under
                        // `usage.prompt_tokens_details.cached_tokens`; local servers usually omit
                        // it (defaults to 0). This is the board-prefix cache payoff, so record it.
                        cached_tokens: json_uint(&body, "cached_tokens").unwrap_or(0),
                    },
                    text,
                    reasoning,
                    latency_ms,
                }
            }
            Err(e) => {
                eprintln!("  [warn] model call failed: {e}");
                Reply {
                    text: String::new(),
                    reasoning: None,
                    usage: Usage::default(),
                    latency_ms,
                }
            }
        }
    }

    /// POST the chat request and return the raw response body, or an error string.
    fn post(&self, body: &str) -> Result<String, String> {
        let mut req = ureq::post(&self.url)
            .timeout(self.read_timeout)
            .set("Content-Type", "application/json");
        if let Some(key) = &self.api_key {
            req = req.set("Authorization", &format!("Bearer {key}"));
        }
        match req.send_string(body) {
            Ok(resp) => resp.into_string().map_err(|e| format!("read body: {e}")),
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                Err(format!(
                    "HTTP status {code}; body: {}",
                    truncate(&body, 300)
                ))
            }
            Err(e) => Err(format!("request failed: {e}")),
        }
    }

    // --- multi-turn tool loop (the interactive board-access mode) ----------

    /// Serialize a chat request with a `tools` array and a full `messages` list. When caching is on,
    /// TWO `cache_control` breakpoints are placed: on the first message (the stable system prefix —
    /// overview + rules + tool instructions, billed once per board × surface) AND on the LAST message
    /// (a moving breakpoint at the end of the growing transcript). The moving breakpoint is the fix
    /// for the interactive round-trip tax: each turn resends the whole transcript, so without it the
    /// prior turns are re-billed every round-trip (~O(N²) over an N-turn trajectory). With it, turn
    /// k+1 reads turn-k's entire prefix from cache and pays fresh only for the new call + result.
    /// Anthropic honors the markers (max 4 breakpoints; 5-min TTL >> a trajectory's seconds);
    /// DeepSeek/Gemini auto-cache server-side and simply ignore the extra field.
    fn chat_body(&self, messages: &[Message], tools: &[ToolDef]) -> String {
        let tools_json = tools
            .iter()
            .map(|t| {
                format!(
                    "{{\"type\":\"function\",\"function\":{{\"name\":{},\"description\":{},\"parameters\":{}}}}}",
                    quote(t.name),
                    quote(&t.description),
                    t.params_schema
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let last = messages.len().saturating_sub(1);
        let msgs_json = messages
            .iter()
            .enumerate()
            .map(|(i, m)| self.message_json(m, self.cache && (i == 0 || i == last)))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"model\":{},\"temperature\":0,\"stream\":false{},\"tools\":[{}],\
             \"tool_choice\":\"auto\",\"messages\":[{}]}}",
            quote(&self.model_id),
            self.options_fragment(),
            tools_json,
            msgs_json
        )
    }

    /// Whether the moving tool-turn cache breakpoint must be placed at MESSAGE level (a
    /// `cache_control` sibling of `content`, plain-string content) rather than nested inside a
    /// `tool_result` content part.
    ///
    /// The native Anthropic Messages API — spoken by the CLIProxyAPI subscription bridge — 400s on
    /// `cache_control` inside `tool_result.content` and requires it directly on the `tool_result`
    /// block; the proxy's OpenAI->Anthropic translator only surfaces that placement from a
    /// message-level `cache_control`. OpenRouter keeps the documented part-level form (so the
    /// clean-run arms stay byte-identical); LM Studio/Ollama ignore the field either way, and the
    /// plain-string content is the more standard OpenAI shape for them.
    fn tool_cache_at_message_level(&self) -> bool {
        !matches!(self.dialect, ReasoningDialect::OpenRouterUnified)
    }

    /// One message as JSON. `cache_prefix` wraps the content in a parts array with a `cache_control`
    /// breakpoint — used for the stable system prefix AND, in the interactive loop, the moving
    /// breakpoint on the last message (a `User` question/force-answer or a `Tool` result).
    fn message_json(&self, m: &Message, cache_prefix: bool) -> String {
        match m.role {
            Role::System | Role::User => {
                if cache_prefix {
                    format!(
                        "{{\"role\":{},\"content\":[{{\"type\":\"text\",\"text\":{},\
                         \"cache_control\":{{\"type\":\"ephemeral\"}}}}]}}",
                        quote(m.role.as_str()),
                        quote(&m.content)
                    )
                } else {
                    format!(
                        "{{\"role\":{},\"content\":{}}}",
                        quote(m.role.as_str()),
                        quote(&m.content)
                    )
                }
            }
            Role::Assistant => {
                if m.tool_calls.is_empty() {
                    format!(
                        "{{\"role\":\"assistant\",\"content\":{}}}",
                        quote(&m.content)
                    )
                } else {
                    let calls = m
                        .tool_calls
                        .iter()
                        .map(|c| {
                            // `arguments` is a JSON-encoded STRING, so quote the raw args object.
                            format!(
                                "{{\"id\":{},\"type\":\"function\",\"function\":{{\"name\":{},\"arguments\":{}}}}}",
                                quote(&c.id),
                                quote(&c.name),
                                quote(&c.args_json)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(",");
                    format!("{{\"role\":\"assistant\",\"content\":null,\"tool_calls\":[{calls}]}}")
                }
            }
            Role::Tool => {
                let id = quote(m.tool_call_id.as_deref().unwrap_or(""));
                if !cache_prefix {
                    format!(
                        "{{\"role\":\"tool\",\"tool_call_id\":{},\"content\":{}}}",
                        id,
                        quote(&m.content)
                    )
                } else if self.tool_cache_at_message_level() {
                    // Native-Anthropic bridge (CLIProxyAPI): the moving breakpoint must sit at
                    // MESSAGE level with plain-string content. The proxy's OpenAI->Anthropic
                    // translator copies message-level `cache_control` directly onto the
                    // `tool_result` block — the placement the Messages API requires. A
                    // `cache_control` nested inside a `tool_result` content part 400s
                    // ("cache_control may not be specified within `tool_result.content`").
                    format!(
                        "{{\"role\":\"tool\",\"tool_call_id\":{},\"content\":{},\
                         \"cache_control\":{{\"type\":\"ephemeral\"}}}}",
                        id,
                        quote(&m.content)
                    )
                } else {
                    // OpenRouter: keep the documented part-level breakpoint (byte-identical to
                    // the clean-run arms; DeepSeek/Gemini ignore it, Anthropic honors it).
                    format!(
                        "{{\"role\":\"tool\",\"tool_call_id\":{},\"content\":[{{\"type\":\"text\",\
                         \"text\":{},\"cache_control\":{{\"type\":\"ephemeral\"}}}}]}}",
                        id,
                        quote(&m.content)
                    )
                }
            }
        }
    }

    /// POST a chat body and parse the assistant reply (text and/or tool calls, usage).
    fn chat_complete(&self, body: &str) -> ChatReply {
        let started = Instant::now();
        let latency_ms;
        let raw = match self.post(body) {
            Ok(raw) => {
                latency_ms = started.elapsed().as_millis() as u64;
                raw
            }
            Err(e) => {
                eprintln!("  [warn] chat call failed: {e}");
                return ChatReply {
                    text: None,
                    reasoning: None,
                    tool_calls: Vec::new(),
                    usage: Usage::default(),
                    latency_ms: started.elapsed().as_millis() as u64,
                };
            }
        };
        let v = match Json::parse(&raw) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("  [warn] chat parse failed: {e}");
                return ChatReply {
                    text: None,
                    reasoning: None,
                    tool_calls: Vec::new(),
                    usage: Usage::default(),
                    latency_ms,
                };
            }
        };
        let usage = extract_usage(&v);
        let msg = v
            .get("choices")
            .and_then(|c| c.at(0))
            .and_then(|c| c.get("message"));
        let (text, reasoning, tool_calls) = match msg {
            Some(m) => {
                let tool_calls = m
                    .get("tool_calls")
                    .and_then(Json::as_array)
                    .map(|arr| arr.iter().filter_map(parse_tool_call).collect())
                    .unwrap_or_default();
                // Capture raw reasoning WHENEVER present (persist-everything), independently of
                // whether visible content exists.
                let reasoning = m
                    .get("reasoning_content")
                    .and_then(Json::as_str)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string);
                let text = m
                    .get("content")
                    .and_then(Json::as_str)
                    .filter(|s| !s.trim().is_empty())
                    .map(str::to_string)
                    .or_else(|| reasoning.clone());
                (text, reasoning, tool_calls)
            }
            None => (None, None, Vec::new()),
        };
        ChatReply {
            text,
            reasoning,
            tool_calls,
            usage,
            latency_ms,
        }
    }
}

/// Parse one element of the response `tool_calls` array into a [`ToolCall`].
fn parse_tool_call(v: &Json) -> Option<ToolCall> {
    let id = v.get("id").and_then(Json::as_str).unwrap_or("").to_string();
    let func = v.get("function")?;
    let name = func.get("name").and_then(Json::as_str)?.to_string();
    let args_json = func
        .get("arguments")
        .and_then(Json::as_str)
        .unwrap_or("{}")
        .to_string();
    Some(ToolCall {
        id,
        name,
        args_json,
    })
}

/// Pull `usage.{prompt_tokens, completion_tokens}` and the nested
/// `usage.prompt_tokens_details.cached_tokens` from a parsed response.
fn extract_usage(v: &Json) -> Usage {
    let u = v.get("usage");
    let field = |k: &str| u.and_then(|u| u.get(k)).and_then(Json::as_u64).unwrap_or(0);
    let cached = u
        .and_then(|u| u.get("prompt_tokens_details"))
        .and_then(|d| d.get("cached_tokens"))
        .and_then(Json::as_u64)
        .unwrap_or(0);
    Usage {
        prompt_tokens: field("prompt_tokens"),
        completion_tokens: field("completion_tokens"),
        cached_tokens: cached,
    }
}

impl Model for OpenAiCompatible {
    fn name(&self) -> &str {
        &self.label
    }

    fn answer(&self, prompt: &str) -> Reply {
        self.complete(&self.request_body(prompt))
    }

    fn answer_cached(&self, cacheable_prefix: &str, tail: &str) -> Reply {
        if self.cache {
            self.complete(&self.request_body_parts(cacheable_prefix, tail))
        } else {
            // Caching off: send the exact flat prompt (unchanged request bytes).
            self.complete(&self.request_body(&format!("{cacheable_prefix}{tail}")))
        }
    }
}

impl ChatModel for OpenAiCompatible {
    fn name(&self) -> &str {
        &self.label
    }

    fn chat(&self, messages: &[Message], tools: &[ToolDef]) -> ChatReply {
        self.chat_complete(&self.chat_body(messages, tools))
    }
}

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Extract the string value of a top-level-ish JSON key `"key": "..."`, handling escapes.
/// Matching includes the opening quote so `"content"` won't match inside `"reasoning_content"`.
fn json_string(body: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\":");
    let mut search_from = 0;
    while let Some(rel) = body[search_from..].find(&pat) {
        let mut i = search_from + rel + pat.len();
        let bytes = body.as_bytes();
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'"' {
            return Some(parse_json_string(&body[i + 1..]));
        }
        search_from = search_from + rel + pat.len();
    }
    None
}

/// Parse a JSON string body starting just after the opening quote; stop at the closing quote.
fn parse_json_string(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => break,
            '\\' => match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('b') => out.push('\u{8}'),
                Some('f') => out.push('\u{c}'),
                Some('/') => out.push('/'),
                Some('\\') => out.push('\\'),
                Some('"') => out.push('"'),
                Some('u') => {
                    let hex: String = (0..4).filter_map(|_| chars.next()).collect();
                    if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                        if let Some(ch) = char::from_u32(cp) {
                            out.push(ch);
                        }
                    }
                }
                Some(other) => out.push(other),
                None => break,
            },
            c => out.push(c),
        }
    }
    out
}

/// Extract an unsigned integer value for `"key": N`.
fn json_uint(body: &str, key: &str) -> Option<u64> {
    let pat = format!("\"{key}\":");
    let start = body.find(&pat)? + pat.len();
    let rest = body[start..].trim_start();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(base_url: &str, think: bool) -> OpenAiCompatible {
        OpenAiCompatible::from_base_url(base_url, "some/model", None, think).unwrap()
    }

    #[test]
    fn model_label_equals_reported_name() {
        // Regression (resume bug): the resume key is built from `model_label(..)` while a result row
        // persists `model.name()`. If the two representations drift, a live `--resume` matches
        // nothing and the refuse-clobber guard silently truncates + re-spends. `OpenAiCompatible`
        // implements BOTH Model and ChatModel (static vs tool-loop encodings persist via each), so
        // lock both trait `name()`s to the label.
        use crate::model::{ChatModel, Model};
        for think in [false, true] {
            let m = build("https://h/v1", think);
            assert_eq!(Model::name(&m), model_label("some/model", think));
            assert_eq!(ChatModel::name(&m), model_label("some/model", think));
        }
    }

    #[test]
    fn openrouter_think_off_emits_unified_reasoning_disabled() {
        let body = build("https://openrouter.ai/api/v1", false).request_body("hi");
        assert!(
            body.contains("\"reasoning\":{\"enabled\":false}"),
            "expected unified reasoning field, got: {body}"
        );
        assert!(
            !body.contains("reasoning_effort"),
            "OpenRouter must NOT use reasoning_effort, got: {body}"
        );
    }

    #[test]
    fn openrouter_think_on_emits_unified_reasoning_enabled() {
        let body = build("https://openrouter.ai/api/v1", true).request_body("hi");
        assert!(
            body.contains("\"reasoning\":{\"enabled\":true}"),
            "expected reasoning enabled:true, got: {body}"
        );
        assert!(!body.contains("reasoning_effort"), "got: {body}");
    }

    #[test]
    fn chat_body_serializes_tools_messages_and_cache() {
        use crate::model::{Message, ToolCall, ToolDef};
        let mut m = build("https://openrouter.ai/api/v1", false);
        m.cache = true;
        let tools = vec![ToolDef {
            name: "get_tile",
            description: "one tile".to_string(),
            params_schema: "{\"type\":\"object\"}".to_string(),
        }];
        let msgs = vec![
            Message::system("SYS PREFIX"),
            Message::user("Question?"),
            Message::assistant_calls(vec![ToolCall {
                id: "call_1".into(),
                name: "get_tile".into(),
                args_json: "{\"x\":3,\"y\":4}".into(),
            }]),
            Message::tool_result("call_1", "(3, 4): Hills"),
        ];
        let body = m.chat_body(&msgs, &tools);
        // tools array with a function schema
        assert!(
            body.contains("\"tools\":[{\"type\":\"function\",\"function\":{\"name\":\"get_tile\""),
            "got: {body}"
        );
        assert!(body.contains("\"tool_choice\":\"auto\""), "got: {body}");
        // system prefix carries the cache_control breakpoint
        assert!(
            body.contains("\"cache_control\":{\"type\":\"ephemeral\"}"),
            "got: {body}"
        );
        // assistant tool_calls: arguments is a JSON *string* (escaped)
        assert!(
            body.contains("\"arguments\":\"{\\\"x\\\":3,\\\"y\\\":4}\""),
            "got: {body}"
        );
        // tool-result message keyed by id
        assert!(
            body.contains("\"role\":\"tool\",\"tool_call_id\":\"call_1\""),
            "got: {body}"
        );
        // moving breakpoint: the LAST message (the tool result) carries cache_control via array content
        assert!(
            body.contains(
                "\"role\":\"tool\",\"tool_call_id\":\"call_1\",\"content\":[{\"type\":\"text\",\
                 \"text\":\"(3, 4): Hills\",\"cache_control\":{\"type\":\"ephemeral\"}}]"
            ),
            "expected moving cache breakpoint on the last (tool) message, got: {body}"
        );
        // exactly two breakpoints: the system prefix + the moving one on the last message
        assert_eq!(body.matches("\"cache_control\"").count(), 2, "got: {body}");
    }

    #[test]
    fn chat_body_tool_breakpoint_is_message_level_off_openrouter() {
        // Native-Anthropic bridge (CLIProxyAPI): the moving tool-turn breakpoint must be a
        // MESSAGE-level `cache_control` with plain-string content, NOT nested inside a
        // `tool_result` content part (which the Messages API 400s). The proxy relocates the
        // message-level marker directly onto the `tool_result` block.
        use crate::model::{Message, ToolCall};
        let mut m = build("http://localhost:8317/v1", true);
        m.cache = true;
        let msgs = vec![
            Message::system("SYS PREFIX"),
            Message::assistant_calls(vec![ToolCall {
                id: "call_1".into(),
                name: "get_tile".into(),
                args_json: "{\"x\":3,\"y\":4}".into(),
            }]),
            Message::tool_result("call_1", "(3, 4): Hills"),
        ];
        let body = m.chat_body(&msgs, &[]);
        // moving breakpoint: plain-string content + message-level cache_control sibling.
        assert!(
            body.contains(
                "\"role\":\"tool\",\"tool_call_id\":\"call_1\",\"content\":\"(3, 4): Hills\",\
                 \"cache_control\":{\"type\":\"ephemeral\"}"
            ),
            "expected message-level tool cache breakpoint, got: {body}"
        );
        // MUST NOT nest cache_control inside a tool_result content part (the 400 shape).
        assert!(
            !body.contains("\"text\":\"(3, 4): Hills\",\"cache_control\""),
            "tool cache_control must not be nested in a content part, got: {body}"
        );
        // still exactly two breakpoints: stable system prefix + moving tool-turn one.
        assert_eq!(body.matches("\"cache_control\"").count(), 2, "got: {body}");
    }

    #[test]
    fn parses_chat_response_with_tool_calls() {
        let raw = r#"{"choices":[{"message":{"role":"assistant","content":null,
            "tool_calls":[{"id":"c1","type":"function","function":{"name":"scan","arguments":"{\"x0\":0}"}}]}}],
            "usage":{"prompt_tokens":50,"completion_tokens":3,"prompt_tokens_details":{"cached_tokens":40}}}"#;
        let v = Json::parse(raw).unwrap();
        let u = extract_usage(&v);
        assert_eq!(
            (u.prompt_tokens, u.completion_tokens, u.cached_tokens),
            (50, 3, 40)
        );
        let msg = v
            .get("choices")
            .unwrap()
            .at(0)
            .unwrap()
            .get("message")
            .unwrap();
        let calls: Vec<_> = msg
            .get("tool_calls")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .filter_map(parse_tool_call)
            .collect();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "scan");
        assert_eq!(calls[0].args_json, "{\"x0\":0}");
    }

    #[test]
    fn localhost_think_off_keeps_reasoning_effort_none() {
        let body = build("http://localhost:1234/v1", false).request_body("hi");
        assert!(
            body.contains("\"reasoning_effort\":\"none\""),
            "local server must keep reasoning_effort:none, got: {body}"
        );
        assert!(
            !body.contains("\"reasoning\":{"),
            "local server must NOT emit the unified reasoning object, got: {body}"
        );
    }

    #[test]
    fn localhost_think_on_emits_no_reasoning_field() {
        let body = build("http://localhost:1234/v1", true).request_body("hi");
        assert!(!body.contains("reasoning_effort"), "got: {body}");
        assert!(!body.contains("\"reasoning\":{"), "got: {body}");
    }

    #[test]
    fn openrouter_reasoning_effort_overrides_on_off_default() {
        // Effort set + think off: effort wins on OpenRouter (graded, not enabled:false).
        let body = build("https://openrouter.ai/api/v1", false)
            .with_reasoning_effort(Some(Effort::High))
            .request_body("hi");
        assert!(
            body.contains("\"reasoning\":{\"effort\":\"high\"}"),
            "expected graded effort, got: {body}"
        );
        assert!(
            !body.contains("enabled"),
            "effort should override enabled, got: {body}"
        );
    }

    #[test]
    fn localhost_reasoning_effort_maps_to_reasoning_effort_field() {
        let body = build("http://localhost:1234/v1", true)
            .with_reasoning_effort(Some(Effort::Low))
            .request_body("hi");
        assert!(
            body.contains("\"reasoning_effort\":\"low\""),
            "expected reasoning_effort:low, got: {body}"
        );
        assert!(!body.contains("\"reasoning\":{"), "got: {body}");
    }
}
