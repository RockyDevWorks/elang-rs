//! The negative-fixture corpus (T1410–T1414).
//!
//! Every fixture here is an archive that **must be rejected** at a declared
//! reader level with a declared diagnostic code. The corpus's anti-laundering
//! rule (see [`crate::corpus`]) means each fixture is checked twice: the
//! lower levels must *accept* it, and the declared level must *reject* it
//! with the exact code.
//!
//! # How the fixtures are built
//!
//! Negative fixtures are produced by **mutating a known-good archive** or by
//! **assembling one from parts**. Two families appear:
//!
//! * *Byte-level mutations* — flip the header magic, truncate the archive,
//!   corrupt the footer's index hash, and so on. These target stages 1–7.
//! * *Content substitutions* — rebuild the archive with a malformed EPF,
//!   ELD or integrity manifest. These target stages 9–12.
//!
//! Mutations are applied to the real writer output, so the resulting
//! fixture differs from a valid archive in exactly the way the fixture
//! name claims — no incidental differences that could mask the real
//! behaviour.
//!
//! # Stage expectations
//!
//! Container-layer diagnostics carry an explicit stage (the reader attaches
//! it in `container_parse_err`). Stage 8–12 diagnostics carry the stage via
//! the upstream `*_err_at` helpers. So every expectation in this module
//! names a stage the reader actually reports, not one the fixture author
//! hoped for.

use elang_container::footer::FOOTER_LEN;
use elang_core::error::Category;
use elang_writer::{write_archive, ExtraResource, PackageConfig};

use crate::corpus::{Expectation, Fixture, Level};
use crate::golden_packages;

/// The path of the integrity manifest inside a package.
const INTEGRITY_PATH: &str = "META-INF/integrity.kdl";

/// The byte offset of the header magic within the archive.
const MAGIC_OFFSET: usize = 0;

/// The byte offset of the header's `major` version field.
const MAJOR_OFFSET: usize = 8;

/// The byte offset of the header's `flags` field.
const FLAGS_OFFSET: usize = 12;

/// The byte offset of the header's 16 reserved bytes.
const RESERVED_OFFSET: usize = 16;

/// The byte offset of the footer within an archive of `len` bytes.
///
/// The footer is the archive's last [`FOOTER_LEN`] bytes. The subtraction
/// is checked: an archive shorter than one footer cannot exist, and an
/// unchecked `len - FOOTER_LEN` would panic on an arithmetic underflow
/// instead of reporting the authoring bug. Using the published constant
/// rather than a literal `64` keeps the offsets correct if the footer
/// layout ever changes.
fn footer_offset(len: usize) -> usize {
    len.checked_sub(FOOTER_LEN)
        .expect("an archive is always at least one footer long")
}

/// The byte offset of the footer magic within the archive.
fn footer_magic_offset(len: usize) -> usize {
    footer_offset(len)
}

/// The byte offset of the footer's `index_offset` field.
fn footer_index_offset_offset(len: usize) -> usize {
    footer_offset(len) + 8
}

/// The byte offset of the footer's `index_size` field.
fn footer_index_size_offset(len: usize) -> usize {
    footer_offset(len) + 16
}

/// The byte offset of the footer's 8 reserved bytes.
fn footer_reserved_offset(len: usize) -> usize {
    footer_offset(len) + 56
}

/// Build the writer configuration for the minimal package, with optional
/// extra resources.
///
/// # Errors
///
/// Returns a human-readable message when the canonical inputs fail to
/// parse.
fn minimal_config_with_extras(
    extra_resources: Vec<ExtraResource>,
) -> Result<PackageConfig, String> {
    let epf =
        elang_epf::parse(golden_packages::MINIMAL_EPF).map_err(|e| format!("minimal EPF: {e}"))?;
    let rep = elang_repertoire::repertoire::load().map_err(|e| format!("repertoire: {e:?}"))?;
    let doc = elang_eld::parse(golden_packages::MINIMAL_ELD, &rep)
        .map_err(|e| format!("minimal ELD: {e}"))?;
    let eld = elang_eld::inventory::from_document(&doc, &rep)
        .map_err(|e| format!("minimal ELD inventory: {e}"))?;
    Ok(PackageConfig {
        epf,
        eld,
        extra_resources,
        profile: crate::golden_packages::GOLDEN_PROFILE,
        format_version: golden_packages::FORMAT_VERSION.to_string(),
    })
}

/// Build the minimal archive.
///
/// # Panics
///
/// Panics if the writer rejects the canonical minimal package. That is a
/// corpus bug, not a runtime condition: an empty byte vector would still
/// be rejected by the reader, but it would be rejected for the WRONG
/// reason (`ARCHIVE_TOO_SMALL` rather than the mutation the fixture
/// claims to test), silently making the fixture vacuous. Failing loudly
/// at authoring time keeps the fixture honest.
fn minimal_archive() -> Vec<u8> {
    let config = minimal_config_with_extras(Vec::new())
        .expect("the canonical minimal package must be buildable");
    write_archive(&config).expect("the writer must accept the canonical minimal package")
}

