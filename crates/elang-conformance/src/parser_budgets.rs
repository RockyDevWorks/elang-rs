//! T1415 — parser budget boundary fixtures.
//!
//! # What "budget boundary" means here
//!
//! Every parser in the ELANG stack has a documented ceiling. A
//! conformance corpus must pin **both sides** of each ceiling:
//!
//! - the *at-budget* input is accepted (the ceiling is not off by one
//!   in the conservative direction), and
//! - the *over-budget* input is rejected with the *specific* limit
//!   diagnostic, not a generic parse error.
//!
//! A parser that rejects an at-budget input is as non-conformant as one
//! that accepts an over-budget input; both are silent capacity bugs that
//! only a boundary fixture can catch.
//!
//! # The budgets covered
//!
//! | Budget | Value | Enforced by |
//! |---|---|---|
//! | `MAX_NESTING_DEPTH` | 8 | `elang_eld::when` (condition nesting) |
//! | `MAX_PREDICATES_PER_WHEN` | 16 | `elang_eld::when` (predicates per block) |
//! | `MAX_EPF_BYTES` | 1 MiB | `elang_epf::parse` (input size) |
//! | `MAX_BOOTSTRAP_RESOURCE_BYTES` | 64 | `elang_reader::bootstrap` |
//! | `MAX_RESOURCE_PATH_BYTES` | 1024 | `elang_core::ResourcePath` |
//!
//! # Boundary introspection
//!
//! A test that only asserts "this input parses" proves nothing about
//! *which* boundary it hit. The condition-nesting fixtures therefore
//! assert the parsed AST's **actual maximum nesting depth** with
//! [`elang_eld::max_nesting_depth`] (which counts a `Condition::Group`
//! or `Condition::Or` container as one level and a leaf predicate as
//! zero). The over-budget fixture walks the raw KDL AST to recover the
//! depth of the tree the parser *would* have built if the budget were
//! not enforced — the parser itself rejects the input, so the only way
//! to measure the would-be depth is a structural walk that does not
//! honour the budget. The `MAX_NESTING_DEPTH` constant itself is pinned
//! to its spec literal (and asserted equal to the implementation
//! constant) so a silent budget change fails the suite before the
//! boundary tests start coincidentally passing at the wrong depth.
//!
//! # The differential proof
//!
//! `MAX_BOOTSTRAP_RESOURCE_BYTES` and `MAX_RESOURCE_PATH_BYTES` are
//! ceilings that no *conformant* archive actually reaches: a 64-byte
//! `Header/format.version` cannot match the semver-shaped content rule,
//! and a 1024-byte resource path serves no conformant purpose either.
//! Their boundaries are pinned through **differential pairs**:
//!
//! - bootstrap at-limit (64) → rejected with `FORMAT_VERSION_INVALID`
//!   (Container/stage 8) — proving the *budget* passed and a different
//!   check fired; and
//! - bootstrap over-limit (65) → rejected with
//!   `DECOMPRESSION_BUDGET_EXCEEDED` (Frame/stage 8) — proving the
//!   budget now fires.
//!
//! The at/over split into different diagnostic codes is the proof: at
//! the exact boundary the *budget* did not fire (otherwise the frame
//! category would have surfaced), and one byte past it the budget does
//! fire. The same pattern applies to `MAX_RESOURCE_PATH_BYTES` with
//! Accept/Inspect at 1024 vs `PATH_TOO_LONG` at 1025.

use crate::corpus::{DocumentExpectation, DocumentFixture, Expectation, Fixture, Level, Schema};
use crate::negatives::{archive_with_custom_index, encode_index_entry, rebuild_with_resource};
use elang_core::error::Category;
#[cfg(test)]
use elang_kdl::ast::{KdlChildren, KdlNode};

/// The `when`-condition nesting budget, pinned to its **spec literal**.
///
/// Deliberately *not* an alias of `elang_eld::MAX_NESTING_DEPTH`: an
/// alias would make the pin test compare the constant to itself and
/// could never fail. The literal is the value the specification
/// states; `pinned_budgets_match_implementation_constants` then
/// proves the implementation still agrees with it.
pub const MAX_NESTING_DEPTH: usize = 8;

/// The predicates-per-`when`-block budget, pinned to its spec literal.
pub const MAX_PREDICATES_PER_WHEN: usize = 16;

