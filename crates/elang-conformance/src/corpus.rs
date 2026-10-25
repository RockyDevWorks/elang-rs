//! The ELANG conformance corpus format and runner (T1401).
//!
//! This module owns the *format* of the corpus and the *execution* of it. It
//! is deliberately separate from the fixture bodies themselves
//! ([`crate::golden_packages`], [`crate::negatives`]): this module says what a
//! fixture **is** and how a fixture is **run**, and the other modules say
//! what the fixtures **are**.
//!
//! # The fixture manifest
//!
//! A fixture is a named archive plus the expected outcome of running it
//! through the reader. The manifest is deliberately tiny and machine-readable
//! so an independent implementation can consume the same corpus:
//!
//! ```text
//! {
//!   "id": "binary/header-bad-magic",
//!   "kind": "negative",
//!   "level": "Inspect",
//!   "stage": 1,
//!   "code": "HEADER_BAD_MAGIC",
//!   "category": "Container",
//!   "requirements": ["REQ-PKG-001"]
//! }
//! ```
//!
//! `level`, `code` and `category` are the **normative expectation** for a
//! negative fixture: the reader must reject the archive at exactly that
//! reader level with exactly that diagnostic code and category. The
//! observed pipeline **stage** is asserted separately by the
//! `stage_conformance` integration test against the published
//! `specs/registry/error-codes.json` (each registered code carries the
//! set of stages at which it may legitimately be raised). This matches
//! REQ-CONF-003 literally — "expected stage + error code for each
//! fixture under inspect/verify/validate" — without forcing every
//! fixture to restate a stage literal that the registry already
//! publishes once. A conformance failure names the fixture id, the
//! requirement it protects, and the observed-vs-expected divergence —
//! that is what makes a failure actionable by an implementer who has
//! never seen this repository.
//!
//! # The three-level ladder
//!
//! The corpus distinguishes the three reader levels explicitly:
//!
//! | Level | Stages | Guarantee |
//! |---|---|---|
//! | [`Level::Inspect`] | 1–7 | structure only; no decompression, no integrity |
//! | [`Level::Verify`] | 1–11 | every resource decompresses and hashes match |
//! | [`Level::Validate`] | 1–12 | full semantic validity of EPF + ELD |
//!
//! A negative fixture declares the **lowest** level at which the archive
//! must be rejected. This is the corpus's central anti-laundering rule: an
//! archive that fails semantic validation but is structurally intact must
//! pass `inspect` and fail `validate` — a reader that rejected it at
//! `inspect` would be over-eager, and one that accepted it at `validate`
//! would be unsound. The runner therefore asserts the *exact* level, not
//! merely "it fails somewhere".
//!
//! # Positive fixtures
//!
//! A positive fixture declares the level at which the archive must be
//! **accepted**. Acceptance is not a bool: it is the guarantee that the
//! archive survives that level's full pipeline. A positive fixture that is
//! only structurally sound is published at [`Level::Inspect`]; a
//! semantically complete package is published at [`Level::Validate`].
//!
//! # Provenance
//!
//! Golden bytes are never hand-written. Every byte-level expectation in the
//! corpus is produced by running the production code and recording what it
//! produced, and the generator binaries that publish them
//! (`gen_binary_vectors`, `gen_canonical_goldens`, `gen_normative_manifest`)
//! recompute the values from scratch. The *reason* the values are correct is
//! the normative specification; the generator is evidence that the
//! implementation agrees with it, not the source of truth. See
//! `docs/conformance.md` for the provenance rules and the
//! independent-verification procedure.

use elang_core::error::Category;
use elang_reader::Package as ReaderPackage;

/// The reader pipeline level at which a fixture's outcome is asserted.
///
/// The levels are cumulative: [`Level::Validate`] implies
/// [`Level::Verify`] implies [`Level::Inspect`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// Stages 1–7: header, footer, index, frame layout. No decompression.
    Inspect,
    /// Stages 1–11: adds bootstrap, integrity structure, EPF and
    /// per-resource SHA-256 verification.
    Verify,
    /// Stages 1–12: adds ELD semantic validation.
    Validate,
}

