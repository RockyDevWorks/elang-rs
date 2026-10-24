//! The actual `write_archive` implementation.
//!
//! ## Integrity manifest self-exclusion
//!
//! Per the v0.1.0 spec (ADR 0004), the integrity manifest **does not**
//! contain a digest entry for itself. The reader's `verify_coverage`
//! enforces this: every resource the index lists except `META-INF/integrity.kdl`
//! must have a matching manifest entry, and the integrity resource is
//! skipped. This makes the writer a single pass: build the manifest over
//! every other resource, serialize once, use those bytes as the integrity
//! resource's uncompressed content.

use elang_container::footer::{FOOTER_LEN, MAX_INDEX_SIZE};
use elang_container::frame::encode;
use elang_container::header::HEADER_LEN;
use elang_container::index::{Index, IndexEntry, COMPRESSION_ZSTD};
use elang_container::{Footer, Header};
use elang_core::limits::{
    MAX_BOOTSTRAP_RESOURCE_BYTES, MAX_COMPRESSED_ARCHIVE_BYTES, MAX_ELD_BYTES, MAX_EPF_BYTES,
    MAX_GENERIC_RESOURCE_BYTES, MAX_INTEGRITY_BYTES, MAX_RESOURCES,
    MAX_UNCOMPRESSED_AGGREGATE_BYTES,
};
use elang_integrity::manifest::{Algorithm, IntegrityEntry, IntegrityManifest};
use sha2::{Digest, Sha256};

use crate::config::PackageConfig;
use crate::error::{BuildError, SizeKind};

