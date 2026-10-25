//! # elang CLI
//!
//! Command-line front end for the ELANG v0.1.0 package tooling (Phase 13,
//! T1311–T1316). Implemented subcommands in this crate:
//!
//! - `pack` — assemble a `.elang` archive from `--epf` + `--eld` inputs and
//!   write it to `--output`.
//! - `inspect` — print the archive's header, mimetype, resource listing and
//!   total uncompressed size without decoding resources.
//! - `verify` — decode every resource, recompute SHA-256, and compare against
//!   the integrity manifest.
//! - `validate` — full semantic validation: structural integrity plus EPF and
//!   ELD model checks.
//! - `unpack` — safe extraction of every resource to a directory, with
//!   symlink-escape, path-traversal and overwrite checks.
//! - `canonicalize-eld` — read a phoneme inventory and print its canonical
//!   serialized form to stdout.
//! - `repertoire info` — print the pinned IPA repertoire metadata.
//! - `help` — usage.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use elang_writer::{write_archive, PackageConfig, Profile};
use pico_args::Arguments;

mod json;

/// Diagnostic output macro.
///
/// In human mode this is `eprintln!`. In `--json` mode the formatted
/// message is buffered into the envelope's `error.message` field.
/// Use [`diag_code!`] when a stable error code is available.
#[macro_export]
macro_rules! diag {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $crate::json::diagnostic(&msg);
    }};
}

/// Human-mode USAGE printer. Used by `--help` paths inside run_*.
///
/// In human mode this prints to stdout (the established contract; the
/// help output is greppable and script-friendly). In `--json` mode the
/// usage text is suppressed — a JSON consumer that asked for `--help`
/// already has the help text available in the human-mode form, and
/// putting it into the envelope would defeat the point of an
/// exit-0/no-error envelope.
#[macro_export]
macro_rules! husage {
    ($($arg:tt)*) => {{
        if !$crate::json::json_mode() {
            print!($($arg)*);
        }
    }};
}

/// Human-mode `print!` (no trailing newline). Suppressed in `--json` mode.
#[macro_export]
macro_rules! hprint {
    ($($arg:tt)*) => {{
        if !$crate::json::json_mode() {
            print!($($arg)*);
        }
    }};
}

/// Human-mode `println!`. In `--json` mode this is a no-op so the
/// JSON envelope is the only thing written to stdout.
#[macro_export]
macro_rules! hprintln {
    ($($arg:tt)*) => {{
        if !$crate::json::json_mode() {
            println!($($arg)*);
        }
    }};
}

/// Diagnostic output with a stable error code (T1609).
///
/// `diag_code!` carries the same shape as `diag!`, but the *first*
/// argument is the OA-10 error-code registry code that identifies the
/// failure, and the rest is the formatted message. The code is the
/// machine-readable part of the diagnostic; the message is display text.
#[macro_export]
macro_rules! diag_code {
    ($code:expr, $($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $crate::json::diagnostic_with_code($code, &msg);
    }};
}

const HELP: &str = "\
elang - ELANG v0.1.0 package tooling

USAGE:
    elang <SUBCOMMAND> [OPTIONS]

Subcommands:
    pack              Build a .elang archive from --epf + --eld inputs
    inspect           List resources and metadata of a .elang archive
    verify            Verify integrity of a .elang archive against its manifest
    validate          Validate the EPF + ELD semantic content of a .elang archive
    unpack            Extract a .elang archive to a directory
    canonicalize-eld  Print the canonical form of a phoneme inventory
    repertoire info   Print the pinned IPA repertoire metadata
    help              Print this message

Run 'elang <subcommand> --help' for subcommand-specific options.
";

const PACK_USAGE: &str = "\
usage: elang pack --epf <file> --eld <file> --output <file>
                 [--profile fast|compact|canonical] [--format-version <version>]
";

const INSPECT_USAGE: &str = "usage: elang inspect <archive>\n";