/// Build a hand-rolled archive whose index is supplied by `build_index`.
///
/// The caller controls the index byte sequence in full — including any
/// broken property the fixture wants to exercise — and this helper is
/// responsible for the surrounding envelope: a single Zstd frame, an
/// `Header::current`, and a footer whose `index_sha256` matches the
/// bytes it stores. With a matching hash the reader's hash pre-check
/// passes and the broken property is what reaches the index decoder.
///
/// This is the only way the conformance crate can exercise reader-side
/// index checks: the writer's own `IndexEntry::new` pre-check rejects
/// non-UTF-8 / non-NFC / unsorted / duplicate paths before any byte is
/// emitted, so a fixture that targets the *reader* side has to forge the
/// index bytes itself.
///
/// # Errors
///
/// Panics when frame encoding, header encoding, or footer construction
/// fails — these are fixture-authoring bugs, and a silently-skipped
/// fixture would hide a gap in the corpus.
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helper assembles a hand-rolled archive with bounded arithmetic"
)]
pub(crate) fn archive_with_custom_index<F>(build_index: F) -> Vec<u8>
where
    F: FnOnce(u64) -> Vec<u8>,
{
    use elang_container::footer::Footer;
    use elang_container::header::{Header, HEADER_LEN};
    use sha2::Digest;

    let payload = b"x\n";
    let frame = elang_container::frame::encode(payload, crate::golden_packages::GOLDEN_PROFILE)
        .expect("fixture frame encodes");
    let frame_offset = HEADER_LEN as u64;
    let index_bytes = build_index(frame_offset);
    let index_hash: [u8; 32] = sha2::Sha256::digest(&index_bytes).into();
    let index_size = index_bytes.len() as u64;
    let footer = Footer::new(
        frame_offset.saturating_add(frame.len() as u64),
        index_size,
        index_hash,
    )
    .expect("fixture footer constructs");

    let mut out = Vec::new();
    let header = Header::current();
    let mut header_buf = [0u8; HEADER_LEN];
    header
        .encode(&mut header_buf, 0)
        .expect("fixture header encodes");
    out.extend_from_slice(&header_buf);
    out.extend_from_slice(&frame);
    out.extend_from_slice(&index_bytes);
    let mut footer_buf = [0u8; 64];
    footer
        .encode(&mut footer_buf, 0)
        .expect("fixture footer encodes");
    out.extend_from_slice(&footer_buf);
    out
}

/// Encode one index entry's bytes in the same wire format the container's
/// `Index::encode` uses (T0310).
pub(crate) fn encode_index_entry(
    out: &mut Vec<u8>,
    path: &[u8],
    frame_offset: u64,
    compressed_size: u64,
    uncompressed_size: u64,
    compression: u8,
) {
    let path_len = u16::try_from(path.len()).expect("test path fits u16");
    out.extend_from_slice(&path_len.to_le_bytes());
    out.extend_from_slice(path);
    out.extend_from_slice(&frame_offset.to_le_bytes());
    out.extend_from_slice(&compressed_size.to_le_bytes());
    out.extend_from_slice(&uncompressed_size.to_le_bytes());
    out.push(compression);
}

/// Set one byte in a buffer, ignoring out-of-range offsets.
///
/// Out-of-range offsets are a fixture-authoring bug; returning `false`
/// lets the caller assert rather than silently corrupting the wrong byte.
fn poke(bytes: &mut [u8], offset: usize, value: u8) -> bool {
    match bytes.get_mut(offset) {
        Some(slot) => {
            *slot = value;
            true
        }
        None => false,
    }
}

/// Mutate one byte of an archive, failing loudly if the offset is out of
/// range.
///
/// [`poke`] reports an out-of-range offset by returning `false` instead
/// of panicking. A fixture that ignored that return value would leave
/// the archive byte-valid: the mutation would silently not happen, and
/// the fixture would either degenerate into an accept case or trip an
/// earlier check — with no signal at the mutation site. Every fixture
/// mutation goes through this wrapper, so a stale offset fails the
/// build instead of quietly weakening the corpus.
fn poke_at(bytes: &mut [u8], offset: usize, value: u8) {
    assert!(
        poke(bytes, offset, value),
        "mutation offset {offset} is out of range for a {}-byte archive",
        bytes.len()
    );
}

// ---------------------------------------------------------------------------
// T1410 — malformed binary container fixtures
// ---------------------------------------------------------------------------

