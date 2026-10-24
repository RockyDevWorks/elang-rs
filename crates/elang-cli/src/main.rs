//! # elang CLI
//!
//! Command-line front end for the ELANG v0.1.0 package tooling (Phase 13,
//! T1311–T1316). Implemented subcommands in this crate:
//!
//! - `pack` — assemble a `.elang` archive from `--epf` + `--eld` inputs and
//!   write it to `--output`.
//! - `canonicalize-eld` — read a phoneme inventory and print its canonical
//!   serialized form to stdout.
//! - `help` — usage.
//!
//! Reader-side subcommands (`inspect`, `verify`, `validate`, `unpack`) and
//! `repertoire info` land in a follow-up PR; their library surfaces already
//! exist on `elang-reader` and `elang-repertoire`.

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
    canonicalize-eld  Print the canonical form of a phoneme inventory
    help              Print this message

Run 'elang <subcommand> --help' for subcommand-specific options.
";

const PACK_USAGE: &str = "\
usage: elang pack --epf <file> --eld <file> --output <file>
                 [--profile fast|compact|canonical] [--format-version <version>]
";

const CANONICALIZE_ELD_USAGE: &str = "usage: elang canonicalize-eld <ELD-file>\n";

fn main() -> ExitCode {
    let mut args = Arguments::from_env();
    let sub = match args.subcommand() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to read subcommand: {e}");
            return ExitCode::from(2);
        }
    };
    match sub.as_deref() {
        Some("pack") => run_pack(&mut args),
        Some("canonicalize-eld") => run_canonicalize_eld(&mut args),
        Some("help") | None => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("unknown subcommand: {other:?}");
            eprintln!("{HELP}");
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

    let format_version: String = args
        .opt_value_from_str::<_, String>("--format-version")
        .ok()
        .flatten()
        .unwrap_or_else(|| "0.1.0".to_string());

    let profile: Profile = match args
        .opt_value_from_str::<_, String>("--profile")
        .ok()
        .flatten()
        .as_deref()
    {
        Some("fast") => Profile::Fast,
        Some("compact") => Profile::Compact,
        Some("canonical") => Profile::Canonical,
        Some(other) => {
            eprintln!("--profile: unknown profile {other:?} (expected fast|compact|canonical)");
            return ExitCode::from(2);
        }
        None => Profile::Fast,
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
