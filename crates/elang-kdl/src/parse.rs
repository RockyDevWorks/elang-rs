//! KDL 2.0 recursive-descent parser (T0501–T0512).
//!
//! Consumes a token stream from [`crate::lex::Lexer`] and produces a
//! [`KdlDocument`]. The parser enforces every steering-§10 budget via
//! the shared [`BudgetTracker`] and refuses to recurse past
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
/// Validates UTF-8 first; then runs the lexer + parser with the shared
/// budget tracker. Resets the budget tracker on entry, so the same
/// tracker can be reused for sequential resources.
pub fn parse(input: &[u8]) -> Result<KdlDocument, ParseError> {
    let _ = validate_utf8(input)?;
    let mut budgets = BudgetTracker::new();
    parse_with_budgets(input, &mut budgets)
}

/// Parse `input` with a caller-supplied budget tracker. The tracker is
/// **not** reset on entry; callers that parse multiple resources should
/// invoke [`BudgetTracker::reset`] between them.
pub fn parse_with_budgets(
    input: &[u8],
    budgets: &mut BudgetTracker,
) -> Result<KdlDocument, ParseError> {
    let mut parser = Parser::new(input, budgets);
    parser.parse_document()
}

/// A pre-validated KDL 2.0 parser.
struct Parser<'a> {
    input: &'a [u8],
    lexer: Lexer<'a>,
    /// Two-element lookahead buffer. `peek[0]` is the next token the parser
    /// would consume; `peek[1]` is the one after that. Used by `parse_node`
    /// to disambiguate property keys from sibling-node names.
    peek: [Option<Token>; 2],
    /// Whether the document still has to consume a node terminator before
    /// the next node begins.
    pending_newline: bool,
}