/// The T1410 fixtures: malformed index / footer / path / truncation /
/// overflow containers.
pub fn binary_fixtures() -> Vec<Fixture> {
    let mut fixtures = Vec::new();

    // --- header --------------------------------------------------------
    let mut bad_magic = minimal_archive();
    poke_at(&mut bad_magic, MAGIC_OFFSET, 0x00);
    fixtures.push(Fixture {
        id: "binary/header-bad-magic",
        description: "The 8-byte header magic is corrupted (stage 1).",
        requirements: &["REQ-BIN-001", "REQ-BIN-002"],
        expectation: Expectation::Reject {
            level: Level::Inspect,
            code: "HEADER_BAD_MAGIC",
            category: Category::Container,
        },
        archive: bad_magic,
    });

    let mut bad_version = minimal_archive();
    poke_at(&mut bad_version, MAJOR_OFFSET, 0xFF);
    fixtures.push(Fixture {
        id: "binary/header-bad-version",
        description: "The header's major version is unsupported (stage 2).",
        requirements: &["REQ-BIN-002", "REQ-BIN-018"],
        expectation: Expectation::Reject {
            level: Level::Inspect,
            code: "HEADER_BAD_VERSION",
            category: Category::Container,
        },
        archive: bad_version,
    });

    let mut nonzero_flags = minimal_archive();
    poke_at(&mut nonzero_flags, FLAGS_OFFSET, 0x01);
    fixtures.push(Fixture {
        id: "binary/header-nonzero-flags",
        description: "The header's flags field must be zero in v0.1.0 (stage 2).",
        requirements: &["REQ-BIN-002", "REQ-BIN-018"],
        expectation: Expectation::Reject {
            level: Level::Inspect,
            code: "HEADER_NONZERO_FLAGS",
            category: Category::Container,
        },
        archive: nonzero_flags,
    });

    let mut nonzero_reserved = minimal_archive();
    poke_at(&mut nonzero_reserved, RESERVED_OFFSET, 0x01);
    fixtures.push(Fixture {
        id: "binary/header-reserved-nonzero",
        description: "The header's 16 reserved bytes must be zero (stage 2).",
        requirements: &["REQ-BIN-002", "REQ-BIN-018"],
        expectation: Expectation::Reject {
            level: Level::Inspect,
            code: "HEADER_RESERVED_NONZERO",
            category: Category::Container,
        },
        archive: nonzero_reserved,
    });

    // --- footer --------------------------------------------------------
    let mut bad_footer_magic = minimal_archive();
    let footer_at = footer_magic_offset(bad_footer_magic.len());
    poke_at(&mut bad_footer_magic, footer_at, 0x00);
    fixtures.push(Fixture {
        id: "binary/footer-bad-magic",
        description: "The 8-byte footer magic is corrupted (stage 3).",
        requirements: &["REQ-BIN-004", "REQ-BIN-005", "REQ-BIN-006"],
        expectation: Expectation::Reject {
            level: Level::Inspect,
            code: "FOOTER_BAD_MAGIC",
            category: Category::Container,
        },
        archive: bad_footer_magic,
    });

    let mut bad_footer_reserved = minimal_archive();
    let reserved_at = footer_reserved_offset(bad_footer_reserved.len());
    poke_at(&mut bad_footer_reserved, reserved_at, 0x01);
    fixtures.push(Fixture {
        id: "binary/footer-reserved-nonzero",
        description: "The footer's 8 reserved bytes must be zero (stage 3).",
        requirements: &["REQ-BIN-004", "REQ-BIN-005", "REQ-BIN-006"],
        expectation: Expectation::Reject {
            level: Level::Inspect,
            code: "FOOTER_RESERVED_NONZERO",
            category: Category::Container,
        },
        archive: bad_footer_reserved,
    });

    let mut bad_footer_offset = minimal_archive();
    let offset_at = footer_index_offset_offset(bad_footer_offset.len());
    poke_at(&mut bad_footer_offset, offset_at, 0xFF);
    fixtures.push(Fixture {
        id: "binary/footer-offset-inconsistent",
        description: "The footer's index offset disagrees with the archive layout (stage 1).",
        requirements: &["REQ-BIN-004", "REQ-BIN-005", "REQ-BIN-006"],
        expectation: Expectation::Reject {
            level: Level::Inspect,
            code: "ARCHIVE_SIZE_INCONSISTENT",
            category: Category::Container,
        },
        archive: bad_footer_offset,
    });

    let mut bad_footer_size = minimal_archive();
    let size_at = footer_index_size_offset(bad_footer_size.len());
    poke_at(&mut bad_footer_size, size_at, 0xFF);
    fixtures.push(Fixture {
        id: "binary/footer-index-size-inconsistent",
        description: "The footer's index size disagrees with the archive layout (stage 1).",
        requirements: &["REQ-BIN-004", "REQ-BIN-005", "REQ-BIN-006"],
        expectation: Expectation::Reject {
            level: Level::Inspect,
            code: "ARCHIVE_SIZE_INCONSISTENT",
            category: Category::Container,
        },
        archive: bad_footer_size,
    });

    // --- truncation ----------------------------------------------------
    let truncated = minimal_archive();
    let mut truncated = truncated;
    truncated.truncate(truncated.len() / 2);
    fixtures.push(Fixture {
        id: "binary/truncated-container",
        description:
            "The archive is truncated to half its length — footer magic corrupted (stage 3).",
        requirements: &["REQ-BIN-001", "REQ-BIN-002"],
        expectation: Expectation::Reject {
            level: Level::Inspect,
            code: "FOOTER_BAD_MAGIC",
            category: Category::Container,
        },
        archive: truncated,
    });

    let mut too_small = minimal_archive();
    // Keep the 32-byte header intact so the header decode passes, then cut
    // the archive below the minimum (header + footer = 96 bytes). The
    // reader's size pre-check then fires with `ARCHIVE_TOO_SMALL`.
    too_small.truncate(64);
    fixtures.push(Fixture {
        id: "binary/archive-too-small",
        description: "A 64-byte archive is below the minimum container size (stage 1).",
        requirements: &["REQ-BIN-001", "REQ-BIN-002"],
        expectation: Expectation::Reject {
            level: Level::Inspect,
            code: "ARCHIVE_TOO_SMALL",
            category: Category::Container,
        },
        archive: too_small,
    });

    // --- resource path -------------------------------------------------
    // The path-NFC rule has two enforcement points: the writer's
    // `IndexEntry::new` pre-check (writer tests cover that), and the
    // reader's `Index::decode` mirror (this fixture exercises the reader
    // side). The archive is hand-rolled because the writer's pre-check
    // would otherwise prevent the path from reaching the on-disk form.
    let non_nfc = archive_with_custom_index(|frame_offset| {
        use elang_container::index::{COMPRESSION_ZSTD, ENTRY_FORMAT, INDEX_MAGIC, INDEX_VERSION};
        // Decomposed "café" — the combining acute accent follows an `e`
        // that has a precomposed form (U+00E9). The reader's index
        // decoder must reject this with `PATH_NOT_NFC`.
        let path: String = String::from("Linguistics/caf") + "e\u{0301}" + ".txt";
        let mut idx = Vec::new();
        idx.extend_from_slice(&INDEX_MAGIC);
        idx.extend_from_slice(&INDEX_VERSION.to_le_bytes());
        idx.extend_from_slice(&ENTRY_FORMAT.to_le_bytes());
        idx.extend_from_slice(&1u32.to_le_bytes());
        encode_index_entry(
            &mut idx,
            path.as_bytes(),
            frame_offset,
            2,
            2,
            COMPRESSION_ZSTD,
        );
        idx
    });
    fixtures.push(Fixture {
        id: "binary/resource-path-not-nfc",
        description: "An indexed resource path is not in Unicode NFC (stage 5).",
        requirements: &["REQ-BIN-011", "REQ-BIN-012"],
        expectation: Expectation::Reject {
            level: Level::Inspect,
            code: "PATH_NOT_NFC",
            category: Category::ResourcePath,
        },
        archive: non_nfc,
    });

    // --- index parse ---------------------------------------------------
    // The index magic check only fires after the footer hash pre-check,
    // so the fixture has to carry a *correct* SHA-256 over its broken
    // index bytes. `archive_with_custom_index` computes the hash from
    // the bytes the caller supplies, then patches it into the footer.
    let bad_index_magic = archive_with_custom_index(|_| {
        // Replace the leading 8 bytes with non-magic content; the hash is
        // computed over the resulting byte sequence, so the footer hash
        // matches and the reader reaches `Index::decode`. The index must
        // still be at least `MIN_INDEX_SIZE` (16) bytes for the footer to
        // construct, so pad the broken header out to 16 bytes.
        let mut idx = vec![0xDEu8, 0xAD, 0xBE, 0xEF, 0x00, 0x00, 0x00, 0x00];
        idx.resize(16, 0x00);
        idx
    });
    fixtures.push(Fixture {
        id: "binary/index-bad-magic",
        description: "The 8-byte index magic is corrupted (stage 5).",
        requirements: &["REQ-BIN-009"],
        expectation: Expectation::Reject {
            level: Level::Inspect,
            code: "INDEX_BAD_MAGIC",
            category: Category::Index,
        },
        archive: bad_index_magic,
    });

    // Index version field set to an unsupported value.
    let bad_index_version = archive_with_custom_index(|_| {
        use elang_container::index::{ENTRY_FORMAT, INDEX_MAGIC};
        let mut idx = Vec::new();
        idx.extend_from_slice(&INDEX_MAGIC);
        // Index version = 0xFFFF, entry format still 1, zero entries. The
        // reader must reject this with `INDEX_BAD_VERSION`.
        idx.extend_from_slice(&0xFFFFu16.to_le_bytes());
        idx.extend_from_slice(&ENTRY_FORMAT.to_le_bytes());
        idx.extend_from_slice(&0u32.to_le_bytes());
        idx
    });
    fixtures.push(Fixture {
        id: "binary/index-bad-version",
        description: "The index version field is unsupported (stage 5).",
        requirements: &["REQ-BIN-009"],
        expectation: Expectation::Reject {
            level: Level::Inspect,
            code: "INDEX_BAD_VERSION",
            category: Category::Index,
        },
        archive: bad_index_version,
    });

    // An index entry whose frame end cannot be represented: the frame
    // offset plus the compressed size overflows u64.
    //
    // The overflow guard itself was only reachable through unit tests in
    // `elang-container` (`rejects_a_frame_end_that_overflows_u64`,
    // `rejects_a_first_frame_offset_at_u64_max`). The corpus published
    // invalid-offset fixtures but no integer-overflow fixture, so a
    // second implementation reading the corpus had no way to learn that
    // the frame end must be computed with checked arithmetic and
    // rejected rather than allowed to wrap.
    //
    // `compressed_size = u64::MAX` makes `offset + size` wrap. The layout
    // validator checks the overflow *before* the contiguity comparison, so
    // the diagnostic is the precise one: the frame describes a range that
    // cannot exist.
    let overflowing_frame_end = archive_with_custom_index(|frame_offset| {
        use elang_container::index::{COMPRESSION_ZSTD, ENTRY_FORMAT, INDEX_MAGIC, INDEX_VERSION};
        let mut idx = Vec::new();
        idx.extend_from_slice(&INDEX_MAGIC);
        idx.extend_from_slice(&INDEX_VERSION.to_le_bytes());
        idx.extend_from_slice(&ENTRY_FORMAT.to_le_bytes());
        idx.extend_from_slice(&1u32.to_le_bytes());
        encode_index_entry(
            &mut idx,
            b"Linguistics/phonemes.eld",
            frame_offset,
            u64::MAX,
            2,
            COMPRESSION_ZSTD,
        );
        idx
    });
    fixtures.push(Fixture {
        id: "binary/index-frame-end-overflows-u64",
        description:
            "An index entry whose frame offset plus compressed size overflows u64 (stage 5).",
        requirements: &["REQ-BIN-010", "REQ-BIN-005"],
        expectation: Expectation::Reject {
            level: Level::Inspect,
            code: "INDEX_ENTRY_MALFORMED",
            category: Category::Index,
        },
        archive: overflowing_frame_end,
    });

    // A non-UTF-8 byte in the path field. The footer hash matches the
    // (deliberately broken) bytes; the reader's index decoder is the
    // first place to detect it.
    let non_utf8_path = archive_with_custom_index(|frame_offset| {
        use elang_container::index::{COMPRESSION_ZSTD, ENTRY_FORMAT, INDEX_MAGIC, INDEX_VERSION};
        let mut idx = Vec::new();
        idx.extend_from_slice(&INDEX_MAGIC);
        idx.extend_from_slice(&INDEX_VERSION.to_le_bytes());
        idx.extend_from_slice(&ENTRY_FORMAT.to_le_bytes());
        idx.extend_from_slice(&1u32.to_le_bytes());
        // path: "Linguistics/caf\xFF.txt" — the 0xFF makes it non-UTF-8.
        let mut path = b"Linguistics/caf".to_vec();
        path.push(0xFF);
        path.extend_from_slice(b".txt");
        encode_index_entry(&mut idx, &path, frame_offset, 2, 2, COMPRESSION_ZSTD);
        idx
    });
    fixtures.push(Fixture {
        id: "binary/index-path-not-utf8",
        description: "An indexed resource path is not valid UTF-8 (stage 5).",
        requirements: &["REQ-BIN-011", "REQ-BIN-012"],
        expectation: Expectation::Reject {
            level: Level::Inspect,
            code: "PATH_NOT_UTF8",
            category: Category::ResourcePath,
        },
        archive: non_utf8_path,
    });

    fixtures
}

