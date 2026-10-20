//! Reader-mode result types (REQ-CORE-MODEL-007).
//!
//! The reader has three modes with strictly increasing guarantees
//! (steering §4): `inspect` (stages 1–7), `verify` (1–11), `validate` (1–12).
//! Each returns a *distinct* type so a caller cannot mistake an inspection for
//! a verification, and so partially validated state is never exposed as fully
//! valid. The types form a one-way ladder: you can obtain a `Verified` only by
//! completing verification, and a `Validated` only by completing validation.

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

/// Result of `inspect`: stages 1–7 completed. This explicitly does **not**
/// assert integrity (REQ-READER-003); it only means the container structure and
/// per-frame headers are well-formed.
#[derive(Debug, Clone)]
pub struct Inspected {
    reached: Stage,
}

/// Result of `verify`: stages 1–11 completed, including decompressing every
/// resource and checking it against the integrity manifest.
#[derive(Debug, Clone)]
pub struct Verified {
    reached: Stage,
}

/// Result of `validate`: all 12 stages completed, including full ELD semantic
/// validation.
#[derive(Debug, Clone)]
pub struct Validated {
    reached: Stage,
}

impl Inspected {
    /// Construct from the reader after completing stages 1–7.
    ///
    /// Crate-internal to the reader: only the container state machine, having
    /// actually run the stages, calls this. Exposed as `pub` within the SDK but
    /// never from untrusted input.
    #[must_use]
    pub fn new() -> Self {
        Self { reached: Stage::FrameInspect }
    }

    /// The highest stage reached (always [`Stage::FrameInspect`]).
    #[must_use]
    pub fn reached(&self) -> Stage {
        self.reached
    }
}

impl Default for Inspected {
    fn default() -> Self {
        Self::new()
    }
}

impl Verified {
    /// Construct from the reader after completing stages 1–11.
    #[must_use]
    pub fn new() -> Self {
        Self { reached: Stage::ResourceIntegrity }
    }

    /// The highest stage reached (always [`Stage::ResourceIntegrity`]).
    #[must_use]
    pub fn reached(&self) -> Stage {
        self.reached
    }
}

impl Default for Verified {
    fn default() -> Self {
        Self::new()
    }
}

impl Validated {
    /// Construct from the reader after completing all 12 stages.
    #[must_use]
    pub fn new() -> Self {
        Self { reached: Stage::EldSemantics }
    }

    /// The highest stage reached (always [`Stage::EldSemantics`]).
    #[must_use]
    pub fn reached(&self) -> Stage {
        self.reached
    }
}

impl Default for Validated {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_ordering_is_monotone() {
        assert!(Stage::Preflight < Stage::FrameInspect);
        assert!(Stage::FrameInspect < Stage::ResourceIntegrity);
        assert!(Stage::ResourceIntegrity < Stage::EldSemantics);
    }

    #[test]
    fn mode_results_report_their_ceiling() {
        assert_eq!(Inspected::new().reached(), Stage::FrameInspect);
        assert_eq!(Verified::new().reached(), Stage::ResourceIntegrity);
        assert_eq!(Validated::new().reached(), Stage::EldSemantics);
    }
}