/// Assemble a `.elang` archive from a [`PackageConfig`].
pub(crate) fn write_archive(config: &PackageConfig) -> Result<Vec<u8>, BuildError> {
    config.validate_paths()?;
    validate_resource_count(config)?;
    validate_format_version(&config.format_version)?;

    // 1. Canonicalize the typed inputs.
    let epf_bytes = elang_epf::canon::to_bytes(&config.epf);
    let eld_bytes = elang_eld::canon::to_bytes(&config.eld);

    // 2. Lay out the resources. The integrity manifest is built last so
    //    its bytes can join the resource list before sorting.
    let mut resources: Vec<(String, Vec<u8>)> =
        Vec::with_capacity(config.extra_resources.len().saturating_add(5));
    resources.push((
        "Header/mimetype".to_string(),
        b"application/vnd.elang".to_vec(),
    ));
    resources.push((
        "Header/format.version".to_string(),
        config.format_version.as_bytes().to_vec(),
    ));
    resources.push(("package.epf".to_string(), epf_bytes));
    resources.push(("Linguistics/phonemes.eld".to_string(), eld_bytes));
    for extra in &config.extra_resources {
        resources.push((extra.path.clone(), extra.canonical_bytes.clone()));
    }

    // 3. Build the integrity manifest over every resource *except* the
    //    integrity manifest itself. The map is keyed by ResourcePath so the
    //    canonical serialization's child order is bytewise ascending.
    let mut integrity = IntegrityManifest {
        algorithm: Algorithm::Sha256,
        resources: Default::default(),
    };
    for (path, uncompressed) in &resources {
        let resource_path = elang_core::ResourcePath::new(path)?;
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

    // 4. Append the integrity manifest as the last resource, then sort
    //    canonically. `validate_paths` and the five hardcoded mandatory
    //    names already guarantee that every path in `resources` is
    //    unique, so `Index::with_entries` is the only remaining
    //    uniqueness gate — no need for a separate post-sort dedup pass.
    resources.push(("META-INF/integrity.kdl".to_string(), integrity_bytes));
    resources.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));

    // 5. Per-resource size ceilings (before encoding).
    for (path, bytes) in &resources {
        let ceiling = per_resource_ceiling(path);
        let len = bytes.len() as u64;
        if len > ceiling {
            return Err(BuildError::SizeCeiling {
                kind: SizeKind::ResourceBytes,
                requested: len,
                ceiling,
            });
        }
    }

    // 6. Aggregate uncompressed ceiling.
    let total_uncompressed: u64 = resources
        .iter()
        .fold(0u64, |acc, (_, b)| acc.saturating_add(b.len() as u64));
    if total_uncompressed > MAX_UNCOMPRESSED_AGGREGATE_BYTES {
        return Err(BuildError::SizeCeiling {
            kind: SizeKind::UncompressedAggregate,
            requested: total_uncompressed,
            ceiling: MAX_UNCOMPRESSED_AGGREGATE_BYTES,
        });
    }

    // 7. Encode each resource as an independent Zstd frame.
    let mut frames: Vec<(String, Vec<u8>, u64)> = Vec::with_capacity(resources.len());
    let mut compressed_total: u64 = HEADER_LEN as u64;
    for (path, uncompressed) in &resources {
        let compressed = encode(uncompressed, config.profile)?;
        let compressed_len = compressed.len() as u64;
        compressed_total = compressed_total.saturating_add(compressed_len);
        frames.push((path.clone(), compressed, uncompressed.len() as u64));
    }
    if compressed_total.saturating_add(MAX_INDEX_SIZE) > MAX_COMPRESSED_ARCHIVE_BYTES {
        return Err(BuildError::SizeCeiling {
            kind: SizeKind::ArchiveBytes,
            requested: compressed_total.saturating_add(MAX_INDEX_SIZE),
            ceiling: MAX_COMPRESSED_ARCHIVE_BYTES,
        });
    }

    // 8. Build the index.
    let mut index_entries: Vec<IndexEntry> = Vec::with_capacity(frames.len());
    let mut frame_offset: u64 = HEADER_LEN as u64;
    for (path, compressed, uncompressed_size) in &frames {
        let entry = IndexEntry::new(
            path,
            frame_offset,
            compressed.len() as u64,
            *uncompressed_size,
            COMPRESSION_ZSTD,
        )?;
        index_entries.push(entry);
        frame_offset = frame_offset.saturating_add(compressed.len() as u64);
    }
    let index = Index::with_entries(index_entries)?;
    let (index_bytes, index_hash) = index.encode_and_hash()?;
    let index_size = index_bytes.len() as u64;

    // 9. Build the footer.
    let footer = Footer::new(frame_offset, index_size, index_hash)?;

    // 10. Assemble: header + frames + index + footer.
    let total_size = frame_offset
        .saturating_add(index_size)
        .saturating_add(FOOTER_LEN as u64);
    if total_size > MAX_COMPRESSED_ARCHIVE_BYTES {
        return Err(BuildError::SizeCeiling {
            kind: SizeKind::ArchiveBytes,
            requested: total_size,
            ceiling: MAX_COMPRESSED_ARCHIVE_BYTES,
        });
    }

    let capacity = usize::try_from(total_size).map_err(|_| BuildError::SizeCeiling {
        kind: SizeKind::ArchiveBytes,
        requested: total_size,
        ceiling: MAX_COMPRESSED_ARCHIVE_BYTES,
    })?;
    let mut out: Vec<u8> = Vec::with_capacity(capacity);
    let header = Header::current();
    let mut header_buf = [0u8; HEADER_LEN];
    header.encode(&mut header_buf, 0)?;
    out.extend_from_slice(&header_buf);
    for (_, compressed, _) in &frames {
        out.extend_from_slice(compressed);
    }
    out.extend_from_slice(&index_bytes);
    let mut footer_buf = [0u8; FOOTER_LEN];
    footer.encode(&mut footer_buf, 0)?;
    out.extend_from_slice(&footer_buf);

    debug_assert_eq!(out.len() as u64, total_size);
    Ok(out)
}

