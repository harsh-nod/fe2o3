#!/usr/bin/env bash
# Build-only synthetic fixtures. Never executes an issuer or claims production
# startup/recovery evidence. Run explicitly with bash; no Cargo manifest changes.
set -euo pipefail
umask 077
export LC_ALL=C

fail() {
  printf 'native readiness fixture: %s\n' "$*" >&2
  exit 1
}

if [[ $# -lt 1 || $# -gt 2 ]]; then
  fail 'usage: bash scripts/build-native-ready-fixture.sh EXISTING_EMPTY_PRIVATE_OUTPUT_DIR [v2|v3]'
fi
readonly family="${2-v2}"
case "${family}" in
  v2)
    readonly artifact_prefix='native-ready-fixture'
    readonly export_prefix='FE2O3_NATIVE_READY_FIXTURE'
    ;;
  v3)
    readonly artifact_prefix='native-ready-fixture-v3'
    readonly export_prefix='FE2O3_NATIVE_READY_FIXTURE_V3'
    ;;
  *) fail 'family must be v2 or v3' ;;
esac
readonly repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly toolchain='nightly-2026-04-03'
readonly host='x86_64-unknown-linux-gnu'
readonly target='x86_64-unknown-linux-musl'
readonly package='fe2o3-compiler-execution-issuer'
readonly example='native_ready_fixture'

[[ "$(uname -s)" == Linux && "$(uname -m)" == x86_64 ]] || fail 'requires Linux x86-64'
[[ -d "$1" && ! -L "$1" ]] || fail 'output must be an existing directory, not a symlink'
readonly output="$(cd -- "$1" && pwd -P)"
[[ "$(stat -c '%u:%a' -- "${output}")" == "${EUID}:700" ]] \
  || fail 'output must be owned by the caller and have mode 0700'
