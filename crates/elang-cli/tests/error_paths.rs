//! Integration tests for the `elang` CLI's failure paths.
//!
//! Every test here drives the compiled binary and asserts on the exit code
//! and the diagnostic the CLI prints. The point of the file is the paths
//! that a happy-path smoke test never reaches: unreadable inputs, missing
//! flags, unwritable outputs, tampered archives and hostile output trees.
//!
//! The tests never write inside the repository: archives and scratch files
//! live in a per-test temporary directory that is removed on drop.

#![allow(clippy::expect_used, clippy::panic, clippy::arithmetic_side_effects)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

// Constructing a non-UTF-8 `OsStr` needs the platform's own extension
// trait: Unix takes raw bytes, Windows takes UTF-16. Only the Unix side is
// exercised here; the Windows runner skips the argument-decoding tests
// rather than carrying a second, differently-shaped implementation.
#[cfg(unix)]
use std::ffi::OsStr;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

use elang_container::ArchiveListing;

/// A minimal unique-directory abstraction, matching the one used by
/// `scaffold.rs` so the two files read the same way.
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// A path inside the temporary directory.
    fn child(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempDir {
    /// Remove the directory tree on drop.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Create a fresh temporary directory.
fn tempdir() -> TempDir {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let base = std::env::temp_dir();
    let unique = format!(
        "elang-cli-error-paths-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let path = base.join(unique);
    std::fs::create_dir_all(&path).expect("create tempdir");
    TempDir { path }
}

/// The path of the compiled `elang` binary.
fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_elang")
}

/// Run the CLI with the given arguments.
fn run(args: &[&str]) -> Output {
    Command::new(bin()).args(args).output().expect("run elang")
}

/// The captured stderr as lossy UTF-8.
fn stderr_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// The captured stdout as lossy UTF-8.
fn stdout_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// An argument that is not valid UTF-8.
///
/// `pico-args` reports `NonUtf8Argument` for both a non-UTF-8 subcommand and
/// a non-UTF-8 option value, which is the only way to reach the CLI's
/// argument-decoding failure paths.
///
/// Unix only: the raw-byte constructor does not exist on Windows, where an
/// `OsStr` is UTF-16 and a non-UTF-8 value needs a lone surrogate.
#[cfg(unix)]
fn non_utf8_arg() -> &'static OsStr {
    OsStr::from_bytes(&[0xff, 0xfe])
}

const EPF_FIXTURE: &[u8] = b"epf \"0.1.0\"\neld \"0.1.0\"\nlanguage-version \"1.0.0\"\nmetadata {\n    identifier \"urn:elang:language:rocky:test:1.0.0\"\n    name \"Writer Test\"\n}\nmanifest {\n    phonemes \"Linguistics/phonemes.eld\"\n}\n";

const ELD_FIXTURE: &[u8] = b"phonemes {\n    consonants {\n        phoneme \"p\" {\n            roman \"p\"\n        }\n    }\n    vowels {\n        phoneme \"a\" {\n            roman \"a\"\n        }\n    }\n}\n";

/// The same EPF, but carrying an optional `description`.
const EPF_WITH_DESCRIPTION: &[u8] = b"epf \"0.1.0\"\neld \"0.1.0\"\nlanguage-version \"1.0.0\"\nmetadata {\n    identifier \"urn:elang:language:rocky:test:1.0.0\"\n    name \"Writer Test\"\n    description \"A small constructed language.\"\n}\nmanifest {\n    phonemes \"Linguistics/phonemes.eld\"\n}\n";

/// Pack the fixture EPF + ELD into a valid `.elang` archive.
fn build_archive(tmp: &TempDir) -> PathBuf {
    build_archive_from(tmp, EPF_FIXTURE)
}

/// Pack the given EPF plus the fixture ELD into a valid `.elang` archive.
fn build_archive_from(tmp: &TempDir, epf_bytes: &[u8]) -> PathBuf {
    let epf = tmp.child("build-epf.bin");
    let eld = tmp.child("build-eld.bin");
    let out = tmp.child("build.elang");
    std::fs::write(&epf, epf_bytes).expect("write epf");
    std::fs::write(&eld, ELD_FIXTURE).expect("write eld");
    let status = Command::new(bin())
        .args(["pack", "--epf"])
        .arg(&epf)
        .arg("--eld")
        .arg(&eld)
        .arg("--output")
        .arg(&out)
        .output()
        .expect("run elang pack");
    assert_eq!(
        status.status.code(),
        Some(0),
        "pack should succeed, stderr={}",
        stderr_of(&status)
    );
    out
}

/// Pack an archive that carries one extra resource of `size` bytes.
///
/// The writer API is used directly because the CLI has no flag for extra
/// resources; the payload is what makes the extraction hit a file-size
/// limit.
///
/// Linux only: the only consumer is the `ulimit -f` test below.
#[cfg(target_os = "linux")]
fn build_archive_with_a_large_resource(tmp: &TempDir, size: usize) -> PathBuf {
    let epf = elang_epf::parse(EPF_FIXTURE).expect("parse epf");
    let repertoire = elang_repertoire::repertoire::load().expect("load repertoire");
    let document = elang_eld::parse(ELD_FIXTURE, &repertoire).expect("parse eld");
    let eld = elang_eld::inventory::from_document(&document, &repertoire).expect("build inventory");
    let payload: Vec<u8> = (0..size)
        .map(|i| u8::try_from(i % 251).unwrap_or(0))
        .collect();
    let config = elang_writer::PackageConfig {
        epf,
        eld,
        extra_resources: vec![elang_writer::ExtraResource {
            path: "big.bin".to_string(),
            canonical_bytes: payload,
        }],
        profile: elang_writer::Profile::Fast,
        format_version: "0.1.0".to_string(),
    };
    let bytes = elang_writer::write_archive(&config).expect("write archive");
    let out = tmp.child("big.elang");
    std::fs::write(&out, &bytes).expect("write archive file");
    out
}

// ---------------------------------------------------------------------------
// Argument decoding
// ---------------------------------------------------------------------------

// Unix only: a non-UTF-8 subcommand can only be built from raw bytes.
#[cfg(unix)]
#[test]
/// `elang rejects a non utf-8 subcommand and exits two`.
fn a_non_utf8_subcommand_is_reported_and_exits_two() {
    let out = Command::new(bin())
        .arg(non_utf8_arg())
        .output()
        .expect("run elang");
    assert_eq!(
        out.status.code(),
        Some(2),
        "a non-UTF-8 subcommand should exit 2, stderr={}",
        stderr_of(&out)
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("failed to read subcommand"),
        "stderr should report the argument-decoding failure: {stderr}"
    );
}

// Unix only: a non-UTF-8 subcommand can only be built from raw bytes.
#[cfg(unix)]
#[test]
/// `elang json mode reports a non utf-8 subcommand as a dispatch envelope`.
fn a_non_utf8_subcommand_emits_a_dispatch_envelope_in_json_mode() {
    let out = Command::new(bin())
        .arg("--json")
        .arg(non_utf8_arg())
        .output()
        .expect("run elang");
    assert_eq!(
        out.status.code(),
        Some(2),
        "a non-UTF-8 subcommand should exit 2, stderr={}",
        stderr_of(&out)
    );
    let stdout = stdout_of(&out);
    assert!(
        stdout.contains("\"command\": \"<dispatch>\""),
        "the envelope should name the dispatch failure: {stdout}"
    );
    assert!(
        stdout.contains("\"ok\": false"),
        "the envelope should report failure: {stdout}"
    );
}

// Unix only: a non-UTF-8 option value can only be built from raw bytes.
#[cfg(unix)]
#[test]
/// `elang pack rejects a non utf-8 option value and exits two`.
fn pack_rejects_a_non_utf8_option_value_and_exits_two() {
    let out = Command::new(bin())
        .args(["pack", "--epf"])
        .arg(non_utf8_arg())
        .output()
        .expect("run elang pack");
    assert_eq!(
        out.status.code(),
        Some(2),
        "a non-UTF-8 option value should exit 2, stderr={}",
        stderr_of(&out)
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("--epf:"),
        "stderr should name the offending option: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Missing flags
// ---------------------------------------------------------------------------

#[test]
/// `elang pack without an eld flag exits two`.
fn pack_without_an_eld_flag_exits_two() {
    let tmp = tempdir();
    let epf = tmp.child("epf.bin");
    std::fs::write(&epf, EPF_FIXTURE).expect("write epf");
    let out = run(&["pack", "--epf", &epf.to_string_lossy()]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "pack without --eld should exit 2, stderr={}",
        stderr_of(&out)
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("--eld <value> is required"),
        "stderr should report the missing flag: {stderr}"
    );
}

#[test]
/// `elang pack without an output flag exits two`.
fn pack_without_an_output_flag_exits_two() {
    let tmp = tempdir();
    let epf = tmp.child("epf.bin");
    let eld = tmp.child("eld.bin");
    std::fs::write(&epf, EPF_FIXTURE).expect("write epf");
    std::fs::write(&eld, ELD_FIXTURE).expect("write eld");
    let out = run(&[
        "pack",
        "--epf",
        &epf.to_string_lossy(),
        "--eld",
        &eld.to_string_lossy(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "pack without --output should exit 2, stderr={}",
        stderr_of(&out)
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("--output <value> is required"),
        "stderr should report the missing flag: {stderr}"
    );
}

// Unix only: a non-UTF-8 option value can only be built from raw bytes.
#[cfg(unix)]
#[test]
/// `elang unpack rejects a non utf-8 output value and exits two`.
fn unpack_rejects_a_non_utf8_output_value_and_exits_two() {
    let tmp = tempdir();
    let archive = build_archive(&tmp);
    let out = Command::new(bin())
        .args(["unpack"])
        .arg(&archive)
        .args(["--output"])
        .arg(non_utf8_arg())
        .output()
        .expect("run elang unpack");
    assert_eq!(
        out.status.code(),
        Some(2),
        "a non-UTF-8 --output should exit 2, stderr={}",
        stderr_of(&out)
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("--output:"),
        "stderr should name the offending option: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Unreadable and unwritable inputs
// ---------------------------------------------------------------------------

#[test]
/// `elang pack reports an unwritable output path and exits one`.
fn pack_reports_an_unwritable_output_path_and_exits_one() {
    let tmp = tempdir();
    let epf = tmp.child("epf.bin");
    let eld = tmp.child("eld.bin");
    std::fs::write(&epf, EPF_FIXTURE).expect("write epf");
    std::fs::write(&eld, ELD_FIXTURE).expect("write eld");
    // The parent directory does not exist, so the write cannot succeed.
    let target = tmp.child("missing-dir/out.elang");
    let out = run(&[
        "pack",
        "--epf",
        &epf.to_string_lossy(),
        "--eld",
        &eld.to_string_lossy(),
        "--output",
        &target.to_string_lossy(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "pack to an unwritable path should exit 1, stderr={}",
        stderr_of(&out)
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("write "),
        "stderr should report the failed write: {stderr}"
    );
}

#[test]
/// `elang verify reports a missing archive and exits one`.
fn verify_reports_a_missing_archive_and_exits_one() {
    let tmp = tempdir();
    let out = run(&["verify", &tmp.child("absent.elang").to_string_lossy()]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "verify on a missing file should exit 1, stderr={}",
        stderr_of(&out)
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("read "),
        "stderr should report the failed read: {stderr}"
    );
}

#[test]
/// `elang validate reports a missing archive and exits one`.
fn validate_reports_a_missing_archive_and_exits_one() {
    let tmp = tempdir();
    let out = run(&["validate", &tmp.child("absent.elang").to_string_lossy()]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "validate on a missing file should exit 1, stderr={}",
        stderr_of(&out)
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("read "),
        "stderr should report the failed read: {stderr}"
    );
}

#[test]
/// `elang validate reports an unreadable archive and exits one`.
fn validate_reports_an_unreadable_archive_and_exits_one() {
    let tmp = tempdir();
    let junk = tmp.child("junk.elang");
    std::fs::write(&junk, b"not an archive at all").expect("write junk");
    let out = run(&["validate", &junk.to_string_lossy()]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "validate on junk should exit 1, stderr={}",
        stderr_of(&out)
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("open "),
        "stderr should report the failed open: {stderr}"
    );
}

#[test]
/// `elang unpack reports a missing archive and exits one`.
fn unpack_reports_a_missing_archive_and_exits_one() {
    let tmp = tempdir();
    let out = run(&[
        "unpack",
        &tmp.child("absent.elang").to_string_lossy(),
        "--output",
        &tmp.child("out").to_string_lossy(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "unpack on a missing file should exit 1, stderr={}",
        stderr_of(&out)
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("read "),
        "stderr should report the failed read: {stderr}"
    );
}

#[test]
/// `elang unpack reports an unreadable archive and exits one`.
fn unpack_reports_an_unreadable_archive_and_exits_one() {
    let tmp = tempdir();
    let junk = tmp.child("junk.elang");
    std::fs::write(&junk, b"not an archive at all").expect("write junk");
    let out = run(&[
        "unpack",
        &junk.to_string_lossy(),
        "--output",
        &tmp.child("out").to_string_lossy(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "unpack on junk should exit 1, stderr={}",
        stderr_of(&out)
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("open "),
        "stderr should report the failed open: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Validation output
// ---------------------------------------------------------------------------

#[test]
/// `elang validate prints the epf description when the epf carries one`.
fn validate_prints_the_epf_description_when_the_epf_carries_one() {
    let tmp = tempdir();
    let archive = build_archive_from(&tmp, EPF_WITH_DESCRIPTION);
    let out = run(&["validate", &archive.to_string_lossy()]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "validate should exit 0, stderr={}",
        stderr_of(&out)
    );
    let stdout = stdout_of(&out);
    assert!(
        stdout.contains("description: A small constructed language."),
        "stdout should report the EPF description: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// Unpack: verification and hostile output trees
// ---------------------------------------------------------------------------

/// Corrupt the payload of one resource frame without disturbing the frame
/// structure, so the archive still opens but no longer matches its
/// integrity manifest.
///
/// The frame's trailing four bytes are its content checksum; the byte just
/// before them is the last byte of the payload, which is where the tamper
/// goes.
fn tamper_with_a_resource_frame(archive: &Path, target: &Path) {
    let bytes = std::fs::read(archive).expect("read archive");
    let listing = ArchiveListing::parse(&bytes).expect("parse archive listing");
    let entry = listing
        .listing()
        .into_iter()
        .find(|e| e.path == "Header/mimetype")
        .expect("archive carries the mimetype resource");
    let last_payload_byte = entry.frame_offset + entry.compressed_size - 5;
    let position = usize::try_from(last_payload_byte).expect("frame offset fits in usize");
    let mut tampered = bytes;
    let byte = tampered
        .get_mut(position)
        .expect("tamper position is inside the archive");
    *byte = byte.wrapping_add(1);
    std::fs::write(target, &tampered).expect("write tampered archive");
}

#[test]
/// `elang unpack refuses to extract an archive that fails verification`.
fn unpack_refuses_to_extract_an_archive_that_fails_verification() {
    let tmp = tempdir();
    let archive = build_archive(&tmp);
    let tampered = tmp.child("tampered.elang");
    tamper_with_a_resource_frame(&archive, &tampered);

    let out = run(&[
        "unpack",
        &tampered.to_string_lossy(),
        "--output",
        &tmp.child("out").to_string_lossy(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "unpack of a tampered archive should exit 1, stderr={}",
        stderr_of(&out)
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("verify "),
        "stderr should report the verification failure: {stderr}"
    );
    assert!(
        !tmp.child("out/Header/mimetype").exists(),
        "nothing may be extracted from an archive that fails verification"
    );
}

#[test]
/// `elang unpack reports an output directory it cannot create`.
fn unpack_reports_an_output_directory_it_cannot_create() {
    let tmp = tempdir();
    let archive = build_archive(&tmp);
    // A regular file stands where the output directory's parent should be,
    // so creating the output directory cannot succeed.
    let blocker = tmp.child("blocker");
    std::fs::write(&blocker, b"in the way").expect("write blocker");
    let out = run(&[
        "unpack",
        &archive.to_string_lossy(),
        "--output",
        &blocker.join("out").to_string_lossy(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "unpack into an uncreatable directory should exit 1, stderr={}",
        stderr_of(&out)
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("create "),
        "stderr should report the failed directory creation: {stderr}"
    );
}

#[test]
/// `elang unpack reports an output directory it cannot resolve`.
fn unpack_reports_an_output_directory_it_cannot_resolve() {
    let tmp = tempdir();
    let archive = build_archive(&tmp);
    // An empty output path creates nothing and resolves to nothing.
    let out = run(&["unpack", &archive.to_string_lossy(), "--output", ""]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "unpack with an empty --output should exit 1, stderr={}",
        stderr_of(&out)
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("canonicalize "),
        "stderr should report the failed canonicalization: {stderr}"
    );
}

// Unix only: POSIX symlinks. Windows needs a privilege to create one.
#[cfg(unix)]
#[test]
/// `elang unpack refuses to write through a symlinked parent directory`.
fn unpack_refuses_to_write_through_a_symlinked_parent_directory() {
    let tmp = tempdir();
    let archive = build_archive(&tmp);
    let out_dir = tmp.child("out");
    std::fs::create_dir_all(&out_dir).expect("create out dir");
    let outside = tmp.child("outside");
    std::fs::create_dir_all(&outside).expect("create outside dir");
    // `Linguistics` is a directory in the archive; pointing it outside the
    // output tree must be refused.
    std::os::unix::fs::symlink(&outside, out_dir.join("Linguistics")).expect("create symlink");

    let out = run(&[
        "unpack",
        &archive.to_string_lossy(),
        "--output",
        &out_dir.to_string_lossy(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "unpack through an escaping symlink should exit 1, stderr={}",
        stderr_of(&out)
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("unsafe extraction"),
        "stderr should report the unsafe extraction: {stderr}"
    );
    assert!(
        !outside.join("phonemes.eld").exists(),
        "nothing may be written outside the output tree"
    );
}

// Unix only: POSIX symlinks. Windows needs a privilege to create one.
#[cfg(unix)]
#[test]
/// `elang unpack follows a symlinked parent directory that stays inside the output tree`.
fn unpack_follows_a_symlinked_parent_directory_inside_the_output_tree() {
    let tmp = tempdir();
    let archive = build_archive(&tmp);
    let out_dir = tmp.child("out");
    let inner = out_dir.join("inner");
    std::fs::create_dir_all(&inner).expect("create inner dir");
    // `Linguistics` is a directory in the archive; pointing it at another
    // directory *inside* the output tree is allowed.
    std::os::unix::fs::symlink(&inner, out_dir.join("Linguistics")).expect("create symlink");

    let out = run(&[
        "unpack",
        &archive.to_string_lossy(),
        "--output",
        &out_dir.to_string_lossy(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "unpack through an in-tree symlink should exit 0, stderr={}",
        stderr_of(&out)
    );
    assert!(
        inner.join("phonemes.eld").is_file(),
        "the resource should be extracted through the in-tree symlink"
    );
}

/// Restores a directory's write permission when it goes out of scope.
///
/// Unix only: the mode is a POSIX permission bit set.
#[cfg(unix)]
struct WritePermGuard {
    path: PathBuf,
}

#[cfg(unix)]
impl Drop for WritePermGuard {
    /// Restore the directory mode so the temporary directory can be removed.
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o755));
    }
}

// Unix only: removing write permission is a POSIX mode bit.
#[cfg(unix)]
#[test]
/// `elang unpack reports a directory it cannot create inside the output tree`.
fn unpack_reports_a_directory_it_cannot_create_inside_the_output_tree() {
    let tmp = tempdir();
    let archive = build_archive(&tmp);
    let out_dir = tmp.child("out");
    std::fs::create_dir_all(&out_dir).expect("create out dir");
    // The output tree is searchable but not writable, so creating the
    // per-resource directories inside it fails.
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&out_dir, std::fs::Permissions::from_mode(0o555))
        .expect("chmod out dir");
    let _guard = WritePermGuard {
        path: out_dir.clone(),
    };
    // A privileged process bypasses the mode bits, so the setup below
    // cannot make the CLI fail. Probe for that and skip the test rather
    // than asserting an outcome the environment cannot produce.
    let probe = out_dir.join("probe");
    if std::fs::create_dir(&probe).is_ok() {
        let _ = std::fs::remove_dir(&probe);
        eprintln!("skipped: this process can write to a 0555 directory");
        return;
    }

    let out = run(&[
        "unpack",
        &archive.to_string_lossy(),
        "--output",
        &out_dir.to_string_lossy(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "unpack into a read-only directory should exit 1, stderr={}",
        stderr_of(&out)
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("create "),
        "stderr should report the failed directory creation: {stderr}"
    );
}

#[test]
/// `elang unpack force reports a target it cannot remove`.
fn unpack_force_reports_a_target_it_cannot_remove() {
    let tmp = tempdir();
    let archive = build_archive(&tmp);
    let out_dir = tmp.child("out");
    // A directory where a resource file belongs: `--force` tries to remove
    // it first, and removing a directory is not allowed.
    std::fs::create_dir_all(out_dir.join("Header/mimetype")).expect("create blocking directory");

    let out = run(&[
        "unpack",
        &archive.to_string_lossy(),
        "--output",
        &out_dir.to_string_lossy(),
        "--force",
    ]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "unpack --force over a directory should exit 1, stderr={}",
        stderr_of(&out)
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("remove "),
        "stderr should report the failed removal: {stderr}"
    );
}

#[test]
/// `elang unpack reports a resource it cannot create`.
fn unpack_reports_a_resource_it_cannot_create() {
    let tmp = tempdir();
    let archive = build_archive(&tmp);
    let out_dir = tmp.child("out");
    std::fs::create_dir_all(&out_dir).expect("create out dir");
    // A regular file where the `Linguistics` directory belongs: the
    // resource file underneath it can never be created.
    std::fs::write(out_dir.join("Linguistics"), b"in the way").expect("write blocker");

    let out = run(&[
        "unpack",
        &archive.to_string_lossy(),
        "--output",
        &out_dir.to_string_lossy(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "unpack over a blocked resource should exit 1, stderr={}",
        stderr_of(&out)
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("create "),
        "stderr should report the failed creation: {stderr}"
    );
}

// Linux only: `ulimit -f` raises SIGXFSZ on exceeding the ceiling. Linux
// returns EFBIG from the write when the signal is ignored; on macOS the
// process is killed by the signal instead, so the CLI never reaches its
// own diagnostic. The behaviour under test is Linux's.
#[cfg(target_os = "linux")]
#[test]
/// `elang unpack reports a resource it cannot write`.
fn unpack_reports_a_resource_it_cannot_write() {
    // `ulimit -f` counts 512-byte blocks, so 2048 blocks is a 1 MiB ceiling
    // on every file the CLI writes. The archive carries a resource one byte
    // larger than that, so writing it fails with EFBIG. SIGXFSZ is ignored
    // so the CLI survives to report the failure instead of being killed by
    // the signal, and the ceiling is far above the size of the coverage
    // profile the process writes on exit.
    const LIMIT_BLOCKS: u32 = 2048;
    const LIMIT_BYTES: usize = 2048 * 512;

    let tmp = tempdir();
    let archive = build_archive_with_a_large_resource(&tmp, LIMIT_BYTES + 1);
    let out_dir = tmp.child("out");
    // The paths are passed as positional parameters rather than
    // interpolated into the script text: a temporary path containing an
    // apostrophe would otherwise terminate the shell quote.
    let script = format!(
        "trap '' XFSZ; ulimit -f {LIMIT_BLOCKS}; exec \"$1\" unpack \"$2\" --output \"$3\""
    );
    let out = Command::new("sh")
        .arg("-c")
        .arg(&script)
        .arg("sh")
        .arg(bin())
        .arg(&archive)
        .arg(&out_dir)
        .output()
        .expect("run elang under a file-size limit");
    assert_eq!(
        out.status.code(),
        Some(1),
        "unpack under a file-size limit should exit 1, stderr={}",
        stderr_of(&out)
    );
    let stderr = stderr_of(&out);
    assert!(
        stderr.contains("write "),
        "stderr should report the failed write: {stderr}"
    );
}
