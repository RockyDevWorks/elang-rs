//! Reader-mode result types (REQ-CORE-MODEL-007).
//!
//! The reader has three modes with strictly increasing guarantees
//! (steering §4): `inspect` (stages 1–7), `verify` (1–11), `validate` (1–12).
//! Each returns a *distinct* type so a caller cannot mistake an inspection for
//! a verification, and so partially validated state is never exposed as fully
//! valid. The types form a one-way ladder: you can obtain a `Verified` only by
//! completing verification, and a `Validated` only by completing validation.
//!
//! # Why a tracker, not constructors
//!
//! These result types must be unobtainable without actually running the stages.
//! A public `Validated::new()` would let any downstream crate mint a
//! "fully validated" package without reading a single byte — the type would
//! carry no guarantee at all. So the fields are private, there is no public
//! constructor and no `Default`, and results are handed out only by
//! [`StageTracker`] once it has been advanced monotonically through the
//! required stages. Skipping or going backwards is refused, so the ladder is
//! enforced by the state machine rather than by convention.

/// The highest validation stage a result reflects (1–12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    /// Archive preflight.
    Preflight = 1,
    /// Binary header.
    Header = 2,
    /// Terminal footer and offsets.
    Footer = 3,
    /// SHA-256 of index bytes vs footer hash.
    IndexHash = 4,
    /// Index + path parse/validate.
    Index = 5,
    /// Physical frame layout.
    FrameLayout = 6,
    /// Per-frame header inspection.
    FrameInspect = 7,
    /// `Header/format.version` + `mimetype`.
    Bootstrap = 8,
    /// Integrity manifest structure.
    IntegrityStructure = 9,
    /// EPF parse/validate.
    Epf = 10,
    /// All-resource decompression + SHA-256 verify.
    ResourceIntegrity = 11,
    /// ELD semantic validation.
    EldSemantics = 12,
}

impl Stage {
    /// The stage `inspect` completes.
    pub const INSPECT_CEILING: Stage = Stage::FrameInspect;
    /// The stage `verify` completes.
    pub const VERIFY_CEILING: Stage = Stage::ResourceIntegrity;
    /// The stage `validate` completes.
    pub const VALIDATE_CEILING: Stage = Stage::EldSemantics;

    /// The stage that immediately follows this one, or `None` at the end of
    /// the ladder.
    ///
    /// Used by [`StageTracker::advance`] to require consecutive progression.
    #[must_use]
    pub fn successor(self) -> Option<Stage> {
        let next = (self as u8).checked_add(1)?;
        // Guard against a `successor` that would run past the final stage even
        // if the discriminant arithmetic allowed it.
        if next > Stage::VALIDATE_CEILING as u8 {
            return None;
        }
        match next {
            1 => Some(Stage::Preflight),
            2 => Some(Stage::Header),
            3 => Some(Stage::Footer),
            4 => Some(Stage::IndexHash),
            5 => Some(Stage::Index),
            6 => Some(Stage::FrameLayout),
            7 => Some(Stage::FrameInspect),
            8 => Some(Stage::Bootstrap),
            9 => Some(Stage::IntegrityStructure),
            10 => Some(Stage::Epf),
            11 => Some(Stage::ResourceIntegrity),
            12 => Some(Stage::EldSemantics),
            _ => None,
        }
    }
}

/// Result of `inspect`: stages 1–7 completed. This explicitly does **not**
/// assert integrity (REQ-READER-003); it only means the container structure and
/// per-frame headers are well-formed.
///
/// Obtain via [`StageTracker::inspected`]; there is no public constructor.
#[derive(Debug, Clone)]
pub struct Inspected {
    reached: Stage,
}

/// Result of `verify`: stages 1–11 completed, including decompressing every
/// resource and checking it against the integrity manifest.
///
/// Obtain via [`StageTracker::verified`]; there is no public constructor.
#[derive(Debug, Clone)]
pub struct Verified {
    reached: Stage,
}

