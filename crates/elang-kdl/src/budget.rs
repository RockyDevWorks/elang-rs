//! Per-resource budget tracker (T0503–T0510).
//!
//! The lexer and parser charge cumulative node, decoded-string and comment
//! totals through this tracker. Ceilings reference [`elang_core::limits`].
//! Nesting depth and per-node entry counts are checked locally by the parser;
//! token, name and individual comment sizes are checked at their use sites.
//!
//! Each failed increment clamps that counter to its ceiling and returns its
//! error code. Errors are not stored: later calls are evaluated independently.
//! [`BudgetTracker::reset`] clears resource totals while preserving archive
//! totals; [`BudgetTracker::reset_all`] clears both.

use crate::error::ErrorCode;
use elang_core::limits;

/// A numeric or byte budget with a ceiling and an error code.
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    /// Current value, clamped to `ceiling`.
    current: u64,
    /// Inclusive upper bound.
    ceiling: u64,
    /// Error code to surface when the ceiling is exceeded.
    code: ErrorCode,
}

impl Budget {
    /// Construct a budget with the given ceiling and error code.
    pub const fn new(ceiling: u64, code: ErrorCode) -> Budget {
        Budget {
            current: 0,
            ceiling,
            code,
        }
    }

    /// Current value. Clamped to the ceiling.
    pub fn current(&self) -> u64 {
        self.current
    }

    /// Inclusive ceiling.
    pub fn ceiling(&self) -> u64 {
        self.ceiling
    }

    /// Error code returned on breach.
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// Returns `Ok(())` and increments by `n` if the increment stays within
    /// the ceiling; returns `Err(self.code)` and clamps the current value
    /// to the ceiling otherwise.
    ///
    /// Uses `saturating_add` so a runaway additive loop clamps the
    /// counter to `u64::MAX` instead of overflowing.
    pub fn try_add(&mut self, n: u64) -> Result<(), ErrorCode> {
        let new = self.current.saturating_add(n);
        if new > self.ceiling {
            self.current = self.ceiling;
            Err(self.code)
        } else {
            self.current = new;
            Ok(())
        }
    }

    /// Reset to zero. The budget keeps its ceiling and code.
    pub fn reset(&mut self) {
        self.current = 0;
    }
}

/// Cumulative resource and archive budgets shared by the lexer and parser.
/// Call [`BudgetTracker::reset`] between resources in the same archive.
#[derive(Debug)]
pub struct BudgetTracker {
    nodes_resource: Budget,
    nodes_archive: Budget,
    decoded_string_bytes_resource: Budget,
    decoded_string_bytes_archive: Budget,
    comment_bytes_resource: Budget,
}

impl BudgetTracker {
    /// Construct a tracker using the limits pinned in `elang_core::limits`.
    pub fn new() -> BudgetTracker {
        BudgetTracker {
            nodes_resource: Budget::new(
                limits::MAX_NODES_PER_RESOURCE,
                ErrorCode::TooManyNodesResource,
            ),
            nodes_archive: Budget::new(
                limits::MAX_NODES_PER_ARCHIVE,
                ErrorCode::TooManyNodesArchive,
            ),
            decoded_string_bytes_resource: Budget::new(
                limits::MAX_DECODED_STRING_BYTES_PER_RESOURCE,
                ErrorCode::StringBudgetResourceExceeded,
            ),
            decoded_string_bytes_archive: Budget::new(
                limits::MAX_DECODED_STRING_BYTES_PER_ARCHIVE,
                ErrorCode::StringBudgetArchiveExceeded,
            ),
            comment_bytes_resource: Budget::new(
                limits::MAX_COMMENT_BYTES_PER_RESOURCE,
                ErrorCode::CommentBudgetExceeded,
            ),
        }
    }

    /// Per-resource node-count budget.
    pub fn nodes_resource(&mut self) -> &mut Budget {
        &mut self.nodes_resource
    }

    /// Archive-wide node-count budget (T0507). The container layer drives
    /// this across sequential resources by calling `parse_with_budgets`
    /// with the same tracker and resetting only the per-resource budgets
    /// between resources.
    pub fn nodes_archive(&mut self) -> &mut Budget {
        &mut self.nodes_archive
    }

    /// Per-resource decoded-string-byte budget.
    pub fn decoded_string_bytes_resource(&mut self) -> &mut Budget {
        &mut self.decoded_string_bytes_resource
    }

    /// Archive-wide decoded-string-byte budget (T0509). The container
    /// layer drives this across sequential resources.
    pub fn decoded_string_bytes_archive(&mut self) -> &mut Budget {
        &mut self.decoded_string_bytes_archive
    }

    /// Per-resource total comment-byte budget.
    pub fn comment_bytes_resource(&mut self) -> &mut Budget {
        &mut self.comment_bytes_resource
    }

    /// Reset the per-resource counters. Archive-wide counters are left
    /// alone so a multi-resource parse enforces the aggregate budget
    /// continuously. Call [`BudgetTracker::reset_all`] to also zero the
    /// archive counters.
    pub fn reset(&mut self) {
        self.nodes_resource.reset();
        self.decoded_string_bytes_resource.reset();
        self.comment_bytes_resource.reset();
    }

    /// Reset every counter, including the archive-wide ones. Use this
    /// when starting a fresh archive.
    pub fn reset_all(&mut self) {
        self.reset();
        self.nodes_archive.reset();
        self.decoded_string_bytes_archive.reset();
    }
}

impl Default for BudgetTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "budget tests assert exact `Budget` / `BudgetTracker` state transitions; a panic signals a budget-tracking regression"
)]
mod tests {
    use super::*;

    #[test]
    fn budget_increments_within_ceiling() {
        let mut b = Budget::new(10, ErrorCode::TooManyArgs);
        assert!(b.try_add(5).is_ok());
        assert_eq!(b.current(), 5);
        assert!(b.try_add(5).is_ok());
        assert_eq!(b.current(), 10);
    }

    #[test]
    fn budget_rejects_at_ceiling() {
        let mut b = Budget::new(2, ErrorCode::TooManyArgs);
        b.try_add(1).unwrap();
        b.try_add(1).unwrap();
        let err = b.try_add(1).unwrap_err();
        assert_eq!(err, ErrorCode::TooManyArgs);
        assert_eq!(b.current(), 2);
    }

    #[test]
    fn budget_saturates_huge_increment() {
        let mut b = Budget::new(100, ErrorCode::TokenTooLong);
        // Simulate a runaway loop: ask for way more than the ceiling.
        let err = b.try_add(u64::MAX).unwrap_err();
        assert_eq!(err, ErrorCode::TokenTooLong);
        assert_eq!(b.current(), 100);
    }

    #[test]
    fn budget_reset_zeroes_counter() {
        let mut b = Budget::new(10, ErrorCode::NestingTooDeep);
        b.try_add(7).unwrap();
        b.reset();
        assert_eq!(b.current(), 0);
    }

    #[test]
    fn tracker_uses_limits_constants() {
        let t = BudgetTracker::new();
        assert_eq!(t.nodes_resource.ceiling(), 2_000_000);
        assert_eq!(t.decoded_string_bytes_resource.ceiling(), 256 * 1024 * 1024);
        assert_eq!(t.comment_bytes_resource.ceiling(), 16 * 1024 * 1024);
    }
}
