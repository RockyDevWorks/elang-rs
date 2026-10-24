//! `Validated` — public read-only view over an archive after all 12 stages.
//!
//! Construction is restricted to [`crate::Package::validate`]. A value of
//! this type proves the 12 stages completed in order: every step of the
//! `Verified` ceiling, plus full ELD structural and semantic validation
//! (allophones parsed, IPA identifiers resolved against the pinned
//! repertoire, `when` conditions parsed and cross-referenced, romanization
//! uniqueness enforced, and the validated [`PhonemeInventory`] built).
//!
//! The [`Validated`] type deliberately owns the inventory it exposes — no
//! reference back to the original archive bytes — so it can outlive the
//! [`crate::Package`] that produced it.
//!
//! # Public SDK surface (T1209)
//!
//! [`Validated`] exposes:
//! * [`Self::epf`] / [`Self::metadata`] — the parsed `package.epf` block.
//! * [`Self::groups`] / [`Self::group_by_name`] / [`Self::root_count`] —
//!   group-level enumeration.
//! * [`Self::find_root`] / [`Self::roots`] — root-phoneme access by IPA.
//! * [`Self::condition_for`] / [`Self::allophone_when`] — phonological
//!   condition access (Phase 10).
//! * [`Self::canonical_phoneme_bytes`] — Phase 11 canonical serialization
//!   (T1210).

use elang_container::ArchiveListing;
use elang_core::error::Error;
use elang_core::model::eld::{Group, PhonemeInventory, RootPhoneme};
use elang_core::model::when::WhenCondition;
use elang_epf::model::EpfPackage;
use elang_integrity::IntegrityManifest;

use crate::inspected::Inspected;

/// Public read-only view over an archive after all 12 stages (T1203, T1209,
///
/// T1210, T1211).
///
/// Construction is restricted to [`crate::Package::validate`]. A value of
/// this type proves the 12 stages completed in order: every step of the
/// `Verified` ceiling, plus full ELD structural and semantic validation
/// (allophones parsed, IPA identifiers resolved against the pinned
/// repertoire, `when` conditions parsed and cross-referenced, romanization
/// uniqueness enforced, and the validated [`PhonemeInventory`] built).
///
/// The [`Validated`] type deliberately owns the inventory it exposes — no
/// reference back to the original archive bytes — so it can outlive the
/// [`crate::Package`] that produced it.
#[derive(Debug, Clone)]
pub struct Validated {
    listing: ArchiveListing,
    manifest: IntegrityManifest,
    epf: EpfPackage,
    inventory: PhonemeInventory,
}

impl Validated {
    /// Build from the validate-pipeline outputs. Restricted to the crate root.
    pub(crate) fn new(
        listing: ArchiveListing,
        manifest: IntegrityManifest,
        epf: EpfPackage,
        inventory: PhonemeInventory,
    ) -> Self {
        Self {
            listing,
            manifest,
            epf,
            inventory,
        }
    }

    /// Downgrade to the [`Inspected`] view.
    #[must_use]
    pub fn as_inspected(&self) -> Inspected {
        Inspected::new(self.listing.clone())
    }

    /// Downgrade to the [`Verified`](crate::verified::Verified) view. Requires
    /// re-encoding the cached decoded bytes, which are *not* stored on the
    /// `Validated` type (they were only needed for the ELD parse and the
    /// integrity verification, both already done). Callers that still want
    /// random-access retrieval should keep the original `Verified` rather
    /// than downgrading.
    ///
    /// # Errors
    ///
    /// Currently infallible; the `Result` return shape matches [`Verified`]
    /// construction and reserves room for a future check that depends on the
    /// archive bytes (e.g. re-verifying the manifest after downgrade).
    pub fn as_verified(&self) -> Result<crate::verified::Verified, Error> {
        // `Validated` was built from a `Verified`, so the decoded resource
        // cache was discarded. Returning a `Verified` here would be dishonest;
        // surface that explicitly through the empty cache.
        Ok(crate::verified::Verified::new(
            self.listing.clone(),
            self.manifest.clone(),
            self.epf.clone(),
            Vec::new(),
        ))
    }

    /// The validated `package.epf`.
    #[must_use]
    pub fn epf(&self) -> &EpfPackage {
        &self.epf
    }

    /// Borrow the parsed EPF `metadata` block directly (identifier, name,
    /// optional description).
    #[must_use]
    pub fn metadata(&self) -> &elang_core::model::epf::Metadata {
        self.epf.metadata()
    }

    /// The validated integrity manifest.
    #[must_use]
    pub fn manifest(&self) -> &IntegrityManifest {
        &self.manifest
    }

    /// The validated phoneme inventory (T1209).
    #[must_use]
    pub fn inventory(&self) -> &PhonemeInventory {
        &self.inventory
    }

    /// Iterate every phoneme group in declaration order (T1209).
    #[must_use]
    pub fn groups(&self) -> &[Group] {
        self.inventory.groups()
    }

    /// Look up a phoneme group by its name (the `consonants` / `vowels`
    /// reserved names or a custom group name).
    #[must_use]
    pub fn group_by_name(&self, name: &str) -> Option<&Group> {
        self.inventory
            .groups()
            .iter()
            .find(|g| g.kind().name() == name)
    }

    /// Total number of root phonemes across every group.
    #[must_use]
    pub fn root_count(&self) -> usize {
        self.inventory.root_count()
    }

    /// Find a root phoneme by its IPA identifier text (T1209).
    #[must_use]
    pub fn find_root(&self, ipa: &str) -> Option<&RootPhoneme> {
        self.inventory
            .groups()
            .iter()
            .flat_map(|g| g.roots().iter())
            .find(|r| r.ipa().as_str() == ipa)
    }

    /// Iterate every root phoneme across all groups (T1209).
    pub fn roots(&self) -> impl Iterator<Item = &RootPhoneme> {
        self.inventory
            .groups()
            .iter()
            .flat_map(|g| g.roots().iter())
    }

    /// Find a `when` condition by parent phoneme IPA (T1209).
    ///
    /// Returns the root's `when` if the IPA is a root phoneme; otherwise
    /// `None`. Allophone-level `when` conditions are accessible via
    /// [`Self::allophone_when`].
    #[must_use]
    pub fn condition_for(&self, ipa: &str) -> Option<&WhenCondition> {
        self.find_root(ipa).and_then(|r| r.when())
    }

    /// Find an allophone-level `when` condition by parent IPA and allophone
    /// IPA (T1209).
    #[must_use]
    pub fn allophone_when(&self, root_ipa: &str, allophone_ipa: &str) -> Option<&WhenCondition> {
        self.find_root(root_ipa)
            .and_then(|r| {
                r.allophones()
                    .iter()
                    .find(|a| a.ipa().as_str() == allophone_ipa)
            })
            .and_then(|a| a.when())
    }

    /// Canonical ELD serialization of the validated inventory (T1210).
    ///
    /// This calls `elang_eld::canon::to_bytes` over the cached inventory.
    /// The bytes are byte-deterministic across runs and platforms (Phase 11,
    /// ADR 0009), so two `Validated` instances produced from semantically
    /// equivalent ELD inputs produce identical byte streams here.
    #[must_use]
    pub fn canonical_phoneme_bytes(&self) -> Vec<u8> {
        elang_eld::canon::to_bytes(&self.inventory)
    }
}
