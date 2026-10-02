#!/usr/bin/env bash
# Invoked inside one ci-local run_step deadline; no nested build or timeout policy.
set -Eeuo pipefail
umask 077

library_kinds=
if [[ "${1:-}" == --lib ]]; then
  [[ $# -ge 2 && -n "$2" ]] || exit 2
  library_kinds="$2"
  shift 2
fi
[[ $# == 3 && "$1" =~ ^[A-Za-z0-9_][A-Za-z0-9_-]*$ && "$2" =~ ^[A-Za-z0-9_][A-Za-z0-9_-]*$ \
   && "$3" =~ ^[A-Za-z_][A-Za-z0-9_]*(::[A-Za-z_][A-Za-z0-9_]*)*$ ]] || {
  printf '%s\n' 'usage: ci-cargo-test-json.sh [--lib <exact-comma-separated-kinds>] <package> <target> <exact-test-name>' >&2
  exit 2
}
readonly package="$1" test_target="$2" test_name="$3"
declare -a target_arguments=(--test "${test_target}") verifier_arguments=() cargo_environment=()
if [[ -n "${library_kinds}" ]]; then
  [[ "${library_kinds}" =~ ^(lib|rlib|dylib|cdylib|staticlib|proc-macro)(,(lib|rlib|dylib|cdylib|staticlib|proc-macro))*$ ]] || exit 2
  declare -A seen_library_kinds=()
  IFS=, read -r -a kind_roster <<<"${library_kinds}"
  for kind in "${kind_roster[@]}"; do
    [[ ! -v "seen_library_kinds[${kind}]" ]] || exit 2
    seen_library_kinds["${kind}"]=1
  done
  target_arguments=(--lib)
  verifier_arguments=(--lib "${library_kinds}")
fi
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd -- "${script_dir}/.." && pwd)"
readonly script_dir repo
readonly limit=10485760
base="${TMPDIR:-/tmp}"
[[ "${base}" != *$'\n'* && "${base}" != *$'\r'* ]] || exit 2
base="$(realpath --canonicalize-existing -- "${base}")"
[[ -d "${base}" ]] || exit 2
# ci-local's local default temporary directory may be inside target/ in source.
# Preserve an external caller TMPDIR; source-internal fallback also isolates the child.
if [[ "${base}" == "${repo}" || "${base}" == "${repo}/"* ]]; then
  base=/tmp
  cargo_environment=(TMPDIR=)
fi
receipt_root="$(mktemp -d -- "${base}/fe2o3-source-test-json.XXXXXXXXXX")"
[[ "$(realpath -- "${receipt_root}")" == "${receipt_root}" \
   && ! -L "${receipt_root}" && -d "${receipt_root}" \
   && "$(stat -c %u -- "${receipt_root}")" == "$(id -u)" \
   && "$(stat -c %a -- "${receipt_root}")" == 700 \
   && "${receipt_root}" != "${repo}" && "${receipt_root}" != "${repo}/"* ]] || exit 2
readonly receipt="${receipt_root}/events.jsonl"
if ((${#cargo_environment[@]})); then
  cargo_environment=(TMPDIR="${receipt_root}")
fi
printf 'Cargo/libtest JSON evidence retained at %s\n' "${receipt}"
cd -- "${repo}"

# Bound only captured stdout and mirror that prefix into the existing CI log.
# Drain excess bytes; run_step still bounds the complete process tree.
set +e
env "${cargo_environment[@]}" cargo test --locked --offline -p "${package}" "${target_arguments[@]}" \
  "${test_name}" --message-format=json -- \
  -Z unstable-options --format=json --ignored --exact --test-threads=1 |
  python3 -I -B -c '
import pathlib
import sys

path, maximum = pathlib.Path(sys.argv[1]), int(sys.argv[2])
remaining = maximum
overflow = False
with path.open("xb") as output:
    while chunk := sys.stdin.buffer.read(65536):
        accepted = min(remaining, len(chunk))
        output.write(chunk[:accepted])
        sys.stdout.buffer.write(chunk[:accepted])
        sys.stdout.buffer.flush()
        remaining -= accepted
        overflow |= accepted != len(chunk)
if overflow:
    print(f"Cargo JSON exceeded {maximum} bytes; retained prefix only", file=sys.stderr)
    raise SystemExit(75)
' "${receipt}" "${limit}"
statuses=("${PIPESTATUS[@]}")
set -e
printf 'Cargo exit: %s; JSON capture exit: %s\n' "${statuses[0]}" "${statuses[1]}" >&2
(( statuses[0] == 0 )) || exit "${statuses[0]}"
(( statuses[1] == 0 )) || exit "${statuses[1]}"
python3 -I -B "${script_dir}/verify-cargo-test-json.py" "${receipt}" \
  --test-target "${test_target}" --test-name "${test_name}" --allow-filtered "${verifier_arguments[@]}"
