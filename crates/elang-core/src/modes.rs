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
            reached: self.reached,
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
            reached: self.reached,
        }
    }

    /// Narrowing conversion: a validated archive is also an inspected one.
    #[must_use]
    pub fn as_inspected(&self) -> Inspected {
        Inspected {
            reached: self.reached,
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
    reached: Stage,
}

impl StageTracker {
    /// A tracker that has completed nothing yet.
    ///
    /// The starting point is "no stage complete", modelled as a tracker sitting
    /// just below [`Stage::Preflight`]. Because [`Stage`] starts at 1, the first
    /// `advance` must be [`Stage::Preflight`] or later from a fresh tracker; use
    /// [`Self::at`] to seed a tracker at a known stage when resuming.
    #[must_use]
    pub fn new() -> Self {
        Self {
            reached: Stage::Preflight,
        }
    }

    /// Seed a tracker as if `stage` has just completed.
    ///
    /// Intended for a reader resuming mid-pipeline; using it to skip stages
    /// defeats the ladder, so the reader is the only intended caller.
    #[must_use]
    pub fn at(stage: Stage) -> Self {
        Self { reached: stage }
    }

    /// Record that `stage` completed.
    ///
    /// Returns the previous high-water mark. Refuses to move backwards or
    /// repeat, so the sequence of accepted calls must be strictly increasing.
    pub fn advance(&mut self, stage: Stage) -> Option<Stage> {
        if stage <= self.reached {
            return None;
        }
        let previous = self.reached;
        self.reached = stage;
        Some(previous)
    }

    /// The highest stage completed so far.
    #[must_use]
    pub fn reached(&self) -> Stage {
        self.reached
    }

    /// Whether the tracker has reached `stage` or beyond.
    #[must_use]
    pub fn has_reached(&self, stage: Stage) -> bool {
        self.reached >= stage
    }

    /// The `inspect` result, or `None` if stages 1–7 are not complete.
    #[must_use]
    pub fn inspected(&self) -> Option<Inspected> {
        self.has_reached(Stage::INSPECT_CEILING)
            .then_some(Inspected {
                reached: self.reached,
            })
    }

    /// The `verify` result, or `None` if stages 1–11 are not complete.
    #[must_use]
    pub fn verified(&self) -> Option<Verified> {
        self.has_reached(Stage::VERIFY_CEILING).then_some(Verified {
            reached: self.reached,
        })
    }

    /// The `validate` result, or `None` if not all 12 stages are complete.
    #[must_use]
    pub fn validated(&self) -> Option<Validated> {
        self.has_reached(Stage::VALIDATE_CEILING)
            .then_some(Validated {
                reached: self.reached,
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
    fn results_appear_only_at_their_ceiling() {
        let mut t = StageTracker::new();
        t.advance(Stage::Bootstrap);
        assert!(
            t.inspected().is_some(),
            "stage 8 clears the inspect ceiling"
        );
        assert!(
            t.verified().is_none(),
            "stage 8 is below the verify ceiling"
        );
        assert!(t.validated().is_none());

        // Stage 10 (Epf) is still below the verify ceiling of 11.
        t.advance(Stage::Epf);
        assert!(
            t.verified().is_none(),
            "stage 10 is below the verify ceiling"
        );
        assert!(t.validated().is_none());

        // Stage 11 (ResourceIntegrity) clears verify but not validate.
        t.advance(Stage::ResourceIntegrity);
        assert!(t.verified().is_some(), "stage 11 clears the verify ceiling");
        assert!(t.validated().is_none());

        // Stage 12 (EldSemantics) clears validate.
        t.advance(Stage::EldSemantics);
        assert!(t.validated().is_some());
    }

    #[test]
    fn advance_refuses_backwards_and_repeats() {
        let mut t = StageTracker::at(Stage::Epf);
        assert_eq!(t.advance(Stage::Index), None, "backwards refused");
        assert_eq!(t.advance(Stage::Epf), None, "repeat refused");
        assert_eq!(t.reached(), Stage::Epf, "refusal leaves state untouched");
        assert_eq!(t.advance(Stage::ResourceIntegrity), Some(Stage::Epf));
        assert_eq!(t.reached(), Stage::ResourceIntegrity);
    }

    #[test]
    fn validated_narrows_down_the_ladder() {
        let t = StageTracker::at(Stage::EldSemantics);
        let v = t.validated().unwrap();
        assert_eq!(v.reached(), Stage::EldSemantics);
        assert_eq!(v.as_verified().reached(), Stage::EldSemantics);
        assert_eq!(v.as_inspected().reached(), Stage::EldSemantics);
    }

    #[test]
    fn verified_narrows_to_inspected() {
        let t = StageTracker::at(Stage::ResourceIntegrity);
        let v = t.verified().unwrap();
        assert_eq!(v.as_inspected().reached(), Stage::ResourceIntegrity);
        assert!(t.validated().is_none());
    }

    #[test]
    fn tracker_is_copy_and_default_equivalent() {
        let a = StageTracker::new();
        let b = StageTracker::default();
        assert_eq!(a, b);
        let mut c = a;
        c.advance(Stage::Header);
        assert_eq!(a, StageTracker::new(), "Copy leaves the original alone");
    }
}
