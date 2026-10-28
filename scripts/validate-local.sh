#!/usr/bin/env bash
#
# scripts/validate-local.sh — the local validation suite for ELANG-rs.
#
# Runs a local subset of CI checks with available tools. It omits the
# workflow's fuzz smoke test, release-mode conformance tests, server-specific
# checks, Miri, and Scorecard, plus CI build, MSRV, Docker, and coverage jobs.
# A passing summary covers only the checks listed below; CI remains required.
#
# Usage:
#   scripts/validate-local.sh                # run everything that is available
#   scripts/validate-local.sh --required      # fail fast on required check failure
#   scripts/validate-local.sh --skip jscpd    # skip a single named check
#   scripts/validate-local.sh --help
#
# A check is skipped (not failed) when its binary is not on PATH. Every
# skipped check is reported in the summary so the contributor can install
# the missing tool and re-run. A `--strict` mode (TODO: future) would
# fail when any check is skipped — useful for CI mirror environments.
#
# Why a shell script and not a cargo alias: the suite includes tools
# that are not cargo-installable (gitleaks, zizmor, lychee, syft, knip,
# cargo-hack, cargo-mutants, scorecard) and would each need a custom
# cargo wrapper otherwise. A single script with `command -v` gates is
# shorter, more honest about the tool boundary, and runs without
# `cargo install` first.
#
# Adapted from `RockyDevWorks/elang-hub/scripts/validate-local.sh`
# (merged as elang-hub PR #35).

set -uo pipefail

# ─────────────────────────────────────────────────────────────────────────────
# Configuration
# ─────────────────────────────────────────────────────────────────────────────

# Resolve the repo root from the script's own location so the script
# works regardless of the caller's cwd.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

# Required Rust-quality checks. These always run because they are part
# of the project's own toolchain.
REQUIRED_CHECKS=(
    "fmt:cargo fmt --all -- --check"
    "clippy:cargo clippy --workspace --all-targets --all-features -- -D warnings"
    "test:cargo test --workspace --all-features"
)

# Optional Rust checks. Skipped via --skip or by name.
OPTIONAL_CARGO_CHECKS=(
    "doc:cargo doc --workspace --no-deps -q"
)

# Tool-gated checks. Each entry maps a display name to the binary that
# must be on PATH. The argument list is the command to invoke from the
# repo root.
TOOL_CHECKS=(
    "jscpd:npx --yes jscpd@3 --config .jscpd.json crates"
    # `.lycheeignore` is auto-detected; URL/host patterns live
    # there. File-system exclusions use `--exclude-path`.
    "lychee:lychee --offline --no-progress --exclude-path target --exclude-path fuzz/target --exclude-path fuzz/corpus --exclude-path fuzz/artifacts --exclude-path crates/*/proptest-regressions --exclude-path site --exclude-path cache --exclude-path .jscpd-report '**/*.md'"
    # `gitleaks detect --source .` is the historical-entry sweep;
    # `scripts/test-gitleaks.sh` exercises the `credential-url` rule
    # end-to-end against generated fixtures.
    "gitleaks:gitleaks detect --source . --no-banner --config .gitleaks.toml && bash scripts/test-gitleaks.sh"
    # `--persona=regular --no-exit-codes` matches the workflow:
    # findings surface in the log without blocking the contributor.
    "zizmor:zizmor --persona=regular --no-exit-codes --format plain .github/workflows"
    # `dir:.` is the explicit directory scheme for syft scan; matches
    # the workflow's command exactly. Output to stdout is the SPDX
    # JSON; we redirect to /dev/null so the script doesn't dump
    # megabytes of SBOM to the terminal.
    "syft:syft scan dir:. -o spdx-json > /dev/null"
)

# Tools that have no work to do on elang-rs because the repo has no
# JS/TS source. Listed for menu parity with elang-hub (which lists the
# inverse set). The local script verifies that no source is present
# before reporting each tool as "not applicable".
JS_ONLY_TOOLS=(
    "knip"
)

# Tools that require cargo subcommands not on the default toolchain.
# These get their own gate because the binary is `cargo hack`,
# `cargo mutants`, etc., which `command -v` cannot find.
CARGO_PLUGIN_CHECKS=(
    "audit:cargo audit"
    "cargo-hack:cargo hack check --workspace --each-feature --no-dev-deps"
    "cargo-mutants:cargo mutants -p elang-core --timeout 60"
)