impl Level {
    /// The highest pipeline stage this level covers.
    pub const fn ceiling_stage(self) -> u8 {
        match self {
            Self::Inspect => 7,
            Self::Verify => 11,
            Self::Validate => 12,
        }
    }

    /// Run the reader up to this level and report the outcome.
    ///
    /// Returns `Ok(())` when the archive passes every stage up to the
    /// level's ceiling, and `Err` with the reader's diagnostic when it does
    /// not. The diagnostic's `stage` field is the stage that fired; for
    /// container-layer failures the reader attaches the stage explicitly, so
    /// the value is always meaningful.
    pub fn run(self, archive: &[u8]) -> Result<(), elang_core::error::Error> {
        let pkg = ReaderPackage::open(archive)?;
        match self {
            Self::Inspect => {
                // `inspect` borrows and cannot fail: stages 1–7 already ran
                // to completion inside `open`.
                let _inspected = pkg.inspect();
                Ok(())
            }
            Self::Verify => {
                let _verified = pkg.verify()?;
                Ok(())
            }
            Self::Validate => {
                let _validated = pkg.validate()?;
                Ok(())
            }
        }
    }
}

/// Whether a fixture must be accepted or rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expectation {
    /// The archive must be accepted at this level.
    Accept(Level),
    /// The archive must be rejected at exactly this level, with exactly
    /// this diagnostic code and category.
    Reject {
        /// The level at which the rejection must occur.
        level: Level,
        /// The normative diagnostic code the reader must emit.
        code: &'static str,
        /// The registered category the code belongs to.
        category: Category,
    },
}

impl Expectation {
    /// The level this expectation is asserted at.
    pub const fn level(&self) -> Level {
        match self {
            Self::Accept(level) => *level,
            Self::Reject { level, .. } => *level,
        }
    }

    /// True when the archive is expected to be accepted.
    pub const fn is_accept(&self) -> bool {
        matches!(self, Self::Accept(_))
    }
}

/// One corpus fixture: a stable id, the archive bytes, and the expected
/// outcome of running it.
#[derive(Debug, Clone)]
pub struct Fixture {
    /// Stable, slash-separated fixture id. Unique across the corpus.
    pub id: &'static str,
    /// One-line description of the behavior under test.
    pub description: &'static str,
    /// Normative requirement ids this fixture protects.
    pub requirements: &'static [&'static str],
    /// The expected outcome.
    pub expectation: Expectation,
    /// The archive bytes the fixture feeds to the reader.
    pub archive: Vec<u8>,
}

/// Which document schema a [`DocumentFixture`] runs against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schema {
    /// `elang_epf::parse` on the source bytes.
    Epf,
    /// `elang_eld::parse` on the source bytes (with the pinned repertoire).
    Eld,
}

/// One schema-level fixture: a KDL document and the expected outcome of
/// parsing it.
#[derive(Debug, Clone)]
pub struct DocumentFixture {
    /// Stable, slash-separated fixture id.
    pub id: &'static str,
    /// One-line description.
    pub description: &'static str,
    /// Normative requirement ids this fixture protects.
    pub requirements: &'static [&'static str],
    /// Which schema the source is parsed against.
    pub schema: Schema,
    /// The document bytes.
    pub source: Vec<u8>,
    /// The expected outcome (Accept or Reject with code/category).
    pub expectation: DocumentExpectation,
}

/// Outcome of a schema-level parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentExpectation {
    /// The document must parse and validate cleanly.
    Accept,
    /// The document must be rejected with exactly this code and category.
    Reject {
        /// The diagnostic code the parser must emit.
        code: &'static str,
        /// The category the code belongs to.
        category: Category,
    },
}

