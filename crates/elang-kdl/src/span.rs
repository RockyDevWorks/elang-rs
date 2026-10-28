//! Source position tracking for KDL diagnostics (T0502).
//!
//! `Position` is a byte offset into the input plus the line/column it lies on
//! (1-indexed for both, matching what users see in editors). `SourceSpan` is a
//! half-open `[start, end)` byte-offset pair; the start position is always
//! inclusive and the end position is always exclusive.
//!
//! All public positions in this crate are byte offsets, not character offsets.
//! That choice makes the parser O(1) per byte and avoids a Unicode-aware
//! indexing dependency. Diagnostics that need to display characters convert
//! to chars before slicing.
//!
//! The `INVALID` position is the only sentinel value; it is produced by
//! constructors when the underlying input slice is too short to contain a
//! span, or when a synthetic AST node is created that has no real source.

use core::fmt;

use elang_core::error::KdlSpan;

/// Convert a [`SourceSpan`] into the workspace's [`KdlSpan`] type.
///
/// Returns `None` for an invalid span (e.g. synthetic AST node with no real
/// source position), so the caller can build the diagnostic without a span
/// instead of constructing one from a sentinel.
///
/// This is the single canonical implementation of the conversion; all
/// diagnostic helpers that need to project a KDL source span into a
/// workspace `KdlSpan` call through this function (was duplicated across
/// `elang-eld`, `elang-epf`, and `elang-integrity` error modules before
/// issue #77 consolidated them here).
pub fn to_kdl_span(span: SourceSpan) -> Option<KdlSpan> {
    if span.is_invalid() {
        return None;
    }
    Some(KdlSpan {
        line: span.start.line,
        column: span.start.column,
        length: span.len(),
    })
}

/// Byte offsets immediately after each KDL newline, preceded by zero.
/// CRLF is one newline; all other KDL newline scalars are recognized too.
pub(crate) fn line_starts(input: &[u8]) -> impl Iterator<Item = usize> + '_ {
    let mut pos = 0usize;
    core::iter::once(0).chain(core::iter::from_fn(move || {
        while let Some(rest) = input.get(pos..) {
            let width = match rest {
                [b'\r', b'\n', ..] => 2,
                [b'\r' | b'\n' | 0x0b | 0x0c, ..] => 1,
                [0xc2, 0x85, ..] => 2,
                [0xe2, 0x80, 0xa8 | 0xa9, ..] => 3,
                [] => return None,
                _ => 0,
            };
            pos = pos.checked_add(width.max(1))?;
            if width != 0 {
                return Some(pos);
            }
        }
        None
    }))
}

/// A 1-based line number (`1..=u32::MAX`) and 1-based column number, where
/// column counts UTF-8 bytes (not characters) to stay O(1) per byte.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Position {
    /// Zero-based byte offset into the input.
    pub byte: u32,
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number, measured in UTF-8 bytes from the start of `line`.
    pub column: u32,
}

impl Position {
    /// A sentinel position that never appears in any real source. It is the
    /// only position whose `byte` field is `u32::MAX`.
    pub const INVALID: Position = Position {
        byte: u32::MAX,
        line: 0,
        column: 0,
    };

    /// Construct a position from a raw byte offset, scanning the supplied
    /// input for KDL newlines. Cost is O(input length).
    ///
    /// Returns `Position::INVALID` if `byte` exceeds the input length.
    pub fn from_byte(input: &[u8], byte: u32) -> Position {
        let idx = byte as usize;
        if idx > input.len() {
            return Position::INVALID;
        }
        let mut line = 0u32;
        let mut line_start = 0u32;
        for offset in line_starts(input).take_while(|offset| *offset <= idx) {
            line = line.saturating_add(1);
            line_start = u32::try_from(offset).unwrap_or(u32::MAX);
        }
        Position {
            byte,
            line,
            column: byte.saturating_sub(line_start).saturating_add(1),
        }
    }

    /// True if this is the sentinel `INVALID` position.
    pub fn is_invalid(self) -> bool {
        self.byte == u32::MAX
    }
}

impl fmt::Debug for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_invalid() {
            f.write_str("Position(<invalid>)")
        } else {
            write!(f, "Position({}, {}, {})", self.byte, self.line, self.column)
        }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_invalid() {
            f.write_str("<invalid>")
        } else {
            write!(f, "{}:{}", self.line, self.column)
        }
    }
}

/// A half-open `[start, end)` byte range into the parser input.
///
/// `start` is the byte offset of the first byte of the construct; `end` is
/// the byte offset **one past** the last byte of the construct. This matches
/// Rust's standard slice semantics.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct SourceSpan {
    /// Inclusive start.
    pub start: Position,
    /// Exclusive end.
    pub end: Position,
}

