#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly REPO_ROOT
readonly ROCMINFO=/opt/rocm/bin/rocminfo
readonly ROCM_RELEASE=/opt/rocm/.info/version
readonly TARGET_DIR="${REPO_ROOT}/target/runtime-identity-oracle"
readonly EVIDENCE="${TARGET_DIR}/device-identity-measured-v1.txt"
readonly COMPARATOR="${SCRIPT_DIR}/runtime_identity_oracle.py"
readonly AUDITOR="${SCRIPT_DIR}/runtime_pure_rust_audit.py"
readonly POLICY="${SCRIPT_DIR}/runtime-pure-rust-policy.json"

if [[ "${FE2O3_ALLOW_RUNTIME_IDENTITY_ORACLE:-}" != 1 ]]; then
  printf '%s\n' \
    'refusing runtime identity oracle without FE2O3_ALLOW_RUNTIME_IDENTITY_ORACLE=1' >&2
  exit 2
fi
if [[ ! -c /dev/kfd || ! -r /dev/kfd || ! -w /dev/kfd ]]; then
  printf '%s\n' 'runtime identity oracle requires read/write access to /dev/kfd' >&2
  exit 2
fi
for path in "${ROCMINFO}" "${ROCM_RELEASE}" "${COMPARATOR}" "${AUDITOR}" "${POLICY}"; do
  if [[ ! -f "${path}" || -L "${path}" ]]; then
    printf 'runtime identity oracle requires a regular non-symlink input: %s\n' \
      "${path}" >&2
    exit 2
  fi
done
if [[ ! -x "${ROCMINFO}" || ! -x "${COMPARATOR}" ]]; then
  printf '%s\n' 'runtime identity oracle executable is not executable' >&2
  exit 2
fi

for directory in "${REPO_ROOT}/target" "${TARGET_DIR}"; do
  if [[ -L "${directory}" || (-e "${directory}" && ! -d "${directory}") ]]; then
    printf 'runtime identity oracle rejects substituted output directory: %s\n' \
      "${directory}" >&2
    exit 2
  fi
  mkdir -p -- "${directory}"
done
rm -f -- "${EVIDENCE}"
CAPTURE_DIR="$(mktemp -d "${TARGET_DIR}/capture.XXXXXX")"
readonly CAPTURE_DIR
chmod 700 "${CAPTURE_DIR}"
trap 'rm -rf -- "${CAPTURE_DIR}"' EXIT
readonly BUILD_DIR="${CAPTURE_DIR}/build"
readonly PURE_EXECUTABLE="${BUILD_DIR}/debug/examples/kfd-device-identity"

cd -- "${REPO_ROOT}"

# Re-establish that neither the separately launched oracle nor this harness is a
# production Cargo edge. The linked pure-Rust evidence producer is audited too.
python3 "${AUDITOR}" --policy "${POLICY}" metadata --cargo \
  --root fe2o3-kfd \
  --root fe2o3-drm-uapi \
  --root fe2o3-kfd-uapi \
  --root fe2o3-runtime-model
env CARGO_TARGET_DIR="${BUILD_DIR}" \
  cargo build --locked -p fe2o3-kfd --example kfd-device-identity
python3 "${AUDITOR}" --policy "${POLICY}" elf --input "${PURE_EXECUTABLE}"

pure_output="${CAPTURE_DIR}/pure-rust.txt"
pure_error="${CAPTURE_DIR}/pure-rust.err"
rocminfo_output="${CAPTURE_DIR}/rocminfo.txt"
rocminfo_error="${CAPTURE_DIR}/rocminfo.err"
evidence_temporary="${CAPTURE_DIR}/evidence.txt"

(
  ulimit -f 256
  timeout --signal=TERM --kill-after=5s 60s \
    env -i HOME="${CAPTURE_DIR}" LC_ALL=C PATH=/usr/bin:/bin \
      "${PURE_EXECUTABLE}" --all
) >"${pure_output}" 2>"${pure_error}"
if [[ -s "${pure_error}" ]]; then
  printf '%s\n' 'pure-Rust identity evidence producer wrote to stderr' >&2
  exit 2
fi

# rocminfo is an isolated HSA subprocess. Its bytes are never arguments,
# environment, files, descriptors, or authority inputs to the process above.
(
  ulimit -f 2048
  timeout --signal=TERM --kill-after=5s 60s \
    env -i HOME="${CAPTURE_DIR}" LC_ALL=C PATH=/usr/bin:/bin TERM=dumb \
      "${ROCMINFO}"
) >"${rocminfo_output}" 2>"${rocminfo_error}"
if [[ -s "${rocminfo_error}" ]]; then
  printf '%s\n' 'rocminfo wrote to stderr' >&2
  exit 2
fi

python3 "${COMPARATOR}" \
  --pure-rust-output "${pure_output}" \
  --rocminfo-output "${rocminfo_output}" \
  --rocm-release "${ROCM_RELEASE}" \
  --pure-rust-executable "${PURE_EXECUTABLE}" \
  --rocminfo-executable "${ROCMINFO}" >"${evidence_temporary}"
chmod 600 "${evidence_temporary}"
mv -f -- "${evidence_temporary}" "${EVIDENCE}"
cat -- "${EVIDENCE}"
