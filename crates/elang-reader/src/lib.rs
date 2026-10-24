//! `elang-reader` — the integrated ELANG v0.1.0 package reader and public
//! Rust SDK.
//!
//! # What this crate delivers
//!
//! Phase 12 (Issue #22) integrates every previously-landed validation
//! subsystem — binary container parsing (Phase 3), Zstandard frames (Phase 4),
//! KDL 2.0 budgets (Phase 5), EPF schema (Phase 6), integrity manifest
//! (Phase 7), Unicode 18.0 + IPA repertoire (Phase 8), ELD phoneme schema
//! (Phase 9), `when` condition evaluator (Phase 10) and canonical ELD
//! serialization (Phase 11) — into a single 12-stage pipeline with three
//! distinct operations:
//!
//! * [`Package::inspect`] — stages 1–7, no resource decompression (T1201).
//! * [`Package::verify`] — stages 1–11, full resource integrity (T1202).
//! * [`Package::validate`] — stages 1–12, full ELD semantic validation (T1203).
//!
//! The three operations return three distinct types ([`Inspected`],
//! [`Verified`], [`Validated`]) so a caller cannot mistake an inspection
//! for a verification, and partially-validated state is never exposed as
//! fully valid (T1211).
//!
//! # Public SDK surface
//!
//! * Bounded random-access retrieval by index and by path (T1208).
//! * Read-only access to validated language metadata, phoneme groups, root
//!   phonemes, allophones and phonological conditions (T1209).
//! * Optional canonical ELD serialization for downstream consumers
//!   (T1210).
//! * Stable diagnostic codes, resource paths and stage numbers on every
//!   error (T1207).
//!
//! # Thread-safety / streaming / memory contracts (T1212)
//!
//! * **Thread-safety:** [`Package`] and every result type ([`Inspected`],
//!   [`Verified`], [`Validated`]) are `Send + Sync` because their fields are.
//! * **Streaming:** only [`Package::open`] (stages 1–7) and the bootstrap
//!   reads are partial-decode; stages 9–12 read entire resources into
//!   memory. The largest resource allowed is
//!   [`MAX_ELD_BYTES`](elang_core::limits::MAX_ELD_BYTES) (1 GiB) and the
//!   aggregate uncompressed ceiling is
//!   [`MAX_UNCOMPRESSED_AGGREGATE_BYTES`](elang_core::limits::MAX_UNCOMPRESSED_AGGREGATE_BYTES)
//!   (8 GiB).
//! * **Memory:** a [`Verified`] holds every decoded resource's bytes plus
//!   the index, the manifest and the EPF. Plan for roughly the sum of every
//!   indexed resource's `uncompressed_size`. A [`Validated`] discards the
//!   decoded-resource cache and keeps only the validated inventory.
//! * **Ownership:** each operation either borrows (`inspect`) or consumes
//!   (`verify`, `validate`). A [`Verified`] / [`Validated`] is owned and
//!   outlives the [`Package`] that produced it.
//!
//! # What this crate does not do
//!
//! * Build packages — the writer belongs to Phase 13
//!   (Issue #23, `elang-cli`).
//! * Compute the corpus of conformance fixtures — that belongs to
//!   Phase 14.
//! * Hardening against power-loss mid-decode — the resource-integrity
//!   stage reads-and-hashes a frame before comparing; a torn write is
//!   caught by the SHA-256 mismatch.
//!
//! # Example
//!
//! ```no_run
//! use elang_reader::Package;
//!
//! let archive = std::fs::read("package.elang").expect("read archive");
//! let pkg = Package::open(&archive).expect("stages 1–7");
//! let inspected = pkg.inspect();
//! assert_eq!(inspected.len(), inspected.paths().len());
//!
//! let verified = pkg.verify().expect("stages 8–11");
//! assert!(!verified.manifest().resources.is_empty());
//!
//! let validated = elang_reader::Package::open(&archive)
//!     .expect("open")
//!     .validate()
//!     .expect("stages 8–12");
//! assert_eq!(validated.root_count(), validated.roots().count());
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod archive;
pub mod bootstrap;
pub mod error;
pub mod inspected;
pub mod package;
pub mod stages;
pub mod validated;
pub mod verified;

pub use crate::archive::{ArchiveView, Span};
pub use crate::inspected::Inspected;
pub use crate::package::Package;
pub use crate::validated::Validated;
pub use crate::verified::Verified;

// Re-export the two staging errors stages 8 / 9 use, so callers can
// construct match arms against the diagnostic helpers without a deep
// import.
pub use crate::bootstrap::{EXPECTED_MIMETYPE, FORMAT_VERSION_PATH, MIMETYPE_PATH};
pub use crate::stages::{EPF_PATH, INTEGRITY_PATH};
