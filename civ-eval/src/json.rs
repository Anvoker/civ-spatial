//! Minimal JSON writing for result rows — no external dependency. We only emit flat objects of
//! strings, integers, and booleans, so a tiny writer suffices.

/// A JSON scalar for a result field.
pub enum Val {
    S(String),
    I(i64),
    U(u64),
    B(bool),
}

impl From<&str> for Val {
    fn from(s: &str) -> Self {
        Val::S(s.to_string())
    }
}
impl From<String> for Val {
    fn from(s: String) -> Self {
        Val::S(s)
    }
}
impl From<i64> for Val {
    fn from(n: i64) -> Self {
        Val::I(n)
    }
}
impl From<u64> for Val {
    fn from(n: u64) -> Self {
        Val::U(n)
    }
}
impl From<bool> for Val {
    fn from(b: bool) -> Self {
        Val::B(b)
    }
}

/// Serialize an ordered list of key/value pairs as a compact JSON object.
pub fn object(fields: &[(&str, Val)]) -> String {
    let mut s = String::from("{");
    for (i, (k, v)) in fields.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&quote(k));
        s.push(':');
        match v {
            Val::S(x) => s.push_str(&quote(x)),
            Val::I(x) => s.push_str(&x.to_string()),
            Val::U(x) => s.push_str(&x.to_string()),
            Val::B(x) => s.push_str(if *x { "true" } else { "false" }),
        }
    }
    s.push('}');
    s
}

// --------------------------------------------------------------------------
// JSON value parser — dependency-free, enough to read chat/tool-call responses
// --------------------------------------------------------------------------
//
// The hand-rolled `json_string`/`json_uint` scanners in `remote.rs` can't robustly read the nested
// `choices[0].message.tool_calls[]` array. This is a small recursive-descent parser into a `Json`
// value tree, with the few accessors the response reader needs. Tested offline.

/// A parsed JSON value.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    /// Parse a complete JSON document. Trailing whitespace is allowed; trailing junk is an error.
    pub fn parse(s: &str) -> Result<Json, String> {
        let mut p = Parser {
            b: s.as_bytes(),
            i: 0,
        };
        p.ws();
        let v = p.value()?;
        p.ws();
        if p.i != p.b.len() {
            return Err(format!("trailing data at byte {}", p.i));
        }
        Ok(v)
    }

    /// Object field by key (first match).
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Array element by index.
    pub fn at(&self, i: usize) -> Option<&Json> {
        match self {
            Json::Arr(items) => items.get(i),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(items) => Some(items),
            _ => None,
        }
    }

    /// A non-negative integer value (JSON numbers are stored as f64; this rounds toward zero).
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Json::Num(n) if *n >= 0.0 && n.is_finite() => Some(*n as u64),
            _ => None,
        }
    }

    /// Convenience for `self.get(a)?.get(b)?...` chains isn't provided; callers use `get`/`at`.
    pub fn is_null(&self) -> bool {
        matches!(self, Json::Null)
    }
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn value(&mut self) -> Result<Json, String> {
        self.ws();
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') | Some(b'f') => self.boolean(),
            Some(b'n') => self.null(),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
            other => Err(format!("unexpected {other:?} at byte {}", self.i)),
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.i += 1; // '{'
        let mut fields = Vec::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Json::Obj(fields));
        }
        loop {
            self.ws();
            let key = self.string()?;
            self.ws();
            if self.peek() != Some(b':') {
                return Err(format!("expected ':' at byte {}", self.i));
            }
            self.i += 1;
            let val = self.value()?;
            fields.push((key, val));
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Json::Obj(fields));
                }
                other => {
                    return Err(format!(
                        "expected ',' or '}}' at byte {} (got {other:?})",
                        self.i
                    ))
                }
            }
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.i += 1; // '['
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            let val = self.value()?;
            items.push(val);
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Json::Arr(items));
                }
                other => {
                    return Err(format!(
                        "expected ',' or ']' at byte {} (got {other:?})",
                        self.i
                    ))
                }
            }
        }
    }

    fn string(&mut self) -> Result<String, String> {
        if self.peek() != Some(b'"') {
            return Err(format!("expected string at byte {}", self.i));
        }
        self.i += 1;
        let mut out = String::new();
        loop {
            let c = *self.b.get(self.i).ok_or("unterminated string")?;
            self.i += 1;
            match c {
                b'"' => return Ok(out),
                b'\\' => {
                    let e = *self.b.get(self.i).ok_or("unterminated escape")?;
                    self.i += 1;
                    match e {
                        b'n' => out.push('\n'),
                        b't' => out.push('\t'),
                        b'r' => out.push('\r'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'/' => out.push('/'),
                        b'\\' => out.push('\\'),
                        b'"' => out.push('"'),
                        b'u' => {
                            let hex = std::str::from_utf8(
                                self.b.get(self.i..self.i + 4).ok_or("short \\u escape")?,
                            )
                            .map_err(|_| "bad \\u escape")?;
                            self.i += 4;
                            let cp = u32::from_str_radix(hex, 16).map_err(|_| "bad \\u hex")?;
                            out.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                        }
                        other => return Err(format!("bad escape \\{}", other as char)),
                    }
                }
                // Non-ASCII UTF-8 continuation bytes: push raw bytes back through a small buffer.
                c if c < 0x80 => out.push(c as char),
                c => {
                    // Multi-byte UTF-8: collect the full sequence.
                    let len = if c >= 0xF0 {
                        4
                    } else if c >= 0xE0 {
                        3
                    } else {
                        2
                    };
                    let start = self.i - 1;
                    self.i = start + len;
                    let s = std::str::from_utf8(self.b.get(start..self.i).ok_or("bad utf8")?)
                        .map_err(|_| "bad utf8 in string")?;
                    out.push_str(s);
                }
            }
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || matches!(c, b'.' | b'e' | b'E' | b'+' | b'-') {
                self.i += 1;
            } else {
                break;
            }
        }
        let s = std::str::from_utf8(&self.b[start..self.i]).map_err(|_| "bad number bytes")?;
        s.parse::<f64>()
            .map(Json::Num)
            .map_err(|_| format!("bad number {s:?}"))
    }

    fn boolean(&mut self) -> Result<Json, String> {
        if self.b[self.i..].starts_with(b"true") {
            self.i += 4;
            Ok(Json::Bool(true))
        } else if self.b[self.i..].starts_with(b"false") {
            self.i += 5;
            Ok(Json::Bool(false))
        } else {
            Err(format!("bad literal at byte {}", self.i))
        }
    }

    fn null(&mut self) -> Result<Json, String> {
        if self.b[self.i..].starts_with(b"null") {
            self.i += 4;
            Ok(Json::Null)
        } else {
            Err(format!("bad literal at byte {}", self.i))
        }
    }
}

