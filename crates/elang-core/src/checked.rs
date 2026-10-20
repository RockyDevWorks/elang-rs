//! Checked integer conversion and offset arithmetic (REQ-CORE-MODEL-005).
//!
//! The container layer computes byte offsets and sizes from untrusted archive
//! bytes. All such arithmetic MUST be checked: an overflow or an out-of-range
//! conversion is a validation failure, never a panic or a wrap (steering §3.5,
//! threat model in design §4). These helpers return `Option`, so callers map a
//! `None` to a typed error at their layer.

/// Checked `u64` addition. `None` on overflow.
#[must_use]
pub fn add_u64(a: u64, b: u64) -> Option<u64> {
    a.checked_add(b)
}

/// Checked `u64` multiplication. `None` on overflow.
#[must_use]
pub fn mul_u64(a: u64, b: u64) -> Option<u64> {
    a.checked_mul(b)
}

/// Checked `u64` subtraction. `None` on underflow.
#[must_use]
pub fn sub_u64(a: u64, b: u64) -> Option<u64> {
    a.checked_sub(b)
}

/// Convert a `u64` to `usize`, returning `None` if it does not fit (e.g. a
/// 64-bit offset on a 32-bit platform). Never truncates.
#[must_use]
pub fn u64_to_usize(v: u64) -> Option<usize> {
    usize::try_from(v).ok()
}

/// Convert a `usize` to `u64`. Infallible on all supported platforms (usize ≤
/// 64 bits) but expressed as `Option` for a uniform call shape.
#[must_use]
pub fn usize_to_u64(v: usize) -> Option<u64> {
    u64::try_from(v).ok()
}

/// A byte range `[offset, offset + len)` that is guaranteed in-bounds for a
/// buffer of `total` bytes, computed with checked arithmetic.
///
/// Returns `None` if `offset + len` overflows or exceeds `total`.
#[must_use]
pub fn checked_range(offset: u64, len: u64, total: u64) -> Option<(usize, usize)> {
    let end = offset.checked_add(len)?;
    if end > total {
        return None;
    }
    let start = u64_to_usize(offset)?;
    let end = u64_to_usize(end)?;
    Some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_detects_overflow() {
        assert_eq!(add_u64(u64::MAX, 1), None);
        assert_eq!(add_u64(10, 5), Some(15));
    }

    #[test]
    fn sub_detects_underflow() {
        assert_eq!(sub_u64(3, 5), None);
        assert_eq!(sub_u64(5, 3), Some(2));
    }

    #[test]
    fn mul_detects_overflow() {
        assert_eq!(mul_u64(u64::MAX, 2), None);
        assert_eq!(mul_u64(6, 7), Some(42));
    }

    #[test]
    fn range_rejects_overflow_and_oob() {
        assert_eq!(checked_range(u64::MAX, 1, u64::MAX), None); // overflow
        assert_eq!(checked_range(10, 5, 12), None); // exceeds total
        assert_eq!(checked_range(10, 5, 15), Some((10, 15)));
        assert_eq!(checked_range(0, 0, 0), Some((0, 0)));
    }
}
