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
    let sub = match args.subcommand() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to read subcommand: {e}");
            return ExitCode::from(2);
        }
    };
    match parse_subcommand(sub.as_deref()) {
        Ok(Subcommand::Pack) => run_pack(&mut args),
        Ok(Subcommand::Inspect) => run_inspect(&mut args),
        Ok(Subcommand::Verify) => run_verify(&mut args),
        Ok(Subcommand::Validate) => run_validate(&mut args),
        Ok(Subcommand::Unpack) => run_unpack(&mut args),
        Ok(Subcommand::CanonicalizeEld) => run_canonicalize_eld(&mut args),
        Ok(Subcommand::Repertoire) => run_repertoire(&mut args),
        Ok(Subcommand::Help) => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("{msg}");
            eprint!("{HELP}");
            ExitCode::from(2)
        }
    }
}

/// Read a required flag value or print a usage error and set `err`.
fn required_opt<T: std::str::FromStr>(
    args: &mut Arguments,
    flag: &'static str,
    err: &mut Option<ExitCode>,
) -> Option<T>
where
    T::Err: std::fmt::Display,
{
    match args.opt_value_from_str::<_, T>(flag) {
        Ok(Some(v)) => Some(v),
        Ok(None) => {
            eprintln!("{flag} <value> is required");
            *err = Some(ExitCode::from(2));
            None
        }
        Err(e) => {
            eprintln!("{flag}: {e}");
            *err = Some(ExitCode::from(2));
            None
        }
    }
}
/// Implement the `pack` subcommand end to end.
fn run_pack(args: &mut Arguments) -> ExitCode {
    if args.contains(["-h", "--help"]) {
        print!("{PACK_USAGE}");
        return ExitCode::SUCCESS;
    }

    let mut err: Option<ExitCode> = None;

    let epf_path: PathBuf = match required_opt::<PathBuf>(args, "--epf", &mut err) {
        Some(p) => p,
        None => return err.unwrap_or(ExitCode::from(2)),
    };
    let eld_path: PathBuf = match required_opt::<PathBuf>(args, "--eld", &mut err) {
        Some(p) => p,
        None => return err.unwrap_or(ExitCode::from(2)),
    };
    let output: PathBuf = match required_opt::<PathBuf>(args, "--output", &mut err) {
        Some(p) => p,
        None => return err.unwrap_or(ExitCode::from(2)),
    };

    let format_version: String = match args.opt_value_from_str::<_, String>("--format-version") {
        Ok(Some(v)) => v,
        Ok(None) => "0.1.0".to_string(),
        Err(e) => {
            eprintln!("--format-version: {e}");
            return ExitCode::from(2);
        }
    };

    let profile: Profile = match args.opt_value_from_str::<_, String>("--profile") {
        Ok(Some(s)) => match s.as_str() {
            "fast" => Profile::Fast,
            "compact" => Profile::Compact,
            "canonical" => Profile::Canonical,
            other => {
                eprintln!("--profile: unknown profile {other:?} (expected fast|compact|canonical)");
                return ExitCode::from(2);
            }
        },
        Ok(None) => Profile::Fast,
        Err(e) => {
            eprintln!("--profile: {e}");
            return ExitCode::from(2);
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
        eprintln!("unexpected positional arguments: {leftover:?}");
        return ExitCode::from(2);
    }

    let epf_bytes = match fs::read(&epf_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("read {}: {e}", epf_path.display());
            return ExitCode::from(1);
        }
    };
    let eld_bytes = match fs::read(&eld_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("read {}: {e}", eld_path.display());
            return ExitCode::from(1);
        }
    };

    let epf = match elang_epf::parse(&epf_bytes) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("parse {}: {e}", epf_path.display());
            return ExitCode::from(1);
        }
    };

    let rep = match elang_repertoire::repertoire::load() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("load IPA repertoire: {e:?}");
            return ExitCode::from(1);
        }
    };
    let eld_doc = match elang_eld::parse(&eld_bytes, &rep) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("parse {}: {e}", eld_path.display());
            return ExitCode::from(1);
        }
    };
    let eld = match elang_eld::inventory::from_document(&eld_doc, &rep) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("validate {}: {e}", eld_path.display());
            return ExitCode::from(1);
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
            eprintln!("write archive: [{}] {e}", e.code());
            return ExitCode::from(1);
        }
    };

    if let Err(e) = fs::write(&output, &bytes) {
        eprintln!("write {}: {e}", output.display());
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
/// Implement the `canonicalize-eld` subcommand.
fn run_canonicalize_eld(args: &mut Arguments) -> ExitCode {
    if args.contains(["-h", "--help"]) {
        print!("{CANONICALIZE_ELD_USAGE}");
        return ExitCode::SUCCESS;
    }

    let operands = args.clone().finish();
    let input: PathBuf = match operands.as_slice() {
        [input] => PathBuf::from(input),
        _ => {
            eprint!("{CANONICALIZE_ELD_USAGE}");
            return ExitCode::from(2);
        }
    };

    let bytes = match fs::read(&input) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("read {}: {e}", input.display());
            return ExitCode::from(1);
        }
    };

    let rep = match elang_repertoire::repertoire::load() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("load IPA repertoire: {e:?}");
            return ExitCode::from(1);
        }
    };
    let doc = match elang_eld::parse(&bytes, &rep) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("parse {}: {e}", input.display());
            return ExitCode::from(1);
        }
    };
    let inv = match elang_eld::inventory::from_document(&doc, &rep) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("validate {}: {e}", input.display());
            return ExitCode::from(1);
        }
    };

    let canonical = elang_eld::canon::to_string(&inv);
    print!("{canonical}");
    ExitCode::SUCCESS
}

