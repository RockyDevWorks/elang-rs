//! End-to-end quickstart for the `elang-reader` SDK.
//!
//! This example is the runnable companion to
//! `docs/public/getting-started.md`. It uses the published example
//! package at `examples/hello-en/hello-en.elang` (relative to the
//! workspace root) to demonstrate the three reader operations:
//!
//! * `Package::inspect` — header, footer, index, frames
//! * `Package::verify`   — integrity manifest over every resource
//! * `Package::validate` — full ELD semantic validation
//!
//! Run it with:
//!
//! ```sh
//! cargo run --example quickstart -p elang-reader
//! ```
//!
//! or as part of the test suite:
//!
//! ```sh
//! cargo test --example quickstart -p elang-reader
//! ```

use elang_reader::Package;

/// Path to the published example package. `CARGO_MANIFEST_DIR` is the
/// directory containing `Cargo.toml` of the crate being compiled, so
/// the path is stable regardless of where `cargo run --example` is
/// invoked from.
fn example_path() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // CARGO_MANIFEST_DIR is `crates/elang-reader/`; two pops reach the
    // workspace root.
    assert!(
        p.pop() && p.pop(),
        "the elang-reader crate is not under a two-level parent"
    );
    p.join("examples/hello-en/hello-en.elang")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = example_path();
    let bytes = std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;

    // ── 1. Open the archive (stages 1–7). ──────────────────────────────
    let package = Package::open(&bytes).map_err(|e| format!("open: {e}"))?;

    // ── 2. Inspect — list resources without decompressing them. ────────
    let inspected = package.inspect();
    println!(
        "inspected: header version {:?}, {} resources",
        inspected.header_version(),
        inspected.len()
    );
    for path in inspected.paths() {
        println!("  {path}");
    }
    println!(
        "  total uncompressed: {} bytes",
        inspected.total_uncompressed()
    );

    // ── 3. Verify — decompress every resource and check the integrity
    //         manifest (stages 1–11).
    //
    //         `Package::verify` and `Package::validate` are two entry
    //         points on `Package`, and each *consumes* the package it is
    //         called on. To run both, open the archive twice.
    let verified = package.verify().map_err(|e| format!("verify: {e}"))?;
    println!(
        "verified: {} resources matched their SHA-256",
        verified.resource_count()
    );

    // ── 4. Random access — pull one resource by path. No re-parse, no
    //         re-hash: the bytes are already decoded and verified.
    let phonemes = verified
        .get_resource("Linguistics/phonemes.eld")
        .ok_or("Linguistics/phonemes.eld not found")?;
    let text = String::from_utf8_lossy(phonemes);
    println!(
        "Linguistics/phonemes.eld: {} bytes (first line: {})",
        phonemes.len(),
        text.lines().next().unwrap_or("")
    );

    // ── 5. Validate — full pipeline including ELD semantics
    //         (stages 1–12).
    let validated = Package::open(&bytes)
        .map_err(|e| format!("open: {e}"))?
        .validate()
        .map_err(|e| format!("validate: {e}"))?;
    println!(
        "validated: {} phoneme groups, {} root phonemes",
        validated.groups().len(),
        validated.root_count()
    );
    println!(
        "  language: {} ({})",
        validated.metadata().name(),
        validated.metadata().identifier().namespace()
    );

    // ── 6. Canonical ELD — the canonical byte form of the validated
    //         inventory, for consumers that need a stable rendering.
    let canonical = validated.canonical_phoneme_bytes();
    println!("canonical ELD: {} bytes", canonical.len());

    Ok(())
}