const VERIFY_USAGE: &str = "usage: elang verify <archive>\n";

const VALIDATE_USAGE: &str = "usage: elang validate <archive>\n";

const UNPACK_USAGE: &str = "usage: elang unpack <archive> --output <dir> [--force]\n";

const CANONICALIZE_ELD_USAGE: &str = "usage: elang canonicalize-eld <ELD-file>\n";

const REPERTOIRE_INFO_USAGE: &str = "usage: elang repertoire info\n";

/// Subcommand that the dispatch table matched on.
enum Subcommand {
    Pack,
    Inspect,
    Verify,
    Validate,
    Unpack,
    CanonicalizeEld,
    Repertoire,
    Help,
}

/// Parse the first non-flag argument as a known subcommand.
fn parse_subcommand(sub: Option<&str>) -> Result<Subcommand, String> {
    match sub {
        Some("pack") => Ok(Subcommand::Pack),
        Some("inspect") => Ok(Subcommand::Inspect),
        Some("verify") => Ok(Subcommand::Verify),
        Some("validate") => Ok(Subcommand::Validate),
        Some("unpack") => Ok(Subcommand::Unpack),
        Some("canonicalize-eld") => Ok(Subcommand::CanonicalizeEld),
        Some("repertoire") => Ok(Subcommand::Repertoire),
        Some("help") => Ok(Subcommand::Help),
        Some(other) => Err(format!("unknown subcommand: {other:?}")),
        None => Ok(Subcommand::Help),
    }
}
/// Dispatch the first positional argument as a subcommand.
fn main() -> ExitCode {
    let mut args = Arguments::from_env();

    // The `--json` flag is global. It must be detected before any
    // diagnostic is emitted so the JSON envelope can capture them all.
    json::set_json_mode(args.contains("--json"));

    let sub = match args.subcommand() {
        Ok(s) => s,
        Err(e) => {
            json::diagnostic(&format!("failed to read subcommand: {e}"));
            if json::json_mode() {
                json::emit_envelope("<dispatch>", 2);
            }
            return ExitCode::from(2);
        }
    };
    let command_name = sub.clone().unwrap_or_else(|| "help".into());
    let exit_code: u8 = match parse_subcommand(sub.as_deref()) {
        Ok(Subcommand::Pack) => run_pack(&mut args),
        Ok(Subcommand::Inspect) => run_inspect(&mut args),
        Ok(Subcommand::Verify) => run_verify(&mut args),
        Ok(Subcommand::Validate) => run_validate(&mut args),
        Ok(Subcommand::Unpack) => run_unpack(&mut args),
        Ok(Subcommand::CanonicalizeEld) => run_canonicalize_eld(&mut args),
        Ok(Subcommand::Repertoire) => run_repertoire(&mut args),
        Ok(Subcommand::Help) => {
            // In human mode the help text *is* the output, printed to stdout
            // and greppable. In `--json` mode the documented contract is that
            // stdout carries exactly one JSON object and nothing else, so the
            // help text is suppressed and the envelope stands alone.
            if json::json_mode() {
                0
            } else {
                print!("{HELP}");
                return ExitCode::from(0);
            }
        }
        Err(msg) => {
            json::diagnostic(&msg);
            if !json::json_mode() {
                eprint!("{HELP}");
            }
            2
        }
    };
    if json::json_mode() {
        json::emit_envelope(&command_name, exit_code);
    }
    ExitCode::from(exit_code)
}