/// Validate the pinned format-version string. Must be NFC, must encode
/// `{major.minor}.{patch}` with ASCII digits and non-empty components, must
/// fit in the reader's [`MAX_BOOTSTRAP_RESOURCE_BYTES`] decode budget for
/// `Header/format.version`, and major/minor must textually match the
/// current container header.
fn validate_format_version(version: &str) -> Result<(), BuildError> {
    if version.is_empty() {
        return Err(BuildError::InvalidConfig {
            code: "WRITER_INVALID_FORMAT_VERSION",
            message: "format_version must not be empty".to_string(),
        });
    }
    // The 64-byte ceiling is well within usize on every supported target;
    // the cast is for matching against `version.len()` which is `usize`.
    #[allow(clippy::cast_possible_truncation)]
    if version.len() > MAX_BOOTSTRAP_RESOURCE_BYTES as usize {
        return Err(BuildError::InvalidConfig {
            code: "WRITER_INVALID_FORMAT_VERSION",
            message: format!(
                "format_version is {} bytes, ceiling {}",
                version.len(),
                MAX_BOOTSTRAP_RESOURCE_BYTES
            ),
        });
    }
    if !elang_unicode::is_nfc(version) {
        return Err(BuildError::InvalidConfig {
            code: "WRITER_INVALID_FORMAT_VERSION",
            message: "format_version is not in Unicode NFC".to_string(),
        });
    }
    // Slice-match the split output so every arm is reachable: after the
    // empty/length/NFC checks above, `version.split('.')` yields ≥1
    // element, so the match has three viable cases plus the catch-all
    // for 4+ components. The earlier `ok_or_else` shape had a
    // guaranteed-Some first element and therefore a dead branch.
    let parts: Vec<&str> = version.split('.').collect();
    let (major, minor, patch) = match parts.as_slice() {
        [major, minor, patch] => (*major, *minor, *patch),
        [_major, _minor] => {
            return Err(BuildError::InvalidConfig {
                code: "WRITER_INVALID_FORMAT_VERSION",
                message: "format_version must include a patch component".to_string(),
            });
        }
        [_major] => {
            return Err(BuildError::InvalidConfig {
                code: "WRITER_INVALID_FORMAT_VERSION",
                message: format!(
                    "format_version must include the minor and patch components ({version:?})"
                ),
            });
        }
        _ => {
            return Err(BuildError::InvalidConfig {
                code: "WRITER_INVALID_FORMAT_VERSION",
                message: format!(
                    "format_version must be exactly three dot-separated components ({version:?})"
                ),
            });
        }
    };
    // Empty components ("0.1.", "..", ".1.0") fire BEFORE the digits
    // check: `is_ascii_digits("")` returns false (the `!is_empty()`
    // guard), so checking digits first would shadow the "must not be
    // empty" message with the generic "must be ASCII digits".
    if major.is_empty() || minor.is_empty() || patch.is_empty() {
        return Err(BuildError::InvalidConfig {
            code: "WRITER_INVALID_FORMAT_VERSION",
            message: "format_version components must not be empty".to_string(),
        });
    }
    if !is_ascii_digits(major) || !is_ascii_digits(minor) || !is_ascii_digits(patch) {
        return Err(BuildError::InvalidConfig {
            code: "WRITER_INVALID_FORMAT_VERSION",
            message: format!("format_version components must be ASCII digits ({version:?})"),
        });
    }
    let header = Header::current();
    if major != header.major().to_string() || minor != header.minor().to_string() {
        return Err(BuildError::InvalidConfig {
            code: "WRITER_INVALID_FORMAT_VERSION",
            message: format!(
                "format_version major/minor must match the binary header {}.{} ({version:?})",
                header.major(),
                header.minor()
            ),
        });
    }
    Ok(())
}
/// True when `s` is non-empty and every byte is an ASCII digit.
fn is_ascii_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// Reject a resource count that would exceed `MAX_RESOURCES`.
fn validate_resource_count(config: &PackageConfig) -> Result<(), BuildError> {
    let total = config.extra_resources.len().saturating_add(5);
    if u32::try_from(total).map_or(true, |n| n > MAX_RESOURCES) {
        return Err(BuildError::SizeCeiling {
            kind: SizeKind::ResourceCount,
            requested: total as u64,
            ceiling: MAX_RESOURCES as u64,
        });
    }
    Ok(())
}

/// Return the per-resource byte ceiling for the named path.
fn per_resource_ceiling(path: &str) -> u64 {
    match path {
        "META-INF/integrity.kdl" => MAX_INTEGRITY_BYTES,
        "package.epf" => MAX_EPF_BYTES,
        "Linguistics/phonemes.eld" => MAX_ELD_BYTES,
        _ => MAX_GENERIC_RESOURCE_BYTES,
    }
}