/// Result of `validate`: all 12 stages completed, including full ELD semantic
/// validation.
///
/// Obtain via [`StageTracker::validated`]; there is no public constructor.
#[derive(Debug, Clone)]
pub struct Validated {
    reached: Stage,
}

impl Inspected {
    /// The highest stage reached (always [`Stage::FrameInspect`]).
    #[must_use]
    pub fn reached(&self) -> Stage {
        self.reached
    }
}

impl Verified {
    /// The highest stage reached (always [`Stage::ResourceIntegrity`]).
    #[must_use]
    pub fn reached(&self) -> Stage {
        self.reached
    }

    /// Narrowing conversion: a verified archive is also an inspected one.
    #[must_use]
    pub fn as_inspected(&self) -> Inspected {
        Inspected {
            reached: Stage::INSPECT_CEILING,
        }
    }
}

impl Validated {
    /// The highest stage reached (always [`Stage::EldSemantics`]).
    #[must_use]
    pub fn reached(&self) -> Stage {
        self.reached
    }

    /// Narrowing conversion: a validated archive is also a verified one.
    #[must_use]
    pub fn as_verified(&self) -> Verified {
        Verified {
            reached: Stage::VERIFY_CEILING,
        }
    }

    /// Narrowing conversion: a validated archive is also an inspected one.
    #[must_use]
    pub fn as_inspected(&self) -> Inspected {
        Inspected {
            reached: Stage::INSPECT_CEILING,
        }
    }
}

/// A monotonic tracker of how far the reader has progressed through the 12
/// stages.
///
/// `advance` is the only way to move forward and it refuses to skip stages or
/// move backwards, so reaching [`Stage::EldSemantics`] necessarily means every
/// earlier stage ran. The mode results are handed out only once the tracker has
/// reached the corresponding ceiling, which is what makes them unforgeable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageTracker {
    reached: Option<Stage>,
}

impl StageTracker {
    /// A tracker that has completed nothing yet.
    ///
    /// No stage is complete, so the first `advance` must be
    /// [`Stage::Preflight`]. A resumed reader inside this crate can seed a
    /// tracker at a known stage; external callers must replay `advance` from
    /// the start.
    #[must_use]
    pub fn new() -> Self {
        Self { reached: None }
    }

    /// Seed a tracker as if `stage` has just completed.
    ///
    /// Intended for a reader resuming mid-pipeline. This is `pub(crate)` rather
    /// than `pub` precisely because it can bypass the ladder: a public
    /// `at` would let any downstream crate mint a `Validated` without reading a
    /// byte. A resumed reader inside this crate is trusted; an external caller
    /// must replay `advance` from the start.
    #[allow(
        dead_code,
        reason = "tests seed the tracker; production readers use advance"
    )]
    #[must_use]
    pub(crate) fn at(stage: Stage) -> Self {
        Self {
            reached: Some(stage),
        }
    }

    /// Record that `stage` completed.
    ///
    /// Accepts **only the immediate successor** of the current stage, so the
    /// accepted sequence from a fresh tracker is exactly
    /// `Preflight, Header, Footer, …, EldSemantics`. Skipping a stage or
    /// repeating one returns `None` and leaves the tracker untouched. Because
    /// every stage must be visited in order, reaching
    /// [`Stage::EldSemantics`] necessarily means all twelve ran.
    ///
    /// Returns the previously completed stage, or `None` if nothing had
    /// completed yet (in which case the transition is refused).
    pub fn advance(&mut self, stage: Stage) -> Option<Stage> {
        let expected = match self.reached {
            None => Stage::Preflight,
            Some(current) => current.successor()?,
        };
        if stage != expected {
            return None;
        }
        let previous = self.reached.replace(stage);
        // `previous` is None only on the first stage, which has no predecessor.
        previous
    }

    /// The highest stage completed so far, or `None` if nothing has.
    #[must_use]
    pub fn reached(&self) -> Option<Stage> {
        self.reached
    }

    /// Whether the tracker has reached `stage` or beyond.
    #[must_use]
    pub fn has_reached(&self, stage: Stage) -> bool {
        self.reached.is_some_and(|r| r >= stage)
    }

    /// The `inspect` result, or `None` if stages 1–7 are not complete.
    #[must_use]
    pub fn inspected(&self) -> Option<Inspected> {
        self.has_reached(Stage::INSPECT_CEILING)
            .then_some(Inspected {
                reached: Stage::INSPECT_CEILING,
            })
    }

    /// The `verify` result, or `None` if stages 1–11 are not complete.
    #[must_use]
    pub fn verified(&self) -> Option<Verified> {
        self.has_reached(Stage::VERIFY_CEILING).then_some(Verified {
            reached: Stage::VERIFY_CEILING,
        })
    }

    /// The `validate` result, or `None` if not all 12 stages are complete.
    #[must_use]
    pub fn validated(&self) -> Option<Validated> {
        self.has_reached(Stage::VALIDATE_CEILING)
            .then_some(Validated {
                reached: Stage::VALIDATE_CEILING,
            })
    }
}