/// Read a required flag value or print a usage error and set `err`.
fn required_opt<T: std::str::FromStr>(
    args: &mut Arguments,
    flag: &'static str,
    err: &mut Option<u8>,
) -> Option<T>
where
    T::Err: std::fmt::Display,
{
    match args.opt_value_from_str::<_, T>(flag) {
        Ok(Some(v)) => Some(v),
        Ok(None) => {
            diag!("{flag} <value> is required");
            *err = Some(2);
            None
        }
        Err(e) => {
            diag!("{flag}: {e}");
            *err = Some(2);
            None
        }
    }
}
/// Implement the `pack` subcommand end to end.
fn run_pack(args: &mut Arguments) -> u8 {
    if args.contains(["-h", "--help"]) {
        husage!("{PACK_USAGE}");
        return 0;
    }

    let mut err: Option<u8> = None;

    let epf_path: PathBuf = match required_opt::<PathBuf>(args, "--epf", &mut err) {
        Some(p) => p,
        None => return err.unwrap_or(2),
    };
    let eld_path: PathBuf = match required_opt::<PathBuf>(args, "--eld", &mut err) {
        Some(p) => p,
        None => return err.unwrap_or(2),
    };
    let output: PathBuf = match required_opt::<PathBuf>(args, "--output", &mut err) {
        Some(p) => p,
        None => return err.unwrap_or(2),
    };

    let format_version: String = match args.opt_value_from_str::<_, String>("--format-version") {
        Ok(Some(v)) => v,
        Ok(None) => "0.1.0".to_string(),
        Err(e) => {
            diag!("--format-version: {e}");
            return 2;
        }
    };

    let profile: Profile = match args.opt_value_from_str::<_, String>("--profile") {
        Ok(Some(s)) => match s.as_str() {
            "fast" => Profile::Fast,
            "compact" => Profile::Compact,
            "canonical" => Profile::Canonical,
            other => {
                diag!("--profile: unknown profile {other:?} (expected fast|compact|canonical)");
                return 2;
            }
        },
        Ok(None) => Profile::Fast,
        Err(e) => {
            diag!("--profile: {e}");
            return 2;
        }
    };

    // Surface any leftover positionals as a usage error.
    let leftover: Vec<String> = args
        .clone()
        .finish()
        .into_iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    if !leftover.is_empty() {
        diag!("unexpected positional arguments: {leftover:?}");
        return 2;
    }

    let epf_bytes = match fs::read(&epf_path) {
        Ok(b) => b,
        Err(e) => {
            diag!("read {}: {e}", epf_path.display());
            return 1;
        }
    };
    let eld_bytes = match fs::read(&eld_path) {
        Ok(b) => b,
        Err(e) => {
            diag!("read {}: {e}", eld_path.display());
            return 1;
        }
    };

    let epf = match elang_epf::parse(&epf_bytes) {
        Ok(p) => p,
        Err(e) => {
            diag_code!(e.code(), "parse {}: {e}", epf_path.display());
            return 1;
        }
    };

    let rep = match elang_repertoire::repertoire::load() {
        Ok(r) => r,
        Err(e) => {
            diag!("load IPA repertoire: {e:?}");
            return 1;
        }
    };
    let eld_doc = match elang_eld::parse(&eld_bytes, &rep) {
        Ok(d) => d,
        Err(e) => {
            diag_code!(e.code(), "parse {}: {e}", eld_path.display());
            return 1;
        }
    };
    let eld = match elang_eld::inventory::from_document(&eld_doc, &rep) {
        Ok(i) => i,
        Err(e) => {
            diag_code!(e.code(), "validate {}: {e}", eld_path.display());
            return 1;
        }
    };

    let config = PackageConfig {
        epf,
        eld,
        extra_resources: Vec::new(),
        profile,
        format_version,
    };

    let bytes = match write_archive(&config) {
        Ok(b) => b,
        Err(e) => {
            diag_code!(e.code(), "write archive: {e}");
            return 1;
        }
    };

    if let Err(e) = fs::write(&output, &bytes) {
        diag!("write {}: {e}", output.display());
        return 1;
    }
    0
}
/// Implement the `canonicalize-eld` subcommand.
fn run_canonicalize_eld(args: &mut Arguments) -> u8 {
    if args.contains(["-h", "--help"]) {
        husage!("{CANONICALIZE_ELD_USAGE}");
        return 0;
    }

    let operands = args.clone().finish();
    let input: PathBuf = match operands.as_slice() {
        [input] => PathBuf::from(input),
        _ => {
            diag!("{CANONICALIZE_ELD_USAGE}");
            return 2;
        }
    };

    let bytes = match fs::read(&input) {
        Ok(b) => b,
        Err(e) => {
            diag!("read {}: {e}", input.display());
            return 1;
        }
    };

    let rep = match elang_repertoire::repertoire::load() {
        Ok(r) => r,
        Err(e) => {
            diag!("load IPA repertoire: {e:?}");
            return 1;
        }
    };
    let doc = match elang_eld::parse(&bytes, &rep) {
        Ok(d) => d,
        Err(e) => {
            diag_code!(e.code(), "parse {}: {e}", input.display());
            return 1;
        }
    };
    let inv = match elang_eld::inventory::from_document(&doc, &rep) {
        Ok(i) => i,
        Err(e) => {
            diag_code!(e.code(), "validate {}: {e}", input.display());
            return 1;
        }
    };

    let canonical = elang_eld::canon::to_string(&inv);
    hprint!("{canonical}");
    0
}

