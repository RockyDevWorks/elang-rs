//! Deterministic builder for the **binary header / index / footer reference
//! vectors** (T0118, open artifact OA-11).
//!
//! This module is the single source of truth for the published golden byte
//! layouts under `fixtures/valid/container/minimal-archive/` and the
//! reference-vectors document `specs/registry/binary-reference-vectors.json`.
//! It serializes the exact little-endian bytes described by steering §3 and
//! computes the **real** SHA-256 of the serialized index bytes — no digest or
//! offset is hand-written anywhere, so the published numbers cannot drift from
//! the bytes they describe.
//!
//! # Why placeholder frame payloads
//!
//! Real Zstandard frames are produced in Phase 4 (T0401–T0419). The *framing*
//! vectors here concern the header, index and footer only, so each resource's
//! frame body is a clearly-labelled deterministic placeholder of a fixed size
//! (ASCII `ELANGFRAME:<path>#<n>` padded with `.`). Offsets and sizes stay
//! internally consistent; nothing claims to be a valid Zstd frame. When Phase 4
//! lands, the same layout math is reused with real frame bytes.

use sha2::{Digest, Sha256};

/// 8-byte header magic: `ELANG\r\n\x1A` (steering §3.1).
pub const HEADER_MAGIC: [u8; 8] = [0x45, 0x4C, 0x41, 0x4E, 0x47, 0x0D, 0x0A, 0x1A];
/// 8-byte index-header magic (steering §3.2).
pub const INDEX_MAGIC: [u8; 8] = *b"ELIDX001";
/// 8-byte footer magic (steering §3.3).
pub const FOOTER_MAGIC: [u8; 8] = *b"ELEND001";

/// Fixed sizes (bytes) from steering §3.
pub const HEADER_LEN: u64 = 32;
/// Index header length (steering §3.2).
pub const INDEX_HEADER_LEN: u64 = 16;
/// Footer length (steering §3.3).
pub const FOOTER_LEN: u64 = 64;
/// Fixed portion of each index entry: `u16 + 3*u64 + u8` (steering §3.2).
pub const ENTRY_FIXED_LEN: u64 = 2 + 8 + 8 + 8 + 1;
/// Zstd compression code (steering §3.2 / §3.6).
pub const COMPRESSION_ZSTD: u8 = 1;

/// One logical resource and its (placeholder) frame payload.
#[derive(Clone)]
pub struct ResourceSpec {
    /// NFC UTF-8, slash-separated, package-relative path (steering §3.4).
    pub path: &'static str,
    /// The *uncompressed* logical bytes this resource represents. For the
    /// framing vectors these are illustrative payloads (e.g. the real
    /// `mimetype` string); the frame body on disk is a placeholder (see below).
    pub uncompressed: Vec<u8>,
    /// The placeholder frame payload actually written to the archive in place
    /// of a real Zstd frame. Deterministic and clearly labelled.
    pub frame_body: Vec<u8>,
}

/// The fully resolved layout of one resource inside the archive.
#[derive(Clone)]
pub struct ResourceLayout {
    /// Resource path.
    pub path: &'static str,
    /// Absolute byte offset of this resource's frame within the archive.
    pub frame_offset: u64,
    /// Compressed (on-disk) frame size. Placeholder frame length here.
    pub compressed_size: u64,
    /// Declared uncompressed size (what a real Zstd frame would expand to).
    pub uncompressed_size: u64,
    /// Compression code (always `1` = Zstd).
    pub compression: u8,
    /// The placeholder frame bytes.
    pub frame_body: Vec<u8>,
}

/// The complete, internally consistent reference archive.
pub struct ReferenceArchive {
    /// 32 header bytes.
    pub header: Vec<u8>,
    /// Per-resource layout in index (sorted) order.
    pub resources: Vec<ResourceLayout>,
    /// Exact serialized index bytes (16-byte header + entries).
    pub index_bytes: Vec<u8>,
    /// 64 footer bytes.
    pub footer: Vec<u8>,
    /// Absolute offset of the index (== end of the last frame).
    pub index_offset: u64,
    /// Length of the serialized index (`index_bytes.len()`).
    pub index_size: u64,
    /// Real SHA-256 of `index_bytes` (32 bytes), as stored in the footer.
    pub index_sha256: [u8; 32],
    /// The whole archive: header || frames || index || footer.
    pub archive: Vec<u8>,
}

