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
readonly RUNNER="${SCRIPT_DIR}/runtime-identity-oracle.sh"
readonly COMPARATOR="${SCRIPT_DIR}/runtime_identity_oracle.py"
readonly AUDITOR="${SCRIPT_DIR}/runtime_pure_rust_audit.py"
readonly POLICY="${SCRIPT_DIR}/runtime-pure-rust-policy.json"
readonly CARGO_LOCK="${REPO_ROOT}/Cargo.lock"

capture_git_observation() {
  local output="$1"
  local head_file="${output}.head"
  local status_file="${output}.status"
  local head

  (
    ulimit -f 2
    git rev-parse --verify 'HEAD^{commit}'
  ) >"${head_file}"
  if ! IFS= read -r head <"${head_file}" ||
    [[ ! "${head}" =~ ^[0-9a-f]{40}$ ]]; then
    printf '%s\n' 'runtime identity oracle could not capture a canonical Git HEAD' >&2
    return 2
  fi
  (
    ulimit -f 32
    git status --porcelain=v1 --untracked-files=all
  ) >"${status_file}"
  if [[ -s "${status_file}" ]]; then
    printf 'head=%s\nworktree=dirty\n' "${head}" >"${output}"
  else
    printf 'head=%s\nworktree=clean\n' "${head}" >"${output}"
  fi
  rm -f -- "${head_file}" "${status_file}"
}

if [[ "${FE2O3_ALLOW_RUNTIME_IDENTITY_ORACLE:-}" != 1 ]]; then
  printf '%s\n' \
    'refusing runtime identity oracle without FE2O3_ALLOW_RUNTIME_IDENTITY_ORACLE=1' >&2
  exit 2
fi
if [[ ! -c /dev/kfd || ! -r /dev/kfd || ! -w /dev/kfd ]]; then
  printf '%s\n' 'runtime identity oracle requires read/write access to /dev/kfd' >&2
  exit 2
fi
for path in \
  "${ROCMINFO}" \
  "${ROCM_RELEASE}" \
  "${RUNNER}" \
  "${COMPARATOR}" \
  "${AUDITOR}" \
  "${POLICY}" \
  "${CARGO_LOCK}"; do
  if [[ ! -f "${path}" || -L "${path}" ]]; then
    printf 'runtime identity oracle requires a regular non-symlink input: %s\n' \
      "${path}" >&2
    exit 2
  fi
done
if [[ ! -x "${ROCMINFO}" || ! -x "${RUNNER}" || ! -x "${COMPARATOR}" ||
  ! -x "${AUDITOR}" ]]; then
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
readonly GIT_OBSERVATION="${CAPTURE_DIR}/git-observation.txt"
readonly GIT_OBSERVATION_AFTER="${CAPTURE_DIR}/git-observation-after.txt"
readonly GIT_OBSERVATION_FINAL="${CAPTURE_DIR}/git-observation-final.txt"
readonly METADATA_AUDIT_REPORT="${CAPTURE_DIR}/metadata-audit-report.txt"
readonly METADATA_AUDIT_ERROR="${CAPTURE_DIR}/metadata-audit-error.txt"
readonly ELF_AUDIT_REPORT="${CAPTURE_DIR}/elf-audit-report.txt"
readonly ELF_AUDIT_ERROR="${CAPTURE_DIR}/elf-audit-error.txt"
readonly MEASUREMENT_TIME="${CAPTURE_DIR}/measurement-time.txt"
readonly MAX_AUDIT_REPORT_BYTES=4096

cd -- "${REPO_ROOT}"

capture_git_observation "${GIT_OBSERVATION}"
if ! grep -qx 'worktree=clean' "${GIT_OBSERVATION}"; then
  printf '%s\n' 'runtime identity oracle requires a clean Git worktree' >&2
  exit 2
fi

# Re-establish that neither the separately launched oracle nor this harness is a
# production Cargo edge. Bound the report through the pipe instead of applying
# RLIMIT_FSIZE to Cargo: Cargo may update its shared global-cache database while
# producing metadata, and that unrelated file can already exceed the report cap.
if ! python3 "${AUDITOR}" --policy "${POLICY}" metadata --cargo \
    --root fe2o3-kfd \
    --root fe2o3-drm-uapi \
    --root fe2o3-kfd-uapi \
    --root fe2o3-runtime-model \
    2>"${METADATA_AUDIT_ERROR}" | \
    head -c "$((MAX_AUDIT_REPORT_BYTES + 1))" >"${METADATA_AUDIT_REPORT}"; then
  cat -- "${METADATA_AUDIT_ERROR}" >&2
  exit 2
fi
if [[ $(stat -c %s -- "${METADATA_AUDIT_REPORT}") -gt ${MAX_AUDIT_REPORT_BYTES} ]]; then
  printf '%s\n' 'metadata audit report exceeded its byte bound' >&2
  exit 2
fi
if [[ -s "${METADATA_AUDIT_ERROR}" ]]; then
  printf '%s\n' 'metadata audit wrote to stderr' >&2
  exit 2
fi
cat -- "${METADATA_AUDIT_REPORT}"
env CARGO_TARGET_DIR="${BUILD_DIR}" \
  cargo build --locked -p fe2o3-kfd --example kfd-device-identity
if ! (
  ulimit -f 16
  python3 "${AUDITOR}" --policy "${POLICY}" elf --input "${PURE_EXECUTABLE}"
) >"${ELF_AUDIT_REPORT}" 2>"${ELF_AUDIT_ERROR}"; then
  cat -- "${ELF_AUDIT_ERROR}" >&2
  exit 2
fi
if [[ -s "${ELF_AUDIT_ERROR}" ]]; then
  printf '%s\n' 'ELF audit wrote to stderr' >&2
  exit 2
fi
cat -- "${ELF_AUDIT_REPORT}"

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

capture_git_observation "${GIT_OBSERVATION_AFTER}"
if ! cmp -s -- "${GIT_OBSERVATION}" "${GIT_OBSERVATION_AFTER}"; then
  printf '%s\n' 'Git HEAD or worktree changed during identity measurement' >&2
  exit 2
fi
(
  ulimit -f 2
  date --utc '+%Y-%m-%dT%H:%M:%SZ'
) >"${MEASUREMENT_TIME}"

python3 "${COMPARATOR}" \
  --pure-rust-output "${pure_output}" \
  --rocminfo-output "${rocminfo_output}" \
  --rocm-release "${ROCM_RELEASE}" \
  --pure-rust-executable "${PURE_EXECUTABLE}" \
  --rocminfo-executable "${ROCMINFO}" \
  --runner "${RUNNER}" \
  --policy "${POLICY}" \
  --auditor "${AUDITOR}" \
  --cargo-lock "${CARGO_LOCK}" \
  --metadata-audit-report "${METADATA_AUDIT_REPORT}" \
  --elf-audit-report "${ELF_AUDIT_REPORT}" \
  --git-observation "${GIT_OBSERVATION}" \
  --measurement-time "${MEASUREMENT_TIME}" >"${evidence_temporary}"
capture_git_observation "${GIT_OBSERVATION_FINAL}"
if ! cmp -s -- "${GIT_OBSERVATION}" "${GIT_OBSERVATION_FINAL}"; then
  printf '%s\n' 'Git HEAD or worktree changed while producing identity evidence' >&2
  exit 2
fi
chmod 600 "${evidence_temporary}"
mv -f -- "${evidence_temporary}" "${EVIDENCE}"
cat -- "${EVIDENCE}"