/// Read an archive file into memory, reporting a read failure on stderr.
fn read_archive_file(path: &std::path::Path) -> Result<Vec<u8>, u8> {
    fs::read(path).map_err(|e| {
        diag!("read {}: {e}", path.display());
        1
    })
}

/// Open an archive for inspection, reporting a parse failure on stderr.
fn open_package(path: &std::path::Path, bytes: &[u8]) -> Result<elang_reader::Package, u8> {
    elang_reader::Package::open(bytes).map_err(|e| {
        diag_code!(e.code(), "open {}: {e}", path.display());
        1
    })
}

/// Take exactly one positional operand, printing usage on anything else.
fn single_operand(args: &mut Arguments, usage: &str) -> Result<PathBuf, u8> {
    let operands: Vec<String> = args
        .clone()
        .finish()
        .into_iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    match operands.as_slice() {
        [one] => Ok(PathBuf::from(one)),
        _ => {
            diag!("{usage}");
            Err(2)
        }
    }
}

/// Print an index-only listing of the archive: header version, resource
/// count, total uncompressed size and one line per indexed resource.
///
/// This is the cheapest of the reader-side subcommands: it decompresses no
/// frame body, so it stays fast on large archives.
fn run_inspect(args: &mut Arguments) -> u8 {
    if args.contains(["-h", "--help"]) {
        husage!("{INSPECT_USAGE}");
        return 0;
    }
    let path = match single_operand(args, INSPECT_USAGE) {
        Ok(p) => p,
        Err(code) => return code,
    };
    let bytes = match read_archive_file(&path) {
        Ok(b) => b,
        Err(code) => return code,
    };
    let pkg = match open_package(&path, &bytes) {
        Ok(p) => p,
        Err(code) => return code,
    };
    let inspected = pkg.inspect();
    let (major, minor) = inspected.header_version();
    hprintln!("archive: {}", path.display());
    hprintln!("format version: {major}.{minor}");
    hprintln!("resource count: {}", inspected.len());
    hprintln!(
        "total uncompressed bytes: {}",
        inspected.total_uncompressed()
    );
    hprintln!("resources:");
    for entry in inspected.listing().listing() {
        hprintln!(
            "  {}  ({} compressed, {} uncompressed)",
            entry.path,
            entry.compressed_size,
            entry.uncompressed_size
        );
    }
    0
}

