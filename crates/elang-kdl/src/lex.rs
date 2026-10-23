//! KDL 2.0 lexer (T0501–T0512).
//!
//! Produces a token stream on demand; nothing is pre-materialised, so a
//! 1 MiB document that fails on byte 3 costs three bytes of work.
//!
//! ## Budget enforcement
//!
//! The lexer is the **first** layer that can reject an oversized construct,
//! and it does so before allocating:
//!
//! - `MAX_ENCODED_TOKEN_BYTES` — a single token (identifier, string,
//!   number, comment) longer than 1 MiB returns
//!   [`ErrorCode::TokenTooLong`] before the token buffer grows past it.
//! - `MAX_DECODED_STRING_BYTES` — a string whose *decoded* form would
//!   exceed 1 MiB returns [`ErrorCode::StringTooLong`]; the decoded
//!   buffer is never grown past the limit.
//! - `MAX_COMMENT_BYTES` — a single comment longer than 1 MiB returns
//!   [`ErrorCode::CommentTooLong`].
//! - `MAX_COMMENT_BYTES_PER_RESOURCE` — cumulative comment bytes across
//!   the document are checked via the shared [`BudgetTracker`].
//!
//! ## Whitespace and comments (T0512)
//!
//! All KDL 2.0 whitespace forms are accepted and discarded: space, tab,
//! CR, LF, U+000B (vertical tab), U+000C (form feed), and U+00A0 /
//! U+FEFF (treated as whitespace per the spec). Comment forms:
//!
//! - `// …` line comment
//! - `/* … */` block comment
//! - `/** … */` doc comment (lexed as a block comment; the distinction
//!   is a schema-layer concern)
//! - `/*! … */` module doc comment (same treatment)

// The lexer walks the source byte-by-byte after explicit bounds checks;
// the arithmetic is on values that are statically known to be in range.
// Same pattern as the escape decoder and elang-container's frame reader.
//
// `usize → u32` casts in span / position helpers are safe because the
// shared `BudgetTracker` caps every input below `u32::MAX`.
#![allow(
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation
)]

use crate::budget::BudgetTracker;
use crate::error::{ErrorCode, ParseError};
use crate::span::SourceSpan;
use crate::value::KdlValue;
use elang_core::limits::{
    MAX_COMMENT_BYTES, MAX_DECODED_STRING_BYTES, MAX_ENCODED_TOKEN_BYTES, MAX_NAME_BYTES,
};

/// A lexed token.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// Bare identifier (not a keyword, not a number).
    Ident(SourceSpan, Box<str>),
    /// `true` or `false`.
    Bool(SourceSpan, bool),
    /// `null`.
    Null(SourceSpan),
    /// Integer literal; lexeme preserved verbatim.
    Integer(SourceSpan, Box<str>),
    /// Float literal; lexeme preserved verbatim.
    Float(SourceSpan, Box<str>),
    /// Standard double-quoted string; value already decoded.
    Str(SourceSpan, Box<str>),
    /// Raw string `r#"…"#`; value is the raw content (no escapes).
    RawStr(SourceSpan, Box<str>),
    /// Line terminator: `\n`, `\r\n`, `\r`, U+000C, or `;`.
    Newline(SourceSpan),
    /// `{` opening a children block.
    OpenBrace(SourceSpan),
    /// `}` closing a children block.
    CloseBrace(SourceSpan),
    /// `=` property assignment.
    Equals(SourceSpan),
    /// `(` opening a type annotation.
    OpenParen(SourceSpan),
    /// `)` closing a type annotation.
    CloseParen(SourceSpan),
    /// `/` in a `/kdl-version` pragma or a slash-dash node (`/-foo`).
    Slash(SourceSpan),
    /// `-` immediately followed by `/` (`/-`), the slash-dash node prefix.
    SlashDash(SourceSpan),
    /// End of input.
    Eof(SourceSpan),
}