// ---------------------------------------------------------------------------
// T1412 — resource integrity negatives
// ---------------------------------------------------------------------------

/// The T1412 fixtures: integrity manifest omission, corruption and
/// extra-entry cases.
///
/// These are built by assembling an archive whose `META-INF/integrity.kdl`
/// resource content is deliberately wrong. The writer computes the manifest
/// itself, so the fixture is built by writing a valid archive, then
/// rebuilding one with a hand-authored manifest that disagrees with the
/// resources it indexes.
pub fn integrity_fixtures() -> Vec<Fixture> {
    let mut fixtures = Vec::new();

    // A manifest whose digests are wrong for every entry: the archive is
    // structurally perfect, the manifest covers every indexed resource,
    // and the SHA-256 digests disagree with the resource bytes.
    // `INTEGRITY_HASH_MISMATCH` must be reported by the verify stage.
    let bytes = rebuild_with_resource(
        "META-INF/integrity.kdl",
        b"integrity {\n    algorithm \"sha-256\"\n    resource \"Header/format.version\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"\n    resource \"Header/mimetype\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"\n    resource \"Linguistics/phonemes.eld\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"\n    resource \"package.epf\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"\n}\n",
    );
    fixtures.push(Fixture {
        id: "integrity/digest-mismatch",
        description: "Every manifest digest disagrees with the resource's bytes (stage 11).",
        requirements: &["REQ-INTEGRITY-001"],
        expectation: Expectation::Reject {
            level: Level::Verify,
            code: "INTEGRITY_HASH_MISMATCH",
            category: Category::Integrity,
        },
        archive: bytes,
    });

    // A manifest that lists one resource but omits the other three indexed
    // ones. The structure parse succeeds (at least one `resource` child),
    // and the coverage check reports the first omitted path.
    let bytes = rebuild_with_resource(
        "META-INF/integrity.kdl",
        b"integrity {\n    algorithm \"sha-256\"\n    resource \"Header/mimetype\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"\n}\n",
    );
    fixtures.push(Fixture {
        id: "integrity/missing-entry",
        description: "The manifest omits indexed resources (stage 9).",
        requirements: &["REQ-INTEGRITY-002"],
        expectation: Expectation::Reject {
            level: Level::Verify,
            code: "INTEGRITY_MISSING_ENTRY",
            category: Category::Integrity,
        },
        archive: bytes,
    });

    // A manifest that carries an entry for a resource the index does not
    // list, in addition to covering every indexed resource. The coverage
    // check runs its "missing" sweep before its "extra" sweep, so the
    // manifest must be complete on the indexed side for the extra entry
    // to be the reported failure.
    let bytes = rebuild_with_resource(
        "META-INF/integrity.kdl",
        b"integrity {\n    algorithm \"sha-256\"\n    resource \"Header/format.version\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"\n    resource \"Header/mimetype\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"\n    resource \"Linguistics/phonemes.eld\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"\n    resource \"package.epf\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"\n    resource \"Linguistics/ghost.eld\" sha256=\"0000000000000000000000000000000000000000000000000000000000000000\"\n}\n",
    );
    fixtures.push(Fixture {
        id: "integrity/extra-entry",
        description: "The manifest carries an entry the index does not list (stage 9).",
        requirements: &["REQ-INTEGRITY-002"],
        expectation: Expectation::Reject {
            level: Level::Verify,
            code: "INTEGRITY_EXTRA_ENTRY",
            category: Category::Integrity,
        },
        archive: bytes,
    });

    // A manifest naming an unsupported algorithm.
    let bytes = rebuild_with_resource(
        "META-INF/integrity.kdl",
        b"integrity {\n    algorithm \"md5\"\n}\n",
    );
    fixtures.push(Fixture {
        id: "integrity/bad-algorithm",
        description: "The manifest names an unsupported digest algorithm (stage 9).",
        requirements: &["REQ-INTEGRITY-003"],
        expectation: Expectation::Reject {
            level: Level::Verify,
            code: "INTEGRITY_BAD_ALGORITHM",
            category: Category::Integrity,
        },
        archive: bytes,
    });

    fixtures
}

