#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 4 ]]; then
  printf 'usage: %s BUILD_DIR EVIDENCE_DIR CARGO RUSTC\n' "$0" >&2
  exit 64
fi

repo_root=$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
build_dir=$1
evidence_dir=$2
cargo_bin=$3
rustc_bin=$4
golden="$repo_root/tests/fixtures/compiler-evidence/gfx942-alpha-zeta-cov6.json"
llvm_dir=/opt/rocm/lib/llvm/lib/cmake/llvm
lld_dir=/opt/rocm/lib/llvm/lib/cmake/lld
llvm_build_id_file=/opt/rocm/.info/version

for directory in "$build_dir" "$evidence_dir"; do
  if [[ $directory != /* || -e $directory ]]; then
    printf 'error: output must be an absent absolute path: %s\n' "$directory" >&2
    exit 65
  fi
  parent=$(realpath -e -- "$(dirname -- "$directory")")
  if [[ $parent/$(basename -- "$directory") != "$directory" ]]; then
    printf 'error: output parent or path is noncanonical: %s\n' "$directory" >&2
    exit 65
  fi
done

for executable in "$cargo_bin" "$rustc_bin"; do
  if [[ $executable != /* || ! -f $executable || ! -x $executable || -L $executable ]]; then
    printf 'error: toolchain executable is not pinned: %s\n' "$executable" >&2
    exit 66
  fi
  if [[ $(realpath -e -- "$executable") != "$executable" ]]; then
    printf 'error: toolchain executable path is noncanonical: %s\n' "$executable" >&2
    exit 66
  fi
done
if [[ ${cargo_bin%/*} != "${rustc_bin%/*}" ]]; then
  printf 'error: Cargo and rustc must come from one pinned toolchain\n' >&2
  exit 66
fi
toolchain_root=$(realpath -e -- "${cargo_bin%/*}/..")
toolchain_lib="$toolchain_root/lib"

if [[ -n $(git -C "$repo_root" status --porcelain --untracked-files=all) ]]; then
  printf 'error: compiler evidence requires a clean committed source tree\n' >&2
  exit 70
fi

for input in "$golden" "$llvm_dir/LLVMConfig.cmake" "$lld_dir/LLDConfig.cmake" "$llvm_build_id_file"; do
  if [[ ! -f $input || -L $input ]]; then
    printf 'error: required pinned input is unavailable: %s\n' "$input" >&2
    exit 66
  fi
done

cmake -S "$repo_root/tools/fe2o3-llvm-link-worker" -B "$build_dir" \
  -DLLVM_DIR="$llvm_dir" \
  -DLLD_DIR="$lld_dir" \
  -DFE2O3_PINNED_LLVM_VERSION=22.0.0git \
  -DFE2O3_LLVM_BUILD_ID_FILE="$llvm_build_id_file" \
  -DFE2O3_EXPECTED_LLVM_BUILD_ID=7.2.4 \
  -DBUILD_TESTING=ON \
  -DCMAKE_BUILD_TYPE=Release
cmake --build "$build_dir" --parallel "${FE2O3_BUILD_JOBS:-8}"
ctest --test-dir "$build_dir" --output-on-failure

worker="$build_dir/fe2o3-llvm-link-worker"
worker_build_id_file="$build_dir/fe2o3-worker-build-id.txt"
python3 - "$golden" "$worker" "$worker_build_id_file" <<'PY'
import hashlib
import json
import pathlib
import sys

golden_path, worker_path, build_id_path = map(pathlib.Path, sys.argv[1:])
golden = json.loads(golden_path.read_text(encoding="utf-8"))
worker = worker_path.read_bytes()
actual_digest = hashlib.sha256(worker).hexdigest()
actual_build_id = build_id_path.read_text(encoding="utf-8").strip()
if actual_digest != golden["worker_executable_sha256"]:
    raise SystemExit("fresh Worker executable does not match repository golden")
if actual_build_id != golden["worker_build_identity"]:
    raise SystemExit("fresh Worker build identity does not match repository golden")
if golden["linker_path"] != "llvm-lld-library-apis":
    raise SystemExit("repository golden does not require direct LLVM/LLD APIs")
PY

mkdir "$evidence_dir"
first="$evidence_dir/alpha-zeta-cov6-first.hsaco"
second="$evidence_dir/alpha-zeta-cov6-second.hsaco"
generator=worker_v2_general_v3_alpha_zeta_build_links_and_validate_backend_witnesses
for output in "$first" "$second"; do
  FE2O3_LLVM_LINK_WORKER="$worker" \
  FE2O3_LLVM_LINK_WORKER_BUILD_ID="$(<"$worker_build_id_file")" \
  FE2O3_LLVM_BUILD_ID=7.2.4 \
  FE2O3_GFX942_ALPHA_ZETA_OUTPUT="$output" \
  RUSTC="$rustc_bin" \
  LD_LIBRARY_PATH="$toolchain_lib" \
    "$cargo_bin" test --locked -p rustc-codegen-fe2o3 \
      --test kernel_ir_codegen "$generator" -- \
      --ignored --exact --nocapture
done
cmp -- "$first" "$second"

python3 - "$golden" "$evidence_dir" <<'PY'
import hashlib
import json
import pathlib
import sys

golden_path = pathlib.Path(sys.argv[1])
evidence_dir = pathlib.Path(sys.argv[2])
golden = json.loads(golden_path.read_text(encoding="utf-8"))
artifacts = sorted(evidence_dir.iterdir())
if [path.name for path in artifacts] != [
    "alpha-zeta-cov6-first.hsaco",
    "alpha-zeta-cov6-second.hsaco",
]:
    raise SystemExit("evidence directory contains an unexpected artifact set")
for artifact in artifacts:
    data = artifact.read_bytes()
    if len(data) != golden["hsaco_bytes"] or len(data) > golden["max_hsaco_bytes"]:
        raise SystemExit(f"{artifact.name}: bounded artifact size mismatch")
    if hashlib.sha256(data).hexdigest() != golden["hsaco_sha256"]:
        raise SystemExit(f"{artifact.name}: repository golden digest mismatch")
PY

RUSTC="$rustc_bin" LD_LIBRARY_PATH="$toolchain_lib" \
  "$cargo_bin" test --locked -p fe2o3-hsa-runtime \
    --features hardware-test-hooks --test gfx942_two_kernel_hardware

expected_sha=$(python3 - "$golden" <<'PY'
import json
import pathlib
import sys
print(json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))["hsaco_sha256"])
PY
)
FE2O3_RUN_GFX942_TWO_KERNEL=1 \
FE2O3_GFX942_ALPHA_ZETA_HSACO="$first" \
FE2O3_GFX942_ALPHA_ZETA_SHA256="$expected_sha" \
RUSTC="$rustc_bin" \
LD_LIBRARY_PATH="$toolchain_lib" \
  "$cargo_bin" test --locked -p fe2o3-hsa-runtime \
    --features hardware-test-hooks --test gfx942_two_kernel_hardware \
    gfx942_cov6_repository_golden_alpha_then_zeta_one_executable -- \
    --ignored --exact --nocapture

printf 'source commit: %s\n' "$(git -C "$repo_root" rev-parse HEAD)"
printf 'worker build identity: %s\n' "$(<"$worker_build_id_file")"
printf 'artifact SHA-256: %s\n' "$expected_sha"
printf 'artifact bytes: %s\n' "$(stat -c %s -- "$first")"
printf 'reproducible Worker V2 COV6 alpha/zeta MI300X evidence: PASS\n'