impl Token {
    /// Source span of the token.
    pub fn span(&self) -> SourceSpan {
        match self {
            Token::Ident(s, _)
            | Token::Bool(s, _)
            | Token::Null(s)
            | Token::Integer(s, _)
            | Token::Float(s, _)
            | Token::Str(s, _)
            | Token::RawStr(s, _)
            | Token::Newline(s)
            | Token::OpenBrace(s)
            | Token::CloseBrace(s)
            | Token::Equals(s)
            | Token::OpenParen(s)
            | Token::CloseParen(s)
            | Token::Slash(s)
            | Token::SlashDash(s)
            | Token::Eof(s) => *s,
        }
    }

    /// Human-readable kind, for diagnostics.
    pub fn kind(&self) -> &'static str {
        match self {
            Token::Ident(..) => "identifier",
            Token::Bool(..) => "boolean",
            Token::Null(..) => "null",
            Token::Integer(..) => "integer",
            Token::Float(..) => "float",
            Token::Str(..) => "string",
            Token::RawStr(..) => "raw string",
            Token::Newline(..) => "newline",
            Token::OpenBrace(..) => "`{`",
            Token::CloseBrace(..) => "`}`",
            Token::Equals(..) => "`=`",
            Token::OpenParen(..) => "`(`",
            Token::CloseParen(..) => "`)`",
            Token::Slash(..) => "`/`",
            Token::SlashDash(..) => "`/-`",
            Token::Eof(..) => "end of input",
        }
    }
}

/// Streaming lexer over a validated UTF-8 byte slice.
pub struct Lexer<'a> {
    input: &'a [u8],
    pos: usize,
    budgets: &'a mut BudgetTracker,
    /// Buffer reused across string decodes to avoid per-token allocation.
    scratch: Vec<u8>,
}

