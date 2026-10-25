#!/usr/bin/env bash
# build-release-artifacts.sh — assemble a deterministic source-release
# artifact for ELANG (T1617 preparation).
#
# WHAT THIS DOES
#   1. Collects every file that would ship: every git-tracked file plus
#      every untracked-but-not-ignored file (so a generated artifact that
#      has not been committed yet still lands in the tarball).
#   2. Assembles them into a single `.tar.gz` whose bytes depend on the
#      *content* only:
#        - entries sorted by path,
#        - uid/gid forced to 0, numeric owner,
#        - mtime forced to the Unix epoch,
#        - mode forced to `u=rwX,go=rX`, so the archive does not inherit the
#          building machine's umask (the executable bit is preserved for
#          files that already have one),
#        - `gzip -n`, so no filename or timestamp is embedded in the gzip
#          header.
#   3. Writes a `<artifact>.sha256` sidecar in `sha256sum` format next to
#      it.
#
# REQUIREMENTS
#   GNU tar (Linux). BSD tar on macOS cannot produce a byte-reproducible
#   archive, so the script refuses to run there instead of silently
#   emitting a platform-dependent artifact. Linux is the release platform.
#
# WHAT THIS DELIBERATELY DOES NOT DO
#   * It does NOT create a git tag.
#   * It does NOT push, publish, upload or sign anything.
#   * It does NOT modify the working tree.
#   * It does NOT make a network call.
#   * It does NOT claim publisher authenticity — a SHA-256 sidecar attests
#     that the bytes you hold are the bytes this process produced, not who
#     produced them. See `docs/release/v0.1.0/README.md`.
#
# The tag, the signature and the publication are the human release step
# (T1617 / T1620). This script exists so that when that step happens it is
# one command and one decision, not a hand-assembled directory.
#
# USAGE
#   scripts/build-release-artifacts.sh [--out-dir DIR] [--version VER] [--verify]
#
#   --out-dir DIR   Where to write the artifact (default: target/release-artifacts)
#   --version VER   Version string used in the filename (default: read from
#                   Cargo.toml `[workspace.package].version`)
#   --verify        Build twice into two scratch directories and assert the
#                   two artifacts are byte-identical. Exits non-zero if not.
#
# EXAMPLE
#   scripts/build-release-artifacts.sh --verify
#   scripts/build-release-artifacts.sh --out-dir /tmp/elang-dist --version 0.1.0

set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

OUT_DIR="target/release-artifacts"
VERSION=""
VERIFY=0

die() {
	printf 'build-release-artifacts: %s\n' "$*" >&2
	exit 1
}

while [ "$#" -gt 0 ]; do
	case "$1" in
	--out-dir)
		[ "$#" -ge 2 ] || die "--out-dir needs a value"
		OUT_DIR="$2"
		shift 2
		;;
	--version)
		[ "$#" -ge 2 ] || die "--version needs a value"
		VERSION="$2"
		shift 2
		;;
	--verify)
		VERIFY=1
		shift
		;;
	-h | --help)
		sed -n '2,45p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
		exit 0
		;;
	*)
		die "unknown argument: $1"
		;;
	esac
done

# --- toolchain requirements -------------------------------------------------
# The archive must be byte-reproducible, which means the tar implementation
# has to be pinned: GNU tar and BSD tar (macOS) do not write identical
# archives for the same inputs. Rather than silently produce a
# platform-dependent artifact, require GNU tar and say where to build it.
#
# Linux is the release platform: the reproducible-build CI workflow and the
# release itself run on ubuntu. The `crates/elang-conformance/tests/
# release_packaging.rs` guard is therefore Linux-only.
if ! tar --version 2>/dev/null | grep -q '^tar (GNU tar)'; then
	found="$(tar --version 2>/dev/null | head -n 1 || printf 'unknown')"
	die "GNU tar is required; found: ${found}. BSD tar cannot produce a byte-reproducible archive. Build the release artifact on Linux (the release platform)."
fi

# SHA-256: GNU coreutils provides sha256sum; macOS provides shasum -a 256.
# The sidecar is written in the same `sha256sum` format either way.
sha256_of() {
	if command -v sha256sum >/dev/null 2>&1; then
		sha256sum "$1" | cut -d' ' -f1
	elif command -v shasum >/dev/null 2>&1; then
		shasum -a 256 "$1" | cut -d' ' -f1
	else
		die "neither sha256sum nor shasum is available"
	fi
}

# --- version ---------------------------------------------------------------
if [ -z "$VERSION" ]; then
	VERSION="$(
		sed -n '/^\[workspace\.package\]/,/^\[/p' Cargo.toml |
			sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' |
			head -n 1
	)"
fi
[ -n "$VERSION" ] || die "could not read [workspace.package].version from Cargo.toml"

ARTIFACT_NAME="elang-v${VERSION}.tar.gz"

