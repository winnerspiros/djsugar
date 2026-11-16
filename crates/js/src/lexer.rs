//! Tokens from source text, one at a time on the parser's demand. A `/` or
//! a template's `}` reads differently depending on where it stands, so the
//! parser asks for those to be read again ([`Lexer::regex`],
//! [`Lexer::template_continue`]).

use crate::ast::Name;

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Ident(Name),
    /// An identifier written with a `\u` escape: never a keyword.
    EscapedIdent(Name),
    Private(Name),
    Num(f64),
    BigInt(Name),
    Str(Name),
    /// A template from its start up to the first `${` (or its end):
    /// cooked text (None where an escape is invalid), raw text, and whether
    /// it ended there.
    Template(Option<Name>, Name, bool),
    Regex(Name, Name),
    Punct(&'static str),
    Eof,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    pub start: usize,
    pub end: usize,
    /// Whether a line break came before it (for automatic semicolons).
    pub newline_before: bool,
}

#[derive(Debug, Clone)]
pub struct SyntaxError {
    pub message: String,
    pub pos: usize,
}

pub type Result<T> = std::result::Result<T, SyntaxError>;

/// Longest first, so the longest match wins.
const PUNCT: [&str; 54] = [
    ">>>=", "...", "===", "!==", "**=", "<<=", ">>=", ">>>", "&&=", "||=", "??=", "=>", "==", "!=",
    "<=", ">=", "&&", "||", "??", "?.", "++", "--", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=",
    "<<", ">>", "**", "{", "}", "(", ")", "[", "]", ";", ",", "<", ">", "+", "-", "*", "/", "%",
    "&", "|", "^", "!", "~", "?",
];
const PUNCT_TAIL: [&str; 3] = [":", "=", "."];

#[derive(Clone)]
pub struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pub pos: usize,
}

fn err<T>(message: impl Into<String>, pos: usize) -> Result<T> {
    Err(SyntaxError {
        message: message.into(),
        pos,
    })
}

pub fn is_id_start(c: char) -> bool {
    c == '$' || c == '_' || c.is_alphabetic()
}

pub fn is_id_part(c: char) -> bool {
    c == '$' || c == '_' || c == '\u{200c}' || c == '\u{200d}' || c.is_alphanumeric()
}