impl<'a> Lexer<'a> {
    /// Construct a lexer over `input`. The caller is responsible for
    /// validating UTF-8 first (T0502) — [`crate::parse::parse`] does this.
    pub fn new(input: &'a [u8], budgets: &'a mut BudgetTracker) -> Lexer<'a> {
        Lexer {
            input,
            pos: 0,
            budgets,
            scratch: Vec::new(),
        }
    }

    /// Advance the lexer past the rest of the current line. Used after
    /// seeing a `Token::SlashDash` so the parser can treat `/- node "x"`
    /// as a comment that runs to end-of-line, per KDL 2.0.
    pub fn skip_to_eol(&mut self) {
        while let Some(b) = self.byte() {
            self.pos += 1;
            if b == b'\n' {
                break;
            }
        }
    }

    /// Current byte offset.
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// The underlying input.
    pub fn input(&self) -> &'a [u8] {
        self.input
    }

    /// Borrow the shared budget tracker.
    pub fn budgets(&mut self) -> &mut BudgetTracker {
        self.budgets
    }

    fn span(&self, start: usize) -> SourceSpan {
        SourceSpan::from_range(self.input, start as u32, self.pos as u32)
    }

    fn byte(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn peek(&self, offset: usize) -> Option<u8> {
        self.input.get(self.pos + offset).copied()
    }

    /// Returns `true` for bytes KDL 2.0 treats as whitespace.
    fn is_ws(b: u8) -> bool {
        matches!(
            b,
            b' ' | b'\t'
                | b'\r'
                | b'\n'
                | 0x0B // vertical tab
                | 0x0C // form feed
                | 0xC2 // first byte of U+00A0 NBSP
                | 0xEF // first byte of U+FEFF BOM
        )
    }

    /// Returns `true` for bytes that may appear in a bare identifier.
    ///
    /// KDL 2.0 identifiers permit any non-ASCII-Unicode-whitespace,
    /// non-control character except the reserved set `{ } ( ) \ " / = ; [ ]`
    /// and the comment introducers `/*` and `//`. Hyphens and dots are
    /// allowed.
    fn is_ident_byte(b: u8) -> bool {
        if b < 0x80 {
            !(matches!(
                b,
                b' ' | b'\t'
                    | b'\r'
                    | b'\n'
                    | 0x0B
                    | 0x0C
                    | b'"'
                    | b'\\'
                    | b'/'
                    | b'{'
                    | b'}'
                    | b'('
                    | b')'
                    | b'='
                    | b';'
                    | b'['
                    | b']'
                    | b','
            ) || b < 0x20
                || b == 0x7F)
        } else {
            // Non-ASCII: reject C1 controls and NBSP. Full Unicode
            // whitespace classification is delegated to the later
            // `validate` pass, which uses `char::is_whitespace`.
            !(b == 0xC2 || b == 0xE2 || b == 0xEF)
        }
    }

    /// Skips horizontal whitespace and comments. **Newlines are
    /// preserved** so the parser can emit [`Token::Newline`] tokens
    /// and use them as KDL's statement terminator.
    fn skip_trivia(&mut self) -> Result<(), ParseError> {
        loop {
            match self.byte() {
                None => return Ok(()),
                // Newlines are statement terminators, not trivia.
                Some(b'\n') | Some(b'\r') | Some(0x0C) => return Ok(()),
                Some(b) if Lexer::is_ws(b) => {
                    self.pos += 1;
                }
                Some(b'/') if self.peek(1) == Some(b'/') => {
                    let start = self.pos;
                    self.pos += 2;
                    while let Some(c) = self.byte() {
                        if c == b'\n' || c == b'\r' {
                            break;
                        }
                        self.pos += 1;
                        let len = self.pos - start;
                        if len > MAX_COMMENT_BYTES {
                            return Err(ParseError::at(
                                ErrorCode::CommentTooLong,
                                self.span(start),
                                "line comment exceeds MAX_COMMENT_BYTES",
                            ));
                        }
                    }
                    // Consume the terminating newline so the caller never
                    // sees a `Newline` token between a line comment and
                    // the next statement.
                    match self.byte() {
                        Some(b'\r') => {
                            self.pos += 1;
                            if self.byte() == Some(b'\n') {
                                self.pos += 1;
                            }
                        }
                        Some(b'\n') => {
                            self.pos += 1;
                        }
                        _ => {}
                    }
                    self.charge_comment(start)?;
                }
                Some(b'/') if self.peek(1) == Some(b'*') => {
                    let start = self.pos;
                    self.pos += 2;
                    loop {
                        match self.byte() {
                            None => {
                                return Err(ParseError::at(
                                    ErrorCode::UnterminatedComment,
                                    self.span(start),
                                    "unterminated block comment",
                                ))
                            }
                            Some(b'*') if self.peek(1) == Some(b'/') => {
                                self.pos += 2;
                                break;
                            }
                            Some(_) => {
                                self.pos += 1;
                                let len = self.pos - start;
                                if len > MAX_COMMENT_BYTES {
                                    return Err(ParseError::at(
                                        ErrorCode::CommentTooLong,
                                        self.span(start),
                                        "block comment exceeds MAX_COMMENT_BYTES",
                                    ));
                                }
                            }
                        }
                    }
                    self.charge_comment(start)?;
                }
                Some(_) => return Ok(()),
            }
        }
    }

    fn charge_comment(&mut self, start: usize) -> Result<(), ParseError> {
        let len = (self.pos - start) as u64;
        self.budgets
            .comment_bytes_resource()
            .try_add(len)
            .map_err(|code| {
                ParseError::at(
                    code,
                    self.span(start),
                    "cumulative comment bytes exceed MAX_COMMENT_BYTES_PER_RESOURCE",
                )
            })
    }

    /// Lex the next token. Returns `Err` on the first problem.
    pub fn next_token(&mut self) -> Result<Token, ParseError> {
        self.skip_trivia()?;
        let start = self.pos;

        let Some(b) = self.byte() else {
            return Ok(Token::Eof(self.span(start)));
        };

        match b {
            b'{' => {
                self.pos += 1;
                Ok(Token::OpenBrace(self.span(start)))
            }
            b'}' => {
                self.pos += 1;
                Ok(Token::CloseBrace(self.span(start)))
            }
            b'=' => {
                self.pos += 1;
                Ok(Token::Equals(self.span(start)))
            }
            b'(' => {
                self.pos += 1;
                Ok(Token::OpenParen(self.span(start)))
            }
            b')' => {
                self.pos += 1;
                Ok(Token::CloseParen(self.span(start)))
            }
            b';' | b'\r' | b'\n' | 0x0C => {
                self.pos += 1;
                Ok(Token::Newline(self.span(start)))
            }
            b'"' => self.lex_string(start),
            b'r' if matches!(self.peek(1), Some(b'"') | Some(b'#')) => self.lex_raw_string(start),
            b'/' if self.peek(1) == Some(b'-') => {
                self.pos += 2;
                Ok(Token::SlashDash(self.span(start)))
            }
            b'/' => {
                self.pos += 1;
                Ok(Token::Slash(self.span(start)))
            }
            b'0'..=b'9' | b'+' | b'-' | b'.' => self.lex_number(start),
            _ => self.lex_ident(start),
        }
    }

    fn lex_ident(&mut self, start: usize) -> Result<Token, ParseError> {
        // KDL 2.0 keywords.
        let rest = &self.input[start..];
        if rest.starts_with(b"true") {
            let after = self.input.get(start + 4).copied();
            let word_end = after.is_none_or(|c| !Lexer::is_ident_byte(c));
            if word_end {
                self.pos = start + 4;
                return Ok(Token::Bool(self.span(start), true));
            }
        }
        if rest.starts_with(b"false") {
            let after = self.input.get(start + 5).copied();
            let word_end = after.is_none_or(|c| !Lexer::is_ident_byte(c));
            if word_end {
                self.pos = start + 5;
                return Ok(Token::Bool(self.span(start), false));
            }
        }
        if rest.starts_with(b"null") {
            let after = self.input.get(start + 4).copied();
            let word_end = after.is_none_or(|c| !Lexer::is_ident_byte(c));
            if word_end {
                self.pos = start + 4;
                return Ok(Token::Null(self.span(start)));
            }
        }

        while self.byte().is_some_and(Lexer::is_ident_byte) {
            self.pos += 1;
            let len = self.pos - start;
            if len > MAX_ENCODED_TOKEN_BYTES {
                return Err(ParseError::at(
                    ErrorCode::TokenTooLong,
                    self.span(start),
                    "identifier exceeds MAX_ENCODED_TOKEN_BYTES",
                ));
            }
            if len > MAX_NAME_BYTES {
                return Err(ParseError::at(
                    ErrorCode::NameTooLong,
                    self.span(start),
                    "identifier exceeds MAX_NAME_BYTES",
                ));
            }
        }

        if self.pos == start {
            // No progress: the byte is not a valid identifier start.
            self.pos += 1;
            return Err(ParseError::at(
                ErrorCode::InvalidIdentifier,
                self.span(start),
                "invalid identifier character",
            ));
        }

        let raw = &self.input[start..self.pos];
        let s = core::str::from_utf8(raw).map_err(|_| {
            ParseError::at(
                ErrorCode::InvalidUtf8,
                self.span(start),
                "identifier is not valid UTF-8",
            )
        })?;
        Ok(Token::Ident(self.span(start), s.into()))
    }

    fn lex_number(&mut self, start: usize) -> Result<Token, ParseError> {
        // A leading `+` or `-` is a sign, not an exponent marker. Consume it
        // first so the digit scan below does not reject it (the exponent-sign
        // branch requires a preceding `e`/`E`, which a leading sign lacks).
        let signed = matches!(self.byte(), Some(b'+') | Some(b'-'));
        if signed {
            self.pos += 1;
        }
        // A bare sign is not a number.
        let digits_start = self.pos;

        // Base prefixes: 0x / 0o / 0b
        if self.byte() == Some(b'0') {
            if let Some(p) = self.peek(1) {
                let radix = match p {
                    b'x' | b'X' => Some(16u32),
                    b'o' | b'O' => Some(8u32),
                    b'b' | b'B' => Some(2u32),
                    _ => None,
                };
                if let Some(_radix) = radix {
                    self.pos += 2;
                    let prefix_digits_start = self.pos;
                    while let Some(c) = self.byte() {
                        if c.is_ascii_alphanumeric() || c == b'_' {
                            self.pos += 1;
                        } else {
                            break;
                        }
                        if self.pos - start > MAX_ENCODED_TOKEN_BYTES {
                            return Err(ParseError::at(
                                ErrorCode::TokenTooLong,
                                self.span(start),
                                "number exceeds MAX_ENCODED_TOKEN_BYTES",
                            ));
                        }
                    }
                    if self.pos == prefix_digits_start {
                        return Err(ParseError::at(
                            ErrorCode::InvalidNumber,
                            self.span(start),
                            "number has no digits after base prefix",
                        ));
                    }
                    let lexeme = core::str::from_utf8(&self.input[start..self.pos])
                        .unwrap_or("")
                        .to_string();
                    return Ok(Token::Integer(self.span(start), lexeme.into_boxed_str()));
                }
            }
        }

        let mut is_float = false;
        while let Some(c) = self.byte() {
            match c {
                b'0'..=b'9' | b'_' => {}
                b'.' => is_float = true,
                b'e' | b'E' => is_float = true,
                b'+' | b'-' => {
                    // Only valid as an exponent sign; the previous byte must
                    // be `e`/`E`.
                    let prev = self.input.get(self.pos - 1).copied();
                    if !matches!(prev, Some(b'e') | Some(b'E')) {
                        break;
                    }
                }
                _ => break,
            }
            self.pos += 1;
            if self.pos - start > MAX_ENCODED_TOKEN_BYTES {
                return Err(ParseError::at(
                    ErrorCode::TokenTooLong,
                    self.span(start),
                    "number exceeds MAX_ENCODED_TOKEN_BYTES",
                ));
            }
        }

        // Either nothing followed the sign, or the scan consumed nothing.
        if self.pos == digits_start {
            return Err(ParseError::at(
                ErrorCode::InvalidNumber,
                self.span(start),
                "malformed number",
            ));
        }

        let lexeme = core::str::from_utf8(&self.input[start..self.pos])
            .unwrap_or("")
            .to_string();
        if is_float {
            Ok(Token::Float(self.span(start), lexeme.into_boxed_str()))
        } else {
            Ok(Token::Integer(self.span(start), lexeme.into_boxed_str()))
        }
    }

    fn lex_string(&mut self, start: usize) -> Result<Token, ParseError> {
        self.pos += 1; // opening quote
        let content_start = self.pos;
        // Scan for the closing quote, tracking the maximum encoded length as
        // we go so a 1 GiB string with no closing quote is rejected at
        // 1 MiB rather than after a full scan.
        loop {
            match self.byte() {
                None => {
                    return Err(ParseError::at(
                        ErrorCode::UnterminatedString,
                        self.span(start),
                        "unterminated string",
                    ))
                }
                Some(b'"') => break,
                Some(b'\\') => {
                    self.pos += 2;
                    if self.pos - start > MAX_ENCODED_TOKEN_BYTES {
                        return Err(ParseError::at(
                            ErrorCode::TokenTooLong,
                            self.span(start),
                            "string exceeds MAX_ENCODED_TOKEN_BYTES",
                        ));
                    }
                }
                Some(_) => {
                    self.pos += 1;
                    if self.pos - start > MAX_ENCODED_TOKEN_BYTES {
                        return Err(ParseError::at(
                            ErrorCode::TokenTooLong,
                            self.span(start),
                            "string exceeds MAX_ENCODED_TOKEN_BYTES",
                        ));
                    }
                }
            }
        }
        let content_end = self.pos;
        self.pos += 1; // closing quote
        let encoded_end = self.pos;

        self.scratch.clear();
        self.scratch.resize(
            (content_end - content_start).min(MAX_DECODED_STRING_BYTES),
            0,
        );
        let n = crate::escape::decode_string(
            self.input,
            content_start as u32,
            content_end as u32,
            encoded_end as u32,
            &mut self.scratch,
        )?;
        let s = core::str::from_utf8(&self.scratch[..n as usize]).map_err(|_| {
            ParseError::at(
                ErrorCode::InvalidUtf8,
                self.span(start),
                "decoded string is not valid UTF-8",
            )
        })?;
        let decoded: Box<str> = s.into();
        let bytes = decoded.len() as u64;
        // Charge the per-resource decoded-string budget.
        self.budgets
            .decoded_string_bytes_resource()
            .try_add(bytes)
            .map_err(|code| {
                ParseError::at(
                    code,
                    self.span(start),
                    "decoded string bytes exceed MAX_DECODED_STRING_BYTES_PER_RESOURCE",
                )
            })?;
        // Charge the archive-wide decoded-string budget (T0509). The
        // container layer drives this across sequential resources via
        // `BudgetTracker::reset` (per-resource) without resetting
        // `decoded_string_bytes_archive`, so the aggregate is enforced.
        self.budgets
            .decoded_string_bytes_archive()
            .try_add(bytes)
            .map_err(|code| {
                ParseError::at(
                    code,
                    self.span(start),
                    "decoded string bytes exceed MAX_DECODED_STRING_BYTES_PER_ARCHIVE",
                )
            })?;
        Ok(Token::Str(self.span(start), decoded))
    }

    fn lex_raw_string(&mut self, start: usize) -> Result<Token, ParseError> {
        // `r` then `#`* then `"`.
        self.pos += 1; // r
        let mut hash_count = 0usize;
        while self.byte() == Some(b'#') {
            hash_count += 1;
            self.pos += 1;
            if hash_count > MAX_ENCODED_TOKEN_BYTES {
                return Err(ParseError::at(
                    ErrorCode::TokenTooLong,
                    self.span(start),
                    "raw string exceeds MAX_ENCODED_TOKEN_BYTES",
                ));
            }
        }
        if self.byte() != Some(b'"') {
            // `r#` not followed by `"` — treat `r` as an identifier.
            self.pos = start + 1;
            let raw = &self.input[start..self.pos];
            let s = core::str::from_utf8(raw).unwrap_or("r");
            return Ok(Token::Ident(self.span(start), s.into()));
        }
        self.pos += 1; // opening quote

        let content_start = self.pos;
        // The closing fence is `"` followed by `hash_count` `#` bytes.
        let mut fence = vec![b'"'; 1 + hash_count];
        for b in &mut fence[1..] {
            *b = b'#';
        }
        loop {
            match self.byte() {
                None => {
                    return Err(ParseError::at(
                        ErrorCode::UnterminatedRawString,
                        self.span(start),
                        "unterminated raw string",
                    ))
                }
                Some(b'"') if self.input[self.pos..].starts_with(&fence) => {
                    break;
                }
                Some(_) => {
                    self.pos += 1;
                    if self.pos - start > MAX_ENCODED_TOKEN_BYTES {
                        return Err(ParseError::at(
                            ErrorCode::TokenTooLong,
                            self.span(start),
                            "raw string exceeds MAX_ENCODED_TOKEN_BYTES",
                        ));
                    }
                }
            }
        }
        let content_end = self.pos;
        self.pos += 1 + hash_count; // closing quote + fences

        let raw = &self.input[content_start..content_end];
        let s = core::str::from_utf8(raw).map_err(|_| {
            ParseError::at(
                ErrorCode::InvalidUtf8,
                self.span(start),
                "raw string is not valid UTF-8",
            )
        })?;
        self.budgets
            .decoded_string_bytes_resource()
            .try_add(s.len() as u64)
            .map_err(|code| {
                ParseError::at(
                    code,
                    self.span(start),
                    "raw string bytes exceed MAX_DECODED_STRING_BYTES_PER_RESOURCE",
                )
            })?;
        Ok(Token::RawStr(self.span(start), s.into()))
    }

    /// Build a [`KdlValue`] from a token. Called by the parser.
    pub fn token_to_value(token: &Token) -> Option<KdlValue> {
        match token {
            Token::Null(s) => Some(KdlValue::Null(*s)),
            Token::Bool(s, b) => Some(KdlValue::Bool {
                span: *s,
                value: *b,
            }),
            Token::Integer(s, lex) => Some(KdlValue::Integer {
                span: *s,
                lexeme: lex.clone(),
                value: parse_integer(lex),
            }),
            Token::Float(s, lex) => Some(KdlValue::Float {
                span: *s,
                lexeme: lex.clone(),
                approx: parse_float(lex),
            }),
            Token::Str(s, v) => Some(KdlValue::String {
                span: *s,
                value: v.clone(),
                raw: false,
            }),
            Token::RawStr(s, v) => Some(KdlValue::String {
                span: *s,
                value: v.clone(),
                raw: true,
            }),
            _ => None,
        }
    }
}