/// The EPF input-size ceiling, pinned to its spec literal (1 MiB).
pub const MAX_EPF_BYTES: u64 = 1024 * 1024;

/// The bootstrap resource-size ceiling, pinned to its spec literal.
pub const MAX_BOOTSTRAP_RESOURCE_BYTES: u64 = 64;

/// The resource-path byte ceiling, pinned to its spec literal.
pub const MAX_RESOURCE_PATH_BYTES: u64 = 1024;

/// Build an ELD document whose `when` block nests `group` conditions
/// `depth` levels deep.
///
/// Each `group` carries **two** children: at the deepest level those
/// children are two atomic predicates (`word-position "initial"` and
/// `syllable-position "onset"`); at every other level one child is the
/// nested `group` and the other is the `word-position "initial"`
/// predicate. The resulting tree has `depth` nested groups plus the
/// predicate leaves, so the *maximum path depth from the root to a
/// leaf* equals `depth + 1`.
///
/// The block is emitted one node per line — a same-line sibling would
/// collapse into the previous node's argument list under the KDL-1.0
/// grammar and produce a different diagnostic entirely.
fn eld_with_nested_when(depth: usize) -> Vec<u8> {
    let mut src = String::from("phonemes {\n");
    src.push_str("    consonants {\n");
    src.push_str("        phoneme \"p\" {\n");
    src.push_str("            roman \"p\"\n");
    src.push_str("            when {\n");

    // Open `depth` nested groups; each group has two children — the
    // next group and the word-position predicate.
    for level in 0..depth {
        for _ in 0..(level + 2) {
            src.push_str("    ");
        }
        src.push_str("group {\n");
        for _ in 0..(level + 3) {
            src.push_str("    ");
        }
        src.push_str("word-position \"initial\"\n");
    }
    // At the deepest level: two predicates.
    for _ in 0..(depth + 2) {
        src.push_str("    ");
    }
    src.push_str("word-position \"initial\"\n");
    for _ in 0..(depth + 2) {
        src.push_str("    ");
    }
    src.push_str("syllable-position \"onset\"\n");

    // Close the nested groups.
    for level in (0..depth).rev() {
        for _ in 0..(level + 2) {
            src.push_str("    ");
        }
        src.push_str("}\n");
    }

    src.push_str("            }\n");
    src.push_str("        }\n");
    src.push_str("    }\n");
    src.push_str("    vowels {\n");
    src.push_str("        phoneme \"a\" {\n            roman \"a\"\n        }\n");
    src.push_str("    }\n");
    src.push_str("}\n");
    src.into_bytes()
}

/// The maximum nesting depth of the `when` block in the document built
/// by [`eld_with_nested_when`], **as measured by the production parser**.
///
/// Parses `source` with [`elang_eld::parse`], walks the document to the
/// first root phoneme that carries a `when` block, calls
/// [`elang_eld::parse_block`] to convert that block into a
/// [`elang_core::model::when::WhenCondition`] AST, and returns the
/// maximum [`elang_eld::max_nesting_depth`] across the block's top-level
/// children. Returns `None` when the parser rejects the document (the
/// over-budget case — the budget fires before we can recover the AST).
#[cfg(test)]
fn parsed_when_depth(source: &[u8]) -> Option<usize> {
    let rep = elang_repertoire::repertoire::load().expect("repertoire loads");
    let doc = elang_eld::parse(source, &rep).ok()?;
    let refs = elang_eld::InventoryRefs::from_document(&doc);
    let root_with_when = doc
        .groups()
        .iter()
        .flat_map(|g| g.roots())
        .find(|r| r.when().is_some())?;
    let block = root_with_when.when()?;
    let cond = elang_eld::parse_block(block, &refs, &rep).ok()?;
    Some(
        cond.children()
            .iter()
            .map(elang_eld::max_nesting_depth)
            .max()
            .unwrap_or(0),
    )
}