# --- output directory guard -------------------------------------------------
# The artifact must never be able to swallow itself. The file list below
# includes untracked-but-not-ignored files, so an output directory that
# lives inside the repository and is NOT git-ignored would put the artifact
# this script writes into the *next* run's list — tar would then pack a
# partial copy of the artifact into itself. Require the output path to be
# git-ignored, or place it outside the repository.
OUT_ABS="$(realpath -m -- "$OUT_DIR")"
case "$OUT_ABS/" in
"$REPO_ROOT"/*)
	REL="${OUT_ABS#"$REPO_ROOT"}"
	REL="${REL#/}"
	if ! git check-ignore -q --no-index -- "${REL:+$REL/}$ARTIFACT_NAME"; then
		die "--out-dir inside the repository must be git-ignored: $OUT_DIR"
	fi
	;;
esac

# --- file list -------------------------------------------------------------
# Tracked files, plus untracked files that are not ignored. Both lists are
# sorted and de-duplicated. Paths are repo-relative and contain no spaces
# outside of the usual; a NUL-separated list is used so that even a path
# with a newline cannot corrupt the list.
LIST="$(mktemp)"
trap 'rm -f "$LIST" "$LIST.files"' EXIT

{
	git ls-files -z
	git ls-files -z --others --exclude-standard
} | sort -zu >"$LIST"

FILE_COUNT="$(tr -cd '\0' <"$LIST" | wc -c | tr -d ' ')"
[ "$FILE_COUNT" -gt 0 ] || die "no files to package"

# Every listed path must exist as a regular file: a tracked path that was
# deleted from the working tree would otherwise produce a tarball that is
# missing a file the release claims to contain.
MISSING=0
while IFS= read -r -d '' path; do
	if [ ! -f "$path" ]; then
		printf 'build-release-artifacts: missing from the working tree: %s\n' "$path" >&2
		MISSING=$((MISSING + 1))
	fi
done <"$LIST"
[ "$MISSING" -eq 0 ] || die "$MISSING tracked file(s) are missing from the working tree"

# --- build -----------------------------------------------------------------
build() {
	local out_dir="$1"
	local tarball="$out_dir/$ARTIFACT_NAME"

	mkdir -p "$out_dir"

	# --sort=name          deterministic entry order
	# --mtime=@0           deterministic timestamps (SOURCE_DATE_EPOCH-style)
	# --owner=0 --group=0  deterministic ownership
	# --numeric-owner      no user/group name lookups
	# --mode=u=rwX,go=rX   deterministic permissions, independent of umask
	# --format=posix       portable, standard pax extended headers
	# --pax-option=...     explicit suppression of atime/ctime in the header
	#
	# The member list comes exclusively from --files-from. No directory
	# member is added: passing `.` would make tar walk the whole tree
	# (including the ignored build output in `target/`, and the artifact
	# this script is writing) instead of the explicit file list.
	#
	# `--null` is positional in GNU tar: it must appear *before*
	# `--files-from`, or tar reads the list as newline-separated names and
	# rejects every name that contains a NUL.
	local tar_status=0
	tar \
		--create \
		--file=- \
		--format=posix \
		--sort=name \
		--mtime='@0' \
		--owner=0 \
		--group=0 \
		--numeric-owner \
		--mode='u=rwX,go=rX' \
		--no-recursion \
		--null \
		--pax-option='exthdr.name=%d/PaxHeaders/%f,delete=atime,delete=ctime,delete=mtime' \
		--files-from="$LIST" |
		gzip -n -9 >"$tarball" || tar_status=$?

	if [ "$tar_status" -ne 0 ]; then
		printf 'build-release-artifacts: tar failed (exit %s)\n' "$tar_status" >&2
		rm -f "$tarball"
		return 1
	fi

	# The artifact must contain exactly the files we asked for. Without
	# this check a silently truncated archive would still hash, still
	# compare equal across two runs, and still look like a successful
	# build.
	[ -s "$tarball" ] || {
		printf 'build-release-artifacts: empty artifact at %s\n' "$tarball" >&2
		return 1
	}

	# Compare the archive's member *names* with the requested list, not just
	# the number of members: an equal count can still hide a substituted or
	# misnamed entry, which is exactly the kind of silent corruption this
	# check exists to catch.
	local expected listed
	expected="$(tr '\0' '\n' <"$LIST" | LC_ALL=C sort)"
	listed="$(tar -tzf "$tarball" | LC_ALL=C sort)"
	if [ "$listed" != "$expected" ]; then
		printf 'build-release-artifacts: archive members do not match the requested file list\n' >&2
		printf '  expected %s entries, archive holds %s entries\n' \
			"$(printf '%s\n' "$expected" | grep -c . || true)" \
			"$(printf '%s\n' "$listed" | grep -c . || true)" >&2
		printf '  differences (expected vs archive):\n' >&2
		comm -3 <(printf '%s\n' "$expected") <(printf '%s\n' "$listed") | head -n 20 >&2
		return 1
	fi

	# sha256sum checkable format: "<hex>  <filename>"
	printf '%s  %s\n' "$(sha256_of "$tarball")" "$ARTIFACT_NAME" \
		>"$out_dir/${ARTIFACT_NAME}.sha256"

	printf '%s\n' "$tarball"
}

if [ "$VERIFY" -eq 0 ]; then
	PATH_OUT="$(build "$OUT_DIR")"
	printf 'artifact:  %s\n' "$PATH_OUT"
	printf 'checksum:  %s.sha256\n' "$PATH_OUT"
	printf 'files:     %s\n' "$FILE_COUNT"
	exit 0
fi

# --- verify mode -----------------------------------------------------------
# Two independent builds into two scratch directories must agree byte for
# byte. This is the same property the reproducible-build CI workflow checks
# for the canonical artifacts; here it is checked for the archive itself.
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"; rm -f "$LIST" "$LIST.files"' EXIT

A="$(build "$SCRATCH/a")"
B="$(build "$SCRATCH/b")"

A_SUM="$(sha256_of "$A")"
B_SUM="$(sha256_of "$B")"

if [ "$A_SUM" != "$B_SUM" ]; then
	printf 'build-release-artifacts: NOT reproducible\n  run 1: %s\n  run 2: %s\n' \
		"$A_SUM" "$B_SUM" >&2
	exit 1
fi

printf 'reproducible: yes\n'
printf 'sha256:       %s\n' "$A_SUM"
printf 'files:        %s\n' "$FILE_COUNT"

# The verified artifact is written to --out-dir as well, so the caller ends
# up holding the exact bytes that were proven reproducible.
OUT="$(build "$OUT_DIR")"
printf 'artifact:     %s\n' "$OUT"