// ---------------------------------------------------------------------------
// Corpus assembly
// ---------------------------------------------------------------------------

/// Every negative fixture in this module, in corpus order.
///
/// The module publishes three families — the malformed-container fixtures
/// ([`binary_fixtures`]), the resource-integrity fixtures
/// ([`integrity_fixtures`]) and the stage-12 validate fixtures
/// ([`validate_negatives`]) — so this function concatenates them into
/// the single list the runner and the catalog enumerate.
#[must_use]
pub fn fixtures() -> Vec<Fixture> {
    let mut fixtures = binary_fixtures();
    fixtures.extend(integrity_fixtures());
    fixtures.extend(validate_negatives());
    fixtures
}

/// Catalog entries for this module's fixtures.
///
/// Every fixture published by this module appears here exactly once, so
/// the corpus-wide requirement-completeness test can enumerate them.
#[must_use]
pub fn catalog_entries() -> Vec<crate::catalog::CatalogEntry> {
    fixtures()
        .iter()
        .map(crate::catalog::CatalogEntry::from_fixture)
        .collect()
}

/// An ELD whose `when` block references an undeclared phoneme.
///
/// The document is structurally well-formed: it parses as an ELD, its
/// EPF still points at it, and the integrity manifest still matches its
/// bytes. The only thing wrong with it is semantic — the `when` block
/// names a phoneme the document never declares.
///
/// "z" is a valid base IPA symbol, so it clears Phase 8's IPA validation
/// and fails precisely as an *unresolved reference*, not as a malformed
/// identifier. That distinction matters: a malformed identifier would be
/// rejected by the structural parser, and this fixture is meant to prove
/// the *reference resolver* is reached and does its job.
const ELD_WITH_UNRESOLVED_REFERENCE: &[u8] = b"phonemes {\n\
    consonants {\n\
        phoneme \"p\" {\n\
            roman \"p\"\n\
            when {\n\
                word-position \"initial\"\n\
                previous-phoneme \"z\"\n\
            }\n\
        }\n\
    }\n\
    vowels {\n\
        phoneme \"a\" {\n\
            roman \"a\"\n\
        }\n\
    }\n\
}\n";