/// The maximum nesting depth of the would-be condition tree inside the
/// `when` block of `source`, measured by walking the raw KDL AST.
///
/// Used to characterise the *rejected* fixture: the parser rejects the
/// over-budget document, so the AST cannot be recovered through
/// [`elang_eld::parse_block`]. We therefore parse the KDL structure
/// with [`elang_kdl::parse`] (which never enforces the when budget)
/// and walk down to the `when` node, then recursively count `group`
/// and `or` containers the same way [`elang_eld::max_nesting_depth`]
/// counts them on the `Condition` AST. With `N` nested groups in the
/// fixture the walker returns exactly `N`.
#[cfg(test)]
fn would_be_when_depth(source: &[u8]) -> usize {
    let kdl = elang_kdl::parse(source).expect("KDL parses for depth walk");
    let when = find_named(kdl.nodes(), "when").expect("document carries a when block");
    container_depth(
        when.children()
            .map_or(&[] as &[KdlNode], KdlChildren::as_slice),
    )
}

#[cfg(test)]
fn find_named<'a>(nodes: &'a [KdlNode], name: &str) -> Option<&'a KdlNode> {
    for n in nodes {
        if n.name() == name {
            return Some(n);
        }
        if let Some(ch) = n.children() {
            if let Some(found) = find_named(ch.as_slice(), name) {
                return Some(found);
            }
        }
    }
    None
}

#[cfg(test)]
fn container_depth(nodes: &[KdlNode]) -> usize {
    nodes
        .iter()
        .map(|n| {
            let child = n.children().map_or(0, |c| container_depth(c.as_slice()));
            match n.name() {
                "group" | "or" => child + 1,
                _ => child,
            }
        })
        .max()
        .unwrap_or(0)
}