/// Run a schema-level fixture against the production parser.
pub fn run_document_fixture(fixture: &DocumentFixture) -> Result<(), Failure> {
    let requirement = fixture
        .requirements
        .first()
        .copied()
        .unwrap_or("(unmapped requirement)");
    let fail = |detail: String| Failure {
        fixture: fixture.id.to_string(),
        requirement: requirement.to_string(),
        detail,
    };
    let repertoire = match elang_repertoire::repertoire::load() {
        Ok(r) => r,
        Err(e) => {
            return Err(fail(format!("repertoire load: {e:?}")));
        }
    };
    let parse_result = match fixture.schema {
        Schema::Epf => elang_epf::parse(&fixture.source).map(|_| ()).map_err(|e| {
            (
                e.diagnostic.code,
                e.diagnostic.category,
                e.diagnostic.message,
            )
        }),
        Schema::Eld => elang_eld::parse(&fixture.source, &repertoire)
            .and_then(|doc| elang_eld::inventory::from_document(&doc, &repertoire))
            .map(|_| ())
            .map_err(|e| {
                (
                    e.diagnostic.code,
                    e.diagnostic.category,
                    e.diagnostic.message,
                )
            }),
    };
    match (fixture.expectation, parse_result) {
        (DocumentExpectation::Accept, Ok(())) => Ok(()),
        (DocumentExpectation::Accept, Err((code, cat, msg))) => Err(fail(format!(
            "expected acceptance, got rejection: code={code} category={cat:?} message={msg}"
        ))),
        (
            DocumentExpectation::Reject {
                code,
                category: exp_cat,
            },
            Err((obs_code, obs_cat, msg)),
        ) => {
            if obs_code != code {
                return Err(fail(format!(
                    "expected code {code} (category {exp_cat:?}), got code {obs_code} (category {obs_cat:?}) message={msg}"
                )));
            }
            if obs_cat != exp_cat {
                return Err(fail(format!(
                    "expected category {exp_cat:?} for code {code}, got {obs_cat:?}"
                )));
            }
            Ok(())
        }
        (DocumentExpectation::Reject { code, .. }, Ok(())) => Err(fail(format!(
            "expected rejection with code {code}, but the document parsed cleanly"
        ))),
    }
}

/// A golden-value fixture: production code computes a value, and the
/// fixture pins the expected bytes. The `actual` field holds whatever
/// the production code produced; the test asserts it matches `expected`
/// byte-for-byte.
#[derive(Debug, Clone)]
pub struct VectorFixture {
    /// Stable, slash-separated fixture id.
    ///
    /// Owned rather than borrowed because some vector families derive the
    /// id from runtime data (a resource path, an input label), so the id
    /// cannot always be a `&'static str`.
    pub id: String,
    /// One-line description.
    pub description: &'static str,
    /// Normative requirement ids this fixture protects.
    pub requirements: &'static [&'static str],
    /// The observed bytes produced by running production code.
    pub actual: Vec<u8>,
    /// The pinned expected bytes (the golden).
    pub expected: Vec<u8>,
}

impl VectorFixture {
    /// A label for the expected value, used in failure messages.
    pub fn expected_label(&self) -> String {
        crate::binary_vectors::hex(&self.expected)
    }
    /// A label for the actual value, used in failure messages.
    pub fn actual_label(&self) -> String {
        crate::binary_vectors::hex(&self.actual)
    }
}

/// Run a golden-vector fixture.
pub fn run_vector_fixture(fixture: &VectorFixture) -> Result<(), Failure> {
    let requirement = fixture
        .requirements
        .first()
        .copied()
        .unwrap_or("(unmapped requirement)");
    if fixture.actual == fixture.expected {
        Ok(())
    } else {
        Err(Failure {
            fixture: fixture.id.clone(),
            requirement: requirement.to_string(),
            detail: format!(
                "expected {} got {}",
                fixture.expected_label(),
                fixture.actual_label()
            ),
        })
    }
}