impl SourceSpan {
    /// Construct a span from a half-open byte range. Both endpoints are
    /// resolved against `input`. If `start > end`, the endpoints are
    /// swapped. If either endpoint exceeds the input length, both become
    /// `Position::INVALID`.
    pub fn from_range(input: &[u8], start_byte: u32, end_byte: u32) -> SourceSpan {
        let mut s = Position::from_byte(input, start_byte);
        let mut e = Position::from_byte(input, end_byte);
        if s.byte > e.byte {
            core::mem::swap(&mut s, &mut e);
        }
        SourceSpan { start: s, end: e }
    }

    /// A span with both endpoints at `Position::INVALID`. Used for synthetic
    /// AST nodes produced by the canonical emitter or by fall-back paths
    /// where no source text exists.
    pub const INVALID: SourceSpan = SourceSpan {
        start: Position::INVALID,
        end: Position::INVALID,
    };

    /// True if either endpoint is the sentinel.
    pub fn is_invalid(self) -> bool {
        self.start.is_invalid() || self.end.is_invalid()
    }

    /// Length of this span in bytes (`end.byte - start.byte`), or 0 if
    /// invalid.
    pub fn len(self) -> u32 {
        if self.is_invalid() {
            0
        } else {
            self.end.byte.saturating_sub(self.start.byte)
        }
    }

    /// True if the span has zero length.
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// Slice the underlying input that this span covers. Returns `None`
    /// when either endpoint is invalid or the slice would not be in
    /// bounds.
    pub fn slice(self, input: &[u8]) -> Option<&[u8]> {
        if self.is_invalid() {
            return None;
        }
        let s = self.start.byte as usize;
        let e = self.end.byte as usize;
        input.get(s..e)
    }
}

impl fmt::Debug for SourceSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SourceSpan({}..{})", self.start, self.end)
    }
}

impl fmt::Display for SourceSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_at_start() {
        let p = Position::from_byte(b"hello", 0);
        assert_eq!(p.byte, 0);
        assert_eq!(p.line, 1);
        assert_eq!(p.column, 1);
    }

    #[test]
    fn position_after_newline() {
        let p = Position::from_byte(b"abc\ndef", 5);
        assert_eq!(p.byte, 5);
        assert_eq!(p.line, 2);
        assert_eq!(p.column, 2);
    }

    #[test]
    fn position_multi_line() {
        // 'c' on line 3 starts at byte 4, column 1 (1-based).
        let p = Position::from_byte(b"a\nb\nc", 4);
        assert_eq!(p.line, 3);
        assert_eq!(p.column, 1);
    }

    #[test]
    fn position_out_of_bounds_is_invalid() {
        let p = Position::from_byte(b"abc", 99);
        assert!(p.is_invalid());
    }

    #[test]
    fn position_at_end_is_valid() {
        let p = Position::from_byte(b"abc", 3);
        assert_eq!(p.byte, 3);
        assert_eq!(p.line, 1);
        assert_eq!(p.column, 4);
    }

    #[test]
    fn span_from_range_swaps_reversed() {
        let s = SourceSpan::from_range(b"abcdef", 4, 2);
        assert_eq!(s.start.byte, 2);
        assert_eq!(s.end.byte, 4);
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn span_slice_round_trip() {
        let input = b"hello world";
        let s = SourceSpan::from_range(input, 6, 11);
        assert_eq!(s.slice(input), Some(&b"world"[..]));
    }

    #[test]
    fn span_invalid_slice_returns_none() {
        let s = SourceSpan::INVALID;
        assert_eq!(s.slice(b"abc"), None);
    }

    #[test]
    fn to_kdl_span_returns_none_for_invalid_span() {
        assert_eq!(to_kdl_span(SourceSpan::INVALID), None);
    }

    #[test]
    fn to_kdl_span_preserves_line_column_and_length() {
        // 'K' on line 2 starts at byte 6, column 1 (1-based). When
        // `to_kdl_span` returns `None`, the sentinel (line=0) makes the
        // first assertion fail loudly so the test cannot silently pass.
        let input = b"first\nKDOC {\n";
        let s = SourceSpan::from_range(input, 6, 11);
        let k = to_kdl_span(s).unwrap_or(KdlSpan {
            line: 0,
            column: 0,
            length: 0,
        });
        assert_eq!(k.line, 2);
        assert_eq!(k.column, 1);
        assert_eq!(k.length, 5);
    }
}