/// All T1415 *document* budget boundary fixtures.
///
/// Archive-level (Fixture) budget fixtures are produced by
/// [`archive_fixtures`]; this function returns only the
/// `DocumentFixture`s so the existing per-family aggregate keeps its
/// shape.
#[must_use]
pub fn fixtures() -> Vec<DocumentFixture> {
    let mut fixtures = Vec::new();

    // -- At-budget: the deepest accepted condition tree ----------------
    fixtures.push(DocumentFixture {
        id: "budget/when-nesting-at-limit",
        description: "A condition tree at exactly MAX_NESTING_DEPTH is accepted",
        requirements: &["REQ-LIMIT-001"],
        schema: Schema::Eld,
        source: eld_with_nested_when(MAX_NESTING_DEPTH),
        expectation: DocumentExpectation::Accept,
    });

    // -- Over-budget: one level past the ceiling -----------------------
    fixtures.push(DocumentFixture {
        id: "budget/when-nesting-over-limit",
        description: "A condition tree one level past MAX_NESTING_DEPTH is rejected",
        requirements: &["REQ-LIMIT-001"],
        schema: Schema::Eld,
        source: eld_with_nested_when(MAX_NESTING_DEPTH + 1),
        expectation: DocumentExpectation::Reject {
            code: "WHEN_TOO_DEEP",
            category: Category::When,
        },
    });

    // -- At-budget: exactly MAX_PREDICATES_PER_WHEN predicates ---------
    {
        let mut src = String::from("phonemes {\n");
        src.push_str("    consonants {\n");
        src.push_str("        phoneme \"p\" {\n");
        src.push_str("            roman \"p\"\n");
        src.push_str("            when {\n");
        for index in 0..MAX_PREDICATES_PER_WHEN {
            // Alternate between two predicate spellings so the block
            // does not trip the duplicate-predicate rule.
            if index % 2 == 0 {
                src.push_str("                word-position \"initial\"\n");
            } else {
                src.push_str("                syllable-position \"onset\"\n");
            }
        }
        src.push_str("            }\n");
        src.push_str("        }\n");
        src.push_str("    }\n");
        src.push_str("    vowels {\n");
        src.push_str("        phoneme \"a\" {\n            roman \"a\"\n        }\n");
        src.push_str("    }\n");
        src.push_str("}\n");
        fixtures.push(DocumentFixture {
            id: "budget/when-predicates-at-limit",
            description: "A when block with exactly MAX_PREDICATES_PER_WHEN predicates is accepted",
            requirements: &["REQ-LIMIT-004"],
            schema: Schema::Eld,
            source: src.into_bytes(),
            expectation: DocumentExpectation::Accept,
        });
    }

    // -- Over-budget: one predicate past the ceiling -------------------
    {
        let mut src = String::from("phonemes {\n");
        src.push_str("    consonants {\n");
        src.push_str("        phoneme \"p\" {\n");
        src.push_str("            roman \"p\"\n");
        src.push_str("            when {\n");
        for index in 0..=MAX_PREDICATES_PER_WHEN {
            if index % 2 == 0 {
                src.push_str("                word-position \"initial\"\n");
            } else {
                src.push_str("                syllable-position \"onset\"\n");
            }
        }
        src.push_str("            }\n");
        src.push_str("        }\n");
        src.push_str("    }\n");
        src.push_str("    vowels {\n");
        src.push_str("        phoneme \"a\" {\n            roman \"a\"\n        }\n");
        src.push_str("    }\n");
        src.push_str("}\n");
        fixtures.push(DocumentFixture {
            id: "budget/when-predicates-over-limit",
            description: "A when block with one predicate past the ceiling is rejected",
            requirements: &["REQ-LIMIT-004"],
            schema: Schema::Eld,
            source: src.into_bytes(),
            expectation: DocumentExpectation::Reject {
                code: "WHEN_TOO_MANY_PREDICATES",
                category: Category::When,
            },
        });
    }

    // -- At-budget: an EPF document exactly MAX_EPF_BYTES long ---------
    // The producer (`MINIMAL_EPF`) is parsed by `elang-epf`, which
    // rejects only `input.len() > MAX_EPF_BYTES`. Reaching the ceiling
    // exactly therefore passes the size budget; the trailing KDL line
    // comment is stripped by the lexer before schema validation, so
    // the schema-level parse must still succeed. The padding is
    // computed at fixture-authoring time so the fixture adapts to any
    // future change in `MINIMAL_EPF.len()` instead of drifting with a
    // hand-coded magic number.
    {
        let base = crate::golden_packages::MINIMAL_EPF;
        // `//` + N `x` + `\n` — three bytes of overhead around the
        // padding. KDL trivia is not part of any schema; the lexer
        // strips it.
        const OVERHEAD: usize = 3;
        let pad = (MAX_EPF_BYTES as usize)
            .checked_sub(base.len())
            .and_then(|v| v.checked_sub(OVERHEAD))
            .unwrap_or_else(|| {
                panic!(
                    "MINIMAL_EPF ({} bytes) plus `//...\\n` overhead ({OVERHEAD}) already \
                     exceeds MAX_EPF_BYTES ({MAX_EPF_BYTES}); an at-budget EPF fixture \
                     cannot be constructed",
                    base.len()
                )
            });
        // The charged line-comment length is `2 + N` (the `//` prefix
        // but not the terminating newline); it must fit under
        // `MAX_COMMENT_BYTES`. The `2 + pad` form is the worst case.
        let max_comment_bytes = elang_core::limits::MAX_COMMENT_BYTES as u64;
        assert!(
            2u64.saturating_add(pad as u64) <= max_comment_bytes,
            "the padding would trip the per-comment ceiling: 2 + {pad} > {max_comment_bytes}"
        );

        let mut source = Vec::with_capacity(MAX_EPF_BYTES as usize);
        source.extend_from_slice(base);
        source.extend_from_slice(b"//");
        source.extend(std::iter::repeat_n(b'x', pad));
        source.push(b'\n');
        assert_eq!(
            source.len() as u64,
            MAX_EPF_BYTES,
            "at-budget EPF fixture must total exactly MAX_EPF_BYTES bytes",
        );
        fixtures.push(DocumentFixture {
            id: "budget/epf-bytes-at-limit",
            description: "An EPF document exactly MAX_EPF_BYTES long is accepted",
            requirements: &["REQ-PKG-004"],
            schema: Schema::Epf,
            source,
            expectation: DocumentExpectation::Accept,
        });
    }

    // -- Over-budget: an EPF document one byte past MAX_EPF_BYTES ------
    // The boundary is exact: the over-budget fixture totals exactly
    // `MAX_EPF_BYTES + 1` bytes. If the implementation ceiling were
    // ever raised, this fixture would slip under the new limit and be
    // accepted, failing the test loudly — proving the fixture probes
    // the boundary rather than over-shooting it by a megabyte. The
    // payload is `MINIMAL_EPF` plus a line comment; comments are
    // stripped by the KDL lexer before schema validation, so the
    // schema-level parse only sees the valid `MINIMAL_EPF`. The raw
    // length check, however, fires first and trips `LIMIT_EPF_BYTES`.
    {
        let base = crate::golden_packages::MINIMAL_EPF;
        const OVERHEAD: usize = 3; // `//` + `\n`
        let pad = (MAX_EPF_BYTES as usize)
            .checked_add(1)
            .and_then(|v| v.checked_sub(base.len()))
            .and_then(|v| v.checked_sub(OVERHEAD))
            .unwrap_or_else(|| {
                panic!(
                    "MINIMAL_EPF ({} bytes) plus overhead ({OVERHEAD}) already covers                      MAX_EPF_BYTES + 1 ({MAX_EPF_BYTES}); an over-budget EPF fixture                      cannot be constructed",
                    base.len()
                )
            });
        let max_comment_bytes = elang_core::limits::MAX_COMMENT_BYTES as u64;
        assert!(
            2u64.saturating_add(pad as u64) <= max_comment_bytes,
            "the over-budget padding would trip the per-comment ceiling: 2 + {pad} > {max_comment_bytes}"
        );
        let mut source = Vec::with_capacity(MAX_EPF_BYTES as usize + 1);
        source.extend_from_slice(base);
        source.extend_from_slice(b"//");
        source.extend(std::iter::repeat_n(b'x', pad));
        source.push(b'\n');
        assert_eq!(
            source.len() as u64,
            MAX_EPF_BYTES + 1,
            "over-budget EPF fixture must total exactly MAX_EPF_BYTES + 1 bytes"
        );
        fixtures.push(DocumentFixture {
            id: "budget/epf-bytes-over-limit",
            description: "An EPF document one byte past MAX_EPF_BYTES is rejected",
            requirements: &["REQ-PKG-004"],
            schema: Schema::Epf,
            source,
            expectation: DocumentExpectation::Reject {
                code: "LIMIT_EPF_BYTES",
                category: Category::Limit,
            },
        });
    }

    fixtures
}