/// Hex-encode a byte slice (lower-case, no separator).
#[must_use]
pub fn hex_lower(bytes: &[u8]) -> String {
    crate::binary_vectors::hex(bytes)
}

/// A conformance failure: what the corpus expected and what it observed.
///
/// The message is written to be actionable by an implementer who has never
/// read this repository — it names the fixture, the requirement, and the
/// concrete divergence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// The fixture id that failed.
    pub fixture: String,
    /// The requirement the fixture protects.
    pub requirement: String,
    /// Human-readable description of the divergence.
    pub detail: String,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "conformance failure: fixture `{}` (requirement {}): {}",
            self.fixture, self.requirement, self.detail
        )
    }
}

impl std::error::Error for Failure {}

/// What the reader reported for a negative fixture that was rejected at
/// its declared level with the declared code and category.
///
/// The stage is always populated: the reader attaches a concrete stage to
/// every diagnostic it surfaces (container codes via
/// `container_parse_err` in `elang-reader/src/package.rs`, semantic codes
/// via the phase's `with_stage(N)` helper), so a missing stage here is a
/// reader regression, not a corpus quirk.
#[derive(Debug, Clone, Copy)]
pub struct ObservedRejection {
    /// The diagnostic code the reader emitted (equal to the fixture's
    /// declared code on a successful observation).
    pub code: &'static str,
    /// The category the reader attached to the diagnostic.
    pub category: Category,
    /// The pipeline stage the reader attached to the diagnostic.
    pub stage: u8,
}

/// Run a negative fixture through the full anti-laundering check and
/// return what the reader actually reported on rejection.
///
/// Performs the same code/category/level assertions as
/// [`run_fixture`] for a `Reject` expectation — this is the single
/// implementation of the rejection-side validation. `run_fixture` is a
/// thin wrapper that discards the [`ObservedRejection`]. The
/// `stage_conformance` integration test uses the returned observation to
/// assert that the observed stage is one of the stages the normative
/// registry (`specs/registry/error-codes.json`) publishes for the code —
/// REQ-CONF-003's "expected stage + error code".
pub fn observe_fixture(fixture: &Fixture) -> Result<ObservedRejection, Failure> {
    let requirement = fixture
        .requirements
        .first()
        .copied()
        .unwrap_or("(unmapped requirement)");

    let fail = |detail: String| Failure {
        fixture: fixture.id.to_string(),
        requirement: requirement.to_string(),
        detail,
    };

    match fixture.expectation {
        Expectation::Accept(level) => Err(fail(format!(
            "observe_fixture called on an Accept fixture (level {level:?}); use run_fixture instead"
        ))),
        Expectation::Reject {
            level,
            code,
            category,
        } => {
            // A rejection must not occur *below* the declared level: the
            // lower levels must have accepted the archive. This is the
            // anti-laundering check — "it failed somewhere" is not enough.
            for lower in lower_levels(level) {
                if let Err(err) = lower.run(&fixture.archive) {
                    return Err(fail(format!(
                        "expected rejection at level {:?}, but the archive was already rejected at the lower level {:?}: code={} category={:?} stage={:?} message={}",
                        level,
                        lower,
                        err.diagnostic.code,
                        err.diagnostic.category,
                        err.diagnostic.stage,
                        err.diagnostic.message,
                    )));
                }
            }

            match level.run(&fixture.archive) {
                Ok(()) => Err(fail(format!(
                    "expected rejection at level {:?} with code {code} (category {category:?}), but the archive was accepted",
                    level,
                ))),
                Err(err) => {
                    let observed_code = err.diagnostic.code;
                    let observed_category = err.diagnostic.category;
                    if observed_code != code {
                        return Err(fail(format!(
                            "expected code {code} (category {category:?}) at level {:?}, got code {observed_code} (category {observed_category:?}) stage={:?} message={}",
                            level,
                            err.diagnostic.stage,
                            err.diagnostic.message,
                        )));
                    }
                    if observed_category != category {
                        return Err(fail(format!(
                            "expected category {category:?} for code {code}, got {observed_category:?}",
                        )));
                    }
                    let stage = err.diagnostic.stage.ok_or_else(|| {
                        fail(format!(
                            "diagnostic for code {code} carried no stage (level={level:?}); reader regression",
                        ))
                    })?;
                    Ok(ObservedRejection {
                        code: observed_code,
                        category: observed_category,
                        stage,
                    })
                }
            }
        }
    }
}