/// Read an archive file into memory, reporting a read failure on stderr.
fn read_archive_file(path: &std::path::Path) -> Result<Vec<u8>, ExitCode> {
    fs::read(path).map_err(|e| {
        eprintln!("read {}: {e}", path.display());
        ExitCode::from(1)
    })
}

/// Open an archive for inspection, reporting a parse failure on stderr.
fn open_package(path: &std::path::Path, bytes: &[u8]) -> Result<elang_reader::Package, ExitCode> {
    elang_reader::Package::open(bytes).map_err(|e| {
        eprintln!("open {}: {e}", path.display());
        ExitCode::from(1)
    })
}

/// Take exactly one positional operand, printing usage on anything else.
fn single_operand(args: &mut Arguments, usage: &str) -> Result<PathBuf, ExitCode> {
    let operands: Vec<String> = args
        .clone()
        .finish()
        .into_iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    match operands.as_slice() {
        [one] => Ok(PathBuf::from(one)),
        _ => {
            eprint!("{usage}");
            Err(ExitCode::from(2))
        }
    }
}

/// Print an index-only listing of the archive: header version, resource
/// count, total uncompressed size and one line per indexed resource.
///
/// This is the cheapest of the reader-side subcommands: it decompresses no
/// frame body, so it stays fast on large archives.
fn run_inspect(args: &mut Arguments) -> ExitCode {
    if args.contains(["-h", "--help"]) {
        print!("{INSPECT_USAGE}");
        return ExitCode::SUCCESS;
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
    println!("archive: {}", path.display());
    println!("format version: {major}.{minor}");
    println!("resource count: {}", inspected.len());
    println!(
        "total uncompressed bytes: {}",
        inspected.total_uncompressed()
    );
    println!("resources:");
    for entry in inspected.listing().listing() {
        println!(
            "  {}  ({} compressed, {} uncompressed)",
            entry.path, entry.compressed_size, entry.uncompressed_size
        );
    }
    ExitCode::SUCCESS
}

/// Decode every resource, recompute its SHA-256, and compare against the
/// archive's integrity manifest. Exits 0 when every digest matches.
fn run_verify(args: &mut Arguments) -> ExitCode {
    if args.contains(["-h", "--help"]) {
        print!("{VERIFY_USAGE}");
        return ExitCode::SUCCESS;
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
            eprintln!("verify {}: {e}", path.display());
            return ExitCode::from(1);
        }
    };
    println!("archive: {}", path.display());
    println!("integrity: OK");
    println!(
        "manifest resources: {}",
        verified.manifest().resources.len()
    );
    println!("verified resources: {}", verified.resource_count());
    ExitCode::SUCCESS
}

