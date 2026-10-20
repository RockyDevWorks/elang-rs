//! # elang CLI
//!
//! Command-line front end exposing `inspect`, `verify`, `validate`, `pack`,
//! `unpack`, `canonicalize-eld` and `repertoire info`. Implemented in Phase 13.
//!
//! This binary is a thin shell over the library crates; it performs no
//! validation logic of its own.

fn main() {
    // Phase 13 wires up argument parsing and the reader/writer SDK. For now the
    // binary exists so the workspace builds end-to-end on all platforms (T0001).
    eprintln!(
        "elang {} (reference tooling scaffold; CLI commands land in Phase 13)",
        env!("CARGO_PKG_VERSION")
    );
    std::process::exit(2);
}