/// All T1415 *archive* budget boundary fixtures.
///
/// The bootstrap and resource-path ceilings are not reachable by a
/// conformant package, so the boundary is pinned through differential
/// pairs (see the module docs). Each pair is wired into the aggregate
/// corpus run by `tests/corpus_run.rs` and the stage conformance check
/// by `tests/stage_conformance.rs`.
#[must_use]
pub fn archive_fixtures() -> Vec<Fixture> {
    let mut fixtures = Vec::new();

    // -- Bootstrap at-budget: 64 bytes, grammar fires ------------------
    // A 64-byte format.version is *exactly* the bootstrap budget. The
    // content starts with the "0.1." prefix so the prefix check passes
    // and the budget is the only thing distinguishing 64 from 65 — the
    // patch-component check then fires. The fixture is therefore
    // rejected with `FORMAT_VERSION_INVALID`, not the frame-budget code
    // the byte-over case uses, which is the differential proof that the
    // 64-byte boundary is *under* the budget.
    {
        let mut payload = Vec::with_capacity(MAX_BOOTSTRAP_RESOURCE_BYTES as usize);
        payload.extend_from_slice(b"0.1.");
        payload.extend(std::iter::repeat_n(
            b'x',
            (MAX_BOOTSTRAP_RESOURCE_BYTES as usize) - 4,
        ));
        assert_eq!(payload.len() as u64, MAX_BOOTSTRAP_RESOURCE_BYTES);
        let archive = rebuild_with_resource("Header/format.version", &payload);
        fixtures.push(Fixture {
            id: "budget/bootstrap-at-limit",
            description: "A 64-byte Header/format.version passes the budget but fails the version grammar (stage 8)",
            requirements: &["REQ-PKG-002"],
            expectation: Expectation::Reject {
                level: Level::Verify,
                code: "FORMAT_VERSION_INVALID",
                category: Category::Container,
            },
            archive,
        });
    }

    // -- Bootstrap over-budget: 65 bytes, budget fires ----------------
    {
        let mut payload = Vec::with_capacity(MAX_BOOTSTRAP_RESOURCE_BYTES as usize + 1);
        payload.extend_from_slice(b"0.1.");
        payload.extend(std::iter::repeat_n(
            b'x',
            (MAX_BOOTSTRAP_RESOURCE_BYTES as usize) - 4 + 1,
        ));
        assert_eq!(
            payload.len() as u64,
            MAX_BOOTSTRAP_RESOURCE_BYTES + 1,
            "fixture must be exactly one byte past the bootstrap budget"
        );
        let archive = rebuild_with_resource("Header/format.version", &payload);
        fixtures.push(Fixture {
            id: "budget/bootstrap-over-limit",
            description:
                "A 65-byte Header/format.version trips the bootstrap decode budget (stage 8)",
            requirements: &["REQ-PKG-002"],
            expectation: Expectation::Reject {
                level: Level::Verify,
                code: "DECOMPRESSION_BUDGET_EXCEEDED",
                category: Category::Frame,
            },
            archive,
        });
    }

    // -- Resource-path at-budget: 1024 bytes, accepted at Inspect ------
    // `archive_with_custom_index` always encodes the same "x\n" payload
    // with the same `GOLDEN_PROFILE`; the frame length is therefore
    // deterministic. The at-limit fixture must declare it correctly so
    // the frame-layout check at stage 6 passes — otherwise a layout
    // diagnostic would mask the boundary we're trying to prove.
    {
        let path_len = MAX_RESOURCE_PATH_BYTES as usize;
        let path = build_path_with_length(path_len, b"L");
        assert_eq!(path.len(), path_len);

        let (compressed_size, uncompressed_size) = frame_size();
        let archive = archive_with_custom_index(|frame_offset| {
            use elang_container::index::{
                COMPRESSION_ZSTD, ENTRY_FORMAT, INDEX_MAGIC, INDEX_VERSION,
            };
            let mut idx = Vec::new();
            idx.extend_from_slice(&INDEX_MAGIC);
            idx.extend_from_slice(&INDEX_VERSION.to_le_bytes());
            idx.extend_from_slice(&ENTRY_FORMAT.to_le_bytes());
            idx.extend_from_slice(&1u32.to_le_bytes());
            encode_index_entry(
                &mut idx,
                path.as_bytes(),
                frame_offset,
                compressed_size,
                uncompressed_size,
                COMPRESSION_ZSTD,
            );
            idx
        });
        fixtures.push(Fixture {
            id: "budget/resource-path-at-limit",
            description: "A 1024-byte resource path is accepted at Inspect",
            requirements: &["REQ-BIN-011"],
            expectation: Expectation::Accept(Level::Inspect),
            archive,
        });
    }

    // -- Resource-path over-budget: 1025 bytes, rejected at Inspect ----
    {
        let path_len = (MAX_RESOURCE_PATH_BYTES as usize) + 1;
        let path = build_path_with_length(path_len, b"L");
        assert_eq!(path.len(), path_len);

        let (compressed_size, uncompressed_size) = frame_size();
        let archive = archive_with_custom_index(|frame_offset| {
            use elang_container::index::{
                COMPRESSION_ZSTD, ENTRY_FORMAT, INDEX_MAGIC, INDEX_VERSION,
            };
            let mut idx = Vec::new();
            idx.extend_from_slice(&INDEX_MAGIC);
            idx.extend_from_slice(&INDEX_VERSION.to_le_bytes());
            idx.extend_from_slice(&ENTRY_FORMAT.to_le_bytes());
            idx.extend_from_slice(&1u32.to_le_bytes());
            encode_index_entry(
                &mut idx,
                path.as_bytes(),
                frame_offset,
                compressed_size,
                uncompressed_size,
                COMPRESSION_ZSTD,
            );
            idx
        });
        fixtures.push(Fixture {
            id: "budget/resource-path-over-limit",
            description: "A 1025-byte resource path is rejected at Inspect (stage 5)",
            requirements: &["REQ-BIN-011"],
            expectation: Expectation::Reject {
                level: Level::Inspect,
                code: "PATH_TOO_LONG",
                category: Category::ResourcePath,
            },
            archive,
        });
    }

    fixtures
}