shopt -s nullglob dotglob
contents=("${output}"/*)
[[ ${#contents[@]} -eq 0 ]] || fail 'output must be empty (use a fresh private directory)'
shopt -u nullglob dotglob

for command in timeout prlimit readelf nm awk grep install sha256sum; do
  command -v "${command}" >/dev/null || fail "missing prerequisite: ${command}"
done
readonly cargo_home="${CARGO_HOME:-${HOME}/.cargo}"
readonly rustup_home="${RUSTUP_HOME:-${HOME}/.rustup}"
# Resolve the installed toolchain without relying on PATH's cargo being a
# rustup proxy. Raw Cargo does not understand rustup's +toolchain argument.
toolchain_root="${rustup_home}/toolchains/${toolchain}-${host}"
[[ -d "${toolchain_root}" ]] || fail "pinned toolchain is not installed: ${toolchain_root}"
toolchain_root="$(cd -- "${toolchain_root}" && pwd -P)"
readonly toolchain_root
readonly cargo="${toolchain_root}/bin/cargo"
readonly rustc="${toolchain_root}/bin/rustc"
readonly rustdoc="${toolchain_root}/bin/rustdoc"
for executable_tool in "${cargo}" "${rustc}" "${rustdoc}"; do
  [[ -f "${executable_tool}" && -x "${executable_tool}" ]] \
    || fail "missing pinned tool: ${executable_tool}"
done
readonly build_path="${toolchain_root}/bin:/usr/bin:/bin"
readonly toolchain_library_path="${toolchain_root}/lib:${toolchain_root}/lib/rustlib/${host}/lib"
# This is the actual target directory, e.g. the primary's CACHE/target, not
# CACHE itself. Preserve an explicitly supplied CARGO_TARGET_DIR as-is.
build_dir="${CARGO_TARGET_DIR:-${output}/target}"
if [[ "${build_dir}" != /* ]]; then
  build_dir="${PWD}/${build_dir}"
fi
readonly build_dir
readonly executable="${build_dir}/${target}/release/examples/${example}"
mkdir -p -- "${build_dir}" "${output}/tmp"
cd -- "${repo_root}"

# All four builds are serialized. Each build process tree gets a 15-minute
# wall-clock deadline (TERM, then KILL after 10 seconds). Inherited per-process
# limits: 15 CPU minutes, 8 GiB address space, 1 GiB/file, 512 FDs, no core dump.
# --frozen --offline requires an installed pinned toolchain, musl std component,
# native C linker/binutils and a populated dependency cache; no network fetches.
# Honor the primary's shared CARGO_TARGET_DIR/CARGO_HOME; only one primary may
# build into that cache at a time. The clean environment removes inherited
# RUSTFLAGS, wrappers and runtime fixture mode/family variables.
for mode in ready no-eof trailing silent; do
  artifact="${output}/${artifact_prefix}-${mode}"
  report="${artifact}.readelf.txt"
  log="${artifact}.build.log"
  printf 'building native readiness fixture mode %s\n' "${mode}" >&2
  if timeout --signal=TERM --kill-after=10s 900s \
    prlimit --cpu=900:900 --as=8589934592:8589934592 \
      --fsize=1073741824:1073741824 --nofile=512:512 --core=0:0 -- \
    /usr/bin/env -i \
      PATH="${build_path}" HOME="${HOME}" \
      CARGO_HOME="${cargo_home}" RUSTUP_HOME="${rustup_home}" \
      RUSTC="${rustc}" RUSTDOC="${rustdoc}" LD_LIBRARY_PATH="${toolchain_library_path}" \
      RUSTUP_AUTO_INSTALL=0 LC_ALL=C TMPDIR="${output}/tmp" \
      CARGO_TARGET_DIR="${build_dir}" CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 \
      CARGO_NET_OFFLINE=true \
      CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 CARGO_PROFILE_RELEASE_STRIP=none \
      RAYON_NUM_THREADS=1 \
      "${cargo}" rustc \
        --frozen --offline --release --jobs 1 --target "${target}" \
        -p "${package}" --example "${example}" -- \
        --check-cfg 'cfg(fe2o3_native_ready_fixture, values("ready", "no-eof", "trailing", "silent"))' \
        --cfg "fe2o3_native_ready_fixture=\"${mode}\"" \
        --check-cfg 'cfg(fe2o3_native_ready_fixture_family, values("v2", "v3"))' \
        --cfg "fe2o3_native_ready_fixture_family=\"${family}\"" \
        -C target-feature=+crt-static \
        -C relocation-model=static \
        -C link-arg=-static \
        -C link-arg=-no-pie \
        -C link-arg=-Wl,-e,fe2o3_secure_start_v1 \
        >"${log}" 2>&1; then
    :
  else
    status=$?
    fail "build failed or exceeded its bound (status ${status}); inspect ${log}"
  fi

  readelf -hW -lW -dW -sW -- "${executable}" >"${report}"
  grep -Eq 'Class:[[:space:]]+ELF64' "${report}" || fail "${mode}: not ELF64"
  grep -Eq 'Machine:[[:space:]]+Advanced Micro Devices X86-64' "${report}" \
    || fail "${mode}: wrong machine"
  grep -Eq 'Type:[[:space:]]+EXEC' "${report}" || fail "${mode}: not static ET_EXEC"
  entry_address="$(awk '/Entry point address:/ { print $4 }' "${report}")"
  secure_start_address="$(
    nm -n --defined-only -- "${executable}" \
      | awk '$3 == "fe2o3_secure_start_v1" { print "0x" $1 }'
  )"
  [[ "${entry_address}" =~ ^0x[0-9a-fA-F]+$ \
    && "${secure_start_address}" =~ ^0x[0-9a-fA-F]+$ ]] \
    || fail "${mode}: missing unique secure entry"
  [[ $((entry_address)) -eq $((secure_start_address)) ]] \
    || fail "${mode}: ELF entry is not fe2o3_secure_start_v1"
  if grep -Eq 'INTERP|DYNAMIC|\(NEEDED\)|\(RPATH\)|\(RUNPATH\)' "${report}"; then
    fail "${mode}: dynamic-loader dependency"
  fi
  grep -Eq 'GNU_STACK.*RW[[:space:]]' "${report}" || fail "${mode}: missing non-executable stack"
  if grep -Eq 'GNU_STACK.*RWE' "${report}"; then
    fail "${mode}: executable stack"
  fi
  undefined_symbols="$(nm -u -- "${executable}")"
  [[ -z "${undefined_symbols}" ]] || fail "${mode}: undefined symbols"
  install -m 0500 -- "${executable}" "${artifact}"
done

# Record identities after all modes pass. No smoke execution or libtest launch.
sha256sum -- \
  "${output}/${artifact_prefix}-ready" \
  "${output}/${artifact_prefix}-no-eof" \
  "${output}/${artifact_prefix}-trailing" \
  "${output}/${artifact_prefix}-silent" >"${output}/SHA256SUMS"
printf '# Synthetic native readiness fixtures only; supply FE2O3_STATIC_PREEXEC_LAUNCHER separately.\n'
printf 'export %s_READY=%q\n' "${export_prefix}" "${output}/${artifact_prefix}-ready"
printf 'export %s_NO_EOF=%q\n' "${export_prefix}" "${output}/${artifact_prefix}-no-eof"
printf 'export %s_TRAILING=%q\n' "${export_prefix}" "${output}/${artifact_prefix}-trailing"
printf 'export %s_SILENT=%q\n' "${export_prefix}" "${output}/${artifact_prefix}-silent"
