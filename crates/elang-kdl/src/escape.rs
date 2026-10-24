//! KDL 2.0 escape sequence processing (T0514, T0513).
//!
//! Two related operations:
//!
//! - [`decode_string`] parses the **content** of a `"…"` literal, processing
//!   KDL 2.0 escapes and writing the resulting UTF-8 bytes into a
//!   destination buffer. Returns the number of bytes written and the byte
//!   length of the encoded form (i.e. the source span's length).
//!
//! - [`encode_string`] emits a canonical double-quoted string for any
//!   `&str`. This is what T0513 calls the "canonical standard
//!   double-quoted KDL string emitter". The escape set is the **minimal**
//!   set defined by ADR 0009 E-3 plus the standard C0/C1 control
//!   escapes:
//!
//!     | Source byte | Canonical escape |
//!     | --- | --- |
//!     | `"` (0x22) | `\"` |
//!     | `\` (0x5C) | `\\` |
//!     | `\b` (0x08) | `\b` |
//!     | `\f` (0x0C) | `\f` |
//!     | `\n` (0x0A) | `\n` |
//!     | `\r` (0x0D) | `\r` |
//!     | `\t` (0x09) | `\t` |
//!     | `\0` (0x00) | `\u{0}` |
//!     | other C0 / C1 (0x01..=0x1F, 0x7F..=0x9F) | `\u{…}` |
//!
//!   Printable Unicode (>= 0x20 and < 0x7F, or any non-control Unicode
//!   scalar value) is emitted as-is. The two-byte sequence `\u{…}` is
//!   always used in lowercase hex with no leading zeros, matching
//!   ADR 0009 E-3.
//!
//! The two functions are kept separate so the parser does not have to
//! walk the canonical emitter (which always expands escapes) for every
//! decoded string — only the encoded form (decode) needs to walk the
//! source bytes.

// The decoder walks the source byte-by-byte after explicit bounds checks;
// the arithmetic is on values that are statically known to be in range.
// This is the same pattern used in elang-container's frame reader.
//
// `decode_string` takes `u32` byte offsets (cap at 4 GiB), but the
// caller passes `usize` positions; the casts are safe because the
// caller enforces the per-token / per-resource budget which caps every
// offset well below `u32::MAX`.
#![allow(
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation
)]

use crate::error::{ErrorCode, ParseError};
use crate::span::SourceSpan;
use elang_core::limits::MAX_DECODED_STRING_BYTES;