/// Parse an integer lexeme (with optional base prefix and `_` separators)
/// into an `i128`. Returns `None` when the value does not fit.
pub fn parse_integer(lex: &str) -> Option<i128> {
    let (neg, rest) = match lex.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, lex.strip_prefix('+').unwrap_or(lex)),
    };
    let (radix, digits) =
        if let Some(r) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
            (16, r)
        } else if let Some(r) = rest.strip_prefix("0o").or_else(|| rest.strip_prefix("0O")) {
            (8, r)
        } else if let Some(r) = rest.strip_prefix("0b").or_else(|| rest.strip_prefix("0B")) {
            (2, r)
        } else {
            (10, rest)
        };
    let cleaned: String = digits.chars().filter(|c| *c != '_').collect();
    if cleaned.is_empty() {
        return None;
    }
    let magnitude = i128::from_str_radix(&cleaned, radix).ok()?;
    if neg {
        magnitude.checked_neg()
    } else {
        Some(magnitude)
    }
}

/// Parse a float lexeme into an `f64`, stripping `_` separators.
pub fn parse_float(lex: &str) -> Option<f64> {
    let cleaned: String = lex.chars().filter(|c| *c != '_').collect();
    cleaned.parse::<f64>().ok()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "lexer tests assert exact token kinds at fixed indices; a panic signals a lexer regression"
)]
mod tests {
    use super::*;

