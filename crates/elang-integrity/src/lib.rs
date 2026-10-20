//! # elang-integrity
//!
//! `META-INF/integrity.kdl` parsing and SHA-256 verification over the exact
//! uncompressed bytes of every indexed resource except the manifest itself.
//!
//! The integrity-manifest grammar is an OPEN ARTIFACT (steering §12.3) and is
//! frozen in Phase 1 (T0103/T0105) before this crate is implemented in Phase 7.
//! SHA-256 integrity is NOT a publisher signature (steering §0.8).
