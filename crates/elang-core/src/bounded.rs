//! Bounded counting reader and decompressed-byte accounting
//! (REQ-CORE-MODEL-006).
//!
//! Decompression and streaming reads run on untrusted input, so the amount of
//! data produced MUST be bounded *as it is produced*, not checked afterwards
//! (steering §3.5, decompression-bomb defense). [`ByteBudget`] tracks a running
//! total against a cap and fails closed the instant a read would exceed it.

use crate::error::{Category, Error};

/// A running byte budget. Each `consume` adds to the total and fails if the cap
/// would be exceeded. Used for per-resource maxima and the aggregate archive
/// budget (steering §3.5).
#[derive(Debug, Clone)]
pub struct ByteBudget {
    used: u64,
    cap: u64,
}

impl ByteBudget {
    /// Create a budget with the given cap in bytes.
    #[must_use]
    pub fn new(cap: u64) -> Self {
        Self { used: 0, cap }
    }

    /// Account for `n` more bytes. Fails (without updating the total) if this
    /// would overflow or exceed the cap.
    pub fn consume(&mut self, n: u64) -> Result<(), Error> {
        let next = self.used.checked_add(n).ok_or_else(|| budget_err("byte budget overflow"))?;
        if next > self.cap {
            return Err(budget_err("byte budget exceeded"));
        }
        self.used = next;
        Ok(())
    }

    /// Bytes consumed so far.
    #[must_use]
    pub fn used(&self) -> u64 {
        self.used
    }

    /// The configured cap.
    #[must_use]
    pub fn cap(&self) -> u64 {
        self.cap
    }

    /// Remaining headroom before the cap.
    #[must_use]
    pub fn remaining(&self) -> u64 {
        self.cap.saturating_sub(self.used)
    }
}

/// A reader adapter that enforces a [`ByteBudget`] over everything read through
/// it, and compares the final count against an expected size.
///
/// This is the shape the container layer wraps a decompression stream in: it
/// counts actual bytes produced and rejects any overrun against the declared
/// `uncompressed_size` and the aggregate budget before buffering more.
#[derive(Debug)]
pub struct BoundedCounter {
    budget: ByteBudget,
    expected: Option<u64>,
}

impl BoundedCounter {
    /// Create a counter with a per-stream budget and an optional expected exact
    /// size (the index `uncompressed_size`).
    #[must_use]
    pub fn new(cap: u64, expected: Option<u64>) -> Self {
        Self {
            budget: ByteBudget::new(cap),
            expected,
        }
    }

    /// Record `n` produced bytes, failing closed on overrun.
    pub fn record(&mut self, n: u64) -> Result<(), Error> {
        self.budget.consume(n)?;
        if let Some(expected) = self.expected {
            if self.budget.used() > expected {
                return Err(budget_err("produced more bytes than declared size"));
            }
        }
        Ok(())
    }

    /// Finalize: verify the produced count equals the expected size (if any).
    pub fn finish(self) -> Result<u64, Error> {
        if let Some(expected) = self.expected {
            if self.budget.used() != expected {
                return Err(budget_err("produced byte count != declared size"));
            }
        }
        Ok(self.budget.used())
    }
}

fn budget_err(msg: &'static str) -> Error {
    Error::new("DECOMPRESSION_BUDGET_EXCEEDED", Category::Frame, msg)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn budget_allows_up_to_cap() {
        let mut b = ByteBudget::new(10);
        assert!(b.consume(4).is_ok());
        assert!(b.consume(6).is_ok());
        assert_eq!(b.used(), 10);
        assert_eq!(b.remaining(), 0);
    }

    #[test]
    fn budget_rejects_overrun_without_updating() {
        let mut b = ByteBudget::new(10);
        assert!(b.consume(8).is_ok());
        assert!(b.consume(5).is_err());
        assert_eq!(b.used(), 8, "failed consume must not advance the total");
    }

    #[test]
    fn budget_rejects_overflow() {
        let mut b = ByteBudget::new(u64::MAX);
        assert!(b.consume(u64::MAX).is_ok());
        assert!(b.consume(1).is_err());
    }

    #[test]
    fn counter_enforces_expected_size() {
        let mut c = BoundedCounter::new(100, Some(10));
        assert!(c.record(6).is_ok());
        assert!(c.record(5).is_err()); // 11 > expected 10
    }

    #[test]
    fn counter_finish_requires_exact() {
        let mut c = BoundedCounter::new(100, Some(10));
        c.record(10).unwrap();
        assert_eq!(c.finish().unwrap(), 10);

        let mut c2 = BoundedCounter::new(100, Some(10));
        c2.record(7).unwrap();
        assert!(c2.finish().is_err()); // short of expected
    }
}
