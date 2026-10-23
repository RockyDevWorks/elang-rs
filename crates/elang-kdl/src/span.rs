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

    /// Construct a position from a raw byte offset, computing the line and
    /// column by scanning the supplied input. Cost is O(line) — fine for
    /// normal diagnostics, since callers always pass the start of a node or
    /// entry, never the middle of a token.
    ///
    /// Returns `Position::INVALID` if `byte` exceeds the input length.
    pub fn from_byte(input: &[u8], byte: u32) -> Position {
        let idx = byte as usize;
        let up_to = match input.get(..idx) {
            Some(s) => s,
            None => return Position::INVALID,
        };
        // Count `\n` to get the 1-based line number; column is the number of
        // bytes since the last newline + 1.
        let mut line: u32 = 1;
        let mut last_nl: Option<usize> = None;
        for (i, &b) in up_to.iter().enumerate() {
            if b == b'\n' {
                line = line.saturating_add(1);
                last_nl = Some(i);
            }
        }
        let column: u32 = match last_nl {
            // `i` is the byte index of the most recent `\n`. Column is the
            // 1-based count of bytes after it, so `byte - i`.
            Some(i) => match u32::try_from(i) {
                Ok(nl_pos) => byte.saturating_sub(nl_pos),
                Err(_) => 0,
            },
            None => byte.saturating_add(1),
        };
        Position { byte, line, column }
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
}