/// Decode every resource, recompute its SHA-256, and compare against the
/// archive's integrity manifest. Exits 0 when every digest matches.
fn run_verify(args: &mut Arguments) -> u8 {
    if args.contains(["-h", "--help"]) {
        husage!("{VERIFY_USAGE}");
        return 0;
    }
    let path = match single_operand(args, VERIFY_USAGE) {
        Ok(p) => p,
        Err(code) => return code,
    };
    let bytes = match read_archive_file(&path) {
        Ok(b) => b,
        Err(code) => return code,
    };
    let pkg = match open_package(&path, &bytes) {
        Ok(p) => p,
        Err(code) => return code,
    };
    let verified = match pkg.verify() {
        Ok(v) => v,
        Err(e) => {
            diag_code!(e.code(), "verify {}: {e}", path.display());
            return 1;
        }
    };
    hprintln!("archive: {}", path.display());
    hprintln!("integrity: OK");
    hprintln!(
        "manifest resources: {}",
        verified.manifest().resources.len()
    );
    hprintln!("verified resources: {}", verified.resource_count());
    0
}

/// Run the full 12-stage pipeline: structural integrity plus the EPF and
/// ELD semantic model checks. Exits 0 when the package is fully valid.
fn run_validate(args: &mut Arguments) -> u8 {
    if args.contains(["-h", "--help"]) {
        husage!("{VALIDATE_USAGE}");
        return 0;
    }
    let path = match single_operand(args, VALIDATE_USAGE) {
        Ok(p) => p,
        Err(code) => return code,
    };
    let bytes = match read_archive_file(&path) {
        Ok(b) => b,
        Err(code) => return code,
    };
    let pkg = match open_package(&path, &bytes) {
        Ok(p) => p,
        Err(code) => return code,
    };
    let validated = match pkg.validate() {
        Ok(v) => v,
        Err(e) => {
            diag_code!(e.code(), "validate {}: {e}", path.display());
            return 1;
        }
    };
    let meta = validated.metadata();
    hprintln!("archive: {}", path.display());
    hprintln!("language: {}", meta.name());
    hprintln!("identifier: {}", meta.identifier().as_str());
    if let Some(description) = meta.description() {
        hprintln!("description: {description}");
    }
    hprintln!("groups: {}", validated.groups().len());
    hprintln!("root phonemes: {}", validated.root_count());
    hprintln!("validation: OK");
    0
}

/// Create the parent directories of `target` underneath `base`, one level at
/// a time, refusing any level that is a symlink escaping `base`.
///
/// The archive's resource paths are already validated (`ResourcePath` rejects
/// absolute paths, `.`/`..` segments, backslashes and NUL), so the lexical
/// join is safe. What remains is the filesystem: a directory inside the
/// output tree could be a symlink pointing outside it. Each level is
/// checked with `symlink_metadata` and, when it is a symlink, resolved and
/// required to stay under `base`.
fn create_safe_parent(base: &std::path::Path, target: &std::path::Path) -> Result<(), String> {
    let relative = target
        .strip_prefix(base)
        .map_err(|_| "path escapes the output directory".to_string())?;
    let parent = relative.parent().unwrap_or(std::path::Path::new(""));
    let mut current = base.to_path_buf();
    for component in parent.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    let canon = fs::canonicalize(&current)
                        .map_err(|e| format!("canonicalize {}: {e}", current.display()))?;
                    if !canon.starts_with(base) {
                        return Err(format!(
                            "symlink escapes the output directory: {}",
                            current.display()
                        ));
                    }
                }
            }
            Err(_) => {
                // A concurrent writer may have created it between the
                // metadata probe and here; `create_dir` reports that as
                // AlreadyExists, which is not an error for us.
                if let Err(e) = fs::create_dir(&current) {
                    if e.kind() != std::io::ErrorKind::AlreadyExists {
                        return Err(format!("create {}: {e}", current.display()));
                    }
                }
            }
        }
    }
    Ok(())
}

