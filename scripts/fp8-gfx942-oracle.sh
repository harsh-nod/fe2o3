#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 [--check|--write|--stdout]" >&2
  exit 2
}

mode="${1:---check}"
case "${mode}" in
  --check|--write|--stdout) ;;
  *) usage ;;
esac

export LC_ALL=C
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(git -C "${script_dir}" rev-parse --show-toplevel)"
oracle_source="${repo_root}/crates/fe2o3-device/tests/oracle/fp8_gfx942_oracle.hip"
golden="${repo_root}/crates/fe2o3-device/tests/fixtures/fp8_gfx942_rocm.golden"
target="gfx942"
cxx_standard="c++20"
hipcc="${HIPCC:-hipcc}"

command -v "${hipcc}" >/dev/null
command -v hipconfig >/dev/null
command -v rocminfo >/dev/null

rocminfo_output="$(rocminfo)"
if ! rg -q "Name:[[:space:]]+${target}([[:space:]]|$)" <<<"${rocminfo_output}"; then
  echo "no visible ${target} agent" >&2
  exit 1
fi

temporary="$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-fp8-oracle.XXXXXX")"
trap 'rm -rf -- "${temporary}"' EXIT
binary="${temporary}/fp8-gfx942-oracle"
actual="${temporary}/fp8_gfx942_rocm.golden"

"${hipcc}" -std="${cxx_standard}" --offload-arch="${target}" -O2 \
  "${oracle_source}" -o "${binary}"

hip_version="$(hipconfig --version | tr -d '\r\n')"
compiler_version="$("${hipcc}" --version)"
clang_version="$(printf '%s\n' "${compiler_version}" | sed -n 's/^AMD clang version \([^ ]*\).*/\1/p')"
rocm_release="$(printf '%s\n' "${compiler_version}" | sed -n 's/.* roc-\([^ ]*\) .*/\1/p')"
clang_revision="$(printf '%s\n' "${compiler_version}" | sed -n 's/.* \([0-9a-f]\{40\}\)).*/\1/p')"
oracle_digest="$(sha256sum "${oracle_source}" | cut -d' ' -f1)"
generator_digest="$(sha256sum "${BASH_SOURCE[0]}" | cut -d' ' -f1)"

{
  echo "meta schema fe2o3-fp8-gfx942-golden-v1"
  echo "meta target ${target}"
  echo "meta cxx-standard ${cxx_standard}"
  echo "meta rocm-release ${rocm_release}"
  echo "meta hip-version ${hip_version}"
  echo "meta clang-version ${clang_version}"
  echo "meta clang-revision ${clang_revision}"
  echo "meta rounding rne"
  echo "meta saturation satfinite"
  echo "meta fnuz-nan-f32 ffc00000"
  echo "meta oracle-sha256 ${oracle_digest}"
  echo "meta generator-sha256 ${generator_digest}"
  "${binary}"
} >"${actual}"

case "${mode}" in
  --stdout)
    cat "${actual}"
    ;;
  --write)
    mkdir -p "$(dirname -- "${golden}")"
    cp "${actual}" "${golden}"
    echo "updated ${golden#"${repo_root}/"}"
    ;;
  --check)
    if ! cmp -s "${golden}" "${actual}"; then
      diff -u "${golden}" "${actual}" || true
      echo "gfx942 FP8 golden is stale; rerun $0 --write on gfx942" >&2
      exit 1
    fi
    echo "gfx942 FP8 golden matches native hardware and toolchain metadata"
    ;;
esac
