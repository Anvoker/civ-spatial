//! A tiny, dependency-free JSON value parser — just enough for [`crate::board::Board::from_viewer_json`]
//! to read the offline viewer's board-export shape. `civ-core` carries no dependencies (the offline
//! ethos), so this is a self-contained recursive-descent parser rather than a `serde_json` pull. It
//! mirrors the (independently maintained) parser in `civ-eval::json`; the duplication is deliberate,
//! keeping `civ-core` free of any crate dependency.

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

    /// A signed integer value (JSON numbers are stored as f64; this rounds toward zero).
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Json::Num(n) if n.is_finite() => Some(*n as i64),
            _ => None,
        }
    }

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
                c if c < 0x80 => out.push(c as char),
                c => {
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

#[cfg(test)]
mod tests {
    use super::Json;

    #[test]
    fn scalars_and_nesting() {
        assert_eq!(Json::parse("true").unwrap(), Json::Bool(true));
        assert_eq!(Json::parse("  null ").unwrap(), Json::Null);
        assert_eq!(Json::parse("-42").unwrap().as_i64(), Some(-42));
        let v = Json::parse(r#"{"a":[1,2,{"b":"x"}],"c":42}"#).unwrap();
        assert_eq!(v.get("c").and_then(Json::as_i64), Some(42));
        assert_eq!(
            v.get("a").and_then(Json::as_array).map(|a| a.len()),
            Some(3)
        );
    }

    #[test]
    fn rejects_trailing_junk() {
        assert!(Json::parse("{} x").is_err());
    }
}