    fn lex_all(input: &str) -> Result<Vec<Token>, ParseError> {
        let mut budgets = BudgetTracker::new();
        let mut lx = Lexer::new(input.as_bytes(), &mut budgets);
        let mut out = Vec::new();
        loop {
            let t = lx.next_token()?;
            let done = matches!(t, Token::Eof(_));
            out.push(t);
            if done {
                break;
            }
        }
        Ok(out)
    }

    #[test]
    fn lexes_simple_node() {
        let toks = lex_all("foo bar").unwrap();
        assert!(matches!(toks[0], Token::Ident(..)));
        assert!(matches!(toks[1], Token::Ident(..)));
        assert!(matches!(toks[2], Token::Eof(_)));
    }

    #[test]
    fn lexes_keywords() {
        let toks = lex_all("true false null").unwrap();
        assert_eq!(
            toks[0],
            Token::Bool(SourceSpan::from_range(b"true false null", 0, 4), true)
        );
        assert!(matches!(toks[1], Token::Bool(_, false)));
        assert!(matches!(toks[2], Token::Null(_)));
    }

    #[test]
    fn lexes_numbers() {
        let toks = lex_all("0xFF 1_000 1.5 1e10").unwrap();
        assert_eq!(toks.len(), 5);
        assert!(matches!(toks[0], Token::Integer(..)));
        assert!(matches!(toks[1], Token::Integer(..)));
        assert!(matches!(toks[2], Token::Float(..)));
        assert!(matches!(toks[3], Token::Float(..)));
        assert!(matches!(toks[4], Token::Eof(_)));
    }

