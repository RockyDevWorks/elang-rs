//! Error model: categories, stable codes, diagnostic metadata and a bounded
//! diagnostics collector (REQ-ERR-001..003, REQ-CORE-MODEL-004,
//! REQ-LIMIT-009). Mirrors `specs/registry/error-codes.md`.

use crate::limits::MAX_RETAINED_DIAGNOSTICS;

/// Error categories (steering §11; registry `categories`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    /// Container header/footer/version/mimetype.
    Container,
    /// Index parse, path order, footer index hash, frame layout.
    Index,
    /// Zstd frame contract.
    Frame,
    /// Resource path validation.
    ResourcePath,
    /// Integrity manifest.
    Integrity,
    /// EPF schema.
    EpfSchema,
    /// ELD schema.
    EldSchema,
    /// Unicode (NFC, category, noncharacter).
    Unicode,
    /// IPA identifier.
    Ipa,
    /// Romanization.
    Roman,
    /// `when` condition language.
    When,
    /// Parser/resource budget.
    Limit,
    /// Canonical serialization.
    Canonical,
}

impl Category {
    /// The registry token for this category.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Container => "CONTAINER",
            Category::Index => "INDEX",
            Category::Frame => "FRAME",
            Category::ResourcePath => "RESOURCE_PATH",
            Category::Integrity => "INTEGRITY",
            Category::EpfSchema => "EPF_SCHEMA",
            Category::EldSchema => "ELD_SCHEMA",
            Category::Unicode => "UNICODE",
            Category::Ipa => "IPA",
            Category::Roman => "ROMAN",
            Category::When => "WHEN",
            Category::Limit => "LIMIT",
            Category::Canonical => "CANONICAL",
        }
    }
}

/// A KDL source span (1-based line/column, length in UTF-8 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdlSpan {
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number.
    pub column: u32,
    /// Span length in UTF-8 bytes.
    pub length: u32,
}

/// A single diagnostic: a stable code plus optional location metadata
/// (REQ-ERR-002). `message` is human-readable and non-authoritative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Stable code from the registry (e.g. `FOOTER_INDEX_HASH_MISMATCH`).
    pub code: &'static str,
    /// Error category.
    pub category: Category,
    /// Validation stage (1–12) where applicable.
    pub stage: Option<u8>,
    /// Indexed resource the diagnostic concerns, where applicable.
    pub resource_path: Option<String>,
    /// Byte offset into the archive/resource, where applicable.
    pub byte_offset: Option<u64>,
    /// KDL source span, where applicable.
    pub span: Option<KdlSpan>,
    /// Human-readable description.
    pub message: String,
}

impl Diagnostic {
    /// Create a minimal diagnostic with a code, category and message.
    #[must_use]
    pub fn new(code: &'static str, category: Category, message: impl Into<String>) -> Self {
        Self {
            code,
            category,
            stage: None,
            resource_path: None,
            byte_offset: None,
            span: None,
            message: message.into(),
        }
    }

    /// Attach a validation stage.
    #[must_use]
    pub fn with_stage(mut self, stage: u8) -> Self {
        self.stage = Some(stage);
        self
    }

    /// Attach a resource path.
    #[must_use]
    pub fn with_resource(mut self, path: impl Into<String>) -> Self {
        self.resource_path = Some(path.into());
        self
    }

    /// Attach a byte offset.
    #[must_use]
    pub fn with_offset(mut self, offset: u64) -> Self {
        self.byte_offset = Some(offset);
        self
    }

    /// Attach a KDL source span.
    #[must_use]
    pub fn with_span(mut self, span: KdlSpan) -> Self {
        self.span = Some(span);
        self
    }
}

/// The primary error type: a diagnostic describing why an operation failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// The leading diagnostic (the failure cause).
    pub diagnostic: Diagnostic,
}