/// Returns a [`Failure`] when the observed outcome diverges from the
/// fixture's expectation. The failure text names the fixture, the
/// requirement, and the observed stage/code/level.
pub fn run_fixture(fixture: &Fixture) -> Result<(), Failure> {
    let requirement = fixture
        .requirements
        .first()
        .copied()
        .unwrap_or("(unmapped requirement)");

    let fail = |detail: String| Failure {
        fixture: fixture.id.to_string(),
        requirement: requirement.to_string(),
        detail,
    };

    match fixture.expectation {
        Expectation::Accept(level) => match level.run(&fixture.archive) {
            Ok(()) => Ok(()),
            Err(err) => Err(fail(format!(
                "expected acceptance at level {:?} (ceiling stage {}), got rejection: code={} category={:?} stage={:?} message={}",
                level,
                level.ceiling_stage(),
                err.diagnostic.code,
                err.diagnostic.category,
                err.diagnostic.stage,
                err.diagnostic.message,
            ))),
        },
        Expectation::Reject { .. } => observe_fixture(fixture).map(|_| ()),
    }
}

/// The levels strictly below `level`, in ascending order.
///
/// Used by [`run_fixture`] to prove a negative fixture is rejected at the
/// *declared* level and not earlier.
fn lower_levels(level: Level) -> Vec<Level> {
    match level {
        Level::Inspect => Vec::new(),
        Level::Verify => vec![Level::Inspect],
        Level::Validate => vec![Level::Inspect, Level::Verify],
    }
}

/// Execute a whole corpus and collect every failure.
///
/// A corpus run does not stop at the first failure: an implementer fixing
/// conformance wants the full list in one pass. The returned vector is
/// ordered by the corpus order, so the report is stable across runs.
pub fn run_corpus(fixtures: &[Fixture]) -> Vec<Failure> {
    fixtures
        .iter()
        .filter_map(|f| run_fixture(f).err())
        .collect()
}

