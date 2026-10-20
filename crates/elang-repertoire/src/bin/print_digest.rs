//! Prints the RFC 8785 JCS SHA-256 digest of the pinned IPA repertoire JSON.
//!
//! This is the reproducible procedure that produces the pinned digest recorded
//! in `specs/sources/SOURCES.md` and asserted by a test. Run with:
//! `cargo run -p elang-repertoire --bin print_digest`.
#![allow(clippy::expect_used, clippy::print_stdout)]

fn main() {
    let digest = elang_repertoire::repertoire::canonical_jcs_sha256(
        elang_repertoire::repertoire::REPERTOIRE_JSON,
    )
    .expect("digest");
    println!("{digest}");
}