/// Run the full 12-stage pipeline: structural integrity plus the EPF and
/// ELD semantic model checks. Exits 0 when the package is fully valid.
fn run_validate(args: &mut Arguments) -> ExitCode {
    if args.contains(["-h", "--help"]) {
        print!("{VALIDATE_USAGE}");
        return ExitCode::SUCCESS;
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
            eprintln!("validate {}: {e}", path.display());
            return ExitCode::from(1);
        }
    };
    let meta = validated.metadata();
    println!("archive: {}", path.display());
    println!("language: {}", meta.name());
    println!("identifier: {}", meta.identifier().as_str());
    if let Some(description) = meta.description() {
        println!("description: {description}");
    }
    println!("groups: {}", validated.groups().len());
    println!("root phonemes: {}", validated.root_count());
    println!("validation: OK");
    ExitCode::SUCCESS
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
fn run_unpack(args: &mut Arguments) -> ExitCode {
    if args.contains(["-h", "--help"]) {
        print!("{UNPACK_USAGE}");
        return ExitCode::SUCCESS;
    }
    let force = args.contains("--force");
    let output_dir = match args.opt_value_from_str::<_, PathBuf>("--output") {
        Ok(Some(p)) => p,
        Ok(None) => {
            eprintln!("--output <dir> is required");
            return ExitCode::from(2);
        }
        Err(e) => {
            eprintln!("--output: {e}");
            return ExitCode::from(2);
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
            eprintln!("verify {}: {e}", archive.display());
            return ExitCode::from(1);
        }
    };
    if let Err(e) = fs::create_dir_all(&output_dir) {
        eprintln!("create {}: {e}", output_dir.display());
        return ExitCode::from(1);
    }
    let base = match fs::canonicalize(&output_dir) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("canonicalize {}: {e}", output_dir.display());
            return ExitCode::from(1);
        }
    };
    let mut written: u64 = 0;
    for (path, contents) in verified.resources() {
        let target = base.join(path);
        if let Err(msg) = create_safe_parent(&base, &target) {
            eprintln!("unsafe extraction: {msg}");
            return ExitCode::from(1);
        }
        // Refuse to follow an existing symlink even with --force: removing
        // it would delete the link, but writing through it would write
        // outside the output tree.
        match fs::symlink_metadata(&target) {
            Ok(meta) if meta.file_type().is_symlink() => {
                eprintln!("refusing to write through symlink: {}", target.display());
                return ExitCode::from(1);
            }
            Ok(_) => {
                if !force {
                    eprintln!("refusing to overwrite existing file: {}", target.display());
                    return ExitCode::from(1);
                }
                if let Err(e) = fs::remove_file(&target) {
                    eprintln!("remove {}: {e}", target.display());
                    return ExitCode::from(1);
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
                    eprintln!("write {}: {e}", target.display());
                    return ExitCode::from(1);
                }
            }
            Err(e) => {
                eprintln!("create {}: {e}", target.display());
                return ExitCode::from(1);
            }
        }
        written = written.saturating_add(1);
    }
    println!("archive: {}", archive.display());
    println!("output: {}", base.display());
    println!("extracted resources: {written}");
    ExitCode::SUCCESS
}

/// Print the pinned IPA repertoire metadata: version, Unicode version, IPA
/// chart issue and the three inventory counts.
fn run_repertoire(args: &mut Arguments) -> ExitCode {
    if args.contains(["-h", "--help"]) {
        print!("{REPERTOIRE_INFO_USAGE}");
        return ExitCode::SUCCESS;
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
            eprint!("{REPERTOIRE_INFO_USAGE}");
            return ExitCode::from(2);
        }
    }
    let repertoire = match elang_repertoire::repertoire::load() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("load IPA repertoire: {e:?}");
            return ExitCode::from(1);
        }
    };
    println!("repertoire version: {}", repertoire.repertoire_version);
    println!("unicode version: {}", repertoire.unicode_version);
    println!("ipa chart: {}", repertoire.ipa_chart);
    println!("base symbols: {}", repertoire.base_symbols.len());
    println!(
        "combining diacritics: {}",
        repertoire.combining_diacritics.len()
    );
    println!("spacing modifiers: {}", repertoire.spacing_modifiers.len());
    ExitCode::SUCCESS
}