impl Default for StageTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn stage_ordering_is_monotone() {
        assert!(Stage::Preflight < Stage::FrameInspect);
        assert!(Stage::FrameInspect < Stage::ResourceIntegrity);
        assert!(Stage::ResourceIntegrity < Stage::EldSemantics);
    }

    #[test]
    fn ceilings_are_the_documented_stages() {
        assert_eq!(Stage::INSPECT_CEILING, Stage::FrameInspect);
        assert_eq!(Stage::VERIFY_CEILING, Stage::ResourceIntegrity);
        assert_eq!(Stage::VALIDATE_CEILING, Stage::EldSemantics);
        assert_eq!(Stage::INSPECT_CEILING as u8, 7);
        assert_eq!(Stage::VERIFY_CEILING as u8, 11);
        assert_eq!(Stage::VALIDATE_CEILING as u8, 12);
    }

    #[test]
    fn fresh_tracker_yields_no_results() {
        let t = StageTracker::new();
        assert!(t.inspected().is_none());
        assert!(t.verified().is_none());
        assert!(t.validated().is_none());
    }

    #[test]
    fn fresh_tracker_is_empty() {
        let t = StageTracker::new();
        assert_eq!(t.reached(), None);
        assert!(!t.has_reached(Stage::Preflight));
        assert!(t.inspected().is_none());
        assert!(t.verified().is_none());
        assert!(t.validated().is_none());
    }

    #[test]
    fn advance_accepts_only_the_immediate_successor() {
        let mut t = StageTracker::new();
        // Skipping ahead from a fresh tracker is refused.
        assert_eq!(t.advance(Stage::EldSemantics), None, "fresh: skip refused");
        assert_eq!(t.advance(Stage::Header), None, "fresh: skip refused");
        assert_eq!(t.reached(), None, "refusal leaves state untouched");

        // The first valid advance is Preflight. There is no predecessor.
        assert_eq!(t.advance(Stage::Preflight), None);
        assert_eq!(t.reached(), Some(Stage::Preflight));

        // From Preflight the only valid next is Header.
        assert_eq!(t.advance(Stage::Header), Some(Stage::Preflight));
        // Skipping ahead from Header to IndexHash is refused — Footer is next.
        assert_eq!(
            t.advance(Stage::IndexHash),
            None,
            "jumping past Footer is refused"
        );
        assert_eq!(t.advance(Stage::Footer), Some(Stage::Header));
    }

    #[test]
    fn advance_refuses_backwards_and_repeats() {
        let mut t = StageTracker::at(Stage::Epf);
        // The only successor of Epf is ResourceIntegrity; anything else is
        // rejected — including earlier stages and Epf itself.
        assert_eq!(t.advance(Stage::Index), None, "backwards refused");
        assert_eq!(t.advance(Stage::Epf), None, "repeat refused");
        assert_eq!(
            t.reached(),
            Some(Stage::Epf),
            "refusal leaves state untouched"
        );
        assert_eq!(t.advance(Stage::ResourceIntegrity), Some(Stage::Epf));
        assert_eq!(t.reached(), Some(Stage::ResourceIntegrity));
    }

    #[test]
    fn advance_past_the_top_is_refused() {
        let mut t = StageTracker::at(Stage::EldSemantics);
        assert_eq!(
            t.advance(Stage::EldSemantics),
            None,
            "no successor at the top"
        );
    }

    #[test]
    fn results_appear_only_at_their_ceiling() {
        // Seed just below the inspect ceiling by the trusted in-crate `at`.
        let mut t = StageTracker::at(Stage::FrameInspect);
        assert!(
            t.inspected().is_some(),
            "stage 7 clears the inspect ceiling"
        );
        assert!(t.verified().is_none(), "below the verify ceiling");
        assert!(t.validated().is_none());

        // Walk one stage at a time up to the verify ceiling.
        for stage in [
            Stage::Bootstrap,
            Stage::IntegrityStructure,
            Stage::Epf,
            Stage::ResourceIntegrity,
        ] {
            assert!(
                t.advance(stage).is_some(),
                "consecutive advance to {stage:?} should succeed"
            );
        }
        assert!(t.verified().is_some(), "stage 11 clears the verify ceiling");
        assert!(t.validated().is_none());

        assert!(
            t.advance(Stage::EldSemantics).is_some(),
            "consecutive advance to validate"
        );
        assert!(t.validated().is_some());
    }

    #[test]
    fn fresh_tracker_cannot_yield_a_validated_even_by_skipping() {
        // The whole point of the strict-successor design: nothing ran, so a
        // Validated is unobtainable no matter what the caller tries.
        let mut t = StageTracker::new();
        assert_eq!(
            t.advance(Stage::EldSemantics),
            None,
            "fresh tracker cannot skip ahead"
        );
        assert!(t.validated().is_none());
        assert!(t.verified().is_none());
        assert!(t.inspected().is_none());
    }

    #[test]
    fn stage_successor_covers_the_ladder() {
        let chain = [
            Stage::Preflight,
            Stage::Header,
            Stage::Footer,
            Stage::IndexHash,
            Stage::Index,
            Stage::FrameLayout,
            Stage::FrameInspect,
            Stage::Bootstrap,
            Stage::IntegrityStructure,
            Stage::Epf,
            Stage::ResourceIntegrity,
            Stage::EldSemantics,
        ];
        for window in chain.windows(2) {
            let (Some(first), Some(second)) = (window.first(), window.get(1)) else {
                unreachable!("windows(2) always yields exactly two elements");
            };
            assert_eq!(first.successor(), Some(*second));
        }
        assert_eq!(Stage::EldSemantics.successor(), None);
    }

    #[test]
    fn validated_clamps_reached_to_ceiling() {
        let t = StageTracker::at(Stage::EldSemantics);
        let v = t.validated().unwrap();
        assert_eq!(v.reached(), Stage::VALIDATE_CEILING);
        assert_eq!(v.as_verified().reached(), Stage::VERIFY_CEILING);
        assert_eq!(v.as_inspected().reached(), Stage::INSPECT_CEILING);
    }

    #[test]
    fn verified_clamps_reached_to_ceiling() {
        let t = StageTracker::at(Stage::ResourceIntegrity);
        let v = t.verified().unwrap();
        assert_eq!(v.reached(), Stage::VERIFY_CEILING);
        assert_eq!(v.as_inspected().reached(), Stage::INSPECT_CEILING);
        assert!(t.validated().is_none());
    }

    #[test]
    fn at_seed_path_still_yields_the_full_results() {
        // `at` is the trusted resume path. Seeded at the top, the tracker must
        // still expose every result the strict-successor path would.
        let t = StageTracker::at(Stage::EldSemantics);
        assert!(t.inspected().is_some());
        assert!(t.verified().is_some());
        assert!(t.validated().is_some());
    }

    #[test]
    fn tracker_is_copy_and_default_equivalent() {
        let a = StageTracker::new();
        let b = StageTracker::default();
        assert_eq!(a, b);
        let mut c = a; // Copy: mutating `c` must not affect `a`.
        let _ = c.advance(Stage::Preflight);
        assert_eq!(a, StageTracker::new(), "Copy leaves the original alone");
    }
}
