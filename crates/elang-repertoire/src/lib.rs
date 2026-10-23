//! # elang-repertoire
//!
//! Pinned Unicode data and the ELANG IPA Repertoire v1 allowlist (base
//! symbols, combining diacritics, spacing modifiers), with RFC 8785 JCS
//! canonicalization and pinned SHA-256 verification.
//!
//! The approved IPA inventory ([`repertoire`]) is generated reproducibly from
//! the UNITIPA 2020 (IPA 2005/2015/2020) chart by `src/bin/gen_repertoire.rs`
//! and pinned as `specs/schema/ipa-v1.json`; provenance and digests are in
//! `specs/sources/SOURCES.md`. [`ipa`] validates IPA identifiers against that
//! allowlist and the locked structural rules (scalar/unit limits, tie bars,
//! modifier placement). [`unicode`] is a thin adapter over the pinned
//! [`elang_unicode`] crate (Unicode 18.0.0, UCD-generated tables; NFC verified
//! against the official 18.0.0 NormalizationTest.txt — ADR 0007). No ICU4X, no
//! host-OS Unicode behavior.
//!
//! The repertoire is loadable, fully validated, and **ratified**: the second
//! human review of the combining-diacritic and spacing-modifier inventories
//! against the pinned UNITIPA 2020 chart completed 2026-10-23 (outcome match;
//! `specs/sources/SOURCES.md` §7; OA-1/OA-4 Ratified per `docs/open-artifacts.md`).
//! The only remaining v0.1.0 release blocker downstream of this crate is
//! OA-14 (canonical ELD goldens, Phase 11).

pub mod ipa;
pub mod repertoire;
pub mod unicode;