/// Stage-12 (`validate`) negative fixtures.
///
/// The corpus published no fixture rejected at `Level::Validate`. Every
/// negative archive it carried was rejected at `Level::Inspect` or
/// `Level::Verify`, so a reader that implemented stages 1–11 and then
/// stubbed out stage 12 would still have passed the entire corpus. The
/// `validate` acceptance criterion could not be falsified by the corpus
/// because the corpus contained nothing for it to falsify.
///
/// These fixtures close that hole. The archive is *structurally perfect*
/// and *digest-correct* — it passes `inspect` and `verify` in full — and
/// only the semantic content is wrong. Because the fixture is declared at
/// `Level::Validate`, the runner also proves the anti-laundering property
/// in the other direction: the lower levels must **accept** it, so the
/// rejection genuinely comes from stage 12 and not from an earlier stage
/// that happens to trip over the same bytes.
#[must_use]
pub fn validate_negatives() -> Vec<Fixture> {
    let archive = rebuild_with_valid_integrity(&[(
        "Linguistics/phonemes.eld",
        ELD_WITH_UNRESOLVED_REFERENCE,
    )]);

    vec![Fixture {
        id: "reader/eld-unresolved-when-reference",
        description:
            "A structurally valid package whose ELD `when` block references an undeclared phoneme (stage 12).",
        requirements: &["REQ-CONF-003", "REQ-READER-002", "REQ-READER-005"],
        expectation: Expectation::Reject {
            level: Level::Validate,
            code: "WHEN_REFERENCE_UNRESOLVED",
            category: Category::When,
        },
        archive,
    }]
}

// ---------------------------------------------------------------------------
// Assembly helper for content-substitution fixtures
// ---------------------------------------------------------------------------

