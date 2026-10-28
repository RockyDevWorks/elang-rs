#!/usr/bin/env bash
# Exercise the real scanner without committing complete credential URLs.
# Fixtures are generated outside the checkout and are not allowlisted by path.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG="${SCRIPT_DIR}/../.gitleaks.toml"
FIXTURE_ROOT="$(mktemp -d)"
trap 'rm -rf "${FIXTURE_ROOT}"' EXIT
mkdir "${FIXTURE_ROOT}/input"
printf '{{range .}}{{.Match}}{{end}}' > "${FIXTURE_ROOT}/report.tmpl"

check_url() {
    local expected="$1" scheme="$2" username="$3" password="$4" host="$5"
    local url="${scheme}://${username}:${password}@${host}"
    local status=0 actual
    printf '%s\n' "${url}" > "${FIXTURE_ROOT}/input/credentials.txt"
    gitleaks dir "${FIXTURE_ROOT}/input" --config "${CONFIG}" \
        --no-banner --log-level error --enable-rule credential-url --exit-code 42 \
        --report-format template --report-template "${FIXTURE_ROOT}/report.tmpl" \
        --report-path "${FIXTURE_ROOT}/report.txt" || status=$?
    actual="$(cat "${FIXTURE_ROOT}/report.txt")"
    if [[ "${status}" -ne "${expected}" ]] || \
        { [[ "${expected}" -eq 42 ]] && [[ "${actual}" != "${url}" ]]; } || \
        { [[ "${expected}" -eq 0 ]] && [[ -n "${actual}" ]]; }; then
        printf 'FAIL: %s fixture (expected exit %s, got %s; match: %s)\n' \
            "${url}" "${expected}" "${status}" "${actual}" >&2
        exit 1
    fi
}

# Percent escapes in either component and both, with upper/lowercase hex.
check_url 42 https 'alice%40team' 'p%3Ass%2Fword' scanner.test
check_url 42 http 'alice%2bteam' plain-pass scanner.test
check_url 42 https alice '%40%3a%2f' scanner.test
check_url 42 http alice plain-pass scanner.test
for malformed in '%' '%2' '%GG' '%2G' '%G2'; do
    check_url 0 https "alice${malformed}" plain-pass scanner.test
    check_url 0 https alice "pass${malformed}" scanner.test
done

# Only the exact local fixture is safe; other credentials/ports must leak.
check_url 0 http user pass localhost:3000
for host in localhost localhost:3001 localhost:30000 localhost.evil:3000 \
    127.0.0.1:3000 0.0.0.0:3000 '[::1]:3000'; do
    check_url 42 http user pass "${host}"
done
check_url 42 https user pass localhost:3000
check_url 42 http other-user pass localhost:3000
check_url 42 http user other-pass localhost:3000
check_url 42 http 'alice%40team' 'p%3Ass' localhost:3000
check_url 42 http 'alice%40team' 'p%3Ass' 127.0.0.1:3000
check_url 42 http 'alice%40team' 'p%3Ass' '[::1]:3000'
check_url 0 http user pass host.invalid
check_url 0 https user pass fake-host.invalid

printf 'Gitleaks credential URL fixtures passed.\n'
