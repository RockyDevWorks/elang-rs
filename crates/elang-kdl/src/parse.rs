//! KDL 2.0 recursive-descent parser (T0501–T0512).
//!
//! Consumes a token stream from [`crate::lex::Lexer`] and produces a
//! [`KdlDocument`]. Cumulative budgets use the shared [`BudgetTracker`];
//! entry counts are checked per node. The parser refuses to recurse past
//! `MAX_KDL_NESTING_DEPTH`.
//!
//! Grammar (simplified):
//!
//! ```text
//! document    := version-pragma? (node-terminator | node)*
//! version     := "/kdl-version" string node-terminator
//! node        := type? name ("=" value)? entries? children-block?
//! type        := "(" value ")"
//! name        := bare-ident | string
//! entries     := (whitespace? (value | property))*
//! property    := name "=" value
//! value       := string | raw-string | number | bool | null
//! children    := "{" (node-terminator | node)* "}"
//! ```
//!
//! Comments and whitespace are interleaved freely.

// The parser builds [`SourceSpan`] values from `usize` lexer positions;
// the casts are safe because `BudgetTracker` caps every input well below
// `u32::MAX`. `depth + 1` is safe because we check `MAX_KDL_NESTING_DEPTH`
// on entry. This is the same byte-parser pattern used by the lexer and
// escape decoder.
#![allow(clippy::cast_possible_truncation, clippy::arithmetic_side_effects)]

use crate::ast::{KdlChildren, KdlDocument, KdlEntry, KdlNode, KdlVersionPragma};
use crate::budget::BudgetTracker;
use crate::error::{ErrorCode, ParseError};
use crate::lex::{Lexer, Token};
use crate::span::SourceSpan;
use crate::value::KdlValue;

/// Validate UTF-8 input (T0502). Returns the input unchanged on success.
pub fn validate_utf8(input: &[u8]) -> Result<&[u8], ParseError> {
    core::str::from_utf8(input).map(|_| input).map_err(|e| {
        ParseError::at(
            ErrorCode::InvalidUtf8,
            SourceSpan::from_range(input, e.valid_up_to() as u32, e.valid_up_to() as u32 + 1),
            "input is not valid UTF-8",
        )
    })
}

/// Parse `input` as a KDL 2.0 document.
///
/// Validates UTF-8 first, then runs the lexer and parser with fresh budgets.
pub fn parse(input: &[u8]) -> Result<KdlDocument, ParseError> {
    let mut budgets = BudgetTracker::new();
    parse_with_budgets(input, &mut budgets)
}

/// Parse `input` with a caller-supplied budget tracker. The tracker is
/// **not** reset on entry; callers that parse multiple resources should
/// invoke [`BudgetTracker::reset`] between them. UTF-8 is validated before parsing.
pub fn parse_with_budgets(
    input: &[u8],
    budgets: &mut BudgetTracker,
) -> Result<KdlDocument, ParseError> {
    let input = validate_utf8(input)?;
    let mut parser = Parser::new(input, budgets);
    parser.parse_document()
}

