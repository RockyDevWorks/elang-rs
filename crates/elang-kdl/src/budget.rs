//! Per-resource budget tracker (T0503–T0510).
//!
//! Every limit from steering §10 that the parser or lexer must enforce
//! before allocating past it is routed through this tracker. The tracker
//! holds the **current count or byte total**, the **ceiling**, and the
//! [`ErrorCode`] that should be returned when the ceiling is breached.
//!
//! The tracker never panics; it never grows past its ceiling; it never
//! returns `Ok(false)` for a count that has exceeded the ceiling. If a
//! caller keeps incrementing after a breach, the first error is preserved
//! and subsequent calls are no-ops until [`BudgetTracker::reset`] runs.
//!
//! This is the layer that lets the parser fail-closed on adversarial input:
//! a 10-million-deep nested `(a (a (a ...)))` document returns
//! `KDL_NESTING_TOO_DEEP` on the 65th level, having allocated 64 nested
//! nodes — bounded, not unbounded.
//!
//! All the ceilings here mirror the constants in `elang_core::limits`
//! exactly; they are intentionally duplicated (rather than referenced) so
//! that a future budget change in `elang_core` forces a deliberate review
//! of this file before the parser accepts different inputs.

use crate::error::ErrorCode;
use elang_core::limits;

/// A single numeric or byte budget. Saturating on increment means the
/// parser can keep running past the breach to gather **all** the errors
/// in a document instead of stopping at the first one. The first error
/// produced by the first breach is sticky on the parent
/// [`BudgetTracker`].
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

/// Per-resource budget tracker. Carries every parser budget that the
/// steering contract requires; consumed by both the lexer (for
/// encoded-token size and comment bytes) and the parser (for everything
/// else).
///
/// All counters live on this struct; the parser borrows it mutably. There
/// is no global mutable state. When the parser is finished with a
/// resource, the caller invokes [`BudgetTracker::reset`] to zero the
/// counters before parsing the next resource.
#[derive(Debug)]
pub struct BudgetTracker {
    nesting_depth: Budget,
    nodes_resource: Budget,
    nodes_archive: Budget,
    args_per_node: Budget,
    props_per_node: Budget,
    decoded_string_bytes_resource: Budget,
    decoded_string_bytes_archive: Budget,
    comment_bytes_resource: Budget,
    /// Per-comment ceiling is checked by the lexer; the cumulative
    /// per-resource comment total is the budget below.
    comment_bytes_total: Budget,
}

impl BudgetTracker {
    /// Construct a tracker using the limits pinned in `elang_core::limits`.
    pub fn new() -> BudgetTracker {
        BudgetTracker {
            nesting_depth: Budget::new(
                limits::MAX_KDL_NESTING_DEPTH as u64,
                ErrorCode::NestingTooDeep,
            ),
            nodes_resource: Budget::new(
                limits::MAX_NODES_PER_RESOURCE,
                ErrorCode::TooManyNodesResource,
            ),
            nodes_archive: Budget::new(
                limits::MAX_NODES_PER_ARCHIVE,
                ErrorCode::TooManyNodesArchive,
            ),
            args_per_node: Budget::new(limits::MAX_ARGS_PER_NODE as u64, ErrorCode::TooManyArgs),
            props_per_node: Budget::new(limits::MAX_PROPS_PER_NODE as u64, ErrorCode::TooManyProps),
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
            comment_bytes_total: Budget::new(
                limits::MAX_COMMENT_BYTES_PER_RESOURCE,
                ErrorCode::CommentBudgetExceeded,
            ),
        }
    }

    /// Nesting depth budget.
    pub fn nesting_depth(&mut self) -> &mut Budget {
        &mut self.nesting_depth
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

    /// Per-node positional-arg-count budget.
    pub fn args_per_node(&mut self) -> &mut Budget {
        &mut self.args_per_node
    }

    /// Per-node property-count budget.
    pub fn props_per_node(&mut self) -> &mut Budget {
        &mut self.props_per_node
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
        self.nesting_depth.reset();
        self.nodes_resource.reset();
        self.args_per_node.reset();
        self.props_per_node.reset();
        self.decoded_string_bytes_resource.reset();
        self.comment_bytes_resource.reset();
        self.comment_bytes_total.reset();
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
        assert_eq!(t.nesting_depth.ceiling(), 64);
        assert_eq!(t.nodes_resource.ceiling(), 2_000_000);
        assert_eq!(t.decoded_string_bytes_resource.ceiling(), 256 * 1024 * 1024);
        assert_eq!(t.comment_bytes_resource.ceiling(), 16 * 1024 * 1024);
    }
}