/// Extract every resource in the archive to a directory.
///
/// The extraction is safe by construction: resource paths are validated by
/// the container layer, the output directory is canonicalized once, each
/// parent directory is created level-by-level with symlink-escape checks,
/// and files are created with `create_new` so an existing file is never
/// silently overwritten (pass `--force` to replace regular files).
fn run_unpack(args: &mut Arguments) -> u8 {
    if args.contains(["-h", "--help"]) {
        husage!("{UNPACK_USAGE}");
        return 0;
    }
    let force = args.contains("--force");
    let output_dir = match args.opt_value_from_str::<_, PathBuf>("--output") {
        Ok(Some(p)) => p,
        Ok(None) => {
            diag!("--output <dir> is required");
            return 2;
        }
        Err(e) => {
            diag!("--output: {e}");
            return 2;
        }
    };
    let archive = match single_operand(args, UNPACK_USAGE) {
        Ok(p) => p,
        Err(code) => return code,
    };
    let bytes = match read_archive_file(&archive) {
        Ok(b) => b,
        Err(code) => return code,
    };
    let pkg = match open_package(&archive, &bytes) {
        Ok(p) => p,
        Err(code) => return code,
    };
    let verified = match pkg.verify() {
        Ok(v) => v,
        Err(e) => {
            diag_code!(e.code(), "verify {}: {e}", archive.display());
            return 1;
        }
    };
    if let Err(e) = fs::create_dir_all(&output_dir) {
        diag!("create {}: {e}", output_dir.display());
        return 1;
    }
    let base = match fs::canonicalize(&output_dir) {
        Ok(p) => p,
        Err(e) => {
            diag!("canonicalize {}: {e}", output_dir.display());
            return 1;
        }
    };
    let mut written: u64 = 0;
    for (path, contents) in verified.resources() {
        let target = base.join(path);
        if let Err(msg) = create_safe_parent(&base, &target) {
            diag!("unsafe extraction: {msg}");
            return 1;
        }
        // Refuse to follow an existing symlink even with --force: removing
        // it would delete the link, but writing through it would write
        // outside the output tree.
        match fs::symlink_metadata(&target) {
            Ok(meta) if meta.file_type().is_symlink() => {
                diag!("refusing to write through symlink: {}", target.display());
                return 1;
            }
            Ok(_) => {
                if !force {
                    diag!("refusing to overwrite existing file: {}", target.display());
                    return 1;
                }
                if let Err(e) = fs::remove_file(&target) {
                    diag!("remove {}: {e}", target.display());
                    return 1;
                }
            }
            Err(_) => {}
        }
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(mut file) => {
                if let Err(e) = std::io::Write::write_all(&mut file, contents) {
                    diag!("write {}: {e}", target.display());
                    return 1;
                }
            }
            Err(e) => {
                diag!("create {}: {e}", target.display());
                return 1;
            }
        }
        written = written.saturating_add(1);
    }
    hprintln!("archive: {}", archive.display());
    hprintln!("output: {}", base.display());
    hprintln!("extracted resources: {written}");
    0
}

/// Print the pinned IPA repertoire metadata: version, Unicode version, IPA
/// chart issue and the three inventory counts.
fn run_repertoire(args: &mut Arguments) -> u8 {
    if args.contains(["-h", "--help"]) {
        husage!("{REPERTOIRE_INFO_USAGE}");
        return 0;
    }
    let operands: Vec<String> = args
        .clone()
        .finish()
        .into_iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    match operands.as_slice() {
        [one] if one == "info" => {}
        _ => {
            diag!("{REPERTOIRE_INFO_USAGE}");
            return 2;
        }
    }
    let repertoire = match elang_repertoire::repertoire::load() {
        Ok(r) => r,
        Err(e) => {
            diag!("load IPA repertoire: {e:?}");
            return 1;
        }
    };
    hprintln!("repertoire version: {}", repertoire.repertoire_version);
    hprintln!("unicode version: {}", repertoire.unicode_version);
    hprintln!("ipa chart: {}", repertoire.ipa_chart);
    hprintln!("base symbols: {}", repertoire.base_symbols.len());
    hprintln!(
        "combining diacritics: {}",
        repertoire.combining_diacritics.len()
    );
    hprintln!("spacing modifiers: {}", repertoire.spacing_modifiers.len());
    0
}
