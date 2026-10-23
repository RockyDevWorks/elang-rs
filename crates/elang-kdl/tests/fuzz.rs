//! Adversarial fuzz harness (T0515).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::arithmetic_side_effects,
    reason = "fuzz harness drives adversarial inputs through the parser; panics are the failure signal"
)]
//!
//! Drives a hand-rolled deterministic PRNG through the lexer and parser
//! with random byte streams of increasing length. The contract is
//! simple: the parser must not panic, must not allocate past its budgets,
//! and must report every failure through the `ParseError` / `ErrorCode`
//! path rather than crashing.
//!
//! This is a deterministic randomized test suite run by `cargo test`, not a
//! libFuzzer harness. It checks for panics and bounded diagnostic spans; it
//! does not measure allocations or prove that all allocation paths are bounded.

use elang_kdl::{parse_with_budgets, BudgetTracker, ErrorCode};

/// Mulberry32 PRNG — small, fast, deterministic, no external deps.
#[allow(dead_code)]
struct Prng(u32);
impl Prng {
    fn new(seed: u32) -> Self {
        Prng(seed)
    }
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(0x6D2B79F5);
        let mut z = self.0;
        z = (z ^ (z >> 15)).wrapping_mul(z | 1);
        z = z ^ z.wrapping_add((z ^ (z >> 7)).wrapping_mul(z | 61));
        z ^ (z >> 14)
    }
    /// Print a KDL-2.0-ish byte stream: mostly printable, but with a
    /// sprinkling of C0 controls, escapes, and pathological bytes.
    fn gen_kdl_ish(&mut self, len: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(len);
        // Keep a share of inputs syntactically valid so whole-input UTF-8
        // validation cannot keep all long cases away from recursive parsing.
        if len >= 5 && self.next().is_multiple_of(4) {
            out.extend_from_slice(b"n {");
            let fragments: &[&[u8]] = &[b"n 1; ", b"n k=#true; ", b"n #\"text\"#; ", b"n {}; "];
            loop {
                let fragment = fragments[self.next() as usize % fragments.len()];
                if out.len() + fragment.len() + 2 > len {
                    break;
                }
                out.extend_from_slice(fragment);
            }
            out.resize(len - 2, b' ');
            out.extend_from_slice(b"}\n");
            return out;
        }
        while out.len() < len {
            let roll = self.next() % 12;
            let b = match roll {
                0..=3 => b'a' + (self.next() % 26) as u8,
                4 => b'"',
                5 => b'\\',
                6..=9 => {
                    let structural = b"{}=()/;#0123456789   ";
                    structural[self.next() as usize % structural.len()]
                }
                10 => match self.next() % 5 {
                    0 => b'\n',
                    1 => b'\r',
                    2 => b'\t',
                    3 => 0x00,
                    _ => 0xC0 + (self.next() & 0x1F) as u8, // invalid UTF-8 lead
                },
                _ => {
                    // ASCII control or random high bit.
                    let n = self.next();
                    if n & 1 == 0 {
                        (0x01 + (n & 0x1E)) as u8
                    } else {
                        (0x80 + (n & 0x7F)) as u8
                    }
                }
            };
            out.push(b);
        }
        out
    }
}

/// Run one random input through the parser. Any panic / OOB / UB is a
/// failure; a `ParseError` (any code) is expected and counted.
fn run_one(seed: u32, len: usize) -> Result<(), ErrorCode> {
    let mut prng = Prng::new(seed);
    let input = prng.gen_kdl_ish(len);
    let mut budgets = BudgetTracker::new();
    let _doc = parse_with_budgets(&input, &mut budgets).map_err(|e| e.code)?;
    Ok(())
}

#[test]
fn fuzz_short_inputs_never_panic() {
    for seed in 0u32..256 {
        let mut prng = Prng::new(seed);
        let len = (prng.next() as usize) % 256;
        let _ = run_one(seed, len); // Result is irrelevant; absence of panic is the contract.
    }
}

#[test]
fn fuzz_long_inputs_never_panic() {
    for seed in 1000u32..1256 {
        let mut prng = Prng::new(seed);
        let len = 1024 + (prng.next() as usize) % (64 * 1024);
        let _ = run_one(seed, len);
    }
}

#[test]
fn fuzz_pathological_repeats_never_panic() {
    // Patterns that have historically crashed or wedged KDL parsers:
    let patterns: &[&[u8]] = &[
        // Deeply nested valid node prefixes with no matching `}`.
        &b"n {".repeat(200),
        // Repeated empty nodes exercise children-block parsing.
        &{
            let mut v = Vec::with_capacity(2 * 200);
            for _ in 0..200 {
                v.extend_from_slice(b"n {}; ");
            }
            v
        },
        // Alternating `"` `\` `"` — exercises the escape state machine.
        &{
            let mut v = Vec::with_capacity(3 * 512);
            for _ in 0..512 {
                v.push(b'"');
                v.push(b'\\');
                v.push(b'"');
            }
            v
        },
        // Long sequence of `#"` opening raw strings with no close.
        &{
            let mut v = Vec::with_capacity(1024);
            for _ in 0..256 {
                v.extend_from_slice(b"#\"");
            }
            v
        },
    ];
    for (i, p) in patterns.iter().enumerate() {
        let mut budgets = BudgetTracker::new();
        // The result is expected to be Err for every pathological input;
        // a panic would abort the process and fail the test.
        let _ = parse_with_budgets(p, &mut budgets).map_err(|e| {
            assert!(
                e.span.end.byte <= p.len() as u32 + 1,
                "span end {} > input len {} for pattern {}",
                e.span.end.byte,
                p.len(),
                i
            );
            e.code
        });
    }
}

#[test]
fn fuzz_archive_budget_rejects_but_does_not_panic() {
    let mut budgets = BudgetTracker::new();
    let budget = budgets.nodes_archive();
    budget.try_add(budget.ceiling() - 1).unwrap();
    parse_with_budgets(b"node \"value\"\n", &mut budgets).expect("last node fits");
    budgets.reset();
    let err = parse_with_budgets(b"node \"value\"\n", &mut budgets).unwrap_err();
    assert_eq!(err.code, ErrorCode::TooManyNodesArchive);
}

#[test]
fn generated_inputs_reach_children_parsing() {
    assert!((0..32).any(|seed| {
        let input = Prng::new(seed).gen_kdl_ish(128);
        parse_with_budgets(&input, &mut BudgetTracker::new()).is_ok_and(|doc| {
            doc.nodes
                .iter()
                .any(|n| n.children().is_some_and(|c| !c.is_empty()))
        })
    }));
}