# ─────────────────────────────────────────────────────────────────────────────
# Argument parsing
# ─────────────────────────────────────────────────────────────────────────────

FAIL_FAST=0
SKIP=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --required)
            FAIL_FAST=1
            ;;
        --skip)
            if [[ $# -lt 2 || "$2" == -* ]]; then
                printf '%s\n' '--skip requires a check name' >&2
                exit 2
            fi
            SKIP+=("$2")
            shift 2
            continue
            ;;
        --skip=*)
            SKIP+=("${1#--skip=}")
            ;;
        -h|--help)
            sed -n '2,30p' "$0"
            exit 0
            ;;
        *)
            printf 'unknown argument: %s\n' "$1" >&2
            exit 2
            ;;
    esac
    shift
done

is_skipped() {
    local name="$1"
    for s in "${SKIP[@]}"; do
        if [[ "${s}" == "${name}" ]]; then
            return 0
        fi
    done
    return 1
}

# ─────────────────────────────────────────────────────────────────────────────
# Output helpers
# ─────────────────────────────────────────────────────────────────────────────

if [[ -t 1 ]]; then
    C_RESET=$'\033[0m'
    C_BOLD=$'\033[1m'
    C_DIM=$'\033[2m'
    C_RED=$'\033[31m'
    C_GREEN=$'\033[32m'
    C_YELLOW=$'\033[33m'
    C_BLUE=$'\033[34m'
else
    C_RESET='' C_BOLD='' C_DIM='' C_RED='' C_GREEN='' C_YELLOW='' C_BLUE=''
fi

PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0
NA_COUNT=0
FAILED_CHECKS=()

run_check() {
    local name="$1"
    local cmd="$2"
    local start
    start=$(date +%s)

    printf '%s==>%s %s%s%s\n' "${C_BLUE}" "${C_RESET}" "${C_BOLD}" "${name}" "${C_RESET}"
    printf '%s   $ %s%s\n' "${C_DIM}" "${cmd}${C_RESET}"

    # shellcheck disable=SC2086
    if eval "${cmd}"; then
        local elapsed=$(( $(date +%s) - start ))
        printf '%s   ✓ pass%s (%ss)\n' "${C_GREEN}" "${C_RESET}" "${elapsed}"
        PASS_COUNT=$((PASS_COUNT + 1))
        return 0
    else
        local code=$?
        local elapsed=$(( $(date +%s) - start ))
        printf '%s   ✗ fail%s (%ss, exit %d)\n' "${C_RED}" "${C_RESET}" "${elapsed}" "${code}"
        FAIL_COUNT=$((FAIL_COUNT + 1))
        FAILED_CHECKS+=("${name}")
        if [[ ${FAIL_FAST} -eq 1 ]]; then
            printf '%s   fail-fast: aborting%s\n' "${C_RED}" "${C_RESET}"
            exit "${code}"
        fi
        return "${code}"
    fi
}

skip_check() {
    local name="$1"
    local reason="$2"
    printf '%s~~>%s %s%s%s %s(skip: %s)%s\n' \
        "${C_YELLOW}" "${C_RESET}" "${C_BOLD}" "${name}" "${C_RESET}" \
        "${C_DIM}" "${reason}" "${C_RESET}"
    SKIP_COUNT=$((SKIP_COUNT + 1))
}

na_check() {
    local name="$1"
    local reason="$2"
    printf '%s--%s %s%s%s %s(n/a: %s)%s\n' \
        "${C_DIM}" "${C_RESET}" "${C_DIM}" "${name}" "${C_RESET}" \
        "${C_DIM}" "${reason}" "${C_RESET}"
    NA_COUNT=$((NA_COUNT + 1))
}

tool_available() {
    command -v "$1" >/dev/null 2>&1
}

# ─────────────────────────────────────────────────────────────────────────────
# Run
# ─────────────────────────────────────────────────────────────────────────────

printf '%sELANG-rs — local validation suite%s\n' "${C_BOLD}" "${C_RESET}"
printf '%s   repo: %s%s\n' "${C_DIM}" "${REPO_ROOT}${C_RESET}"

for entry in "${REQUIRED_CHECKS[@]}"; do
    name="${entry%%:*}"
    cmd="${entry#*:}"
    if is_skipped "${name}"; then
        skip_check "${name}" "explicit --skip"
        continue
    fi
    run_check "${name}" "${cmd}" || true