/// Compute the compressed and uncompressed sizes of the canonical
/// "x\n" frame that [`archive_with_custom_index`] emits, so the
/// custom-index entries can declare the correct sizes for the
/// frame-layout check at stage 6 to pass.
fn frame_size() -> (u64, u64) {
    let payload = b"x\n";
    let compressed =
        elang_container::frame::encode(payload, crate::golden_packages::GOLDEN_PROFILE)
            .expect("the canonical \"x\\n\" frame must encode");
    (compressed.len() as u64, payload.len() as u64)
}

/// Build an absolute, NFC, non-reserved `ResourcePath` of exactly
/// `length` bytes by repeating a non-empty, NFC, lowercase ASCII prefix
/// (e.g. `L`) until the path length is reached.
///
/// `ResourcePath::new` enforces UTF-8, NFC, no NUL, ≤1024 bytes, must
/// be absolute (`/`-prefixed), no `.`/`..` segments, no trailing `/`,
/// no `\` separator, no reserved paths. ASCII lowercase repeated
/// satisfies every rule; the prefix byte(s) live in the segment name so
/// the leading `/` plus the prefix still leave `length` bytes to fill
/// after the segment separator.
fn build_path_with_length(length: usize, _prefix: &[u8]) -> String {
    assert!(
        length >= 3,
        "a path must have at least one non-empty segment, asked for {length} bytes"
    );
    // `L/` + `x` repeated. The reader rejects absolute paths (those
    // beginning with `/`), reserved resource names, and paths whose
    // segments are empty — so the path must have a non-empty first
    // segment, a `/` separator, and a non-empty second segment. A
    // single-letter segment is enough to clear all those rules.
    let mut path = Vec::with_capacity(length);
    path.push(b'L');
    path.push(b'/');
    path.extend(std::iter::repeat_n(b'x', length - 2));
    String::from_utf8(path).expect("ASCII path is valid UTF-8")
}