/// A quoted, escaped JSON string.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod parse_tests {
    use super::Json;

    #[test]
    fn scalars_and_nesting() {
        assert_eq!(Json::parse("true").unwrap(), Json::Bool(true));
        assert_eq!(Json::parse("  null ").unwrap(), Json::Null);
        assert_eq!(Json::parse("-12.5e2").unwrap(), Json::Num(-1250.0));
        assert_eq!(Json::parse("\"a\nb\"").unwrap().as_str(), Some("a\nb"));
        let v = Json::parse(r#"{"a":[1,2,{"b":"x"}],"c":42}"#).unwrap();
        assert_eq!(v.get("c").and_then(Json::as_u64), Some(42));
        assert_eq!(
            v.get("a")
                .and_then(|a| a.at(2))
                .and_then(|o| o.get("b"))
                .and_then(Json::as_str),
            Some("x")
        );
        assert_eq!(
            v.get("a").and_then(Json::as_array).map(|a| a.len()),
            Some(3)
        );
    }

    #[test]
    fn parses_a_tool_call_response_shape() {
        // The escaped `arguments` string is itself JSON — parse it in a second pass.
        let body = r#"{"choices":[{"message":{"role":"assistant","content":null,
            "tool_calls":[{"id":"call_1","type":"function",
            "function":{"name":"scan","arguments":"{\"x0\":0,\"y0\":0,\"x1\":9,\"y1\":9}"}}]}}],
            "usage":{"prompt_tokens":123,"completion_tokens":7,
            "prompt_tokens_details":{"cached_tokens":100}}}"#;
        let v = Json::parse(body).unwrap();
        let msg = v
            .get("choices")
            .unwrap()
            .at(0)
            .unwrap()
            .get("message")
            .unwrap();
        assert!(msg.get("content").unwrap().is_null());
        let call = msg.get("tool_calls").unwrap().at(0).unwrap();
        assert_eq!(call.get("id").and_then(Json::as_str), Some("call_1"));
        let func = call.get("function").unwrap();
        assert_eq!(func.get("name").and_then(Json::as_str), Some("scan"));
        let args = func.get("arguments").and_then(Json::as_str).unwrap();
        let parsed_args = Json::parse(args).unwrap();
        assert_eq!(parsed_args.get("x1").and_then(Json::as_u64), Some(9));
        let usage = v.get("usage").unwrap();
        assert_eq!(usage.get("prompt_tokens").and_then(Json::as_u64), Some(123));
        assert_eq!(
            usage
                .get("prompt_tokens_details")
                .and_then(|d| d.get("cached_tokens"))
                .and_then(Json::as_u64),
            Some(100)
        );
    }

    #[test]
    fn rejects_trailing_junk() {
        assert!(Json::parse("{} x").is_err());
        assert!(Json::parse("[1,2").is_err());
    }
}