/// Decode the content of a `"…"` literal (without the surrounding quotes).
///
/// - `input`: the full source slice the parser is operating on.
/// - `content_start`: byte offset of the first byte **after** the opening
///   quote.
/// - `content_end`: byte offset of the closing quote (or `content_start`
///   for an empty string).
/// - `encoded_form_end`: byte offset **one past** the closing quote.
///   `encoded_form_end - content_start + 1` is the encoded length used to
///   check the `MAX_ENCODED_TOKEN_BYTES` budget.
/// - `out`: destination buffer. Caller is responsible for ensuring `out`
///   has at least `content_end - content_start` bytes available.
/// - `decoded_len`: total decoded bytes emitted, including any UTF-8
///   expansion of `\u{…}` escapes. Caller must enforce the
///   `MAX_DECODED_STRING_BYTES` budget after this returns.
///
/// Returns `Ok(decoded_len)` on success and
/// `Err(ParseError)` with a precise span on failure.
pub fn decode_string(
    input: &[u8],
    content_start: u32,
    content_end: u32,
    encoded_form_end: u32,
    out: &mut [u8],
) -> Result<u32, ParseError> {
    let start = content_start as usize;
    let end = content_end as usize;
    if start > end || end > input.len() {
        return Err(ParseError::new(
            ErrorCode::UnterminatedString,
            "string bounds out of range",
        ));
    }
    let body = &input[start..end];
    let body_text = core::str::from_utf8(body).map_err(|_| {
        ParseError::at(
            ErrorCode::InvalidUtf8,
            SourceSpan::from_range(input, content_start, content_end),
            "string is not valid UTF-8",
        )
    })?;
    if out.len() < body.len() {
        // Caller asked for a too-small buffer; this is a programming error.
        return Err(ParseError::new(
            ErrorCode::StringTooLong,
            "internal: decode buffer too small",
        ));
    }

    let mut w = 0usize;
    let mut r = 0usize;
    while r < body.len() {
        let b = body[r];
        if b != b'\\' {
            out[w] = b;
            w += 1;
            r += 1;
            continue;
        }
        // Escape: at minimum we need `\` + 1 byte.
        if r + 1 >= body.len() {
            return Err(ParseError::at(
                ErrorCode::UnterminatedString,
                SourceSpan::from_range(input, content_start, encoded_form_end),
                "trailing backslash",
            ));
        }
        let esc = body[r + 1];
        match esc {
            b'"' => {
                out[w] = b'"';
                w += 1;
                r += 2;
            }
            b'\\' => {
                out[w] = b'\\';
                w += 1;
                r += 2;
            }
            b'b' => {
                out[w] = 0x08;
                w += 1;
                r += 2;
            }
            b'f' => {
                out[w] = 0x0C;
                w += 1;
                r += 2;
            }
            b'n' => {
                out[w] = 0x0A;
                w += 1;
                r += 2;
            }
            b'r' => {
                out[w] = 0x0D;
                w += 1;
                r += 2;
            }
            b't' => {
                out[w] = 0x09;
                w += 1;
                r += 2;
            }
            b's' => {
                out[w] = b' ';
                w += 1;
                r += 2;
            }
            b'u' => {
                // \u{XXXX} — consume the braces + 1..=6 hex digits.
                if r + 2 >= body.len() || body[r + 2] != b'{' {
                    return Err(ParseError::at(
                        ErrorCode::InvalidEscape,
                        SourceSpan::from_range(
                            input,
                            content_start + r as u32,
                            content_start + r as u32 + 2,
                        ),
                        "\\u must be followed by `{`",
                    ));
                }
                // Find the matching `}`.
                let hex_start = r + 3;
                let mut hex_end = hex_start;
                let mut found_brace = false;
                while hex_end < body.len() {
                    if body[hex_end] == b'}' {
                        found_brace = true;
                        break;
                    }
                    hex_end += 1;
                }
                if !found_brace {
                    return Err(ParseError::at(
                        ErrorCode::InvalidUnicodeEscape,
                        SourceSpan::from_range(
                            input,
                            content_start + hex_start as u32 - 3,
                            content_end,
                        ),
                        "missing `}` in \\u{...}",
                    ));
                }
                let hex = &body[hex_start..hex_end];
                if hex.is_empty() || hex.len() > 6 {
                    return Err(ParseError::at(
                        ErrorCode::InvalidUnicodeEscape,
                        SourceSpan::from_range(
                            input,
                            content_start + hex_start as u32 - 3,
                            content_start + hex_end as u32 + 1,
                        ),
                        "\\u{...} requires 1..=6 hex digits",
                    ));
                }
                let mut code: u32 = 0;
                for &hb in hex {
                    let d = match hb {
                        b'0'..=b'9' => hb - b'0',
                        b'a'..=b'f' => hb - b'a' + 10,
                        b'A'..=b'F' => hb - b'A' + 10,
                        _ => {
                            return Err(ParseError::at(
                                ErrorCode::InvalidUnicodeEscape,
                                SourceSpan::from_range(
                                    input,
                                    content_start + hex_start as u32,
                                    content_start + hex_end as u32,
                                ),
                                "non-hex digit in \\u{...}",
                            ));
                        }
                    };
                    code = code * 16 + d as u32;
                }
                if code > 0x10FFFF {
                    return Err(ParseError::at(
                        ErrorCode::InvalidUnicodeEscape,
                        SourceSpan::from_range(
                            input,
                            content_start + hex_start as u32 - 3,
                            content_start + hex_end as u32 + 1,
                        ),
                        "code point out of Unicode range",
                    ));
                }
                let ch = char::from_u32(code).ok_or_else(|| {
                    ParseError::at(
                        ErrorCode::InvalidUnicodeEscape,
                        SourceSpan::from_range(
                            input,
                            content_start + hex_start as u32 - 3,
                            content_start + hex_end as u32 + 1,
                        ),
                        "surrogate code point",
                    )
                })?;
                let enc = ch.encode_utf8(&mut out[w..]);
                w += enc.len();
                r = hex_end + 1;
            }
            _ if body_text
                .get(r + 1..)
                .and_then(|s| s.chars().next())
                .is_some_and(char::is_whitespace) =>
            {
                r += 1; // backslash
                for c in body_text[r..].chars().take_while(|c| c.is_whitespace()) {
                    r += c.len_utf8();
                }
            }
            _ => {
                return Err(ParseError::at(
                    ErrorCode::InvalidEscape,
                    SourceSpan::from_range(
                        input,
                        content_start + r as u32,
                        content_start + r as u32 + 2,
                    ),
                    "unknown escape sequence",
                ));
            }
        }
    }

    if w as u64 > MAX_DECODED_STRING_BYTES as u64 {
        return Err(ParseError::at(
            ErrorCode::StringTooLong,
            SourceSpan::from_range(input, content_start, encoded_form_end),
            "decoded string exceeds MAX_DECODED_STRING_BYTES",
        ));
    }

    Ok(w as u32)
}