impl<'a> Parser<'a> {
    fn new(input: &'a [u8], budgets: &'a mut BudgetTracker) -> Parser<'a> {
        Parser {
            input,
            lexer: Lexer::new(input, budgets),
            peek: [None, None],
            pending_newline: false,
        }
    }

    fn budgets(&mut self) -> &mut BudgetTracker {
        self.lexer.budgets()
    }

    /// Fill `peek[0]` if it is empty, materialising a fresh token from the
    /// lexer.
    fn fill_peek(&mut self) -> Result<(), ParseError> {
        if self.peek[0].is_none() {
            self.peek[0] = Some(self.lexer.next_token()?);
        }
        Ok(())
    }

    /// Fill `peek[1]` if it is empty.
    fn fill_peek2(&mut self) -> Result<(), ParseError> {
        if self.peek[1].is_none() {
            self.peek[1] = Some(self.lexer.next_token()?);
        }
        Ok(())
    }

    /// Consume the next token. Slides `peek[1]` into `peek[0]`, so any
    /// previously-materialised 2-token lookahead is preserved. The
    /// `.unwrap()` is provably safe: `fill_peek` ran first and only
    /// returns `Ok(())` when `peek[0]` is `Some`.
    #[allow(
        clippy::unwrap_used,
        reason = "`bump` is only called after `fill_peek` returns Ok, which guarantees `peek[0]` is Some"
    )]
    fn bump(&mut self) -> Result<Token, ParseError> {
        self.fill_peek()?;
        let t = self.peek[0].take().unwrap();
        self.peek[0] = self.peek[1].take();
        Ok(t)
    }

    #[allow(
        clippy::unwrap_used,
        reason = "`peek` is only called after `fill_peek` returns Ok, which guarantees `peek[0]` is Some"
    )]
    fn peek(&mut self) -> Result<&Token, ParseError> {
        self.fill_peek()?;
        Ok(self.peek[0].as_ref().unwrap())
    }

    /// Two-token lookahead. Returns the token after the next one.
    /// Used by `parse_node` to disambiguate a property key
    /// (`Ident = value`) from the start of a sibling node
    /// (`Ident` followed by anything else).
    #[allow(
        clippy::unwrap_used,
        reason = "`peek_token_after_peek` is only called after `fill_peek`+`fill_peek2` return Ok, which guarantees `peek[1]` is Some"
    )]
    fn peek_token_after_peek(&mut self) -> Result<&Token, ParseError> {
        self.fill_peek()?;
        self.fill_peek2()?;
        Ok(self.peek[1].as_ref().unwrap())
    }

    /// Consume the `/-` token and everything up to (but not past) the
    /// next `\n`. Implements KDL 2.0 slashdash line comments.
    fn skip_slashdash_to_eol(&mut self) -> Result<(), ParseError> {
        // Drop the SlashDash token.
        let _ = self.bump()?;
        self.lexer.skip_to_eol();
        Ok(())
    }

    fn error_here(&self, code: ErrorCode, message: &str) -> ParseError {
        let pos = self.lexer.pos();
        ParseError::at(
            code,
            SourceSpan::from_range(self.input, pos as u32, pos as u32),
            message,
        )
    }

    /// `document := version-pragma? node*`
    fn parse_document(&mut self) -> Result<KdlDocument, ParseError> {
        let start = 0u32;
        let mut version: Option<KdlVersionPragma> = None;

        // KDL 2.0 requires the version pragma; we enforce this.
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
                let end = self.lexer.pos() as u32;
                return Ok(KdlDocument {
                    span: SourceSpan::from_range(self.input, start, end),
                    version,
                    nodes: Box::new([]),
                });
            }
            other => {
                // No pragma; push the token back so `parse_node` can pick
                // it up. We model "pushback" by stashing the token.
                self.peek[0] = Some(other.clone());
            }
        }

        let mut nodes: Vec<KdlNode> = Vec::new();
        loop {
            // Skip an optional terminator between nodes.
            if self.pending_newline {
                self.pending_newline = false;
                if matches!(self.peek()?, Token::Newline(_)) {
                    let _ = self.bump()?;
                }
            }
            match self.peek()? {
                Token::Eof(_) | Token::CloseBrace(_) => break,
                Token::Newline(_) => {
                    // Stray newlines are tolerated.
                    let _ = self.bump()?;
                }
                Token::SlashDash(_) => {
                    // KDL 2.0 slashdash comment: consume `/-` plus the rest
                    // of the current line so the next iteration sees the
                    // following node or newline.
                    self.skip_slashdash_to_eol()?;
                }
                _ => {
                    let n = self.parse_node(0)?;
                    self.budgets().nodes_resource().try_add(1).map_err(|code| {
                        ParseError::at(code, n.span, "too many nodes in resource")
                    })?;
                    // Archive-wide node ceiling (T0507). The container layer
                    // drives this across sequential resources via
                    // `BudgetTracker::reset` (per-resource) without resetting
                    // `nodes_archive`, so the aggregate is enforced.
                    self.budgets().nodes_archive().try_add(1).map_err(|code| {
                        ParseError::at(code, n.span, "too many nodes in archive")
                    })?;
                    nodes.push(n);
                    self.pending_newline = true;
                }
            }
        }

        let end = self.lexer.pos() as u32;
        Ok(KdlDocument {
            span: SourceSpan::from_range(self.input, start, end),
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
        let end = self.lexer.pos() as u32;
        Ok(KdlVersionPragma {
            span: SourceSpan::from_range(self.input, slash_span.start.byte, end),
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

        let start = self.lexer.pos();

        // Optional type annotation `(type)`.
        let ty = if matches!(self.peek()?, Token::OpenParen(_)) {
            let _ = self.bump()?;
            let ty_value = self.parse_value()?;
            let close = self.bump()?;
            if !matches!(close, Token::CloseParen(_)) {
                return Err(ParseError::at(
                    ErrorCode::UnexpectedByte,
                    close.span(),
                    "expected `)` to close type annotation",
                ));
            }
            let (span, lexeme) = match ty_value {
                KdlValue::String { span, value, .. } => (span, value),
                _ => {
                    return Err(ParseError::at(
                        ErrorCode::InvalidNumber,
                        ty_value.span(),
                        "type annotation must be a string",
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
            Token::Str(s, t) => (s, t),
            Token::RawStr(s, t) => (s, t),
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
        if matches!(self.peek()?, Token::Equals(_)) {
            let eq = self.bump()?;
            let value = self.parse_value()?;
            // The `=value` shorthand is equivalent to a property whose key
            // equals the node name. We store it as a synthetic prop so the
            // AST is uniform.
            entries.push(KdlEntry::Prop {
                key_span: name.0,
                key: name.1.clone(),
                value,
            });
            let _ = eq;
        }

        // Entries (arg | prop), terminated by newline / `;` / `{` / EOF / `}`.
        // Lookahead: an Ident/Str/RawStr that is NOT followed by `=` starts a
        // new sibling node, so we stop here.
        loop {
            match self.peek()? {
                Token::Eof(_) | Token::Newline(_) | Token::OpenBrace(_) | Token::CloseBrace(_) => {
                    break
                }
                Token::SlashDash(_) | Token::Slash(_) => break,
                _ => {}
            }
            // If next token is a bare Ident and the one after it is not `=`,
            // this is a sibling node — stop here. Bare identifiers are
            // never values in KDL 2.0 (only `Str` / `RawStr` / numeric
            // / bool / null are).
            if matches!(self.peek()?, Token::Ident(_, _))
                && !matches!(self.peek_token_after_peek()?, Token::Equals(_))
            {
                break;
            }
            let entry = self.parse_entry()?;
            entries.push(entry);
        }

        // Children block.
        let children = if matches!(self.peek()?, Token::OpenBrace(_)) {
            let _ = self.bump()?;
            Some(self.parse_children(depth + 1)?)
        } else {
            None
        };

        let end = self.lexer.pos();
        let span = SourceSpan::from_range(self.input, start as u32, end as u32);
        // Enforce per-node arg/prop counts *before* constructing the node
        // so the error carries the offending spans.
        let arg_count = entries.iter().filter(|e| e.is_arg()).count();
        let prop_count = entries.iter().filter(|e| e.is_prop()).count();
        if arg_count > elang_core::limits::MAX_ARGS_PER_NODE {
            return Err(ParseError::at(
                ErrorCode::TooManyArgs,
                span,
                format!(
                    "node has {} args, max {}",
                    arg_count,
                    elang_core::limits::MAX_ARGS_PER_NODE
                ),
            ));
        }
        if prop_count > elang_core::limits::MAX_PROPS_PER_NODE {
            return Err(ParseError::at(
                ErrorCode::TooManyProps,
                span,
                format!(
                    "node has {} props, max {}",
                    prop_count,
                    elang_core::limits::MAX_PROPS_PER_NODE
                ),
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
                    // KDL 2.0 slashdash line comment inside a children
                    // block: consume `/-` plus the rest of the line.
                    self.skip_slashdash_to_eol()?;
                }
                _ => {
                    let n = self.parse_node(depth)?;
                    self.budgets().nodes_resource().try_add(1).map_err(|code| {
                        ParseError::at(code, n.span, "too many nodes in resource")
                    })?;
                    self.budgets().nodes_archive().try_add(1).map_err(|code| {
                        ParseError::at(code, n.span, "too many nodes in archive")
                    })?;
                    nodes.push(n);
                }
            }
        }
    }

    fn parse_value(&mut self) -> Result<KdlValue, ParseError> {
        let tok = self.bump()?;
        Lexer::token_to_value(&tok).ok_or_else(|| {
            ParseError::at(
                ErrorCode::UnexpectedByte,
                tok.span(),
                format!("expected value, got {}", tok.kind()),
            )
        })
    }

    fn parse_entry(&mut self) -> Result<KdlEntry, ParseError> {
        // Look for `name = value` first.
        let saved_pos = self.lexer.pos();
        let first = self.bump()?;
        match &first {
            Token::Ident(_, _) | Token::Str(_, _) | Token::RawStr(_, _) => {
                if matches!(self.peek()?, Token::Equals(_)) {
                    let key_span = first.span();
                    let key = match &first {
                        Token::Ident(_, t) | Token::Str(_, t) | Token::RawStr(_, t) => t.clone(),
                        _ => unreachable!(),
                    };
                    let _ = self.bump()?; // =
                    let value = self.parse_value()?;
                    return Ok(KdlEntry::Prop {
                        key_span,
                        key,
                        value,
                    });
                }
                // Otherwise the consumed token is the value.
                if let Some(v) = Lexer::token_to_value(&first) {
                    // Charge the args-per-node budget on a *successful*
                    // value parse — entries counted here are positional
                    // args; properties are charged elsewhere by the
                    // property-count check in `parse_node`.
                    self.budgets().args_per_node().try_add(1).map_err(|code| {
                        ParseError::at(code, first.span(), "too many args in node")
                    })?;
                    return Ok(KdlEntry::Arg(v));
                }
                // Fall through to error.
                let _ = saved_pos;
                Err(ParseError::at(
                    ErrorCode::UnexpectedByte,
                    first.span(),
                    format!("unexpected {} in entries", first.kind()),
                ))
            }
            Token::Null(_) | Token::Bool(..) | Token::Integer(..) | Token::Float(..) => {
                if let Some(v) = Lexer::token_to_value(&first) {
                    self.budgets().args_per_node().try_add(1).map_err(|code| {
                        ParseError::at(code, first.span(), "too many args in node")
                    })?;
                    return Ok(KdlEntry::Arg(v));
                }
                let _ = saved_pos;
                Err(ParseError::at(
                    ErrorCode::UnexpectedByte,
                    first.span(),
                    format!("unexpected {} in entries", first.kind()),
                ))
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
        let d = parse(b"parent { child1 child2 }").unwrap();
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