/// Build the minimal valid phoneme-only package layout (steering §2).
///
/// Paths are deliberately given out of order to the builder to prove the
/// strict ascending UTF-8 sort is applied by the serializer, not assumed.
pub fn minimal_archive() -> ReferenceArchive {
    // The five logical resources of a phoneme-only package (steering §2),
    // supplied intentionally unsorted.
    let specs = vec![
        res("package.epf", EPF_PAYLOAD.as_bytes()),
        res("mimetype", MIMETYPE_PAYLOAD.as_bytes()),
        res("META-INF/integrity.kdl", INTEGRITY_PAYLOAD.as_bytes()),
        res("Linguistics/phonemes.eld", ELD_PAYLOAD.as_bytes()),
        res("Header/format.version", FORMAT_VERSION_PAYLOAD.as_bytes()),
    ];
    build(specs)
}

/// Illustrative resource payloads. These are valid-looking logical contents;
/// the framing vectors do not require them to pass full schema validation
/// (that is exercised by the EPF/ELD/integrity fixtures in later phases).
const FORMAT_VERSION_PAYLOAD: &str = "0.1.0\n";
const MIMETYPE_PAYLOAD: &str = "application/vnd.elang";
const EPF_PAYLOAD: &str = "epf \"0.1.0\"\n";
const INTEGRITY_PAYLOAD: &str = "integrity {\n}\n";
const ELD_PAYLOAD: &str = "phonemes {\n}\n";

/// Build a `ResourceSpec` with a deterministic placeholder frame body.
fn res(path: &'static str, uncompressed: &[u8]) -> ResourceSpec {
    ResourceSpec {
        path,
        uncompressed: uncompressed.to_vec(),
        frame_body: placeholder_frame(path),
    }
}

/// A deterministic, clearly-labelled placeholder standing in for a real Zstd
/// frame. Format: `ELANGFRAME:<path>#` then `.` padding to a fixed 48 bytes so
/// the layout has non-trivial, distinct frame sizes to validate offsets.
fn placeholder_frame(path: &str) -> Vec<u8> {
    const TARGET: usize = 48;
    let mut body = format!("ELANGFRAME:{path}#").into_bytes();
    if body.len() < TARGET {
        body.resize(TARGET, b'.');
    }
    body
}

/// Serialize a `ReferenceArchive` from resource specs, applying the strict
/// ascending UTF-8 path sort and computing every offset, size and digest from
/// the actual bytes.
pub fn build(mut specs: Vec<ResourceSpec>) -> ReferenceArchive {
    // Strict ascending order by raw UTF-8 path bytes (steering §3.2/§3.4).
    specs.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    for w in specs.windows(2) {
        assert_ne!(w[0].path, w[1].path, "duplicate resource path");
    }

    // Header (steering §3.1): magic, major=0, minor=1, flags=0, 16 zero bytes.
    let mut header = Vec::with_capacity(HEADER_LEN as usize);
    header.extend_from_slice(&HEADER_MAGIC);
    header.extend_from_slice(&0u16.to_le_bytes()); // major
    header.extend_from_slice(&1u16.to_le_bytes()); // minor
    header.extend_from_slice(&0u32.to_le_bytes()); // flags
    header.extend_from_slice(&[0u8; 16]); // reserved
    assert_eq!(header.len() as u64, HEADER_LEN);

    // Frames are contiguous starting immediately after the 32-byte header
    // (steering §3.2). Compute each frame's offset and size.
    let mut resources = Vec::with_capacity(specs.len());
    let mut cursor = HEADER_LEN;
    let mut frames = Vec::new();
    for s in &specs {
        let compressed_size = s.frame_body.len() as u64;
        resources.push(ResourceLayout {
            path: s.path,
            frame_offset: cursor,
            compressed_size,
            uncompressed_size: s.uncompressed.len() as u64,
            compression: COMPRESSION_ZSTD,
            frame_body: s.frame_body.clone(),
        });
        frames.extend_from_slice(&s.frame_body);
        cursor = cursor
            .checked_add(compressed_size)
            .expect("frame offset overflow");
    }
    // The index immediately follows the final frame (steering §3.2).
    let index_offset = cursor;

    // Index header (steering §3.2): magic, version=1, entry format=1, count.
    let count = u32::try_from(resources.len()).expect("resource count fits u32");
    assert!(count <= 25_000, "resource count within ceiling");
    let mut index_bytes = Vec::new();
    index_bytes.extend_from_slice(&INDEX_MAGIC);
    index_bytes.extend_from_slice(&1u16.to_le_bytes()); // index version
    index_bytes.extend_from_slice(&1u16.to_le_bytes()); // entry format
    index_bytes.extend_from_slice(&count.to_le_bytes()); // resource count
    assert_eq!(index_bytes.len() as u64, INDEX_HEADER_LEN);

    // Index entries (steering §3.2), in sorted order.
    for r in &resources {
        let path_bytes = r.path.as_bytes();
        let path_len = u16::try_from(path_bytes.len()).expect("path length fits u16");
        index_bytes.extend_from_slice(&path_len.to_le_bytes());
        index_bytes.extend_from_slice(path_bytes);
        index_bytes.extend_from_slice(&r.frame_offset.to_le_bytes());
        index_bytes.extend_from_slice(&r.compressed_size.to_le_bytes());
        index_bytes.extend_from_slice(&r.uncompressed_size.to_le_bytes());
        index_bytes.push(r.compression);
    }

    let index_size = index_bytes.len() as u64;
    assert!(
        (INDEX_HEADER_LEN..=33_554_432).contains(&index_size),
        "index size within 16..=33,554,432"
    );

    // Real SHA-256 over the exact serialized index bytes (steering §3.3).
    let index_sha256: [u8; 32] = Sha256::digest(&index_bytes).into();

    // Footer (steering §3.3): magic, index_offset, index_size, index hash,
    // 8 reserved zero bytes.
    let mut footer = Vec::with_capacity(FOOTER_LEN as usize);
    footer.extend_from_slice(&FOOTER_MAGIC);
    footer.extend_from_slice(&index_offset.to_le_bytes());
    footer.extend_from_slice(&index_size.to_le_bytes());
    footer.extend_from_slice(&index_sha256);
    footer.extend_from_slice(&[0u8; 8]);
    assert_eq!(footer.len() as u64, FOOTER_LEN);

    // Assemble the whole archive.
    let mut archive = Vec::new();
    archive.extend_from_slice(&header);
    archive.extend_from_slice(&frames);
    archive.extend_from_slice(&index_bytes);
    archive.extend_from_slice(&footer);

    // The core framing invariant (steering §3.3), checked arithmetic.
    let file_size = archive.len() as u64;
    assert_eq!(
        index_offset
            .checked_add(index_size)
            .expect("no overflow")
            .checked_add(FOOTER_LEN)
            .expect("no overflow"),
        file_size,
        "index_offset + index_size + 64 == file_size"
    );

    ReferenceArchive {
        header,
        resources,
        index_bytes,
        footer,
        index_offset,
        index_size,
        index_sha256,
        archive,
    }
}