impl Error {
    /// Build an error from a code, category and message.
    #[must_use]
    pub fn new(code: &'static str, category: Category, message: impl Into<String>) -> Self {
        Self {
            diagnostic: Diagnostic::new(code, category, message),
        }
    }

    /// The stable error code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        self.diagnostic.code
    }

    /// The error category.
    #[must_use]
    pub fn category(&self) -> Category {
        self.diagnostic.category
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "[{}:{}] {}",
            self.diagnostic.category.as_str(),
            self.diagnostic.code,
            self.diagnostic.message
        )
    }
}

impl std::error::Error for Error {}

/// The crate-wide result type (REQ-ERR-001).
pub type Result<T> = core::result::Result<T, Error>;

/// A bounded collector of diagnostics (REQ-LIMIT-009). Retains at most
/// [`MAX_RETAINED_DIAGNOSTICS`] entries but records whether any were dropped,
/// so truncation can never turn a failure into a success.
#[derive(Debug, Clone, Default)]
pub struct Diagnostics {
    retained: Vec<Diagnostic>,
    total: usize,
}

impl Diagnostics {
    /// Create an empty collector.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a diagnostic. Beyond the cap, the diagnostic is counted but not
    /// retained; [`Self::truncated`] then returns `true`.
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.total = self.total.saturating_add(1);
        if self.retained.len() < MAX_RETAINED_DIAGNOSTICS {
            self.retained.push(diagnostic);
        }
    }

    /// The retained diagnostics (at most the cap).
    #[must_use]
    pub fn retained(&self) -> &[Diagnostic] {
        &self.retained
    }

    /// Total number of diagnostics recorded (including dropped ones).
    #[must_use]
    pub fn total(&self) -> usize {
        self.total
    }

    /// Whether any diagnostics were dropped due to the cap.
    #[must_use]
    pub fn truncated(&self) -> bool {
        self.total > self.retained.len()
    }

    /// Whether any diagnostic was recorded at all. A caller MUST treat
    /// `has_any() == true` as failure regardless of truncation.
    #[must_use]
    pub fn has_any(&self) -> bool {
        self.total > 0
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn category_tokens_match_registry() {
        assert_eq!(Category::Container.as_str(), "CONTAINER");
        assert_eq!(Category::ResourcePath.as_str(), "RESOURCE_PATH");
        assert_eq!(Category::EldSchema.as_str(), "ELD_SCHEMA");
    }

    #[test]
    fn error_display_includes_code_and_category() {
        let e = Error::new("HEADER_BAD_MAGIC", Category::Container, "bad magic");
        let s = format!("{e}");
        assert!(s.contains("CONTAINER"));
        assert!(s.contains("HEADER_BAD_MAGIC"));
        assert!(s.contains("bad magic"));
    }

    #[test]
    fn diagnostic_builder_attaches_metadata() {
        let d = Diagnostic::new("INDEX_DUPLICATE_PATH", Category::Index, "dup")
            .with_stage(5)
            .with_resource("a/b.eld")
            .with_offset(42)
            .with_span(KdlSpan { line: 2, column: 3, length: 4 });
        assert_eq!(d.stage, Some(5));
        assert_eq!(d.resource_path.as_deref(), Some("a/b.eld"));
        assert_eq!(d.byte_offset, Some(42));
        assert_eq!(d.span.unwrap().line, 2);
    }

    #[test]
    fn diagnostics_cap_does_not_mask_failure() {
        let mut diags = Diagnostics::new();
        for i in 0..(MAX_RETAINED_DIAGNOSTICS + 50) {
            diags.push(Diagnostic::new("LIMIT_NODES_RESOURCE", Category::Limit, format!("d{i}")));
        }
        assert_eq!(diags.retained().len(), MAX_RETAINED_DIAGNOSTICS);
        assert_eq!(diags.total(), MAX_RETAINED_DIAGNOSTICS + 50);
        assert!(diags.truncated());
        assert!(diags.has_any(), "truncation must never hide that failures exist");
    }
}
