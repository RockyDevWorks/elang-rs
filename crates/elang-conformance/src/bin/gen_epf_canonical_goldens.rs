//! Generator for the T1405 EPF canonical golden corpus.
//!
//! Running this:
//!
//! - Produces the byte-exact canonical `package.epf` strings for the
//!   pinned corpus cases (`minimal`, `full`).
//! - Computes the SHA-256 of every canonical string and prints the
//!   digest in the format `id: <hex>`.
//!
//! The output is intended to be pasted into
//! `epf_canonical_goldens::CASES`, which is where the pinned digests
//! live. Nothing in the corpus is hand-written.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::arithmetic_side_effects
)]

use elang_conformance::binary_vectors::{hex, sha256};
use elang_conformance::epf_canonical_goldens::CASES;

fn main() {
    for case in CASES {
        let pkg = elang_epf::parse(case.source.as_bytes()).expect("source parses");
        let canonical = elang_epf::canon::to_string(&pkg);
        let bytes = canonical.as_bytes();
        let digest_hex = hex(&sha256(bytes));
        println!("=== {} ===", case.id);
        println!("--- source ---");
        print!("{}", case.source);
        println!("--- canonical ---");
        print!("{canonical}");
        println!("--- sha256 ---");
        println!("{digest_hex}");
        println!();
    }
}