/// Rebuild the minimal archive with one resource's content replaced.
///
/// The replacement is a *content* change only: the index, the frame
/// offsets, the footer hash and the archive length are all recomputed by
/// the container layer, so the resulting archive is structurally perfect
/// and fails only on the content the fixture targets.
///
/// # Panics
///
/// Panics when the container assembly fails or when `path` is not one of
/// the minimal package's resources. Both are corpus-authoring bugs: an
/// `Option` return would let a caller silently skip the fixture, and a
/// skipped fixture is a vacuous corpus that still looks complete.
pub fn rebuild_with_resource(path: &str, content: &[u8]) -> Vec<u8> {
    use elang_container::footer::Footer;
    use elang_container::frame::encode;
    use elang_container::header::{Header, HEADER_LEN};
    use elang_container::index::{Index, IndexEntry, COMPRESSION_ZSTD};

    // Start from a valid archive and read its resource contents.
    let original = minimal_archive();
    assert!(
        !original.is_empty(),
        "the minimal archive must not be empty"
    );
    let listing = elang_container::ArchiveListing::parse(&original)
        .expect("the minimal archive must parse as an archive listing");
    let view = listing.index();

    // Collect (path, uncompressed bytes) for every indexed resource.
    let mut resources: Vec<(String, Vec<u8>)> = Vec::new();
    for entry in view.entries() {
        let path = entry.path().as_str().to_string();
        let offset = entry.frame_offset() as usize;
        let compressed = entry.compressed_size() as usize;
        let frame = original
            .get(offset..offset + compressed)
            .expect("an indexed frame offset must lie inside the archive");
        let mut budget = elang_container::frame::DecodeBudget::generic();
        let bytes = elang_container::frame::decode(frame, &mut budget)
            .expect("an indexed frame must decode");
        resources.push((path, bytes));
    }

    // Substitute the target resource.
    let mut substituted = false;
    for (p, bytes) in resources.iter_mut() {
        if p == path {
            *bytes = content.to_vec();
            substituted = true;
        }
    }
    assert!(
        substituted,
        "resource {path:?} must exist in the minimal package"
    );

    // Re-assemble: sort, encode, index, footer.
    resources.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let mut frames: Vec<(String, Vec<u8>, u64)> = Vec::with_capacity(resources.len());
    for (p, bytes) in &resources {
        let compressed =
            encode(bytes, crate::golden_packages::GOLDEN_PROFILE).expect("a resource must encode");
        frames.push((p.clone(), compressed, bytes.len() as u64));
    }
    let mut entries: Vec<IndexEntry> = Vec::with_capacity(frames.len());
    let mut offset = HEADER_LEN as u64;
    for (p, compressed, uncompressed_size) in &frames {
        entries.push(
            IndexEntry::new(
                p,
                offset,
                compressed.len() as u64,
                *uncompressed_size,
                COMPRESSION_ZSTD,
            )
            .expect("an index entry must be constructible"),
        );
        offset = offset.saturating_add(compressed.len() as u64);
    }
    let index = Index::with_entries(entries).expect("the index must be constructible");
    let (index_bytes, index_hash) = index.encode_and_hash().expect("the index must encode");
    let index_size = index_bytes.len() as u64;
    let footer =
        Footer::new(offset, index_size, index_hash).expect("the footer must be constructible");

    let mut out = Vec::new();
    let header = Header::current();
    let mut header_buf = [0u8; HEADER_LEN];
    header
        .encode(&mut header_buf, 0)
        .expect("the header must encode");
    out.extend_from_slice(&header_buf);
    for (_, compressed, _) in &frames {
        out.extend_from_slice(compressed);
    }
    out.extend_from_slice(&index_bytes);
    let mut footer_buf = [0u8; 64];
    footer
        .encode(&mut footer_buf, 0)
        .expect("the footer must encode");
    out.extend_from_slice(&footer_buf);
    out
}