done

for entry in "${OPTIONAL_CARGO_CHECKS[@]}"; do
    name="${entry%%:*}"
    cmd="${entry#*:}"
    if is_skipped "${name}"; then
        skip_check "${name}" "explicit --skip"
        continue
    fi
    run_check "${name}" "${cmd}" || true
done

for entry in "${TOOL_CHECKS[@]}"; do
    name="${entry%%:*}"
    cmd="${entry#*:}"
    if is_skipped "${name}"; then
        skip_check "${name}" "explicit --skip"
        continue
    fi
    # The first token of $cmd may be `npx` (which is on PATH) — but
    # we want the resolved package's binary. For npx-managed tools,
    # the "available" check is whether npx is on PATH; the package
    # itself is fetched on first run.
    binary="${cmd%% *}"
    if ! tool_available "${binary}"; then
        skip_check "${name}" "${binary} not on PATH"
        continue
    fi
    run_check "${name}" "${cmd}" || true
done

for entry in "${CARGO_PLUGIN_CHECKS[@]}"; do
    name="${entry%%:*}"
    cmd="${entry#*:}"
    if is_skipped "${name}"; then
        skip_check "${name}" "explicit --skip"
        continue
    fi
    # Plugin checks use `cargo <plugin>`; the binary the user needs
    # is `cargo`, plus the plugin subcommand via cargo install.
    binary="${cmd%% *}"
    if ! tool_available "${binary}"; then
        skip_check "${name}" "${binary} not on PATH"
        continue
    fi
    # Extract the cargo subcommand (second token) and probe it.
    subcommand="$(printf '%s\n' "${cmd}" | awk '{print $2}')"
    if ! cargo "${subcommand}" --help >/dev/null 2>&1; then
        skip_check "${name}" "cargo-${subcommand} plugin not installed (run: cargo install cargo-${subcommand})"
        continue
    fi
    run_check "${name}" "${cmd}" || true
done

js_source_guard() {
    # Keep the source patterns and generated-directory exclusions in sync
    # with validation.yml's _js-source-guard.
    if find . \
        \( -name node_modules -o -name target -o -name 'fuzz/target' -o -name '.jscpd-report' \) -prune -false \
        -o -type f \( -name '*.js' -o -name '*.ts' -o -name '*.svelte' -o -name 'package.json' \
        -o -name '*.jsx' -o -name '*.tsx' -o -name '*.mjs' -o -name '*.cjs' -o -name '*.mts' \) \
        -print -quit | grep -q .; then
        echo '::error::JS source detected; install Knip and remove this guard before proceeding.'
        return 1
    fi
}

for name in "${JS_ONLY_TOOLS[@]}"; do
    if js_source_guard; then
        na_check "${name}" "no JS/TS source in this repo (see RockyDevWorks/elang-hub)"
    else
        run_check "${name}" "false" || true
    fi
done

# ─────────────────────────────────────────────────────────────────────────────
# Summary
# ─────────────────────────────────────────────────────────────────────────────

TOTAL=$((PASS_COUNT + FAIL_COUNT + SKIP_COUNT + NA_COUNT))
printf '\n%sSummary%s\n' "${C_BOLD}" "${C_RESET}"
printf '  %s%d passed%s  ' "${C_GREEN}" "${PASS_COUNT}" "${C_RESET}"
printf '%s%d failed%s  ' "${C_RED}" "${FAIL_COUNT}" "${C_RESET}"
printf '%s%d skipped%s  ' "${C_YELLOW}" "${SKIP_COUNT}" "${C_RESET}"
printf '%s%d n/a%s  ' "${C_DIM}" "${NA_COUNT}" "${C_RESET}"
printf '%stotal %d%s\n' "${C_DIM}" "${TOTAL}" "${C_RESET}"

if [[ ${FAIL_COUNT} -gt 0 ]]; then
    printf '\n%sFailed checks:%s\n' "${C_RED}" "${C_RESET}"
    for name in "${FAILED_CHECKS[@]}"; do
        printf '  - %s\n' "${name}"
    done
    exit 1
fi

if [[ ${SKIP_COUNT} -gt 0 ]]; then
    printf '\n%sInstall skipped tools to widen the local suite — the same tools run unconditionally in CI.%s\n' \
        "${C_DIM}" "${C_RESET}"
fi

exit 0