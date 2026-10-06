#!/usr/bin/env bash
set -euo pipefail

case "$#:${1-}" in
  0:|1:v3) family=v3; suffix="" ;;
  1:v1) family=v1; suffix="-v1" ;;
  *) printf 'usage: %s [v1|v3]\n' "$0" >&2; exit 2 ;;
esac
readonly family suffix

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly repo_root
readonly target_dir="${FE2O3_STATIC_COORDINATOR_TARGET_DIR:-${repo_root}/target/static-coordinator${suffix}}"
readonly target="x86_64-unknown-linux-musl"
readonly binary="fe2o3-compiler-execution-coordinator${suffix}"
readonly executable="${target_dir}/${target}/release/${binary}"

cd -- "${repo_root}"
CARGO_TARGET_DIR="${target_dir}" cargo rustc \
  --locked \
  --release \
  --target "${target}" \
  -p fe2o3-compiler-execution-coordinator \
  --bin "${binary}" \
  -- \
  -C target-feature=+crt-static \
  -C relocation-model=static \
  -C link-arg=-static \
  -C link-arg=-no-pie

readonly report="${target_dir}/${binary}.readelf.txt"
/usr/bin/readelf -hW -lW -dW -sW -- "${executable}" >"${report}"
/usr/bin/grep -Eq 'Class:[[:space:]]+ELF64' "${report}"
/usr/bin/grep -Eq 'Type:[[:space:]]+EXEC' "${report}"
if /usr/bin/grep -Eq 'INTERP|DYNAMIC|\(NEEDED\)|\(RPATH\)|\(RUNPATH\)' "${report}"; then
  printf 'compiler-execution coordinator contains a dynamic-loader dependency\n' >&2
  exit 1
fi
/usr/bin/grep -Eq 'GNU_STACK.*RW[[:space:]]' "${report}"
undefined_symbols="$(/usr/bin/nm -u -- "${executable}")"
if [[ -n "${undefined_symbols}" ]]; then
  printf 'compiler-execution coordinator contains undefined symbols\n' >&2
  exit 1
fi

set +e
smoke_output="$({ /usr/bin/env -i "${executable}" \
  3<&- 4<&- 5<&- 6<&- 7<&- 8<&- 9<&- 10<&- 11<&- 12<&- 13<&- 14<&- 15<&- 16<&-; } 2>&1)"
smoke_status=$?
set -e
if [[ ${family} == v1 ]]; then
  expected_smoke='invalid coordinator activation: LISTEN_PID does not name this process'
elif [[ $(id -u) -eq 0 ]]; then
  expected_smoke='native root activation: missing activation variable'
else
  expected_smoke='native compiler coordinator requires exact root identity'
fi
if [[ ${smoke_status} -ne 1 \
  || "${smoke_output}" != "${expected_smoke}" ]]; then
  printf 'compiler-execution coordinator did not fail closed without activation metadata\n' >&2
  exit 1
fi

set +e
argument_output="$({ /usr/bin/env -i "${executable}" forbidden; } 2>&1)"
argument_status=$?
set -e
if [[ ${family} == v1 ]]; then
  expected_argument='invalid coordinator activation: arguments are forbidden'
elif [[ $(id -u) -eq 0 ]]; then
  expected_argument='native root activation: expected one bounded nonempty argv0'
else
  expected_argument='native compiler coordinator requires exact root identity'
fi
if [[ ${argument_status} -ne 1 \
  || "${argument_output}" != "${expected_argument}" ]]; then
  printf 'compiler-execution coordinator accepted an argument\n' >&2
  exit 1
fi

printf '%s\n' "${executable}"