/// A pre-validated KDL 2.0 parser.
struct Parser<'a> {
    lexer: Lexer<'a>,
    /// The next token, which may already have advanced the lexer.
    peek: Option<Token>,
    /// End byte of the last token consumed by `bump`, excluding lookahead.
    last_end: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a [u8], budgets: &'a mut BudgetTracker) -> Parser<'a> {
        Parser {
            lexer: Lexer::new(input, budgets),
            peek: None,
            last_end: 0,
        }
    }

    fn budgets(&mut self) -> &mut BudgetTracker {
        self.lexer.budgets()
    }

    /// Fill `peek` if it is empty, materialising a fresh token from the
    /// lexer.
    fn fill_peek(&mut self) -> Result<(), ParseError> {
        if self.peek.is_none() {
            self.peek = Some(self.lexer.next_token()?);
        }
        Ok(())
    }

    /// Consume the next token and record its end byte. The
    /// `.unwrap()` is provably safe: `fill_peek` ran first and only
    /// returns `Ok(())` when `peek` is `Some`.
    #[allow(
        clippy::unwrap_used,
        reason = "`bump` is only called after `fill_peek` returns Ok, which guarantees `peek` is Some"
    )]
    fn bump(&mut self) -> Result<Token, ParseError> {
        self.fill_peek()?;
        let t = self.peek.take().unwrap();
        self.last_end = t.span().end.byte as usize;
        Ok(t)
    }

    #[allow(
        clippy::unwrap_used,
        reason = "`peek` is only called after `fill_peek` returns Ok, which guarantees `peek` is Some"
    )]
    fn peek(&mut self) -> Result<&Token, ParseError> {
        self.fill_peek()?;
        Ok(self.peek.as_ref().unwrap())
    }

    /// Slashdash permits newlines between the prefix and its single target.
    fn slashdash_target(&mut self) -> Result<(), ParseError> {
        let _ = self.bump()?;
        while matches!(self.peek()?, Token::Newline(_)) {
            let _ = self.bump()?;
        }
        Ok(())
    }

    fn error_here(&self, code: ErrorCode, message: &str) -> ParseError {
        let pos = self.lexer.pos();
        ParseError::at(code, self.lexer.span_range(pos, pos), message)
    }

    /// `document := version-pragma? node*`
    fn parse_document(&mut self) -> Result<KdlDocument, ParseError> {
        let start = 0usize;
        let mut version: Option<KdlVersionPragma> = None;

        // The version pragma is optional; validate its value when present.
        let first = self.bump()?;
        match &first {
            Token::Slash(s) => {
                // Expect `/kdl-version <string>`.
                let pragma = self.parse_version_pragma_rest(*s)?;
                // Terminated by a newline.
                let term = self.bump()?;
                if !matches!(term, Token::Newline(_)) {
                    return Err(ParseError::at(
                        ErrorCode::UnexpectedByte,
                        term.span(),
                        "/kdl-version must be followed by a newline",
                    ));
                }
                version = Some(pragma);
            }
            Token::Eof(_) => {
                // Empty document is valid KDL 2.0 (no pragma, no nodes). The
                // canonical forms produced by EPF/ELD/integrity always
                // include the pragma; consumers that need it can detect
                // its absence via `KdlDocument::version()`.
                let end = self.last_end;
                return Ok(KdlDocument {
                    span: self.lexer.span_range(start, end),
                    version,
                    nodes: Box::new([]),
                });
            }
            other => {
                // No pragma; push the token back so `parse_node` can pick
                // it up. We model "pushback" by stashing the token.
                self.peek = Some(other.clone());
            }
        }

        let mut nodes: Vec<KdlNode> = Vec::new();
        loop {
            match self.peek()? {
                Token::Eof(_) => break,
                Token::CloseBrace(span) => {
                    return Err(ParseError::at(
                        ErrorCode::UnexpectedByte,
                        *span,
                        "unexpected top-level `}`",
                    ))
                }
                Token::Newline(_) => {
                    // Stray newlines are tolerated.
                    let _ = self.bump()?;
                }
                Token::SlashDash(_) => {
                    self.slashdash_target()?;
                    let _ = self.parse_node(0)?;
                }
                _ => nodes.push(self.parse_node(0)?),
            }
        }

        let end = self.lexer.pos();
        Ok(KdlDocument {
            span: self.lexer.span_range(start, end),
            version,
            nodes: nodes.into_boxed_slice(),
        })
    }

    fn parse_version_pragma_rest(
        &mut self,
        slash_span: SourceSpan,
    ) -> Result<KdlVersionPragma, ParseError> {
        let kw = self.bump()?;
        let Token::Ident(_, kw_text) = &kw else {
            return Err(ParseError::at(
                ErrorCode::UnexpectedByte,
                kw.span(),
                "expected `kdl-version` after `/`",
            ));
        };
        if kw_text.as_ref() != "kdl-version" {
            return Err(ParseError::at(
                ErrorCode::UnexpectedByte,
                kw.span(),
                "expected `kdl-version` after `/`",
            ));
        }
        let v = self.bump()?;
        let Token::Str(s, value) = &v else {
            return Err(ParseError::at(
                ErrorCode::UnexpectedByte,
                v.span(),
                "expected string after `/kdl-version`",
            ));
        };
        if value.as_ref() != "2.0" {
            return Err(ParseError::at(
                ErrorCode::UnsupportedVersion,
                *s,
                "only KDL 2.0 is supported",
            ));
        }
        let end = self.last_end;
        Ok(KdlVersionPragma {
            span: self.lexer.span_range(slash_span.start.byte as usize, end),
            version: value.clone(),
        })
    }

    /// `node := type? name ("=" value)? entries? children-block?`
    fn parse_node(&mut self, depth: usize) -> Result<KdlNode, ParseError> {
        if depth >= elang_core::limits::MAX_KDL_NESTING_DEPTH {
            return Err(self.error_here(
                ErrorCode::NestingTooDeep,
                "node nesting exceeds MAX_KDL_NESTING_DEPTH",
            ));
        }

        // Node span start comes from the **peeked token's span**, not from
        // the lexer's byte position: `parse_document` and the children
        // loop both materialise one token of lookahead before handing off
        // to `parse_node`, so `self.lexer.pos()` would already be past the
        // node name and the span would start after it.
        let start = self.peek()?.span().start.byte as usize;
        let name_span = self.peek()?.span();
        self.budgets()
            .nodes_resource()
            .try_add(1)
            .map_err(|code| ParseError::at(code, name_span, "too many nodes in resource"))?;
        self.budgets()
            .nodes_archive()
            .try_add(1)
            .map_err(|code| ParseError::at(code, name_span, "too many nodes in archive"))?;

        // Optional type annotation `(type)`.
        //
        // KDL 2.0 accepts **either** a bare identifier *or* a string inside
        // the parens: `(u8)port 8080` and `("u8")port 8080` are both legal.
        // A bare identifier is *not* a `KdlValue` (KDL 2.0 reserves bare
        // identifiers for node/property names), so we must accept the token
        // directly rather than routing through `parse_value`, which would
        // reject it.
        let ty = if matches!(self.peek()?, Token::OpenParen(_)) {
            let _ = self.bump()?; // `(`
            let ty_token = self.bump()?;
            let close = self.bump()?;
            if !matches!(close, Token::CloseParen(_)) {
                return Err(ParseError::at(
                    ErrorCode::UnexpectedByte,
                    close.span(),
                    "expected `)` to close type annotation",
                ));
            }
            let (span, lexeme) = match &ty_token {
                Token::Ident(s, t) | Token::Str(s, t) | Token::RawStr(s, t) => (*s, t.clone()),
                other => {
                    return Err(ParseError::at(
                        ErrorCode::UnexpectedByte,
                        other.span(),
                        format!(
                            "type annotation must be an identifier or string, got {}",
                            other.kind()
                        ),
                    ));
                }
            };
            Some((span, lexeme))
        } else {
            None
        };

        // Name (bare ident or quoted string).
        let name = match self.bump()? {
            Token::Ident(s, t) => (s, t),
            Token::Str(s, t) | Token::RawStr(s, t) => {
                if t.len() > elang_core::limits::MAX_NAME_BYTES {
                    return Err(ParseError::at(
                        ErrorCode::NameTooLong,
                        s,
                        "node name exceeds MAX_NAME_BYTES",
                    ));
                }
                (s, t)
            }
            other => {
                return Err(ParseError::at(
                    ErrorCode::UnexpectedByte,
                    other.span(),
                    "expected node name",
                ));
            }
        };

        // Optional `= value` shorthand.
        let mut entries: Vec<KdlEntry> = Vec::new();
        let mut arg_count = 0;
        let mut prop_count = 0;
        if matches!(self.peek()?, Token::Equals(_)) {
            let eq = self.bump()?;
            let value = self.parse_value()?;
            // The `=value` shorthand is equivalent to a property whose key
            // equals the node name. We store it as a synthetic prop so the
            // AST is uniform.
            prop_count += 1;
            entries.push(KdlEntry::Prop {
                key_span: name.0,
                key: name.1.clone(),
                value,
            });
            let _ = eq;
        }

        // Discard exactly one entry after each slashdash. Children blocks
        // are handled below, since only blocks may follow a discarded block.
        loop {
            let discard = matches!(self.peek()?, Token::SlashDash(_));
            if discard {
                self.slashdash_target()?;
                if matches!(self.peek()?, Token::OpenBrace(_)) {
                    // Discard this block and enter the children-only phase.
                    let _ = self.bump()?;
                    let _ = self.parse_children(depth + 1)?;
                    break;
                }
            } else if matches!(
                self.peek()?,
                Token::Eof(_) | Token::Newline(_) | Token::OpenBrace(_) | Token::CloseBrace(_)
            ) {
                break;
            }
            let entry = self.parse_entry()?;
            let (count, max, code) = if entry.is_arg() {
                (
                    &mut arg_count,
                    elang_core::limits::MAX_ARGS_PER_NODE,
                    ErrorCode::TooManyArgs,
                )
            } else {
                (
                    &mut prop_count,
                    elang_core::limits::MAX_PROPS_PER_NODE,
                    ErrorCode::TooManyProps,
                )
            };
            if *count >= max {
                return Err(ParseError::at(
                    code,
                    entry.span(),
                    "too many entries in node",
                ));
            }
            *count += 1;
            if !discard {
                entries.push(entry);
            }
        }

        // At most one retained children block; slashdash blocks are parsed
        // for validation and budget accounting, then discarded.
        let mut children = None;
        loop {
            let discard = matches!(self.peek()?, Token::SlashDash(_));
            if discard {
                self.slashdash_target()?;
            }
            if matches!(self.peek()?, Token::OpenBrace(_)) && (discard || children.is_none()) {
                let _ = self.bump()?;
                let block = self.parse_children(depth + 1)?;
                if !discard {
                    children = Some(block);
                }
            } else if discard {
                return Err(self.error_here(
                    ErrorCode::UnexpectedByte,
                    "expected children block after slashdash",
                ));
            } else {
                break;
            }
        }

        let end = self.last_end;
        let span = self.lexer.span_range(start, end);
        if !matches!(
            self.peek()?,
            Token::Newline(_) | Token::CloseBrace(_) | Token::Eof(_)
        ) {
            return Err(ParseError::at(
                ErrorCode::UnexpectedByte,
                self.peek()?.span(),
                "expected node terminator",
            ));
        }

        Ok(KdlNode {
            span,
            ty,
            name,
            entries: entries.into_boxed_slice(),
            children,
        })
    }

    fn parse_children(&mut self, depth: usize) -> Result<KdlChildren, ParseError> {
        let mut nodes: Vec<KdlNode> = Vec::new();
        loop {
            // Eat one or more terminators.
            while matches!(self.peek()?, Token::Newline(_)) {
                let _ = self.bump()?;
            }
            match self.peek()? {
                Token::CloseBrace(_) => {
                    let _ = self.bump()?;
                    return Ok(KdlChildren::new(nodes.into_boxed_slice()));
                }
                Token::Eof(_) => {
                    return Err(self.error_here(
                        ErrorCode::UnexpectedEof,
                        "unexpected EOF inside children block",
                    ));
                }
                Token::SlashDash(_) => {
                    self.slashdash_target()?;
                    let _ = self.parse_node(depth)?;
                }
                _ => nodes.push(self.parse_node(depth)?),
            }
        }
    }

    fn parse_value(&mut self) -> Result<KdlValue, ParseError> {
        let tok = self.bump()?;
        self.value_from_token(&tok)
    }

    fn value_from_token(&mut self, tok: &Token) -> Result<KdlValue, ParseError> {
        // Quoted/raw strings were charged during lexing. Bare identifiers
        // become strings only in value positions, so charge them here.
        if let Token::Ident(span, text) = tok {
            let bytes = text.len() as u64;
            self.budgets()
                .decoded_string_bytes_resource()
                .try_add(bytes)
                .map_err(|code| {
                    ParseError::at(code, *span, "bare string exceeds resource string budget")
                })?;
            self.budgets()
                .decoded_string_bytes_archive()
                .try_add(bytes)
                .map_err(|code| {
                    ParseError::at(code, *span, "bare string exceeds archive string budget")
                })?;
        }
        Lexer::token_to_value(tok).ok_or_else(|| {
            ParseError::at(
                ErrorCode::UnexpectedByte,
                tok.span(),
                format!("expected value, got {}", tok.kind()),
            )
        })
    }

    fn parse_entry(&mut self) -> Result<KdlEntry, ParseError> {
        // Look for `name = value` first.
        let first = self.bump()?;
        match &first {
            Token::Ident(_, _) | Token::Str(_, _) | Token::RawStr(_, _) => {
                if matches!(self.peek()?, Token::Equals(_)) {
                    let key_span = first.span();
                    let key = match &first {
                        Token::Ident(_, t) | Token::Str(_, t) | Token::RawStr(_, t) => t.clone(),
                        _ => unreachable!(),
                    };
                    if key.len() > elang_core::limits::MAX_NAME_BYTES {
                        return Err(ParseError::at(
                            ErrorCode::NameTooLong,
                            key_span,
                            "property key exceeds MAX_NAME_BYTES",
                        ));
                    }
                    let _ = self.bump()?; // =
                    let value = self.parse_value()?;
                    return Ok(KdlEntry::Prop {
                        key_span,
                        key,
                        value,
                    });
                }
                self.value_from_token(&first).map(KdlEntry::Arg)
            }
            Token::Null(_) | Token::Bool(..) | Token::Integer(..) | Token::Float(..) => {
                self.value_from_token(&first).map(KdlEntry::Arg)
            }
            _ => Err(ParseError::at(
                ErrorCode::UnexpectedByte,
                first.span(),
                format!("unexpected {} in entries", first.kind()),
            )),
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "parser tests assert node/entry counts and error codes; a panic signals a parser regression"
)]
mod tests {
    use super::*;