    #[test]
    fn parses_integer_bases() {
        assert_eq!(parse_integer("0xFF"), Some(255));
        assert_eq!(parse_integer("0o17"), Some(15));
        assert_eq!(parse_integer("0b1010"), Some(10));
        assert_eq!(parse_integer("1_000"), Some(1000));
        assert_eq!(parse_integer("-42"), Some(-42));
    }

    #[test]
    fn lexes_string_with_escapes() {
        let toks = lex_all(r#""a\"b""#).unwrap();
        match &toks[0] {
            Token::Str(_, v) => assert_eq!(v.as_ref(), "a\"b"),
            other => panic!("expected string, got {other:?}"),
        }
    }

    #[test]
    fn lexes_raw_string() {
        let toks = lex_all(r##"r#"a\"b"#"##).unwrap();
        match &toks[0] {
            Token::RawStr(_, v) => assert_eq!(v.as_ref(), r#"a\"b"#),
            other => panic!("expected raw string, got {other:?}"),
        }
    }

    #[test]
    fn lexes_comments_and_skips_them() {
        let toks = lex_all("// hi\n/* block */ foo").unwrap();
        assert!(matches!(toks[0], Token::Ident(..)));
    }

    #[test]
    fn rejects_unterminated_string() {
        let err = lex_all("\"abc").unwrap_err();
        assert_eq!(err.code, ErrorCode::UnterminatedString);
    }

    #[test]
    fn rejects_unterminated_comment() {
        let err = lex_all("/* abc").unwrap_err();
        assert_eq!(err.code, ErrorCode::UnterminatedComment);
    }

    #[test]
    fn rejects_name_too_long() {
        let long = "a".repeat(MAX_NAME_BYTES + 1);
        let err = lex_all(&long).unwrap_err();
        assert_eq!(err.code, ErrorCode::NameTooLong);
    }

    #[test]
    fn rejects_oversized_token() {
        let long = "a".repeat(MAX_ENCODED_TOKEN_BYTES + 10);
        let err = lex_all(&long).unwrap_err();
        // An identifier longer than `MAX_ENCODED_TOKEN_BYTES` is also
        // longer than `MAX_NAME_BYTES`, so the smaller ceiling fires first.
        assert_eq!(err.code, ErrorCode::NameTooLong);
    }

    #[test]
    fn lexes_braces_equals_parens() {
        let toks = lex_all("{a=(b)").unwrap();
        assert_eq!(toks.len(), 7);
        assert!(matches!(toks[0], Token::OpenBrace(_)));
        assert!(matches!(toks[1], Token::Ident(..)));
        assert!(matches!(toks[2], Token::Equals(_)));
        assert!(matches!(toks[3], Token::OpenParen(_)));
        assert!(matches!(toks[4], Token::Ident(..)));
        assert!(matches!(toks[5], Token::CloseParen(_)));
        assert!(matches!(toks[6], Token::Eof(_)));
    }

    #[test]
    fn lexes_slash_dash() {
        let toks = lex_all("/-foo").unwrap();
        assert!(matches!(toks[0], Token::SlashDash(_)));
        assert!(matches!(toks[1], Token::Ident(..)));
    }

    #[test]
    fn lexes_newline_forms() {
        let toks = lex_all("a\nb\rc;d").unwrap();
        // a, nl, b, nl, c, ;, d
        assert!(matches!(toks[1], Token::Newline(_)));
        assert!(matches!(toks[3], Token::Newline(_)));
        assert!(matches!(toks[5], Token::Newline(_)));
    }
}