/// Check that every fixture id in the corpus is unique.
///
/// A duplicate id would make a failure report ambiguous, so the corpus
/// rejects it as a corpus bug rather than a conformance failure.
pub fn duplicate_ids(fixtures: &[Fixture]) -> Vec<&'static str> {
    let mut seen: Vec<&'static str> = fixtures.iter().map(|f| f.id).collect();
    seen.sort_unstable();
    let mut duplicates = Vec::new();
    let mut previous: Option<&'static str> = None;
    for id in seen {
        if Some(id) == previous && !duplicates.contains(&id) {
            duplicates.push(id);
        }
        previous = Some(id);
    }
    duplicates
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trivially-valid archive is not available here (the corpus crate's
    /// own fixtures live in sibling modules), so the level ladder is
    /// exercised with a deliberately broken archive that must be rejected
    /// at the very first stage.
    fn garbage() -> Vec<u8> {
        vec![0u8; 16]
    }

    #[test]
    fn level_ceilings_are_the_documented_stages() {
        assert_eq!(Level::Inspect.ceiling_stage(), 7);
        assert_eq!(Level::Verify.ceiling_stage(), 11);
        assert_eq!(Level::Validate.ceiling_stage(), 12);
    }

    #[test]
    fn lower_levels_are_cumulative() {
        assert!(lower_levels(Level::Inspect).is_empty());
        assert_eq!(lower_levels(Level::Verify), vec![Level::Inspect]);
        assert_eq!(
            lower_levels(Level::Validate),
            vec![Level::Inspect, Level::Verify]
        );
    }

    #[test]
    fn garbage_is_rejected_at_inspect() {
        // 16 zero bytes do not even carry a valid 32-byte header, so
        // the magic check fires before the size check. The actual
        // diagnostic is `HEADER_BAD_MAGIC`, not `ARCHIVE_TOO_SMALL` —
        // the previous test asserted only that the message mentioned
        // either code, masking that. Asserting `Ok` from `run_fixture`
        // IS the exact-code assertion: the runner returns `Err` on any
        // divergence, including "wrong code", "wrong category", or
        // "accepted instead of rejected".
        let fixture = Fixture {
            id: "test/garbage",
            description: "16 zero bytes are not an archive",
            requirements: &["REQ-PKG-001"],
            expectation: Expectation::Reject {
                level: Level::Inspect,
                code: "HEADER_BAD_MAGIC",
                category: Category::Container,
            },
            archive: garbage(),
        };
        match run_fixture(&fixture) {
            Ok(()) => {}
            Err(failure) => panic!("{failure}"),
        }
    }

    #[test]
    fn duplicate_ids_are_detected() {
        let mk = |id: &'static str| Fixture {
            id,
            description: "dup",
            requirements: &[],
            expectation: Expectation::Accept(Level::Inspect),
            archive: Vec::new(),
        };
        let corpus = vec![mk("a"), mk("b"), mk("a")];
        assert_eq!(duplicate_ids(&corpus), vec!["a"]);
        assert!(duplicate_ids(&[mk("a"), mk("b")]).is_empty());
    }

    // ----- Coverage-gap tests for the runner's failure paths ---------
    //
    // The runner's success paths are covered by the published corpus.
    // These tests drive the *divergence* paths — the arms that report
    // what the corpus expected versus what the reader or the parser
    // actually produced — so a runner that silently accepted a
    // mismatch cannot pass unnoticed.

    /// A valid ELD document: one consonant, one vowel.
    fn valid_eld_source() -> Vec<u8> {
        b"phonemes {\n    consonants {\n        phoneme \"p\" {\n            roman \"p\"\n        }\n    }\n    vowels {\n        phoneme \"a\" {\n            roman \"a\"\n        }\n    }\n}\n"
            .to_vec()
    }

    /// An ELD document whose phoneme carries an IPA symbol outside the
    /// v1 allowlist, so the ELD parser rejects it with
    /// `IPA_NOT_IN_REPERTOIRE`.
    fn non_allowlisted_ipa_source() -> Vec<u8> {
        b"phonemes {\n    consonants {\n        phoneme \"g\" {\n            roman \"q\"\n        }\n    }\n    vowels {\n        phoneme \"a\" {\n            roman \"a\"\n        }\n    }\n}\n"
            .to_vec()
    }

    #[test]
    fn coverage_gap_expectation_accessors_report_the_declared_level() {
        assert_eq!(Expectation::Accept(Level::Verify).level(), Level::Verify);
        assert_eq!(
            Expectation::Reject {
                level: Level::Inspect,
                code: "HEADER_BAD_MAGIC",
                category: Category::Container,
            }
            .level(),
            Level::Inspect
        );
        assert!(Expectation::Accept(Level::Inspect).is_accept());
        assert!(!Expectation::Reject {
            level: Level::Inspect,
            code: "HEADER_BAD_MAGIC",
            category: Category::Container,
        }
        .is_accept());
    }

    #[test]
    fn coverage_gap_document_fixture_reports_an_unexpected_rejection() {
        let fixture = DocumentFixture {
            id: "test/accept-but-rejected",
            description: "an ELD with a non-allowlisted IPA must not be accepted",
            requirements: &["REQ-ELD-001"],
            schema: Schema::Eld,
            source: non_allowlisted_ipa_source(),
            expectation: DocumentExpectation::Accept,
        };
        let failure = run_document_fixture(&fixture)
            .expect_err("a document with a non-allowlisted IPA must not be accepted");
        assert!(
            failure.detail.contains("expected acceptance"),
            "unexpected detail: {}",
            failure.detail
        );
    }

    #[test]
    fn coverage_gap_document_fixture_reports_a_code_mismatch() {
        let fixture = DocumentFixture {
            id: "test/code-mismatch",
            description: "the declared diagnostic code differs from the observed one",
            requirements: &["REQ-IPA-002"],
            schema: Schema::Eld,
            source: non_allowlisted_ipa_source(),
            expectation: DocumentExpectation::Reject {
                code: "SomeOtherCode",
                category: Category::Ipa,
            },
        };
        let failure = run_document_fixture(&fixture)
            .expect_err("the observed code differs from the declared one");
        assert!(
            failure.detail.contains("expected code SomeOtherCode"),
            "unexpected detail: {}",
            failure.detail
        );
    }

    #[test]
    fn coverage_gap_document_fixture_reports_a_category_mismatch() {
        let fixture = DocumentFixture {
            id: "test/category-mismatch",
            description: "the declared category differs from the observed one",
            requirements: &["REQ-IPA-002"],
            schema: Schema::Eld,
            source: non_allowlisted_ipa_source(),
            expectation: DocumentExpectation::Reject {
                code: "IPA_NOT_IN_REPERTOIRE",
                category: Category::EldSchema,
            },
        };
        let failure = run_document_fixture(&fixture)
            .expect_err("the observed category differs from the declared one");
        assert!(
            failure.detail.contains("expected category"),
            "unexpected detail: {}",
            failure.detail
        );
    }

    #[test]
    fn coverage_gap_document_fixture_reports_an_unexpected_acceptance() {
        let fixture = DocumentFixture {
            id: "test/reject-but-accepted",
            description: "a valid ELD must not be reported as a rejection",
            requirements: &["REQ-IPA-002"],
            schema: Schema::Eld,
            source: valid_eld_source(),
            expectation: DocumentExpectation::Reject {
                code: "IPA_NOT_IN_REPERTOIRE",
                category: Category::Ipa,
            },
        };
        let failure = run_document_fixture(&fixture)
            .expect_err("a valid document must not satisfy a rejection expectation");
        assert!(
            failure.detail.contains("parsed cleanly"),
            "unexpected detail: {}",
            failure.detail
        );
    }

    #[test]
    fn coverage_gap_vector_fixture_reports_a_mismatch() {
        let fixture = VectorFixture {
            id: "test/vector-mismatch".to_string(),
            description: "the observed bytes differ from the pinned bytes",
            requirements: &["REQ-BIN-001"],
            actual: vec![0x01],
            expected: vec![0x02],
        };
        assert_eq!(fixture.expected_label(), "02");
        assert_eq!(fixture.actual_label(), "01");
        let failure = run_vector_fixture(&fixture).expect_err("a byte mismatch must fail");
        assert!(
            failure.detail.contains("expected 02 got 01"),
            "unexpected detail: {}",
            failure.detail
        );
    }

    #[test]
    fn coverage_gap_hex_lower_encodes_bytes() {
        assert_eq!(hex_lower(&[]), "");
        assert_eq!(hex_lower(&[0x00, 0x0f, 0xff]), "000fff");
    }

    #[test]
    fn coverage_gap_failure_display_names_the_fixture_and_the_requirement() {
        let failure = Failure {
            fixture: "test/display".to_string(),
            requirement: "REQ-PKG-001".to_string(),
            detail: "a detail".to_string(),
        };
        let rendered = failure.to_string();
        assert!(rendered.contains("conformance failure"), "{rendered}");
        assert!(rendered.contains("test/display"), "{rendered}");
        assert!(rendered.contains("REQ-PKG-001"), "{rendered}");
        assert!(rendered.contains("a detail"), "{rendered}");
    }

    #[test]
    fn coverage_gap_observe_fixture_refuses_an_accept_fixture() {
        let fixture = Fixture {
            id: "test/accept-observed",
            description: "observe_fixture only handles negative fixtures",
            requirements: &["REQ-PKG-001"],
            expectation: Expectation::Accept(Level::Inspect),
            archive: garbage(),
        };
        let failure =
            observe_fixture(&fixture).expect_err("observe_fixture must refuse an Accept fixture");
        assert!(
            failure
                .detail
                .contains("observe_fixture called on an Accept fixture"),
            "unexpected detail: {}",
            failure.detail
        );
    }

    #[test]
    fn coverage_gap_observe_fixture_rejects_an_early_rejection() {
        let fixture = Fixture {
            id: "test/early-rejection",
            description: "garbage is rejected below the declared level",
            requirements: &["REQ-PKG-001"],
            expectation: Expectation::Reject {
                level: Level::Validate,
                code: "HEADER_BAD_MAGIC",
                category: Category::Container,
            },
            archive: garbage(),
        };
        let failure = observe_fixture(&fixture)
            .expect_err("a rejection below the declared level must fail the observation");
        assert!(
            failure
                .detail
                .contains("already rejected at the lower level"),
            "unexpected detail: {}",
            failure.detail
        );
    }

    #[test]
    fn coverage_gap_observe_fixture_reports_an_accepted_archive() {
        let archive = crate::golden_packages::minimal()
            .expect("minimal package builds")
            .archive;
        let fixture = Fixture {
            id: "test/accepted",
            description: "a valid golden package is accepted, not rejected",
            requirements: &["REQ-PKG-001"],
            expectation: Expectation::Reject {
                level: Level::Validate,
                code: "AnyCode",
                category: Category::Container,
            },
            archive,
        };
        let failure = observe_fixture(&fixture)
            .expect_err("a valid archive must not satisfy a rejection expectation");
        assert!(
            failure.detail.contains("but the archive was accepted"),
            "unexpected detail: {}",
            failure.detail
        );
    }

    #[test]
    fn coverage_gap_observe_fixture_reports_a_code_mismatch() {
        let fixture = Fixture {
            id: "test/observe-code-mismatch",
            description: "the declared code differs from the observed one",
            requirements: &["REQ-PKG-001"],
            expectation: Expectation::Reject {
                level: Level::Inspect,
                code: "NotTheCode",
                category: Category::Container,
            },
            archive: garbage(),
        };
        let failure =
            observe_fixture(&fixture).expect_err("the observed code differs from the declared one");
        assert!(
            failure.detail.contains("expected code NotTheCode"),
            "unexpected detail: {}",
            failure.detail
        );
    }

    #[test]
    fn coverage_gap_observe_fixture_reports_a_category_mismatch() {
        let fixture = Fixture {
            id: "test/observe-category-mismatch",
            description: "the declared category differs from the observed one",
            requirements: &["REQ-PKG-001"],
            expectation: Expectation::Reject {
                level: Level::Inspect,
                code: "HEADER_BAD_MAGIC",
                category: Category::Ipa,
            },
            archive: garbage(),
        };
        let failure = observe_fixture(&fixture)
            .expect_err("the observed category differs from the declared one");
        assert!(
            failure.detail.contains("expected category"),
            "unexpected detail: {}",
            failure.detail
        );
    }

    #[test]
    fn coverage_gap_run_fixture_reports_a_rejected_archive() {
        let fixture = Fixture {
            id: "test/rejected-accept",
            description: "garbage is not an acceptable archive",
            requirements: &["REQ-PKG-001"],
            expectation: Expectation::Accept(Level::Inspect),
            archive: garbage(),
        };
        let failure = run_fixture(&fixture).expect_err("garbage must not be accepted");
        assert!(
            failure.detail.contains("expected acceptance at level"),
            "unexpected detail: {}",
            failure.detail
        );
    }
}
