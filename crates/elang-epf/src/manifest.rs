//! Manifest path resolution against the archive index (T0614).
//!
//! Constraint **C-12** of `specs/grammar/epf-0.1.0.md` splits into two halves:
//!
//! * *Syntactic* — the path is a safe package-relative resource path ending in
//!   `.eld`. That is checked during EPF parse (see [`crate::parse()`]).
//! * *Semantic* — the path **exists** in the archive index already parsed at
//!   reader stage 5. That check needs external data, so it lives here and is
//!   performed by the caller once both the EPF and the index are available.
//!
//! The dependency direction matters: `elang-epf` depends only on `elang-core`
//! and `elang-kdl` (see `design.md`), so it cannot name `elang-container`'s
//! `Index` type. Instead this module exposes the check over the smallest
//! possible abstraction — "does this index contain this path?" — via
//! [`IndexLookup`], and ships impls for the index types the workspace
//! actually has.

use elang_core::error::Error;
use elang_core::index::ResourceIndex;

/// A source of package resource paths that an EPF manifest can be resolved
/// against.
///
/// Implementors answer a single question: is this exact resource path present
/// in the archive index? Matching is byte-exact on the validated UTF-8 path;
/// no normalization, case folding or prefix matching is ever applied.
pub trait IndexLookup {
    /// Whether `path` (a validated, package-relative resource path) is present
    /// in the index.
    fn contains(&self, path: &str) -> bool;
}

impl IndexLookup for ResourceIndex {
    fn contains(&self, path: &str) -> bool {
        // `ResourceIndex` keys on the typed `ResourcePath`; rebuilding it from
        // the already-validated EPF string cannot fail, but the constructor
        // is fallible so we treat the failure as "absent" rather than
        // panicking. The EPF layer guarantees the path is well-formed, so this
        // branch is unreachable in practice.
        elang_core::ResourcePath::new(path)
            .ok()
            .and_then(|rp| self.get(&rp))
            .is_some()
    }
}

impl IndexLookup for [String] {
    fn contains(&self, path: &str) -> bool {
        self.iter().any(|p| p == path)
    }
}

impl<T: IndexLookup + ?Sized> IndexLookup for &T {
    fn contains(&self, path: &str) -> bool {
        (**self).contains(path)
    }
}

impl<T: IndexLookup> IndexLookup for [T] {
    fn contains(&self, path: &str) -> bool {
        self.iter().any(|lookup| lookup.contains(path))
    }
}

/// Resolve a validated phoneme resource path against the archive index.
///
/// On success the `Ok(())` proves the declared resource is actually present.
/// On failure the diagnostic is `EPF_MANIFEST_PATH_NOT_INDEXED` and names the
/// path that was looked up, so a consumer can report exactly which declaration
/// is dangling.
pub fn resolve_phonemes<L: IndexLookup + ?Sized>(path: &str, index: &L) -> Result<(), Error> {
    if index.contains(path) {
        Ok(())
    } else {
        let diagnostic = elang_core::error::Diagnostic::new(
            "EPF_MANIFEST_PATH_NOT_INDEXED",
            elang_core::error::Category::EpfSchema,
            format!("declared phoneme resource {path:?} is not present in the archive index"),
        )
        .with_stage(10)
        .with_resource(path);
        Err(Error { diagnostic })
    }
}

/// Whether `path` names an `.eld` resource.
///
/// The check is a byte-exact suffix test on the already-validated path, so it
/// accepts `a/b.eld` and rejects `a/b.ELD`, `a/b.eldx` and `b.eld/../c.eld`
/// (the latter cannot survive `ResourcePath` validation anyway).
#[must_use]
pub fn has_eld_extension(path: &str) -> bool {
    path.ends_with(".eld")
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "manifest tests assert exact diagnostic codes; a panic signals a regression"
)]
mod tests {
    use super::*;

    #[test]
    fn eld_extension_is_exact() {
        assert!(has_eld_extension("Linguistics/phonemes.eld"));
        assert!(has_eld_extension("a.eld"));
        assert!(!has_eld_extension("a.ELD"));
        assert!(!has_eld_extension("a.eldx"));
        assert!(!has_eld_extension("eld"));
        assert!(!has_eld_extension(""));
    }

    #[test]
    fn resolves_against_slice_index() {
        let index = vec![
            "Header/format.version".to_string(),
            "Linguistics/phonemes.eld".to_string(),
        ];
        assert!(resolve_phonemes("Linguistics/phonemes.eld", index.as_slice()).is_ok());
        let e = resolve_phonemes("Linguistics/other.eld", index.as_slice()).unwrap_err();
        assert_eq!(e.code(), "EPF_MANIFEST_PATH_NOT_INDEXED");
        assert_eq!(
            e.diagnostic.resource_path.as_deref(),
            Some("Linguistics/other.eld")
        );
    }

    #[test]
    fn resolves_against_resource_index() {
        let rp = elang_core::ResourcePath::new("Linguistics/phonemes.eld").unwrap();
        let entry = elang_core::index::ResourceEntry::new(rp, 0, 10, 20, [0u8; 32], 100).unwrap();
        let index = ResourceIndex::new(vec![entry]).unwrap();
        assert!(resolve_phonemes("Linguistics/phonemes.eld", &index).is_ok());
        assert!(resolve_phonemes("Nope.eld", &index).is_err());
    }
}