/// Rebuild the minimal archive with substituted resources and a
/// *regenerated* integrity manifest.
///
/// [`rebuild_with_resource`] deliberately leaves the integrity manifest
/// stale, because that is exactly what the integrity fixtures need: the
/// digest must stop matching. This helper does the opposite. It
/// recomputes the manifest over the substituted resources using the
/// writer's own scope — every resource except the manifest itself — so
/// the archive still passes stage 11 and any rejection the fixture
/// targets has to come from a *later* stage.
///
/// That distinction is what makes a stage-12 fixture meaningful. Without
/// it, any substituted content is caught at stage 11 as a digest
/// mismatch, the archive never reaches the semantic validator, and the
/// fixture would silently test nothing.
///
/// # Panics
///
/// Panics when the container assembly fails or when a substituted path is
/// not one of the minimal package's resources. Both are corpus-authoring
/// bugs, for the same reason [`rebuild_with_resource`] panics: a silently
/// skipped fixture is a vacuous corpus that still looks complete.
pub fn rebuild_with_valid_integrity(subs: &[(&str, &[u8])]) -> Vec<u8> {
    use elang_container::footer::Footer;
    use elang_container::frame::encode;
    use elang_container::header::{Header, HEADER_LEN};
    use elang_container::index::{Index, IndexEntry, COMPRESSION_ZSTD};
    use elang_integrity::manifest::{Algorithm, IntegrityEntry, IntegrityManifest};
    use sha2::{Digest, Sha256};

    // Start from a valid archive and read its resource contents.
    let original = minimal_archive();
    assert!(
        !original.is_empty(),
        "the minimal archive must not be empty"
    );
    let listing = elang_container::ArchiveListing::parse(&original)
        .expect("the minimal archive must parse as an archive listing");
    let view = listing.index();

    let mut resources: Vec<(String, Vec<u8>)> = Vec::new();
    for entry in view.entries() {
        let path = entry.path().as_str().to_string();
        let offset = entry.frame_offset() as usize;
        let compressed = entry.compressed_size() as usize;
        let frame = original
            .get(offset..offset + compressed)
            .expect("an indexed frame offset must lie inside the archive");
        let mut budget = elang_container::frame::DecodeBudget::generic();
        let bytes = elang_container::frame::decode(frame, &mut budget)
            .expect("an indexed frame must decode");
        resources.push((path, bytes));
    }

    // Substitute the target resources.
    for (path, content) in subs {
        let mut substituted = false;
        for (p, bytes) in resources.iter_mut() {
            if p == path {
                *bytes = content.to_vec();
                substituted = true;
            }
        }
        assert!(
            substituted,
            "resource {path:?} must exist in the minimal package"
        );
    }

    // Regenerate the integrity manifest over every resource *except* the
    // manifest itself — the writer's scope, reproduced exactly so the
    // canonical bytes match byte-for-byte.
    let mut integrity = IntegrityManifest {
        algorithm: Algorithm::Sha256,
        resources: Default::default(),
    };
    for (path, uncompressed) in &resources {
        if path == INTEGRITY_PATH {
            continue;
        }
        let resource_path = elang_core::ResourcePath::new(path.clone())
            .expect("a minimal-package resource path must be valid");
        let digest: [u8; 32] = Sha256::digest(uncompressed.as_slice()).into();
        integrity.resources.insert(
            resource_path.clone(),
            IntegrityEntry {
                path: resource_path,
                digest,
            },
        );
    }
    let integrity_bytes = elang_integrity::write::canonical_bytes(&integrity);
    for (p, bytes) in resources.iter_mut() {
        if p == INTEGRITY_PATH {
            *bytes = integrity_bytes.clone();
        }
    }

    // Re-assemble: sort, encode, index, footer.
    resources.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let mut frames: Vec<(String, Vec<u8>, u64)> = Vec::with_capacity(resources.len());
    for (p, bytes) in &resources {
        let compressed =
            encode(bytes, crate::golden_packages::GOLDEN_PROFILE).expect("a resource must encode");
        frames.push((p.clone(), compressed, bytes.len() as u64));
    }
    let mut entries: Vec<IndexEntry> = Vec::with_capacity(frames.len());
    let mut offset = HEADER_LEN as u64;
    for (p, compressed, uncompressed_size) in &frames {
        entries.push(
            IndexEntry::new(
                p,
                offset,
                compressed.len() as u64,
                *uncompressed_size,
                COMPRESSION_ZSTD,
            )
            .expect("an index entry must be constructible"),
        );
        offset = offset.saturating_add(compressed.len() as u64);
    }
    let index = Index::with_entries(entries).expect("the index must be constructible");
    let (index_bytes, index_hash) = index.encode_and_hash().expect("the index must encode");
    let index_size = index_bytes.len() as u64;
    let footer =
        Footer::new(offset, index_size, index_hash).expect("the footer must be constructible");

    let mut out = Vec::new();
    let header = Header::current();
    let mut header_buf = [0u8; HEADER_LEN];
    header
        .encode(&mut header_buf, 0)
        .expect("the header must encode");
    out.extend_from_slice(&header_buf);
    for (_, compressed, _) in &frames {
        out.extend_from_slice(compressed);
    }
    out.extend_from_slice(&index_bytes);
    let mut footer_buf = [0u8; 64];
    footer
        .encode(&mut footer_buf, 0)
        .expect("the footer must encode");
    out.extend_from_slice(&footer_buf);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::run_fixture;

    #[test]
    fn every_binary_fixture_behaves_as_declared() {
        for fixture in binary_fixtures() {
            run_fixture(&fixture).unwrap_or_else(|f| panic!("{f}"));
        }
    }

    #[test]
    fn every_integrity_fixture_behaves_as_declared() {
        for fixture in integrity_fixtures() {
            run_fixture(&fixture).unwrap_or_else(|f| panic!("{f}"));
        }
    }

    #[test]
    fn every_validate_fixture_behaves_as_declared() {
        for fixture in validate_negatives() {
            run_fixture(&fixture).unwrap_or_else(|f| panic!("{f}"));
        }
    }

    /// The stage-12 fixture must fail for the *semantic* reason it
    /// claims, not because some earlier stage tripped over the substituted
    /// bytes. Pinning the exact code proves the archive reached the
    /// reference resolver with a valid digest manifest.
    #[test]
    fn the_validate_fixture_is_rejected_by_the_reference_resolver() {
        let fixture = &validate_negatives()[0];
        let err = elang_reader::Package::open(fixture.archive.as_slice())
            .expect("the archive must open")
            .validate()
            .expect_err("the archive must fail stage 12");
        assert_eq!(err.diagnostic.code, "WHEN_REFERENCE_UNRESOLVED");
        assert_eq!(err.diagnostic.category, Category::When);
        assert_eq!(err.diagnostic.stage, Some(12));
    }

    #[test]
    fn poke_reports_out_of_range_offsets() {
        let mut buf = [0u8; 4];
        assert!(poke(&mut buf, 0, 0xFF));
        assert_eq!(buf[0], 0xFF);
        assert!(!poke(&mut buf, 99, 0xFF));
    }

    #[test]
    fn poke_at_applies_an_in_range_mutation() {
        let mut buf = [0u8; 4];
        poke_at(&mut buf, 2, 0xFF);
        assert_eq!(buf, [0, 0, 0xFF, 0]);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn poke_at_fails_loudly_on_an_out_of_range_offset() {
        // A stale offset must fail loudly. If it were ignored, the archive
        // would stay byte-valid and the fixture would silently degenerate
        // into an accept case (or trip an earlier check).
        let mut buf = [0u8; 4];
        poke_at(&mut buf, 99, 0xFF);
    }
}
