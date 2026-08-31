#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly repo_root
readonly target_dir="${FE2O3_STATIC_COORDINATOR_TARGET_DIR:-${repo_root}/target/static-coordinator}"
readonly target="x86_64-unknown-linux-musl"
readonly executable="${target_dir}/${target}/release/fe2o3-compiler-execution-coordinator"

cd -- "${repo_root}"
CARGO_TARGET_DIR="${target_dir}" cargo rustc \
  --locked \
  --release \
  --target "${target}" \
  -p fe2o3-compiler-execution-coordinator \
  --bin fe2o3-compiler-execution-coordinator \
  -- \
  -C target-feature=+crt-static \
  -C relocation-model=static \
  -C link-arg=-static \
  -C link-arg=-no-pie

readonly report="${target_dir}/fe2o3-compiler-execution-coordinator.readelf.txt"
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
if [[ ${smoke_status} -ne 1 \
  || "${smoke_output}" != 'invalid coordinator activation: LISTEN_PID does not name this process' ]]; then
  printf 'compiler-execution coordinator did not fail closed without activation metadata\n' >&2
  exit 1
fi

set +e
argument_output="$({ /usr/bin/env -i "${executable}" forbidden; } 2>&1)"
argument_status=$?
set -e
if [[ ${argument_status} -ne 1 \
  || "${argument_output}" != 'invalid coordinator activation: arguments are forbidden' ]]; then
  printf 'compiler-execution coordinator accepted an argument\n' >&2
  exit 1
fi

printf '%s\n' "${executable}"