fn is_line_break(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            src,
            bytes: src.as_bytes(),
            pos: 0,
        }
    }

    pub fn source(&self) -> &'a str {
        self.src
    }

    fn peek_char(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn at(&self, i: usize) -> u8 {
        self.bytes.get(i).copied().unwrap_or(0)
    }

    /// Skips white space and comments; whether a line break was among them.
    fn skip_trivia(&mut self) -> Result<bool> {
        let mut newline = false;
        loop {
            let b = self.at(self.pos);
            match b {
                b' ' | b'\t' | 0x0b | 0x0c => self.pos += 1,
                b'\n' | b'\r' => {
                    newline = true;
                    self.pos += 1;
                }
                b'/' if self.at(self.pos + 1) == b'/' => {
                    while self.pos < self.bytes.len() {
                        let c = self.peek_char().unwrap();
                        if is_line_break(c) {
                            break;
                        }
                        self.pos += c.len_utf8();
                    }
                }
                b'/' if self.at(self.pos + 1) == b'*' => {
                    let start = self.pos;
                    let Some(end) = self.src[self.pos + 2..].find("*/") else {
                        return err("unterminated comment", start);
                    };
                    let body = &self.src[self.pos + 2..self.pos + 2 + end];
                    if body.chars().any(is_line_break) {
                        newline = true;
                    }
                    self.pos += end + 4;
                }
                // `<!--` and `-->` comments are only for HTML pages.
                0 if self.pos >= self.bytes.len() => return Ok(newline),
                _ if b >= 0x80 => {
                    let c = self.peek_char().unwrap();
                    if is_line_break(c) {
                        newline = true;
                    } else if !(c.is_whitespace() || c == '\u{feff}') {
                        return Ok(newline);
                    }
                    self.pos += c.len_utf8();
                }
                _ => return Ok(newline),
            }
        }
    }

    pub fn token(&mut self) -> Result<Token> {
        let newline_before = self.skip_trivia()?;
        let start = self.pos;
        let tok = self.read()?;
        Ok(Token {
            tok,
            start,
            end: self.pos,
            newline_before,
        })
    }

    fn read(&mut self) -> Result<Tok> {
        let start = self.pos;
        let Some(c) = self.peek_char() else {
            return Ok(Tok::Eof);
        };
        if c == '#' && start == 0 && self.at(1) == b'!' {
            while let Some(c) = self.peek_char() {
                if is_line_break(c) {
                    break;
                }
                self.pos += c.len_utf8();
            }
            return self.token().map(|t| t.tok);
        }
        if is_id_start(c) || c == '\\' {
            let (name, escaped) = self.ident()?;
            return Ok(if escaped {
                Tok::EscapedIdent(name)
            } else {
                Tok::Ident(name)
            });
        }
        if c == '#' {
            self.pos += 1;
            let (name, _) = self.ident()?;
            return Ok(Tok::Private(name));
        }
        if c.is_ascii_digit() || (c == '.' && self.at(self.pos + 1).is_ascii_digit()) {
            return self.number();
        }
        if c == '"' || c == '\'' {
            return self.string(c);
        }
        if c == '`' {
            self.pos += 1;
            return self.template();
        }
        let rest = &self.src[self.pos..];
        // `?.` followed by a digit is `?` then a number (`a?.5:b`).
        for p in PUNCT {
            if rest.starts_with(p) {
                if p == "?." && self.at(self.pos + 2).is_ascii_digit() {
                    continue;
                }
                self.pos += p.len();
                return Ok(Tok::Punct(p));
            }
        }
        for p in PUNCT_TAIL {
            if rest.starts_with(p) {
                self.pos += 1;
                return Ok(Tok::Punct(p));
            }
        }
        err(format!("unexpected character {c:?}"), start)
    }

    fn ident(&mut self) -> Result<(Name, bool)> {
        let start = self.pos;
        let mut out = String::new();
        let mut escaped = false;
        let mut first = true;
        while let Some(c) = self.peek_char() {
            let c = if c == '\\' {
                if self.at(self.pos + 1) != b'u' {
                    return err("bad escape in identifier", self.pos);
                }
                self.pos += 2;
                escaped = true;
                let Some(c) = self.unicode_escape()? else {
                    return err("bad escape in identifier", self.pos);
                };
                c
            } else if (first && is_id_start(c)) || (!first && is_id_part(c)) {
                self.pos += c.len_utf8();
                c
            } else {
                break;
            };
            if !(if first { is_id_start(c) } else { is_id_part(c) }) {
                return err("bad identifier character", self.pos);
            }
            out.push(c);
            first = false;
        }
        if out.is_empty() {
            return err("expected an identifier", start);
        }
        Ok((crate::value::intern(&out), escaped))
    }

    /// After `\u`: `XXXX` or `{X...}`.
    fn unicode_escape(&mut self) -> Result<Option<char>> {
        let v = if self.at(self.pos) == b'{' {
            let end = self.src[self.pos..].find('}').map(|e| self.pos + e);
            let Some(end) = end else {
                return Ok(None);
            };
            let v = u32::from_str_radix(&self.src[self.pos + 1..end], 16).ok();
            self.pos = end + 1;
            match v {
                Some(v) if v <= 0x10ffff => v,
                _ => return Ok(None),
            }
        } else {
            let Some(hex) = self.src.get(self.pos..self.pos + 4) else {
                return Ok(None);
            };
            let Ok(v) = u32::from_str_radix(hex, 16) else {
                return Ok(None);
            };
            if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Ok(None);
            }
            self.pos += 4;
            v
        };
        Ok(Some(char::from_u32(v).unwrap_or('\u{fffd}')))
    }

    /// `\uD83D\uDE00` pairs make one character; lone halves stay as the
    /// replacement character (strings here are UTF-8).
    fn unicode_escape_pair(&mut self) -> Result<Option<char>> {
        let at = self.pos;
        let hex = self.src.get(self.pos..self.pos + 4).unwrap_or("");
        if let Ok(hi) = u32::from_str_radix(hex, 16) {
            if (0xd800..0xdc00).contains(&hi)
                && hex.len() == 4
                && self.src[self.pos + 4..].starts_with("\\u")
            {
                if let Ok(lo) =
                    u32::from_str_radix(self.src.get(self.pos + 6..self.pos + 10).unwrap_or(""), 16)
                {
                    if (0xdc00..0xe000).contains(&lo) {
                        self.pos += 10;
                        return Ok(char::from_u32(
                            0x10000 + ((hi - 0xd800) << 10) + (lo - 0xdc00),
                        ));
                    }
                }
            }
        }
        self.pos = at;
        self.unicode_escape()
    }

    fn number(&mut self) -> Result<Tok> {
        let start = self.pos;
        let radix = if self.at(start) == b'0' {
            match self.at(start + 1) | 0x20 {
                b'x' => 16,
                b'o' => 8,
                b'b' => 2,
                _ => 10,
            }
        } else {
            10
        };
        if radix != 10 {
            self.pos += 2;
            let digits = self.digits(|b| (b as char).is_digit(radix));
            if digits.is_empty() {
                return err("missing digits", start);
            }
            if self.at(self.pos) == b'n' {
                self.pos += 1;
                return Ok(Tok::BigInt(self.src[start..self.pos - 1].into()));
            }
            let v = digits.chars().fold(0.0f64, |a, c| {
                a * radix as f64 + c.to_digit(radix).unwrap() as f64
            });
            return self.end_number(Tok::Num(v));
        }
        // Legacy octal (`017`) and `08`.
        if self.at(start) == b'0' && self.at(start + 1).is_ascii_digit() {
            let digits = self.digits(|b| b.is_ascii_digit());
            let v = if digits.bytes().all(|b| b < b'8') {
                digits
                    .chars()
                    .fold(0.0, |a, c| a * 8.0 + c.to_digit(8).unwrap() as f64)
            } else {
                digits.parse().unwrap_or(0.0)
            };
            return self.end_number(Tok::Num(v));
        }
        let mut text = self.digits(|b| b.is_ascii_digit());
        if self.at(self.pos) == b'n' {
            self.pos += 1;
            return Ok(Tok::BigInt(text.into()));
        }
        if self.at(self.pos) == b'.' {
            self.pos += 1;
            text.push('.');
            text += &self.digits(|b| b.is_ascii_digit());
        }
        if self.at(self.pos) | 0x20 == b'e' {
            let save = self.pos;
            self.pos += 1;
            let mut exp = String::from("e");
            if matches!(self.at(self.pos), b'+' | b'-') {
                exp.push(self.at(self.pos) as char);
                self.pos += 1;
            }
            let d = self.digits(|b| b.is_ascii_digit());
            if d.is_empty() {
                self.pos = save;
                return err("missing exponent", save);
            }
            text += &exp;
            text += &d;
        }
        let v: f64 = text.parse().map_err(|_| SyntaxError {
            message: "bad number".into(),
            pos: start,
        })?;
        self.end_number(Tok::Num(v))
    }

    fn end_number(&self, t: Tok) -> Result<Tok> {
        match self.peek_char() {
            Some(c) if is_id_start(c) || c.is_ascii_digit() => {
                err("identifier straight after a number", self.pos)
            }
            _ => Ok(t),
        }
    }

    /// Digits with `_` separators, which are dropped.
    fn digits(&mut self, ok: impl Fn(u8) -> bool) -> String {
        let mut s = String::new();
        loop {
            let b = self.at(self.pos);
            if ok(b) {
                s.push(b as char);
            } else if !(b == b'_' && ok(self.at(self.pos + 1)) && !s.is_empty()) {
                break;
            }
            self.pos += 1;
        }
        s
    }

    fn string(&mut self, quote: char) -> Result<Tok> {
        let start = self.pos;
        self.pos += 1;
        let mut out = String::new();
        loop {
            let Some(c) = self.peek_char() else {
                return err("unterminated string", start);
            };
            self.pos += c.len_utf8();
            match c {
                c if c == quote => return Ok(Tok::Str(crate::value::intern(&out))),
                '\\' => {
                    if let Some(e) = self.escape(false)? {
                        out.push(e);
                    } else if self.at(self.pos - 1) != b'\n' {
                        // Line continuations add nothing; others are errors.
                        if !self.continuation_before() {
                            return err("bad escape", self.pos);
                        }
                    }
                }
                '\n' | '\r' => return err("unterminated string", start),
                c => out.push(c),
            }
        }
    }

    fn continuation_before(&self) -> bool {
        let b = &self.src[..self.pos];
        b.ends_with('\n') || b.ends_with('\r') || b.ends_with('\u{2028}') || b.ends_with('\u{2029}')
    }

    /// After a `\`: the character it stands for, `None` for a line
    /// continuation, or an error where it's invalid (`Err` with `template`
    /// meaning the template's cooked text is undefined).
    fn escape(&mut self, template: bool) -> Result<Option<char>> {
        let at = self.pos;
        let Some(c) = self.peek_char() else {
            return err("unterminated string", at);
        };
        self.pos += c.len_utf8();
        Ok(Some(match c {
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            'b' => '\u{8}',
            'f' => '\u{c}',
            'v' => '\u{b}',
            '0' if !self.at(self.pos).is_ascii_digit() => '\0',
            '0'..='7' if !template => {
                // Legacy octal escapes.
                let mut v = c.to_digit(8).unwrap();
                let max = if c <= '3' { 2 } else { 1 };
                for _ in 0..max {
                    let b = self.at(self.pos);
                    if (b'0'..=b'7').contains(&b) {
                        v = v * 8 + (b - b'0') as u32;
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
                char::from_u32(v).unwrap()
            }
            '8' | '9' if !template => c,
            'x' => {
                let hex = self.src.get(self.pos..self.pos + 2).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(v) if hex.bytes().all(|b| b.is_ascii_hexdigit()) => {
                        self.pos += 2;
                        v as char
                    }
                    _ => return err("bad \\x escape", at),
                }
            }
            'u' => match self.unicode_escape_pair()? {
                Some(c) => c,
                None => return err("bad \\u escape", at),
            },
            '\r' => {
                if self.at(self.pos) == b'\n' {
                    self.pos += 1;
                }
                return Ok(None);
            }
            '\n' | '\u{2028}' | '\u{2029}' => return Ok(None),
            c if template && c.is_ascii_digit() => return err("bad escape", at),
            c => c,
        }))
    }

    /// Reads template text after `` ` `` or a substitution's `}`.
    fn template(&mut self) -> Result<Tok> {
        let start = self.pos;
        let mut cooked = Some(String::new());
        loop {
            let Some(c) = self.peek_char() else {
                return err("unterminated template", start);
            };
            let here = self.pos;
            self.pos += c.len_utf8();
            match c {
                '`' => {
                    let raw = self.src[start..here]
                        .replace("\r\n", "\n")
                        .replace('\r', "\n");
                    return Ok(Tok::Template(cooked.map(Into::into), raw.into(), true));
                }
                '$' if self.at(self.pos) == b'{' => {
                    self.pos += 1;
                    let raw = self.src[start..here]
                        .replace("\r\n", "\n")
                        .replace('\r', "\n");
                    return Ok(Tok::Template(cooked.map(Into::into), raw.into(), false));
                }
                '\\' => match self.escape(true) {
                    Ok(Some(e)) => {
                        if let Some(s) = &mut cooked {
                            s.push(e);
                        }
                    }
                    Ok(None) => {}
                    Err(_) => {
                        cooked = None;
                        // Skip the escape's character so `\`` doesn't end it.
                        if self.pos == here + 1 {
                            if let Some(c) = self.peek_char() {
                                self.pos += c.len_utf8();
                            }
                        }
                    }
                },
                '\r' => {
                    if self.at(self.pos) == b'\n' {
                        self.pos += 1;
                    }
                    if let Some(s) = &mut cooked {
                        s.push('\n');
                    }
                }
                c => {
                    if let Some(s) = &mut cooked {
                        s.push(c);
                    }
                }
            }
        }
    }

    /// Reads on from a template substitution's closing `}` at `pos`.
    pub fn template_continue(&mut self, pos: usize) -> Result<Token> {
        self.pos = pos + 1;
        let tok = self.template()?;
        Ok(Token {
            tok,
            start: pos,
            end: self.pos,
            newline_before: false,
        })
    }

    /// Reads a regular expression literal starting with the `/` at `pos`.
    pub fn regex(&mut self, pos: usize, newline_before: bool) -> Result<Token> {
        self.pos = pos + 1;
        let mut class = false;
        loop {
            let Some(c) = self.peek_char() else {
                return err("unterminated regular expression", pos);
            };
            if is_line_break(c) {
                return err("unterminated regular expression", pos);
            }
            self.pos += c.len_utf8();
            match c {
                '\\' => {
                    let Some(n) = self.peek_char() else {
                        return err("unterminated regular expression", pos);
                    };
                    if is_line_break(n) {
                        return err("unterminated regular expression", pos);
                    }
                    self.pos += n.len_utf8();
                }
                '[' => class = true,
                ']' => class = false,
                '/' if !class => break,
                _ => {}
            }
        }
        let body: Name = self.src[pos + 1..self.pos - 1].into();
        let fstart = self.pos;
        while let Some(c) = self.peek_char() {
            if !is_id_part(c) {
                break;
            }
            self.pos += c.len_utf8();
        }
        Ok(Token {
            tok: Tok::Regex(body, self.src[fstart..self.pos].into()),
            start: pos,
            end: self.pos,
            newline_before,
        })
    }

    /// The line and column (1-based) of a byte position.
    pub fn line_col(src: &str, pos: usize) -> (usize, usize) {
        let before = &src[..pos.min(src.len())];
        let line = before.matches('\n').count() + 1;
        let col = before
            .rfind('\n')
            .map_or(before.len(), |n| before.len() - n - 1)
            + 1;
        (line, col)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<Tok> {
        let mut l = Lexer::new(s);
        let mut out = Vec::new();
        loop {
            let t = l.token().unwrap();
            if t.tok == Tok::Eof {
                return out;
            }
            out.push(t.tok);
        }
    }

    #[test]
    fn numbers() {
        assert_eq!(
            toks("0x1F 0b101 0o17 017 1_000 .5 1e3 2.5e-1 10n"),
            vec![
                Tok::Num(31.0),
                Tok::Num(5.0),
                Tok::Num(15.0),
                Tok::Num(15.0),
                Tok::Num(1000.0),
                Tok::Num(0.5),
                Tok::Num(1000.0),
                Tok::Num(0.25),
                Tok::BigInt("10".into())
            ]
        );
    }

    #[test]
    fn strings_and_escapes() {
        assert_eq!(
            toks(
                r#"'a\nb' "\x41\u0042\u{43}" "\uD83D\uDE00" 'a\
b'"#
            ),
            vec![
                Tok::Str("a\nb".into()),
                Tok::Str("ABC".into()),
                Tok::Str("😀".into()),
                Tok::Str("ab".into())
            ]
        );
    }

    #[test]
    fn punctuation_longest_first() {
        assert_eq!(
            toks("a >>>= b ?. c ?.5 : d"),
            vec![
                Tok::Ident("a".into()),
                Tok::Punct(">>>="),
                Tok::Ident("b".into()),
                Tok::Punct("?."),
                Tok::Ident("c".into()),
                Tok::Punct("?"),
                Tok::Num(0.5),
                Tok::Punct(":"),
                Tok::Ident("d".into())
            ]
        );
    }

    #[test]
    fn comments_and_line_breaks() {
        let mut l = Lexer::new("a /* x\n */ b // c\n d");
        assert!(!l.token().unwrap().newline_before);
        assert!(l.token().unwrap().newline_before);
        assert!(l.token().unwrap().newline_before);
    }

    #[test]
    fn templates_and_regexes() {
        let mut l = Lexer::new("`a${x}b\\u0041`");
        assert_eq!(
            l.token().unwrap().tok,
            Tok::Template(Some("a".into()), "a".into(), false)
        );
        l.token().unwrap();
        let close = l.token().unwrap();
        assert_eq!(close.tok, Tok::Punct("}"));
        assert_eq!(
            l.template_continue(close.start).unwrap().tok,
            Tok::Template(Some("bA".into()), "b\\u0041".into(), true)
        );
        let mut l = Lexer::new("/[/]a\\//gi");
        let t = l.token().unwrap();
        assert_eq!(
            l.regex(t.start, false).unwrap().tok,
            Tok::Regex("[/]a\\/".into(), "gi".into())
        );
    }
}