    #[test]
    fn parses_empty_input() {
        let d = parse(b"").unwrap();
        assert!(d.nodes.is_empty());
        assert!(d.version.is_none());
    }

    #[test]
    fn parses_simple_node() {
        let d = parse(b"foo").unwrap();
        assert_eq!(d.nodes.len(), 1);
        assert_eq!(d.nodes[0].name(), "foo");
    }

    #[test]
    fn parses_node_with_string_arg() {
        let d = parse(br#"epf "0.1.0""#).unwrap();
        let n = &d.nodes[0];
        assert_eq!(n.name(), "epf");
        let args: Vec<_> = n.args().collect();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].as_str(), Some("0.1.0"));
    }

    #[test]
    fn parses_node_with_property() {
        let d = parse(br#"foo key="value""#).unwrap();
        let n = &d.nodes[0];
        let props: Vec<_> = n.props().collect();
        assert_eq!(props.len(), 1);
        assert_eq!(props[0].0, "key");
        assert_eq!(props[0].1.as_str(), Some("value"));
    }

    #[test]
    fn parses_children_block() {
        let d = parse(b"parent { child1; child2 }").unwrap();
        let n = &d.nodes[0];
        assert_eq!(n.name(), "parent");
        let c = n.children().expect("children");
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].name(), "child1");
        assert_eq!(c[1].name(), "child2");
    }

    #[test]
    fn parses_version_pragma() {
        let d = parse(b"/kdl-version \"2.0\"\nfoo").unwrap();
        assert!(d.version.is_some());
        assert_eq!(d.version().unwrap().version.as_ref(), "2.0");
        assert_eq!(d.nodes.len(), 1);
    }

    #[test]
    fn rejects_non_2_0_version() {
        let err = parse(b"/kdl-version \"1.0\"\nfoo").unwrap_err();
        assert_eq!(err.code, ErrorCode::UnsupportedVersion);
    }

    #[test]
    fn rejects_invalid_utf8() {
        let bad = [0xFF, 0xFE, b'f', b'o', b'o'];
        let err = parse(&bad).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidUtf8);
    }

    #[test]
    fn enforces_nesting_depth() {
        let mut src = String::new();
        for _ in 0..65 {
            src.push_str("a { ");
        }
        src.push('a');
        for _ in 0..65 {
            src.push_str(" }");
        }
        let err = parse(src.as_bytes()).unwrap_err();
        assert_eq!(err.code, ErrorCode::NestingTooDeep);
    }

    #[test]
    fn enforces_max_args_per_node() {
        let mut src = String::from("foo");
        for i in 0..300 {
            src.push(' ');
            src.push_str(&format!("\"arg{i}\""));
        }
        let err = parse(src.as_bytes()).unwrap_err();
        assert_eq!(err.code, ErrorCode::TooManyArgs);
    }

    #[test]
    fn parses_full_epf_example() {
        let src = br#"/kdl-version "2.0"

epf "0.1.0"
eld "0.1.0"
language-version "1.0.0"

metadata {
    identifier "urn:elang:language:rocky:aelith:1.0.0"
    name "Aelith"
    description "A small constructed language."
}

manifest {
    phonemes "Linguistics/phonemes.eld"
}
"#;
        let d = parse(src).unwrap();
        assert_eq!(d.nodes.len(), 5);
        let metadata = d.nodes.iter().find(|n| n.name() == "metadata").unwrap();
        let m_children = metadata.children().unwrap();
        assert_eq!(m_children.len(), 3);
    }
}