/// Emit a canonical double-quoted string for the given input bytes.
///
/// This is T0513. The emitter is **deterministic**: the only escape form is
/// the table above; `\u{…}` is always lowercase hex with no leading zeros;
/// printable ASCII is always emitted as itself. The result is the same
/// regardless of which platform or compiler builds the binary.
pub fn encode_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    // Iterate over `char`, not `u8`: bytes in 0x80..=0x9F are UTF-8
    // *continuation* bytes, not C1 control characters, and escaping them
    // would corrupt every non-ASCII string.
    for b in s.chars() {
        match b {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) <= 0x1F => {
                use core::fmt::Write;
                let _ = write!(&mut out, "\\u{{{:x}}}", c as u32);
            }
            c if (0x7F..=0x9F).contains(&(c as u32)) => {
                use core::fmt::Write;
                let _ = write!(&mut out, "\\u{{{:x}}}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "escape tests assert specific decoded bytes and error codes; indices are compile-time constants and panics are test-failure signals"
)]
mod tests {
    use super::*;

    #[test]
    fn decodes_plain_string() {
        let input = b"\"hello\"";
        let mut out = [0u8; 16];
        let n = decode_string(input, 1, 6, 7, &mut out).unwrap();
        assert_eq!(n, 5);
        assert_eq!(&out[..n as usize], b"hello");
    }

    #[test]
    fn decodes_basic_escapes() {
        let input = b"\"a\\\"b\\\\c\\nc\"";
        let mut out = [0u8; 16];
        let n = decode_string(input, 1, 11, 12, &mut out).unwrap();
        assert_eq!(&out[..n as usize], b"a\"b\\c\nc");
    }

    #[test]
    fn decodes_unicode_escape() {
        let input = b"\"\\u{1F600}\"";
        let mut out = [0u8; 16];
        // Bytes: `"` `\` `u` `{` `1` `F` `6` `0` `0` `}` `"`
        //          0    1    2    3    4    5    6    7    8    9   10
        // content starts at byte 1, content ends at byte 10 (inclusive
        // of `}`), encoded form ends at byte 11 (past closing `"`).
        let n = decode_string(input, 1, 10, 11, &mut out).unwrap();
        assert_eq!(&out[..n as usize], "\u{1F600}".as_bytes());
    }

    #[test]
    fn rejects_truncated_unicode_escape() {
        let input = b"\"\\u{123\"";
        let mut out = [0u8; 16];
        let err = decode_string(input, 1, 7, 8, &mut out).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidUnicodeEscape);
    }

    #[test]
    fn rejects_non_hex_in_unicode_escape() {
        let input = b"\"\\u{xyz}\"";
        let mut out = [0u8; 16];
        let err = decode_string(input, 1, 8, 9, &mut out).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidUnicodeEscape);
    }

    #[test]
    fn rejects_unknown_escape() {
        let input = b"\"\\q\"";
        let mut out = [0u8; 16];
        let err = decode_string(input, 1, 3, 4, &mut out).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidEscape);
    }

    #[test]
    fn rejects_trailing_backslash() {
        let input = b"\"abc\\";
        let mut out = [0u8; 16];
        let err = decode_string(input, 1, 5, 5, &mut out).unwrap_err();
        assert_eq!(err.code, ErrorCode::UnterminatedString);
    }

    #[test]
    fn encoder_canonical_escapes() {
        assert_eq!(encode_string("a\"b\\c"), r#""a\"b\\c""#);
        assert_eq!(encode_string("line\nfeed"), r#""line\nfeed""#);
        assert_eq!(encode_string("tab\there"), r#""tab\there""#);
        assert_eq!(encode_string("bell\x07here"), r#""bell\u{7}here""#);
    }

    #[test]
    fn encoder_unicode_escape_for_c0() {
        assert_eq!(encode_string("\x00"), r#""\u{0}""#);
        assert_eq!(encode_string("\x01"), r#""\u{1}""#);
        assert_eq!(encode_string("\x1f"), r#""\u{1f}""#);
    }

    #[test]
    fn encoder_passes_printable_through() {
        assert_eq!(encode_string("hello world"), r#""hello world""#);
        assert_eq!(encode_string("\u{1F600}"), "\"\u{1F600}\"");
    }

    // -----------------------------------------------------------------------
    // decode_string defensive error paths (bounds, UTF-8, buffer sizing)
    // -----------------------------------------------------------------------

    #[test]
    fn rejects_start_after_end() {
        // `content_start > content_end` is a malformed call.
        let input = b"\"abc\"";
        let mut out = [0u8; 8];
        let err = decode_string(input, 5, 1, 6, &mut out).unwrap_err();
        assert_eq!(err.code, ErrorCode::UnterminatedString);
    }

    #[test]
    fn rejects_end_past_input() {
        // `content_end` beyond the input slice is a malformed call.
        let input = b"\"abc\"";
        let mut out = [0u8; 8];
        let err = decode_string(input, 1, 99, 100, &mut out).unwrap_err();
        assert_eq!(err.code, ErrorCode::UnterminatedString);
    }

    #[test]
    fn rejects_invalid_utf8_body() {
        // The body itself contains a byte that is not valid UTF-8.
        let input = b"\"\xFF\xFF\"";
        let mut out = [0u8; 4];
        let err = decode_string(input, 1, 3, 4, &mut out).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidUtf8);
    }

    #[test]
    fn rejects_undersized_buffer() {
        // Caller-provided buffer smaller than the encoded body is an internal
        // programming error and must be surfaced as such.
        let input = b"\"hello\"";
        let mut out = [0u8; 3];
        let err = decode_string(input, 1, 6, 7, &mut out).unwrap_err();
        assert_eq!(err.code, ErrorCode::StringTooLong);
    }

    #[test]
    fn rejects_decoded_string_over_the_limit() {
        // A body of plain bytes decodes one-for-one; once the decoded length
        // passes MAX_DECODED_STRING_BYTES the decoder must reject it even
        // though the caller provided a large enough buffer.
        use elang_core::limits::MAX_DECODED_STRING_BYTES;
        let body: Vec<u8> = vec![b'a'; MAX_DECODED_STRING_BYTES + 1];
        let mut input = Vec::with_capacity(body.len() + 2);
        input.push(b'"');
        input.extend_from_slice(&body);
        input.push(b'"');
        let mut out = vec![0u8; body.len()];
        let content_start = 1u32;
        let content_end = (body.len() + 1) as u32;
        let encoded_end = (body.len() + 2) as u32;
        let err =
            decode_string(&input, content_start, content_end, encoded_end, &mut out).unwrap_err();
        assert_eq!(err.code, ErrorCode::StringTooLong);
    }
}