/// SHA-256 of a byte slice as a fixed 32-byte array.
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Lowercase hex of a byte slice (for documents and `.sha256` sidecars).
pub fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_archive_is_internally_consistent() {
        let a = minimal_archive();

        // Header: 32 bytes, correct magic and version.
        assert_eq!(a.header.len(), 32);
        assert_eq!(&a.header[0..8], &HEADER_MAGIC);
        assert_eq!(&a.header[8..10], &0u16.to_le_bytes());
        assert_eq!(&a.header[10..12], &1u16.to_le_bytes());

        // Footer: 64 bytes, correct magic, hash matches index bytes.
        assert_eq!(a.footer.len(), 64);
        assert_eq!(&a.footer[0..8], &FOOTER_MAGIC);
        let recomputed: [u8; 32] = Sha256::digest(&a.index_bytes).into();
        assert_eq!(recomputed, a.index_sha256);
        assert_eq!(&a.footer[24..56], &a.index_sha256);

        // The framing relation holds under checked arithmetic.
        let file_size = a.archive.len() as u64;
        assert_eq!(a.index_offset + a.index_size + FOOTER_LEN, file_size);

        // Frames are contiguous from byte 32 with no gaps/overlaps.
        let mut cursor = HEADER_LEN;
        for r in &a.resources {
            assert_eq!(r.frame_offset, cursor, "frame contiguity");
            assert_eq!(r.compression, COMPRESSION_ZSTD);
            cursor += r.compressed_size;
        }
        assert_eq!(cursor, a.index_offset, "last frame ends at index offset");

        // Paths are strictly ascending by UTF-8 bytes.
        for w in a.resources.windows(2) {
            assert!(
                w[0].path.as_bytes() < w[1].path.as_bytes(),
                "paths strictly ascending"
            );
        }

        // Each index entry is exactly ENTRY_FIXED_LEN + path bytes; the total
        // index is the header plus all entries.
        let entries_len: u64 = a
            .resources
            .iter()
            .map(|r| ENTRY_FIXED_LEN + r.path.len() as u64)
            .sum();
        assert_eq!(a.index_size, INDEX_HEADER_LEN + entries_len);
    }

    #[test]
    fn build_sorts_and_rejects_duplicates() {
        let a = build(vec![super::res("b", b"x"), super::res("a", b"y")]);
        assert_eq!(a.resources[0].path, "a");
        assert_eq!(a.resources[1].path, "b");
    }
}