/// Catalog entries for this module's document fixtures.
///
/// Archive fixtures are catalogued separately through
/// [`archive_catalog_entries`] so the catalog can distinguish them by
/// kind; the requirement-completeness scan enumerates both.
#[must_use]
pub fn catalog_entries() -> Vec<crate::catalog::CatalogEntry> {
    let mut entries: Vec<crate::catalog::CatalogEntry> = fixtures()
        .iter()
        .map(crate::catalog::CatalogEntry::from_document)
        .collect();
    entries.extend(archive_catalog_entries());
    entries
}

/// Catalog entries for this module's archive fixtures.
#[must_use]
pub fn archive_catalog_entries() -> Vec<crate::catalog::CatalogEntry> {
    archive_fixtures()
        .iter()
        .map(crate::catalog::CatalogEntry::from_fixture)
        .collect()
}

// =======================================================================
// Unit tests for T1415 budget boundaries (driven by `run_document_fixture`)
// =======================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::run_document_fixture;

    /// Every budget pin matches its implementation constant. A silent
    /// budget change elsewhere in the workspace must surface here. The
    /// pinned value is a **spec literal** (not an alias of the
    /// implementation constant), so this assertion is non-vacuous:
    /// raising the implementation limit without updating the pin makes
    /// the test fail loudly.
    #[test]
    fn pinned_budgets_match_implementation_constants() {
        assert_eq!(MAX_NESTING_DEPTH, elang_eld::when::MAX_NESTING_DEPTH);
        assert_eq!(MAX_PREDICATES_PER_WHEN, elang_eld::MAX_PREDICATES_PER_WHEN);
        assert_eq!(MAX_EPF_BYTES, elang_core::limits::MAX_EPF_BYTES);
        assert_eq!(
            MAX_BOOTSTRAP_RESOURCE_BYTES,
            elang_core::limits::MAX_BOOTSTRAP_RESOURCE_BYTES
        );
        assert_eq!(
            MAX_RESOURCE_PATH_BYTES,
            elang_core::limits::MAX_RESOURCE_PATH_BYTES as u64
        );
    }

    /// The at-budget and over-budget nesting fixtures are built at the
    /// depths they *actually contain* — the assertion is on the parsed
    /// AST (at-limit) and the raw KDL walk (over-limit), not on a bare
    /// constant, so a builder regression that opens the wrong number of
    /// `group` blocks is caught even when the parser still rejects the
    /// over-limit fixture.
    #[test]
    fn nesting_fixtures_sit_on_both_sides_of_the_boundary() {
        // At-budget: parse succeeds; the production parser reports the
        // exact depth of the condition tree it accepted.
        let at_limit = eld_with_nested_when(MAX_NESTING_DEPTH);
        assert_eq!(
            parsed_when_depth(&at_limit),
            Some(MAX_NESTING_DEPTH),
            "at-limit fixture's parsed AST must have max nesting depth == MAX_NESTING_DEPTH"
        );

        // Over-budget: parse fails with WHEN_TOO_DEEP (the budget
        // fires); the KDL walker measures the would-be depth of the
        // tree the parser refused to build. With MAX_NESTING_DEPTH + 1
        // nested `group` blocks in the source, that depth is exactly
        // MAX_NESTING_DEPTH + 1.
        let over_limit = eld_with_nested_when(MAX_NESTING_DEPTH + 1);
        assert_eq!(
            parsed_when_depth(&over_limit),
            None,
            "over-limit fixture must be rejected by the parser"
        );
        assert_eq!(
            would_be_when_depth(&over_limit),
            MAX_NESTING_DEPTH + 1,
            "over-limit fixture's would-be AST depth must be exactly one past the limit"
        );
    }

    #[test]
    fn every_document_budget_fixture_passes() {
        let fixtures = fixtures();
        assert!(!fixtures.is_empty(), "expected document budget fixtures");
        for fixture in &fixtures {
            run_document_fixture(fixture).unwrap_or_else(|f| panic!("{f}"));
        }
    }

    /// The bootstrap-at-limit payload is exactly `MAX_BOOTSTRAP_RESOURCE_BYTES`
    /// bytes and the over-limit payload is exactly one byte past it — a
    /// hand-coded length regression in the builder surfaces here before
    /// the byte-level conformance run.
    #[test]
    fn bootstrap_payloads_sit_on_both_sides_of_the_boundary() {
        use crate::corpus::{observe_fixture, run_fixture};

        let pair = archive_fixtures()
            .into_iter()
            .filter(|f| {
                matches!(
                    f.id,
                    "budget/bootstrap-at-limit" | "budget/bootstrap-over-limit"
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(pair.len(), 2);

        for fixture in &pair {
            let observation = observe_fixture(fixture).unwrap_or_else(|f| panic!("{f}"));
            assert_eq!(
                observation.stage, 8,
                "fixture `{}` is a bootstrap error and must surface at stage 8",
                fixture.id
            );
        }
        run_fixture(&pair[0]).unwrap_or_else(|f| panic!("{f}"));
        run_fixture(&pair[1]).unwrap_or_else(|f| panic!("{f}"));
    }

    /// The resource-path pair is the boundary proof: 1024 bytes accepted
    /// at Inspect, 1025 bytes rejected with `PATH_TOO_LONG` at the same
    /// level. Run the pair end-to-end so a regression in the helper
    /// bridge (frame size, path construction) surfaces here rather than
    /// in the aggregate corpus gate.
    #[test]
    fn resource_path_fixtures_sit_on_both_sides_of_the_boundary() {
        use crate::corpus::run_fixture;

        let path_1024 = build_path_with_length(1024, b"L");
        assert_eq!(path_1024.len(), 1024);
        let path_1025 = build_path_with_length(1025, b"L");
        assert_eq!(path_1025.len(), 1025);

        let pair = archive_fixtures()
            .into_iter()
            .filter(|f| {
                matches!(
                    f.id,
                    "budget/resource-path-at-limit" | "budget/resource-path-over-limit"
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(pair.len(), 2);
        for fixture in &pair {
            run_fixture(fixture).unwrap_or_else(|f| panic!("{f}"));
        }
    }

    /// `build_path_with_length` is the only place that synthesises
    /// arbitrary-length resource paths; a bug here would silently weaken
    /// every boundary fixture built on top of it. Pin its invariants.
    #[test]
    fn build_path_with_length_satisfies_resource_path_rules() {
        for length in [3usize, 8, 16, 64, 1024] {
            let p = build_path_with_length(length, b"L");
            assert_eq!(p.len(), length);
            assert!(
                !p.starts_with('/'),
                "must be package-relative (no leading `/`), got {p:?}"
            );
            assert!(
                elang_core::ResourcePath::new(p.clone()).is_ok(),
                "length {length} produced an invalid ResourcePath {p:?}",
            );
        }
    }
}
