#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
DEFAULT_REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly DEFAULT_REPO_ROOT
readonly DEFAULT_STATUS="${DEFAULT_REPO_ROOT}/docs/cuda-oxide-parity-status.tsv"
readonly DEFAULT_MATRIX="${DEFAULT_REPO_ROOT}/docs/cuda-oxide-parity-matrix.md"
readonly DEFAULT_MARKDOWN="${DEFAULT_REPO_ROOT}/docs/generated/cuda-oxide-parity-dashboard.md"
readonly DEFAULT_TSV="${DEFAULT_REPO_ROOT}/docs/generated/cuda-oxide-parity-dashboard.tsv"
readonly DEFAULT_SIGNED_PROMOTIONS="${DEFAULT_REPO_ROOT}/docs/generated/cuda-oxide-parity-signed-promotions.tsv"
readonly MAX_INPUT_BYTES=1048576
readonly MAX_FIELD_LENGTH=4096

declare -A ROW_KIND=()
declare -A ROW_STATUS=()
declare -A ROW_FEATURE=()
declare -A ROW_CLASS=()
declare -A ROW_GATES=()
declare -A ROW_ACCEPTANCE=()
declare -A EVIDENCE_PATHS=()
declare -A EVIDENCE_TESTS=()
declare -A EVIDENCE_COMMIT=()
declare -A EVIDENCE_LANES=()
declare -A EVIDENCE_TOOLCHAINS=()
declare -A EVIDENCE_STRENGTHS=()
declare -A EVIDENCE_LIMITATIONS=()
declare -A EVIDENCE_USED=()
declare -A EVIDENCE_SIGNED=()
declare -A CLAIM_STATUS=()
declare -A CLAIM_EVIDENCE=()

STATUS_COMMIT=""
CUDA_COMMIT=""
CLAIMS_COMMIT=""
TEMP_ROOT=""
REPO_ROOT="${DEFAULT_REPO_ROOT}"

cleanup() {
  if [[ -n "${TEMP_ROOT}" ]]; then
    rm -rf -- "${TEMP_ROOT}"
  fi
}
trap cleanup EXIT

usage() {
  cat <<'EOF'
Usage: scripts/parity-dashboard.sh <check|update|validate|claims> [options]

Options:
  --status FILE       Canonical 109-row status TSV
  --matrix FILE       Canonical parity matrix Markdown
  --claims FILE       Claim/evidence records (default: built-in audited records)
  --repo PATH         Repository root used for stale-path checks
  --markdown FILE     Generated Markdown dashboard
  --tsv FILE          Generated machine-readable dashboard
  --signed-promotions FILE
                      Protected deterministic signed-promotion ledger

  --promotion-baseline FILE
                      Previous canonical status snapshot
  --row-evidence-archive DIR
                      Archive containing the promotion manifest and records
  --row-evidence-manifest PATH
                      Archive-relative signed-evidence promotion manifest
  --row-evidence-trusted-root DIR
                      Protected-base root containing trust inputs and public keys
  --row-evidence-trust-policy FILE
                      Protected-base runner/reviewer public-key policy
  --row-evidence-trusted-policy FILE
                      Protected-base persistent row policy
  --row-evidence-candidate-policy FILE
                      Candidate row policy, required to equal the protected policy

"check" rejects generated drift. "update" writes only the two deterministic
generated dashboard files. "validate" validates claims without writing files.
"claims" prints the canonical built-in claim records.
EOF
}

die() {
  printf 'parity dashboard: %s\n' "$1" >&2
  exit 2
}

trim() {
  local value="$1"
  value="${value#"${value%%[![:space:]]*}"}"
  value="${value%"${value##*[![:space:]]}"}"
  printf '%s' "${value}"
}

row_id_at() {
  local index="$1"
  if ((index <= 94)); then
    printf '%02d' "${index}"
  else
    printf 'S%02d' "$((index - 94))"
  fi
}

row_rank() {
  if [[ "$1" == S* ]]; then
    printf '%d' "$((94 + 10#${1#S}))"
  else
    printf '%d' "$((10#$1))"
  fi
}

valid_hash() {
  [[ "$1" =~ ^[0-9a-f]{40}$ ]]
}

valid_landed_evidence_commit() {
  local commit="$1"
  git -C "${REPO_ROOT}" cat-file -e "${commit}^{commit}" 2>/dev/null &&
    git -C "${REPO_ROOT}" merge-base --is-ancestor "${commit}" "${STATUS_COMMIT}" &&
    git -C "${REPO_ROOT}" merge-base --is-ancestor "${STATUS_COMMIT}" HEAD
}

valid_status() {
  [[ "$1" == Complete || "$1" == Partial || "$1" == Missing || "$1" == N/A ]]
}

valid_class() {
  [[ "$1" == Exact || "$1" == AMD-equivalent || "$1" == N/A ]]
}

require_readable_bounded() {
  local path="$1"
  local label="$2"
  local size
  [[ -f "${path}" && -r "${path}" ]] || die "${label} is not readable: ${path}"
  size="$(wc -c <"${path}")"
  ((size > 0 && size <= MAX_INPUT_BYTES)) || die "${label} has an invalid size"
}

assert_plain_field() {
  local value="$1"
  local label="$2"
  ((${#value} >= 1 && ${#value} <= MAX_FIELD_LENGTH)) ||
    die "${label} is empty or exceeds the field bound"
  [[ "${value}" != *$'\r'* && "${value}" != *$'\n'* && "${value}" != *$'\t'* ]] ||
    die "${label} contains a control character"
}

parse_status() {
  local path="$1"
  local line=""
  local f1=""
  local f2=""
  local f3=""
  local extra=""
  local line_number=0
  local row_index=0
  local expected=""

  require_readable_bounded "${path}" 'status file'
  while IFS= read -r line || [[ -n "${line}" ]]; do
    ((line_number += 1))
    [[ -n "${line}" && "${line}" != *$'\r'* ]] ||
      die "blank or carriage-return status line at ${line_number}"
    IFS=$'\t' read -r f1 f2 f3 extra <<<"${line}"
    [[ -z "${extra}" ]] || die "status line ${line_number} has extra fields"
    case "${line_number}" in
      1)
        [[ "${f1}" == schema_version && "${f2}" == 1 && -z "${f3}" ]] ||
          die 'status schema_version must be exactly 1'
        ;;
      2)
        [[ "${f1}" == cuda_oxide_commit && -z "${f3}" ]] ||
          die 'status cuda_oxide_commit is malformed'
        valid_hash "${f2}" || die 'status cuda_oxide_commit is malformed'
        CUDA_COMMIT="${f2}"
        ;;
      3)
        [[ "${f1}" == cuda_oxide_date && "${f2}" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ && -z "${f3}" ]] ||
          die 'status cuda_oxide_date is malformed'
        ;;
      4)
        [[ "${f1}" == fe2o3_commit && -z "${f3}" ]] ||
          die 'status fe2o3_commit is malformed'
        valid_hash "${f2}" || die 'status fe2o3_commit is malformed'
        STATUS_COMMIT="${f2}"
        ;;
      5)
        [[ "${f1}" == kind && "${f2}" == id && "${f3}" == status ]] ||
          die 'status row header is malformed'
        ;;
      *)
        ((row_index += 1))
        ((row_index <= 109)) || die "status contains an unknown row: ${f2}"
        expected="$(row_id_at "${row_index}")"
        [[ "${f2}" == "${expected}" ]] ||
          die "status rows are duplicate, missing, or out of order: expected ${expected}, found ${f2}"
        if ((row_index <= 94)); then
          [[ "${f1}" == normative ]] || die "invalid row kind for ${f2}: ${f1}"
        else
          [[ "${f1}" == supplemental ]] || die "invalid row kind for ${f2}: ${f1}"
        fi
        valid_status "${f3}" || die "invalid status for ${f2}: ${f3}"
        ROW_KIND["${f2}"]="${f1}"
        ROW_STATUS["${f2}"]="${f3}"
        ;;
    esac
  done <"${path}"
  ((line_number == 114 && row_index == 109)) ||
    die "status must contain exactly 109 canonical rows; found ${row_index}"
}

parse_matrix() {
  local path="$1"
  local line=""
  local id=""
  local feature=""
  local class=""
  local status=""
  local acceptance=""
  local gates=""
  local expected=""
  local row_index=0
  local -a cells=()

  require_readable_bounded "${path}" 'matrix file'
  while IFS= read -r line || [[ -n "${line}" ]]; do
    [[ "${line}" == '| '* ]] || continue
    IFS='|' read -r -a cells <<<"${line}"
    ((${#cells[@]} >= 4)) || continue
    id="$(trim "${cells[1]:-}")"
    [[ "${id}" =~ ^([0-9]{2}|S[0-9]{2})$ ]] || continue
    ((row_index += 1))
    expected="$(row_id_at "${row_index}")"
    [[ "${id}" == "${expected}" ]] ||
      die "matrix rows are duplicate, missing, or out of order: expected ${expected}, found ${id}"

    if [[ "${id}" == S* ]]; then
      ((${#cells[@]} == 7)) || die "supplemental matrix row ${id} must have six columns"
      feature="$(trim "${cells[2]}")"
      class="$(trim "${cells[3]}")"
      status="$(trim "${cells[4]}")"
      acceptance="$(trim "${cells[5]}")"
      gates="$(trim "${cells[6]}")"
    else
      ((${#cells[@]} == 8)) || die "normative matrix row ${id} must have seven columns"
      feature="$(trim "${cells[2]}")"
      class="$(trim "${cells[4]}")"
      status="$(trim "${cells[5]}")"
      acceptance="$(trim "${cells[6]}")"
      gates="$(trim "${cells[7]}")"
    fi

    assert_plain_field "${feature}" "matrix feature ${id}"
    assert_plain_field "${acceptance}" "matrix acceptance target ${id}"
    valid_class "${class}" || die "invalid parity class for ${id}: ${class}"
    valid_status "${status}" || die "invalid matrix status for ${id}: ${status}"
    [[ "${status}" == "${ROW_STATUS[${id}]}" ]] ||
      die "matrix/status transition mismatch for ${id}: ${status} versus ${ROW_STATUS[${id}]}"
    gates="${gates// /}"
    [[ "${gates}" =~ ^G[0-8](,G[0-8])*$ ]] || die "invalid gate list for ${id}: ${gates}"
    if [[ "${status}" == N/A && "${class}" != N/A ]]; then
      die "impossible N/A equivalence for ${id}: class is ${class}"
    fi
    if [[ "${class}" == N/A && "${status}" != N/A ]]; then
      die "N/A class ${id} must have N/A status"
    fi
    ROW_FEATURE["${id}"]="${feature}"
    ROW_CLASS["${id}"]="${class}"
    ROW_GATES["${id}"]="${gates}"
    ROW_ACCEPTANCE["${id}"]="${acceptance}"
  done <"${path}"
  ((row_index == 109)) || die "matrix must contain exactly 109 canonical rows; found ${row_index}"
}

emit_default_claims() {
  printf 'schema_version\t1\n'
  printf 'fe2o3_commit\t%s\n' "${STATUS_COMMIT}"
  cat <<'EOF'
evidence	abi-pack	crates/fe2o3-core/src/memory.rs,crates/fe2o3-core/tests/device_buffer_view_ui.rs,crates/fe2o3-host/src/generated_argument_plan.rs,crates/fe2o3-host/src/argument_alias.rs,crates/fe2o3-host/src/artifact_binding.rs,crates/fe2o3-host/src/generated_alpha_zeta_cov6.rs,crates/fe2o3-host/src/hsa_executable_lifecycle.rs,crates/fe2o3-host/tests/generated_spi_ui.rs,crates/fe2o3-host/tests/ui/generated_spi,crates/fe2o3-macros/src/lib.rs,crates/fe2o3-macros/tests/typed_kernel_fixtures.rs,crates/rustc-codegen-fe2o3/src/rust_type_layout_v3.rs,crates/rustc-codegen-fe2o3/src/compiler_descriptor.rs,crates/rustc-codegen-fe2o3/src/semantic_witness.rs,crates/fe2o3-hsa-runtime/tests/gfx942_two_kernel_hardware.rs	cargo test -p fe2o3-core --all-targets --locked@@cargo test -p fe2o3-host --locked@@cargo test -p fe2o3-host --test generated_spi_ui --locked@@cargo test -p fe2o3-macros --locked@@cargo test -p rustc-codegen-fe2o3 --lib --locked@@cargo test -p fe2o3-hsa-runtime --features hardware-test-hooks --test gfx942_two_kernel_hardware --locked	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic,gfx942	rust-nightly-2026-04-03,rocm-7.2	source-unit,negative-adversarial	General V3 source/unit evidence covers named semantic layouts, exact alpha/zeta COV6 descriptors, checked allocation views and mutable splits, retained packing, alias admission, backend witnesses, and generated preparation/dispatch. The observed generated-safe MI300X run uses test witnesses and a fake authenticator. Aggregate ABI coverage, a mechanical Verus split proof, and machine-code refinement remain incomplete.
evidence	artifacts	crates/fe2o3-artifacts/src/container.rs,crates/fe2o3-artifacts/src/bundle.rs,crates/fe2o3-artifacts/src/gfx942_bundle.rs,crates/fe2o3-artifacts/tests/gfx942_bundle.rs	cargo test -p fe2o3-artifacts --locked	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic,gfx942	rust-nightly-2026-04-03	source-unit,negative-adversarial	The canonical profile admits exactly two kernels bound to one shared gfx942 native payload and per-kernel proof records, with substitution and duplicate rejection. It is a bounded profile rather than general compiler-produced bundles, and this evidence does not load or execute either kernel.
evidence	atomics	crates/fe2o3-kernel-ir/src/ir.rs,crates/dialect-amdgcn/src/lowering.rs	cargo test -p fe2o3-kernel-ir --locked@@cargo test -p dialect-amdgcn --locked@@FE2O3_TARGET=gfx1151 scripts/ci-local.sh rocm-compile@@FE2O3_TARGET=gfx942 scripts/ci-local.sh rocm-compile@@FE2O3_TARGET=gfx950 scripts/ci-local.sh rocm-compile	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic,gfx1151,gfx942,gfx950	rust-nightly-2026-04-03,rocm-7.2	source-unit,negative-adversarial,compile-code-object	Integer atomic lowering covers bounded operations and scopes; float atomics, standard-library integration, coherent-allocation admission, and hardware memory-order execution remain incomplete.
evidence	cfg	crates/fe2o3-kernel-analysis/src/control_flow.rs	cargo test -p fe2o3-kernel-analysis --locked	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic	rust-nightly-2026-04-03	source-unit,negative-adversarial	Deterministic dominators, frontiers, and loop analysis exist, but arbitrary reducible MIR is not yet translated and executed end to end.
evidence	clean	crates/cargo-fe2o3/src/clean.rs	cargo test -p cargo-fe2o3 --test clean_cli --locked	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic	rust-nightly-2026-04-03	source-unit,negative-adversarial	Cleaning is deliberately restricted to project-local fe2o3 output; complete external-project orchestration and baseline behavior differences remain documented gaps.
evidence	cooperative	crates/fe2o3-core/src/cooperative.rs,crates/fe2o3-host/src/cooperative_launch.rs,crates/fe2o3-kernel-descriptor/src/lib.rs	cargo test -p fe2o3-core --locked@@cargo test -p fe2o3-host --locked@@cargo test -p fe2o3-kernel-descriptor --locked	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic	rust-nightly-2026-04-03	source-unit,negative-adversarial	Observed cooperative authority retains the exact context, loaded function, and stream; safe admission is conservatively limited to one workgroup until per-function occupancy is observed, and raw argument/semantic obligations remain unsafe.
evidence	constants	crates/dialect-mir/src/semantic_constant.rs	cargo test -p dialect-mir --locked	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic	rust-nightly-2026-04-03	source-unit,negative-adversarial	Bounded semantic constants and data relocations are modeled; rustc promotions, emitted device globals/statics, supported symbol resolution, and GPU execution are not integrated.
evidence	device-copy	crates/fe2o3-core/src/device_copy.rs	cargo test -p fe2o3-core --locked	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic	rust-nightly-2026-04-03	source-unit,negative-adversarial	Host-transfer bit validity is modeled for approved types; general manifest-derived device interpretation and compiler-enforced ABI identity remain incomplete.
evidence	device-ffi	crates/fe2o3-device/src/ffi.rs,crates/fe2o3-compiler-ffi/src/module_handoff_v2.rs,crates/rustc-codegen-fe2o3/src/device_ffi.rs,crates/rustc-codegen-fe2o3/src/worker_v2_producer.rs,crates/fe2o3-hsaco-finalize/src/request_construction.rs,crates/rustc-codegen-fe2o3/tests/kernel_ir_codegen.rs,crates/cargo-fe2o3/src/binding_wrapper.rs	cargo test -p fe2o3-device --locked@@cargo test -p rustc-codegen-fe2o3 --locked@@cargo test -p rustc-codegen-fe2o3 --test kernel_ir_codegen worker_v2_real_source_publishes_inspected_gfx942_hsaco -- --ignored --exact@@cargo test -p rustc-codegen-fe2o3 --test kernel_ir_codegen worker_v2_real_source_links_an_external_bitcode_provider -- --ignored --exact	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic,gfx942	rust-nightly-2026-04-03,rocm-7.2,llvm-api-mechanics	source-unit,negative-adversarial	The bounded gfx942 Worker V2 slice consumes exact compiler symbol roles, closes an external LLVM provider import, and canonically finalizes descriptor-bearing COV6 before durable publication. Previous MI300X compile, inspection, and publication evidence is not restamped to ceb0e46; compiler origin, Verus refinement, packaged device libraries, ArtifactContainer construction, HSA load, and launch remain outside this source/unit claim.
evidence	differential	crates/fe2o3-differential/src/generate.rs,crates/fe2o3-differential/src/eval.rs,scripts/differential/harness.py,scripts/differential/reference.cpp	cargo test -p fe2o3-differential --locked@@scripts/tests/differential-conformance.sh	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic	rust-nightly-2026-04-03,rocm-7.2	source-unit,negative-adversarial	Deterministic model generation/reduction and a bounded fill/vecadd/affine HIP-or-CPU oracle harness exist; the recorded claim is toolchain-safe evidence only and does not assert broad MIR fuzzing, hardware coverage, or safety proof.
evidence	fence	crates/fe2o3-device/src/sync.rs,crates/dialect-amdgcn/src/lowering.rs	cargo test -p fe2o3-device --locked@@cargo test -p dialect-amdgcn --locked@@FE2O3_TARGET=gfx1151 scripts/ci-local.sh rocm-compile@@FE2O3_TARGET=gfx942 scripts/ci-local.sh rocm-compile	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic,gfx1151,gfx942	rust-nightly-2026-04-03,rocm-7.2	source-unit,negative-adversarial,compile-code-object	Scoped fence lowering is bounded to modeled AMD semantics; CUDA proxy operations and hardware ordering validation are not provided.
evidence	generics	crates/rustc-codegen-fe2o3/src/collector.rs	cargo test -p rustc-codegen-fe2o3 --locked	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic	rust-nightly-2026-04-03	source-unit,negative-adversarial	Concrete generic and const-generic helper instances are collected deterministically across crates; generic registered roots and general final application bundles remain unsupported.
evidence	inspect	crates/cargo-fe2o3/src/inspect.rs,crates/fe2o3-hsaco/src/elf_inspection.rs	cargo test -p cargo-fe2o3 --locked@@cargo test -p fe2o3-hsaco --locked	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic	rust-nightly-2026-04-03	source-unit,negative-adversarial	Bounded IR, artifact, and HSACO inspection exists for supported payloads; a complete stage-by-stage general compiler pipeline is not available.
evidence	layout	crates/rustc-codegen-fe2o3/src/rust_type_layout_general.rs,crates/dialect-mir/src/semantic_type.rs	cargo test -p rustc-codegen-fe2o3 --locked@@cargo test -p dialect-mir --locked	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic	rust-nightly-2026-04-03	source-unit,negative-adversarial	Layout records cover bounded scalar, aggregate, direct-enum, and niche-enum shapes; host packing, constants, general lowering, DSTs, unions, and recursive graphs are not fully connected.
evidence	lds	crates/fe2o3-device/src/lds.rs,crates/fe2o3-kernel-ir/src/ir.rs,crates/dialect-amdgcn/src/lowering.rs,examples/verus_vecadd/verus/wave_lds.rs	cargo test -p fe2o3-device --locked@@cargo test -p dialect-amdgcn --locked@@verus --crate-type lib --triggers-mode silent examples/verus_vecadd/verus/wave_lds.rs@@FE2O3_TARGET=gfx1151 scripts/ci-local.sh rocm-compile@@FE2O3_TARGET=gfx942 scripts/ci-local.sh rocm-compile@@FE2O3_TARGET=gfx950 scripts/ci-local.sh rocm-compile	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic,gfx1151,gfx942,gfx950	rust-nightly-2026-04-03,rocm-7.2,verus-0.2026.08.02.b677dd5	source-unit,negative-adversarial,compile-code-object,verus-proof	Typed dynamic LDS and modeled static LDS have bounded source and proof contracts; launch-byte admission, cooperative ownership phases, backend refinement, and GPU semantic execution are absent.
evidence	link-plan	crates/fe2o3-hsaco-finalize/src/link_plan.rs,crates/fe2o3-hsaco-finalize/src/first_build_worker_v2.rs,crates/fe2o3-hsaco-finalize/src/worker_executor.rs,crates/fe2o3-hsaco-finalize/src/worker_v2_hsaco_admission.rs,crates/fe2o3-hsaco-finalize/src/worker_v2_hsaco_publication.rs,crates/fe2o3-artifact-transaction/src/attempt_scoped_hsaco_publication.rs,crates/fe2o3-artifact-transaction/src/durable_link_publication.rs,crates/fe2o3-artifact-transaction/src/durable_published_claim.rs,crates/cargo-fe2o3/src/worker_v2_restart.rs,crates/cargo-fe2o3/tests/worker_v2_vertical_slice.rs,tools/fe2o3-llvm-link-worker/src/WorkerPipeline.cpp,tools/fe2o3-llvm-link-worker/tests/PipelineTests.cpp,crates/rustc-codegen-fe2o3/tests/kernel_ir_codegen.rs,crates/fe2o3-worker-v2-bundle/src/compiler_transaction.rs,crates/fe2o3-worker-v2-bundle/src/inputs.rs,crates/cargo-fe2o3/src/worker_v2.rs	cargo test -p fe2o3-hsaco-finalize --locked@@cargo test -p fe2o3-artifact-transaction --locked@@cargo test -p cargo-fe2o3 --test worker_v2_vertical_slice --locked@@cargo test -p cargo-fe2o3 --test worker_v2_vertical_slice --features worker-v2-fault-injection-test-only --locked@@cargo test -p rustc-codegen-fe2o3 --test kernel_ir_codegen worker_v2_general_v3_alpha_zeta_build_links_and_validate_backend_witnesses -- --ignored --exact@@scripts/tests/run-parity-snapshot.sh	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic,gfx942	rust-nightly-2026-04-03,rocm-7.2,llvm-api-mechanics	source-unit,negative-adversarial	Direct LLVM and LLD Worker V2 linking publishes the genuine alpha/zeta COV6 payload through the durable currentness path. Required Cargo mode consumes a measured canonical input capsule, binds it before publication, reconstructs the exact envelope across crashes, and cleans attempt-scoped residue. Compiler transaction and envelope values are inert; Cargo does not authenticate proof, compiler, or effect claims, and application handoff remains absent.
evidence	multi-kernel	crates/cargo-fe2o3/src/worker_v2_artifact_container.rs,crates/fe2o3-artifacts/src/gfx942_bundle.rs,crates/fe2o3-verifier/src/multi_kernel_proof.rs,crates/fe2o3-verifier/src/persistent_freshness.rs,crates/fe2o3-worker-v2-bundle/src/model.rs,crates/fe2o3-worker-v2-bundle/src/codec.rs,crates/rustc-codegen-fe2o3/src/mir_import.rs,crates/rustc-codegen-fe2o3/src/kernel_ir_lowering.rs,crates/rustc-codegen-fe2o3/tests/kernel_ir_codegen.rs,crates/fe2o3-host/src/worker_v2_bundle_admission.rs,crates/fe2o3-host/src/hsa_executable_lifecycle.rs,crates/fe2o3-host/src/generated_alpha_zeta_cov6.rs,crates/fe2o3-hsa-runtime/src/lifecycle.rs,crates/fe2o3-hsa-runtime/src/dispatch.rs,crates/fe2o3-hsa-runtime/tests/gfx942_two_kernel_hardware.rs,crates/fe2o3-host/src/recovered_worker_v2_admission.rs,crates/fe2o3-host/tests/recovered_worker_v2_admission_ui.rs,crates/fe2o3-verifier/src/proof_capsule.rs,crates/fe2o3-kernel-analysis/src/machine_effect.rs	cargo test --locked -p cargo-fe2o3 worker_v2_artifact_container::tests@@cargo test -p fe2o3-artifacts --locked@@cargo test -p fe2o3-worker-v2-bundle --locked@@cargo test -p fe2o3-verifier --locked@@cargo test -p rustc-codegen-fe2o3 --locked@@cargo test -p fe2o3-host --locked@@cargo test -p fe2o3-hsa-runtime --locked@@cargo test -p rustc-codegen-fe2o3 --test kernel_ir_codegen worker_v2_general_v3_alpha_zeta_build_links_and_validate_backend_witnesses -- --ignored --exact@@cargo test -p fe2o3-hsa-runtime --features hardware-test-hooks --test gfx942_two_kernel_hardware gfx942_cov6_alpha_then_zeta_generated_safe_spi_with_fake_authenticator -- --ignored --exact	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic,gfx942	rust-nightly-2026-04-03,rocm-7.2,llvm-api-mechanics	source-unit,negative-adversarial	Durable publication, exact envelope recovery, inert recovered host admission, authenticated HSA lifecycle, generated alpha/zeta dispatch, proof capsules, and bounded machine-effect records exist. Recovered descriptors grant no bytes, authentication, load, launch, or prerequisite authority; machine effects are caller-supplied mechanics, the hardware run uses a fake authenticator, and production application handoff remains absent.
evidence	peer	crates/fe2o3-core/src/peer_access.rs,crates/fe2o3-core/src/memory.rs	cargo test -p fe2o3-core --locked	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic	rust-nightly-2026-04-03	source-unit,negative-adversarial	Directional peer observation and enablement retain exact contexts and linear cleanup ownership, including ambiguous-failure retention; they grant no pointer validity, copy, coherence, alias, race, completion, topology, or virtual-memory authority.
evidence	pinned	crates/fe2o3-core/src/pinned_memory.rs,crates/fe2o3-core/src/event.rs	cargo test -p fe2o3-core --locked	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic	rust-nightly-2026-04-03	source-unit,negative-adversarial	Pinned memory and event lifetime APIs are covered at source/unit level; broad hardware ordering, cancellation, and multi-stream execution evidence is not recorded here.
evidence	registration	crates/fe2o3-macros/src/lib.rs,crates/fe2o3-macros/tests/typed_kernel_fixtures.rs,crates/rustc-codegen-fe2o3/src/collector.rs,crates/rustc-codegen-fe2o3/src/rust_type_layout_v3.rs,crates/rustc-codegen-fe2o3/src/semantic_witness.rs,crates/rustc-codegen-fe2o3/src/host_object.rs,crates/rustc-codegen-fe2o3/tests/general_two_kernel_import.rs,crates/rustc-codegen-fe2o3/tests/kernel_ir_codegen.rs	cargo test -p fe2o3-macros --locked@@cargo test -p rustc-codegen-fe2o3 --lib --locked@@cargo test -p rustc-codegen-fe2o3 --test general_two_kernel_import --locked@@cargo test -p rustc-codegen-fe2o3 --test kernel_ir_codegen worker_v2_general_v3_alpha_zeta_build_links_and_validate_backend_witnesses -- --ignored --exact	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic,gfx942	rust-nightly-2026-04-03,rocm-7.2,llvm-api-mechanics	source-unit,negative-adversarial	Versioned V3 registration is authenticated against rustc semantic scalar and trusted slice identities; exact alpha/zeta source roles produce named ABI identities, generated host adapters, backend witnesses, and one linked COV6 payload. Generic roots, broad Rust types/control flow, a production prerequisite authenticator, Verus evidence promotion, and compiler-to-machine refinement remain incomplete.
evidence	sanitize-plan	crates/cargo-fe2o3/src/tool_commands.rs,crates/cargo-fe2o3/tests/tool_execution_cli.rs	cargo test -p cargo-fe2o3 --locked	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic	rust-nightly-2026-04-03,rocm-7.2	source-unit,negative-adversarial	Plan mode and bounded descriptor-pinned ROCgdb execution exist with timeout, output, identity, environment, and process supervision; precise-memory availability is fail-closed and no race, API, initialization, synchronization, source-metadata, or safety proof is claimed.
evidence	scalar-ir	crates/fe2o3-kernel-ir/src/ir.rs,crates/fe2o3-kernel-ir/src/verify.rs	cargo test -p fe2o3-kernel-ir --locked	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic	rust-nightly-2026-04-03	source-unit,negative-adversarial	The target-neutral IR models a bounded arithmetic and cast subset; the complete Rust operation matrix, pointer provenance policy, and CPU/GPU differential coverage are missing.
evidence	single-source	crates/cargo-fe2o3/src/binding_wrapper.rs,crates/cargo-fe2o3/src/worker_v2.rs,crates/rustc-codegen-fe2o3/src/frontend_record_bridge.rs,crates/rustc-codegen-fe2o3/src/mir_import.rs,crates/rustc-codegen-fe2o3/src/kernel_ir_lowering.rs,crates/rustc-codegen-fe2o3/src/worker_v2_producer.rs,crates/rustc-codegen-fe2o3/src/rust_type_layout_v3.rs,crates/rustc-codegen-fe2o3/src/semantic_witness.rs,crates/rustc-codegen-fe2o3/src/host_object.rs,crates/fe2o3-hsaco-finalize/src/worker_v2_hsaco_admission.rs,crates/fe2o3-hsaco-finalize/src/worker_v2_hsaco_publication.rs,crates/fe2o3-artifact-transaction/src/attempt_scoped_hsaco_publication.rs,crates/fe2o3-artifact-transaction/src/durable_published_claim.rs,crates/fe2o3-worker-v2-bundle/src/model.rs,crates/fe2o3-host/src/worker_v2_bundle_admission.rs,crates/fe2o3-host/src/hsa_executable_lifecycle.rs,crates/fe2o3-host/src/generated_alpha_zeta_cov6.rs,crates/rustc-codegen-fe2o3/tests/kernel_ir_codegen.rs,crates/fe2o3-hsa-runtime/tests/gfx942_two_kernel_hardware.rs	cargo test -p cargo-fe2o3 --locked@@cargo test -p fe2o3-worker-v2-bundle --locked@@cargo test -p rustc-codegen-fe2o3 --locked@@cargo test -p fe2o3-macros --locked@@cargo test -p fe2o3-host --locked@@cargo test -p rustc-codegen-fe2o3 --test kernel_ir_codegen worker_v2_general_v3_alpha_zeta_build_links_and_validate_backend_witnesses -- --ignored --exact@@cargo test -p fe2o3-hsa-runtime --features hardware-test-hooks --test gfx942_two_kernel_hardware --locked	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic,gfx942	rust-nightly-2026-04-03,rocm-7.2,llvm-api-mechanics	source-unit,negative-adversarial	One genuine Rust alpha/zeta source fixture lowers through typed IR, emits identity-bound semantic witnesses, links directly with Worker V2, and durably publishes a COV6 payload and required canonical load envelope. Recovered host admission is inert, application handoff is absent, and no production authenticator promotes compiler, Verus, proof, or effect evidence.
evidence	switch	crates/fe2o3-kernel-ir/src/ir.rs,crates/fe2o3-kernel-ir/src/wire.rs	cargo test -p fe2o3-kernel-ir --locked@@cargo test -p fe2o3-kernel-analysis --locked	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic	rust-nightly-2026-04-03	source-unit,negative-adversarial	Canonical integer switches are represented, verified, and analyzed; AMDGPU dialect lowering intentionally fails closed, so executable match parity is not established.
evidence	target	crates/fe2o3-amd-target/src/capabilities.rs,crates/fe2o3-core/src/device_target.rs	cargo test -p fe2o3-amd-target --locked@@cargo test -p fe2o3-core --locked	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic	rust-nightly-2026-04-03	source-unit,negative-adversarial	Target parsing, override, and capability records are bounded; every compiler/runtime payload path is not yet proven to consume the observed target consistently.
evidence	typed-async	crates/fe2o3-completion/src/lifecycle.rs,crates/fe2o3-core/src/operation.rs,crates/fe2o3-hsa-runtime/src/dispatch.rs,crates/fe2o3-hsa-runtime/src/lifecycle.rs,crates/fe2o3-hsa-runtime/tests/ui/authority/executable_cannot_unload_while_kernel_set_is_live.rs,crates/fe2o3-hsa-runtime/tests/ui/authority/kernel_set_cannot_clone.rs	cargo test -p fe2o3-completion --locked@@cargo test -p fe2o3-core --locked@@cargo test -p fe2o3-hsa-runtime --locked	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic,gfx942	rust-nightly-2026-04-03,rocm-7.2	source-unit,negative-adversarial	The completion model is fail-closed with exactly-once reclamation, while the HSA lifecycle resolves an exact non-clone kernel set, retains the executable borrow, and rejects duplicate symbols, native aliases, and cross-kernel queue or kernarg identity. General generated async composition, generic second-kernel dispatch, cancellation proof, and live local or remote hardware execution are not established.
evidence	typed-launch	crates/fe2o3-core/src/memory.rs,crates/fe2o3-host/src/generated_vecadd.rs,crates/fe2o3-host/src/prepared_launch.rs,crates/fe2o3-host/src/generated_argument_plan.rs,crates/fe2o3-host/src/argument_alias.rs,crates/fe2o3-host/src/artifact_binding.rs,crates/fe2o3-host/src/worker_v2_bundle_admission.rs,crates/fe2o3-host/src/hsa_executable_lifecycle.rs,crates/fe2o3-host/src/generated_alpha_zeta_cov6.rs,crates/fe2o3-host/tests/generated_spi_ui.rs,crates/fe2o3-macros/src/lib.rs,crates/rustc-codegen-fe2o3/src/rust_type_layout_v3.rs,crates/rustc-codegen-fe2o3/src/compiler_descriptor.rs,crates/rustc-codegen-fe2o3/src/semantic_witness.rs,crates/reserved-fe2o3-symbols/src/lib.rs,crates/fe2o3-hsa-runtime/src/lifecycle.rs,crates/fe2o3-hsa-runtime/src/dispatch.rs,crates/fe2o3-hsa-runtime/tests/gfx942_two_kernel_hardware.rs	cargo test -p fe2o3-core --all-targets --locked@@cargo test -p fe2o3-host --locked@@cargo test -p fe2o3-host --test generated_spi_ui --locked@@cargo test -p fe2o3-macros --locked@@cargo test -p rustc-codegen-fe2o3 --lib --locked@@cargo test -p fe2o3-hsa-runtime --test gfx942_two_kernel_hardware --locked@@cargo test -p fe2o3-hsa-runtime --features hardware-test-hooks --test gfx942_two_kernel_hardware gfx942_cov6_alpha_then_zeta_generated_safe_spi_with_fake_authenticator -- --ignored --exact	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic,gfx942	rust-nightly-2026-04-03,rocm-7.2	source-unit,negative-adversarial	Production finalized-bundle admission/currentness, recovered inert admission, authenticated HSA lifecycle, generated alpha/zeta preparation/safe dispatch, and reviewed runtime adapters exist. The digest-pinned generated-safe MI300X test uses test semantic witnesses and a fake authenticator. Application handoff and production compiler, Verus, and effect authentication remain absent.
evidence	ui-safety	crates/fe2o3-core/tests/device_buffer_view_ui.rs,crates/fe2o3-core/tests/ui/device_buffer_view,crates/fe2o3-device/tests/ui,crates/fe2o3-host/tests/generated_spi_ui.rs,crates/fe2o3-host/tests/ui,crates/fe2o3-host/tests/ui/authenticated_artifact,crates/fe2o3-host/tests/ui/generated_spi,crates/fe2o3-macros/tests/typed_kernel_fixtures.rs,crates/fe2o3-hsa-runtime/tests/gfx942_two_kernel_hardware.rs,crates/fe2o3-host/tests/recovered_worker_v2_admission_ui.rs	cargo test -p fe2o3-core --all-targets --locked@@cargo test -p fe2o3-device --locked@@cargo test -p fe2o3-host --locked@@cargo test -p fe2o3-host --test generated_spi_ui --locked@@cargo test -p fe2o3-macros --test typed_kernel_fixtures --locked@@cargo test -p fe2o3-hsa-runtime --features hardware-test-hooks --test gfx942_two_kernel_hardware --locked	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic,gfx942	rust-nightly-2026-04-03,rocm-7.2	source-unit,negative-adversarial	Compile-fail coverage includes checked-view privacy, parent and split exclusive borrowing, generated lifetime retention, mutable-alias exclusion, non-clone launch values, private authority fields, inert recovered admission, semantic-witness privacy, and unsafe prerequisite boundaries. Production authentication and exhaustive future compiler/runtime/barrier/FFI coverage remain absent.
evidence	views-proof	crates/fe2o3-device/src/thread.rs,crates/fe2o3-host/src/argument_alias.rs,examples/verus_vecadd/verus/vecadd.rs,crates/fe2o3-core/src/memory.rs,crates/fe2o3-core/tests/device_buffer_view_ui.rs	cargo test -p fe2o3-device --locked@@cargo test -p fe2o3-host --locked@@verus --crate-type lib --triggers-mode silent examples/verus_vecadd/verus/vecadd.rs	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic	rust-nightly-2026-04-03,verus-0.2026.08.02.b677dd5	source-unit,negative-adversarial,verus-proof	Branded witnesses, alias admission, checked allocation-relative views, Rust-borrowed mutable splits, and source-level bounds/race proofs cover bounded profiles. A mechanical Verus proof of split_at_mut, general launch mappings, authenticated machine effects, and machine-code refinement are not established.
evidence	wave	crates/fe2o3-device/src/wave.rs,crates/fe2o3-kernel-ir/src/ir.rs,crates/dialect-amdgcn/src/lowering.rs,examples/verus_vecadd/verus/wave_lds.rs	cargo test -p fe2o3-device --locked@@cargo test -p fe2o3-kernel-ir --locked@@cargo test -p dialect-amdgcn --locked@@verus --crate-type lib --triggers-mode silent examples/verus_vecadd/verus/wave_lds.rs@@FE2O3_TARGET=gfx1151 scripts/ci-local.sh rocm-compile@@FE2O3_TARGET=gfx942 scripts/ci-local.sh rocm-compile@@FE2O3_TARGET=gfx950 scripts/ci-local.sh rocm-compile	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic,gfx1151,gfx942,gfx950	rust-nightly-2026-04-03,rocm-7.2,verus-0.2026.08.02.b677dd5	source-unit,negative-adversarial,compile-code-object,verus-proof	Wave32/wave64 lane, vote, ballot, and bounded shuffle operations exist; partial waves, dynamic tiles, broad types, reductions/scans, hardware execution, and backend refinement remain gaps.
evidence	codegen	crates/rustc-codegen-fe2o3/src/amdgpu_llvm.rs,crates/rustc-codegen-fe2o3/src/kernel_ir_codegen.rs,crates/rustc-codegen-fe2o3/src/kernel_ir_lowering.rs,crates/rustc-codegen-fe2o3/src/mir_import.rs,crates/rustc-codegen-fe2o3/tests/kernel_ir_codegen.rs,crates/fe2o3-hsa-runtime/tests/gfx942_two_kernel_hardware.rs	cargo test -p rustc-codegen-fe2o3 --locked@@cargo test -p rustc-codegen-fe2o3 --test kernel_ir_codegen worker_v2_general_v3_alpha_zeta_build_links_and_validate_backend_witnesses -- --ignored --exact@@FE2O3_TARGET=gfx1151 scripts/ci-local.sh rocm-compile@@FE2O3_TARGET=gfx942 scripts/ci-local.sh rocm-compile@@cargo test -p fe2o3-hsa-runtime --features hardware-test-hooks --test gfx942_two_kernel_hardware --locked	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic,gfx1151,gfx942	rust-nightly-2026-04-03,rocm-7.2	source-unit,negative-adversarial,compile-code-object	AMDGPU lowering carries the bounded genuine alpha/zeta Rust slice through direct Worker V2 COV6 emission with exact 296/312-byte metadata. Separately observed raw and generated-safe MI300X runs passed five boundary lengths, but are not archived dashboard hardware strength; the safe run uses a fake authenticator. Broad MIR/kernel IR coverage, production authentication, other targets, and machine-code refinement remain incomplete.
evidence	managed-memory	crates/fe2o3-core/src/managed_memory.rs,crates/fe2o3-core/src/memory_topology.rs,crates/fe2o3-core/tests/managed_memory_hardware.rs	cargo test -p fe2o3-core --locked	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic,gfx942	rust-nightly-2026-04-03,rocm-7.2	source-unit,negative-adversarial	Managed allocation, advice, prefetch, location queries, and capability records are bounded and fail closed. Safe host-reference capture, general coherent CPU/GPU access, in-flight launch retention, topology breadth, and archived hardware strength remain incomplete.
evidence	half-math	crates/fe2o3-device/src/half.rs,crates/fe2o3-device/src/math.rs,crates/fe2o3-device/tests/half_math_api.rs,crates/dialect-amdgcn/src/device_math.rs,crates/dialect-amdgcn/tests/gfx942_math.rs,crates/rustc-codegen-fe2o3/tests/half_math_compiler.rs	cargo test -p fe2o3-device --locked@@cargo test -p dialect-amdgcn --test gfx942_math --locked@@cargo test -p rustc-codegen-fe2o3 --test half_math_compiler --locked@@FE2O3_TARGET=gfx942 scripts/ci-local.sh rocm-compile	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic,gfx942	rust-nightly-2026-04-03,rocm-7.2	source-unit,negative-adversarial,compile-code-object	Target-gated f16/BF16 conversion, packed BF16x2 FMA, strict divide, and selected OCML calls have exact source, lowering, rejection, and gfx942 code-object tests. The public API, complete type/math/rounding matrix, edge-case GPU execution, and architecture breadth remain incomplete.
evidence	inline-assembly	crates/fe2o3-kernel-ir/src/ir.rs,crates/fe2o3-kernel-ir/src/verify.rs,crates/fe2o3-kernel-ir/tests/inline_assembly.rs,crates/dialect-amdgcn/src/lowering.rs,crates/dialect-amdgcn/tests/gfx942_inline_assembly.rs	cargo test -p fe2o3-kernel-ir --test inline_assembly --locked@@cargo test -p dialect-amdgcn --test gfx942_inline_assembly --locked@@FE2O3_TARGET=gfx942 scripts/ci-local.sh rocm-compile	2fee8b63b77df73b92f4de79caaabc5b623ab867	generic,gfx942	rust-nightly-2026-04-03,rocm-7.2	source-unit,negative-adversarial,compile-code-object	Source-bound AMDGPU inline assembly has bounded operand, option, effect, capability, verification, and gfx942 lowering coverage. It is not a public amdgpu_asm! API, and general rustc operand/clobber semantics and hardware execution remain incomplete.
row	01	Partial	managed-memory
row	02	Partial	layout
row	03	Partial	layout
row	07	Partial	generics
row	08	Partial	layout
row	09	Partial	layout
row	10	Partial	layout
row	12	Partial	abi-pack
row	17	Partial	switch
row	20	Partial	cfg
row	24	Partial	scalar-ir
row	25	Partial	scalar-ir
row	26	Partial	half-math
row	27	Partial	device-ffi
row	28	Partial	device-ffi
row	32	Partial	registration
row	33	Partial	registration
row	35	Partial	multi-kernel
row	36	Partial	single-source
row	37	Partial	codegen
row	38	Partial	codegen
row	39	Partial	link-plan
row	40	Partial	half-math
row	41	Partial	inspect
row	42	Partial	inspect
row	43	Partial	clean
row	44	Partial	sanitize-plan
row	45	Partial	sanitize-plan
row	48	Partial	views-proof
row	49	Partial	views-proof
row	51	Partial	typed-launch
row	53	Partial	atomics
row	54	Partial	atomics
row	55	Partial	atomics
row	57	Partial	lds
row	58	Partial	lds
row	60	Partial	views-proof
row	61	Partial	lds
row	64	Partial	fence
row	65	Partial	wave
row	66	Partial	wave
row	67	Partial	wave
row	70	Partial	wave
row	71	Partial	wave
row	74	Partial	cooperative
row	78	Partial	typed-launch
row	79	Partial	typed-launch
row	80	Partial	typed-launch
row	81	Partial	typed-async
row	87	Partial	inline-assembly
row	S01	Partial	artifacts
row	S02	Partial	link-plan
row	S03	Partial	typed-async
row	S04	Partial	device-copy
row	S05	Partial	pinned
row	S06	Partial	peer
row	S07	Partial	constants
row	S10	Partial	differential
row	S11	Partial	half-math
row	S14	Partial	target
row	S15	Partial	ui-safety
EOF
}

list_contains() {
  local csv="$1"
  local wanted="$2"
  local item
  local -a items=()
  IFS=, read -r -a items <<<"${csv}"
  for item in "${items[@]}"; do
    [[ "${item}" == "${wanted}" ]] && return 0
  done
  return 1
}

validate_paths() {
  local evidence_id="$1"
  local csv="$2"
  local path
  local segment
  local -a paths=()
  local -a segments=()
  declare -A seen=()

  IFS=, read -r -a paths <<<"${csv}"
  ((${#paths[@]} >= 1)) || die "evidence ${evidence_id} has no implementation path"
  for path in "${paths[@]}"; do
    [[ "${path}" =~ ^[A-Za-z0-9._/-]+$ && "${path}" != /* ]] ||
      die "evidence ${evidence_id} has a malformed implementation path: ${path}"
    [[ ! -v "seen[${path}]" ]] || die "evidence ${evidence_id} repeats path ${path}"
    seen["${path}"]=1
    IFS=/ read -r -a segments <<<"${path}"
    for segment in "${segments[@]}"; do
      [[ -n "${segment}" && "${segment}" != . && "${segment}" != .. ]] ||
        die "evidence ${evidence_id} has a traversing implementation path: ${path}"
    done
    [[ -e "${REPO_ROOT}/${path}" ]] ||
      die "evidence ${evidence_id} references a stale implementation path: ${path}"
  done
}

validate_tests() {
  local evidence_id="$1"
  local encoded="$2"
  local command
  local remaining="${encoded}"
  local executable=""
  local count=0
  declare -A seen=()

  while :; do
    if [[ "${remaining}" == *@@* ]]; then
      command="${remaining%%@@*}"
      remaining="${remaining#*@@}"
    else
      command="${remaining}"
      remaining=""
    fi
    ((count += 1))
    assert_plain_field "${command}" "test command for evidence ${evidence_id}"
    [[ "${command}" =~ ^[A-Za-z0-9_./:=+-]+([[:space:]][A-Za-z0-9_./:=+-]+)*$ ]] ||
      die "evidence ${evidence_id} has an unsafe or malformed test command: ${command}"
    case "${command}" in
      cargo\ test\ * | verus\ *) ;;
      scripts/*)
        executable="${command%% *}"
        [[ -x "${REPO_ROOT}/${executable}" ]] ||
          die "evidence ${evidence_id} references a stale test command: ${executable}"
        ;;
      FE2O3_TARGET=gfx*\ scripts/*)
        executable="${command#* }"
        executable="${executable%% *}"
        [[ -x "${REPO_ROOT}/${executable}" ]] ||
          die "evidence ${evidence_id} references a stale test command: ${executable}"
        ;;
      *) die "evidence ${evidence_id} has an unsupported test command: ${command}" ;;
    esac
    [[ ! -v "seen[${command}]" ]] || die "evidence ${evidence_id} repeats a test command"
    seen["${command}"]=1
    [[ -n "${remaining}" ]] || break
  done
  ((count >= 1)) || die "evidence ${evidence_id} has no test command"
}

validate_csv_values() {
  local evidence_id="$1"
  local label="$2"
  local csv="$3"
  shift 3
  local item
  local allowed
  local matched
  local -a items=()
  declare -A seen=()

  IFS=, read -r -a items <<<"${csv}"
  ((${#items[@]} >= 1)) || die "evidence ${evidence_id} has no ${label}"
  for item in "${items[@]}"; do
    matched=false
    for allowed in "$@"; do
      [[ "${item}" == "${allowed}" ]] && matched=true
    done
    [[ "${matched}" == true ]] || die "evidence ${evidence_id} has unknown ${label}: ${item}"
    [[ ! -v "seen[${item}]" ]] || die "evidence ${evidence_id} repeats ${label}: ${item}"
    seen["${item}"]=1
  done
}

validate_evidence_semantics() {
  local id="$1"
  local lanes="${EVIDENCE_LANES[${id}]}"
  local tests="${EVIDENCE_TESTS[${id}]}"
  local toolchains="${EVIDENCE_TOOLCHAINS[${id}]}"
  local strengths="${EVIDENCE_STRENGTHS[${id}]}"
  local has_gpu_lane=false

  list_contains "${lanes}" gfx1151 && has_gpu_lane=true
  list_contains "${lanes}" gfx942 && has_gpu_lane=true
  list_contains "${lanes}" gfx950 && has_gpu_lane=true

  if list_contains "${strengths}" compile-code-object; then
    [[ "${has_gpu_lane}" == true && "${toolchains}" == *rocm-* && "${tests}" == *rocm-compile* ]] ||
      die "evidence ${id} claims compile-code-object without an exact GPU lane, ROCm identity, and compile command"
  fi
  if list_contains "${strengths}" local-hardware; then
    [[ "${has_gpu_lane}" == true && "${tests}" == *hardware-smoke* ]] ||
      die "evidence ${id} claims local-hardware without an exact lane and hardware command"
  fi
  if list_contains "${strengths}" remote-hardware; then
    [[ "${has_gpu_lane}" == true && "${tests}" == *remote-hardware-matrix* ]] ||
      die "evidence ${id} claims remote-hardware without an exact lane and remote command"
  fi
  if list_contains "${strengths}" verus-proof; then
    [[ "${toolchains}" == *verus-* && "${tests}" == *verus* ]] ||
      die "evidence ${id} claims verus-proof without a Verus identity and command"
  fi
  if list_contains "${strengths}" machine-code-refinement; then
    [[ "${tests}" == *refinement* && "${EVIDENCE_PATHS[${id}]}" == *refinement* ]] ||
      die "evidence ${id} claims machine-code-refinement without dedicated evidence"
  fi
}

parse_claims() {
  local path="$1"
  local line=""
  local line_number=0
  local phase=metadata
  local kind=""
  local id=""
  local implementation_paths=""
  local tests=""
  local commit=""
  local lanes=""
  local toolchains=""
  local strengths=""
  local limitations=""
  local status=""
  local evidence_id=""
  local extra=""
  local last_claim_position=0
  local claim_position=0
  local row_index
  local expected=""

  require_readable_bounded "${path}" 'claim file'
  while IFS= read -r line || [[ -n "${line}" ]]; do
    ((line_number += 1))
    [[ -n "${line}" && "${line}" != *$'\r'* ]] ||
      die "blank or carriage-return claim line at ${line_number}"
    kind="${line%%$'\t'*}"
    case "${kind}" in
      schema_version)
        [[ "${line}" == $'schema_version\t1' && "${line_number}" == 1 ]] ||
          die 'claim schema_version must be exactly 1 on line 1'
        ;;
      fe2o3_commit)
        IFS=$'\t' read -r kind commit extra <<<"${line}"
        [[ "${line_number}" == 2 && -z "${extra}" ]] ||
          die 'claim fe2o3_commit is malformed'
        valid_hash "${commit}" || die 'claim fe2o3_commit is malformed'
        CLAIMS_COMMIT="${commit}"
        [[ "${CLAIMS_COMMIT}" == "${STATUS_COMMIT}" ]] ||
          die "claims are newer than their evidence or stale: ${CLAIMS_COMMIT} versus ${STATUS_COMMIT}"
        ;;
      evidence)
        [[ "${phase}" != rows ]] || die 'evidence definitions must precede row claims'
        phase=evidence
        IFS=$'\t' read -r kind id implementation_paths tests commit lanes toolchains strengths limitations extra <<<"${line}"
        [[ -z "${extra}" ]] || die "evidence line ${line_number} has extra fields"
        [[ "${id}" =~ ^[a-z][a-z0-9-]{0,47}$ ]] || die "malformed evidence ID: ${id}"
        [[ ! -v "EVIDENCE_PATHS[${id}]" ]] || die "duplicate evidence ID: ${id}"
        assert_plain_field "${limitations}" "limitations for evidence ${id}"
        ((${#limitations} >= 40)) || die "evidence ${id} has no explicit limitations/trust boundary"
        valid_hash "${commit}" || die "evidence ${id} has a malformed commit"
        valid_landed_evidence_commit "${commit}" ||
          die "claim ${id} evidence commit is not a landed descendant of the status snapshot: ${commit}"
        EVIDENCE_PATHS["${id}"]="${implementation_paths}"
        EVIDENCE_TESTS["${id}"]="${tests}"
        EVIDENCE_COMMIT["${id}"]="${commit}"
        EVIDENCE_LANES["${id}"]="${lanes}"
        EVIDENCE_TOOLCHAINS["${id}"]="${toolchains}"
        EVIDENCE_STRENGTHS["${id}"]="${strengths}"
        EVIDENCE_LIMITATIONS["${id}"]="${limitations}"
        validate_paths "${id}" "${implementation_paths}"
        validate_tests "${id}" "${tests}"
        validate_csv_values "${id}" lane "${lanes}" generic gfx1151 gfx942 gfx950
        validate_csv_values "${id}" toolchain "${toolchains}" \
          rust-nightly-2026-04-03 rocm-7.2 llvm-api-mechanics \
          verus-0.2026.08.02.b677dd5
        validate_csv_values "${id}" strength "${strengths}" \
          source-unit compile-code-object local-hardware remote-hardware \
          negative-adversarial verus-proof machine-code-refinement
        validate_evidence_semantics "${id}"
        ;;
      row)
        phase=rows
        IFS=$'\t' read -r kind id status evidence_id extra <<<"${line}"
        [[ -z "${extra}" ]] || die "claim row line ${line_number} has extra fields"
        [[ -v "ROW_STATUS[${id}]" ]] || die "claim references unknown parity row: ${id}"
        [[ ! -v "CLAIM_STATUS[${id}]" ]] || die "duplicate claim for parity row: ${id}"
        [[ "${status}" == Partial || "${status}" == Complete ]] ||
          die "claim for ${id} has unsupported status: ${status}"
        [[ "${ROW_STATUS[${id}]}" == "${status}" ]] ||
          die "malformed status transition for ${id}: claim ${status}, source ${ROW_STATUS[${id}]}"
        [[ -v "EVIDENCE_PATHS[${evidence_id}]" ]] ||
          die "claim ${id} references unknown evidence: ${evidence_id}"
        claim_position="$(row_rank "${id}")"
        ((claim_position > last_claim_position)) ||
          die "claim rows are duplicate or out of order: ${id}"
        last_claim_position="${claim_position}"
        CLAIM_STATUS["${id}"]="${status}"
        CLAIM_EVIDENCE["${id}"]="${evidence_id}"
        EVIDENCE_USED["${evidence_id}"]=1
        ;;
      *) die "unknown claim record on line ${line_number}: ${kind}" ;;
    esac
  done <"${path}"

  [[ -n "${CLAIMS_COMMIT}" ]] || die 'claim file is missing fe2o3_commit'
}

parse_signed_promotions() {
  local path="$1"
  local line=""
  local line_number=0
  local expected_count=""
  local actual_count=0
  local expected_index=""
  local record=""
  local index=""
  local id=""
  local from_status=""
  local to_status=""
  local source=""
  local target=""
  local lane=""
  local classes=""
  local toolchains=""
  local results=""
  local evidence_set=""
  local extra=""
  local evidence_id=""
  local result=""
  local class=""
  local strengths=""
  local signed_paths=""
  local previous_rank=0
  local current_rank=0
  local previous_class_rank=-1
  local class_rank=0
  local toolchain=""
  local previous_toolchain=""
  local segment=""
  local -a class_values=()
  local -a result_values=()
  local -a toolchain_values=()
  local -a path_segments=()

  require_readable_bounded "${path}" 'signed-promotion ledger'
  while IFS= read -r line || [[ -n "${line}" ]]; do
    ((line_number += 1))
    [[ -n "${line}" && "${line}" != *$'\r'* ]] ||
      die "blank or carriage-return signed-promotion line at ${line_number}"
    case "${line_number}" in
      1)
        [[ "${line}" == $'signed_promotion_projection_schema_version\t1' ]] ||
          die 'signed-promotion ledger schema must be exactly 1'
        ;;
      2)
        IFS=$'\t' read -r record expected_count extra <<<"${line}"
        [[ "${record}" == row_count && "${expected_count}" =~ ^(0|[1-9][0-9]*)$ && -z "${extra}" ]] ||
          die 'signed-promotion ledger row count is malformed'
        ;;
      *)
        IFS=$'\t' read -r record index id from_status to_status source target lane classes toolchains results evidence_set extra <<<"${line}"
        expected_index="$(printf '%04d' "${actual_count}")"
        [[ "${record}" == row && "${index}" == "${expected_index}" && -z "${extra}" ]] ||
          die "signed-promotion ledger row ${actual_count} is malformed"
        [[ "${id}" =~ ^([0-9]{2}|S[0-9]{2})$ && -v "ROW_STATUS[${id}]" ]] ||
          die "signed-promotion ledger has an invalid row: ${id}"
        current_rank="$(row_rank "${id}")"
        ((current_rank > previous_rank)) ||
          die "signed-promotion ledger rows are duplicate or out of order: ${id}"
        [[ ("${from_status}" == Missing && ("${to_status}" == Partial || "${to_status}" == Complete)) ||
          ("${from_status}" == Partial && "${to_status}" == Complete) ]] ||
          die "signed-promotion ledger transition is invalid for ${id}"
        [[ "${ROW_STATUS[${id}]}" == "${to_status}" ]] ||
          die "signed-promotion ledger/status mismatch for ${id}"
        if ! valid_hash "${source}" || ! valid_landed_evidence_commit "${source}"; then
          die "signed-promotion ledger source is invalid for ${id}"
        fi
        [[ "${target}" =~ ^(generic|gfx[0-9a-f]+(:[A-Za-z0-9_+-]+)*)$ &&
          "${lane}" =~ ^[a-z0-9][a-z0-9._-]{0,63}$ &&
          "${toolchains}" =~ ^[a-z][a-z0-9._-]{0,63}(,[a-z][a-z0-9._-]{0,63})*$ &&
          "${evidence_set}" =~ ^[0-9a-f]{64}$ ]] ||
          die "signed-promotion ledger identity is malformed for ${id}"
        [[ ! -v "CLAIM_STATUS[${id}]" ]] ||
          die "signed-promotion ledger overlaps built-in claim for ${id}"

        IFS=, read -r -a class_values <<<"${classes}"
        IFS=, read -r -a result_values <<<"${results}"
        IFS=, read -r -a toolchain_values <<<"${toolchains}"
        (("${#class_values[@]}" > 0 && "${#class_values[@]}" == "${#result_values[@]}")) ||
          die "signed-promotion ledger class/result count mismatch for ${id}"
        previous_class_rank=-1
        previous_toolchain=""
        strengths=""
        signed_paths=""
        for toolchain in "${toolchain_values[@]}"; do
          [[ "${toolchain}" > "${previous_toolchain}" ]] ||
            die "signed-promotion ledger toolchains are duplicate or out of order for ${id}"
          previous_toolchain="${toolchain}"
        done
        for class in "${class_values[@]}"; do
          case "${class}" in
            unit) class_rank=0; list_contains "${strengths}" source-unit || strengths="${strengths:+${strengths},}source-unit" ;;
            ui) class_rank=1; list_contains "${strengths}" negative-adversarial || strengths="${strengths:+${strengths},}negative-adversarial" ;;
            ir) class_rank=2; list_contains "${strengths}" compile-code-object || strengths="${strengths:+${strengths},}compile-code-object" ;;
            compile) class_rank=3; list_contains "${strengths}" compile-code-object || strengths="${strengths:+${strengths},}compile-code-object" ;;
            verus) class_rank=4; strengths="${strengths:+${strengths},}verus-proof" ;;
            hardware) class_rank=5; strengths="${strengths:+${strengths},}remote-hardware" ;;
            debug) class_rank=6; strengths="${strengths:+${strengths},}debug-evidence" ;;
            *) die "signed-promotion ledger has an invalid class for ${id}: ${class}" ;;
          esac
          ((class_rank > previous_class_rank)) ||
            die "signed-promotion ledger classes are duplicate or out of order for ${id}"
          previous_class_rank="${class_rank}"
        done
        for result in "${result_values[@]}"; do
          [[ "${result}" =~ ^[A-Za-z0-9][A-Za-z0-9._/+:-]{0,511}$ &&
            "${result}" != /* ]] ||
            die "signed-promotion ledger result path is malformed for ${id}"
          IFS=/ read -r -a path_segments <<<"${result}"
          for segment in "${path_segments[@]}"; do
            [[ -n "${segment}" && "${segment}" != . && "${segment}" != .. ]] ||
              die "signed-promotion ledger result path traverses for ${id}"
          done
          result="docs/parity-evidence/archive/${result}"
          [[ -f "${REPO_ROOT}/${result}" && ! -L "${REPO_ROOT}/${result}" ]] ||
            die "signed-promotion ledger result is missing for ${id}: ${result}"
          signed_paths="${signed_paths:+${signed_paths},}${result}"
        done

        evidence_id="signed-${id,,}"
        EVIDENCE_PATHS["${evidence_id}"]="${signed_paths}"
        EVIDENCE_TESTS["${evidence_id}"]="scripts/parity-row-evidence.sh gate"
        EVIDENCE_COMMIT["${evidence_id}"]="${source}"
        EVIDENCE_LANES["${evidence_id}"]="${target}"
        EVIDENCE_TOOLCHAINS["${evidence_id}"]="${toolchains}"
        EVIDENCE_STRENGTHS["${evidence_id}"]="${strengths}"
        EVIDENCE_LIMITATIONS["${evidence_id}"]='Signed records satisfy the protected row policy; paths identify archived evidence, while semantic limitations remain in the matrix acceptance target.'
        EVIDENCE_SIGNED["${evidence_id}"]=1
        EVIDENCE_USED["${evidence_id}"]=1
        CLAIM_STATUS["${id}"]="${to_status}"
        CLAIM_EVIDENCE["${id}"]="${evidence_id}"
        previous_rank="${current_rank}"
        ((actual_count += 1))
        ;;
    esac
  done <"${path}"
  ((line_number >= 2 && actual_count == expected_count)) ||
    die 'signed-promotion ledger count does not match its rows'
}

validate_claim_coverage() {
  local row_index
  local id
  local status
  local evidence_id
  local strengths
  for ((row_index = 1; row_index <= 109; row_index++)); do
    id="$(row_id_at "${row_index}")"
    status="${ROW_STATUS[${id}]}"
    if [[ "${status}" == Partial || "${status}" == Complete ]]; then
      [[ -v "CLAIM_STATUS[${id}]" ]] || die "missing evidence claim for ${status} row ${id}"
      evidence_id="${CLAIM_EVIDENCE[${id}]}"
      if [[ "${status}" == Complete && ! -v "EVIDENCE_SIGNED[${evidence_id}]" ]]; then
        for strengths in source-unit negative-adversarial compile-code-object local-hardware remote-hardware; do
          list_contains "${EVIDENCE_STRENGTHS[${evidence_id}]}" "${strengths}" ||
            die "unsupported Complete upgrade for ${id}: missing ${strengths} evidence"
        done
        if [[ "${ROW_GATES[${id}]}" == *G5* || "${ROW_GATES[${id}]}" == *G7* ]]; then
          list_contains "${EVIDENCE_STRENGTHS[${evidence_id}]}" verus-proof ||
            die "unsupported Complete upgrade for ${id}: missing verus-proof evidence"
        fi
      fi
    elif [[ -v "CLAIM_STATUS[${id}]" ]]; then
      die "${status} row ${id} must not carry an implementation claim"
    fi
  done
  for evidence_id in "${!EVIDENCE_PATHS[@]}"; do
    [[ -v "EVIDENCE_USED[${evidence_id}]" ]] || die "unused evidence record: ${evidence_id}"
  done
}

markdown_escape() {
  local value="$1"
  value="${value//|/\\|}"
  printf '%s' "${value}"
}

emit_tsv() {
  local output="$1"
  local index
  local id
  local status
  local evidence_id
  local paths
  local tests
  local commit
  local lanes
  local toolchains
  local strengths
  local limitations

  {
    printf '# GENERATED by scripts/parity-dashboard.sh; DO NOT EDIT.\n'
    printf 'schema_version\t1\n'
    printf 'cuda_oxide_commit\t%s\n' "${CUDA_COMMIT}"
    printf 'fe2o3_commit\t%s\n' "${STATUS_COMMIT}"
    printf 'kind\tid\tfeature\tclass\tgates_csv\tstatus\tevidence_strength_csv\ttarget_lanes_csv\tevidence_commit\timplementation_paths_csv\ttest_commands_atat\ttoolchains_csv\tlimitations_or_reason\n'
    for ((index = 1; index <= 109; index++)); do
      id="$(row_id_at "${index}")"
      status="${ROW_STATUS[${id}]}"
      if [[ "${status}" == Partial || "${status}" == Complete ]]; then
        evidence_id="${CLAIM_EVIDENCE[${id}]}"
        paths="${EVIDENCE_PATHS[${evidence_id}]}"
        tests="${EVIDENCE_TESTS[${evidence_id}]}"
        commit="${EVIDENCE_COMMIT[${evidence_id}]}"
        lanes="${EVIDENCE_LANES[${evidence_id}]}"
        toolchains="${EVIDENCE_TOOLCHAINS[${evidence_id}]}"
        strengths="${EVIDENCE_STRENGTHS[${evidence_id}]}"
        limitations="${EVIDENCE_LIMITATIONS[${evidence_id}]}"
      elif [[ "${status}" == N/A ]]; then
        paths=-
        tests=-
        commit=-
        lanes=generic
        toolchains=-
        strengths=n/a
        limitations="${ROW_ACCEPTANCE[${id}]}"
      else
        paths=-
        tests=-
        commit=-
        lanes=generic
        toolchains=-
        strengths=none
        limitations='No qualifying implementation evidence is recorded.'
      fi
      printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "${ROW_KIND[${id}]}" "${id}" "${ROW_FEATURE[${id}]}" "${ROW_CLASS[${id}]}" \
        "${ROW_GATES[${id}]}" "${status}" "${strengths}" "${lanes}" "${commit}" \
        "${paths}" "${tests}" "${toolchains}" "${limitations}"
    done
  } >"${output}"
}

emit_markdown() {
  local output="$1"
  local index
  local id
  local kind
  local status
  local class
  local gate
  local strength
  local evidence_id
  local -a gates=()
  local -a strengths=()
  declare -A status_counts=()
  declare -A class_counts=()
  declare -A gate_counts=()
  declare -A strength_counts=()

  for ((index = 1; index <= 109; index++)); do
    id="$(row_id_at "${index}")"
    kind="${ROW_KIND[${id}]}"
    status="${ROW_STATUS[${id}]}"
    class="${ROW_CLASS[${id}]}"
    ((status_counts["${kind},${status}"] += 1))
    ((class_counts["${class},${status}"] += 1))
    IFS=, read -r -a gates <<<"${ROW_GATES[${id}]}"
    for gate in "${gates[@]}"; do
      ((gate_counts["${gate},${status}"] += 1))
    done
    if [[ "${status}" == Partial || "${status}" == Complete ]]; then
      evidence_id="${CLAIM_EVIDENCE[${id}]}"
      IFS=, read -r -a strengths <<<"${EVIDENCE_STRENGTHS[${evidence_id}]}"
      for strength in "${strengths[@]}"; do
        ((strength_counts["${strength}"] += 1))
      done
    fi
  done

  {
    printf '<!-- GENERATED by scripts/parity-dashboard.sh; DO NOT EDIT. -->\n'
    printf '# CUDA-Oxide Parity Evidence Dashboard\n\n'
    printf "Comparison baseline: \`%s\`. fe2o3 evidence snapshot: \`%s\`.\n\n" \
      "${CUDA_COMMIT}" "${STATUS_COMMIT}"
    printf 'A Partial or Complete row appears here only when a claim record names current repository paths, test commands, the validated commit, exact target lanes, relevant toolchains, evidence strengths, and limitations. Evidence strengths are independent: this dashboard never treats source tests, code-object compilation, hardware execution, Verus proofs, or machine-code refinement as substitutes for one another.\n\n'
    printf 'N/A rows are visible below and are not parity blockers because their matrix class is explicitly N/A. They do not assert an AMD implementation or semantic equivalence.\n\n'
    printf '## Overall\n\n'
    printf '| Scope | Complete | Partial | Missing | N/A | Blockers | Total |\n'
    printf '|:--|--:|--:|--:|--:|--:|--:|\n'
    for kind in normative supplemental; do
      printf '| %s | %d | %d | %d | %d | %d | %d |\n' \
        "$(tr '[:lower:]' '[:upper:]' <<<"${kind:0:1}")${kind:1}" \
        "${status_counts[${kind},Complete]:-0}" "${status_counts[${kind},Partial]:-0}" \
        "${status_counts[${kind},Missing]:-0}" "${status_counts[${kind},N/A]:-0}" \
        "$(( ${status_counts[${kind},Partial]:-0} + ${status_counts[${kind},Missing]:-0} ))" \
        "$(( ${status_counts[${kind},Complete]:-0} + ${status_counts[${kind},Partial]:-0} + ${status_counts[${kind},Missing]:-0} + ${status_counts[${kind},N/A]:-0} ))"
    done
    printf '\n## Gaps By Gate\n\n'
    printf 'Rows assigned to multiple gates are counted in each listed gate. Partial and Missing rows are blockers.\n\n'
    printf '| Gate | Complete | Partial | Missing | N/A | Blockers |\n'
    printf '|:--|--:|--:|--:|--:|--:|\n'
    for gate in G1 G2 G3 G4 G5 G6 G7 G8; do
      printf '| %s | %d | %d | %d | %d | %d |\n' "${gate}" \
        "${gate_counts[${gate},Complete]:-0}" "${gate_counts[${gate},Partial]:-0}" \
        "${gate_counts[${gate},Missing]:-0}" "${gate_counts[${gate},N/A]:-0}" \
        "$(( ${gate_counts[${gate},Partial]:-0} + ${gate_counts[${gate},Missing]:-0} ))"
    done
    printf '\n## Status By Class\n\n'
    printf '| Class | Complete | Partial | Missing | N/A |\n'
    printf '|:--|--:|--:|--:|--:|\n'
    for class in Exact AMD-equivalent N/A; do
      printf '| %s | %d | %d | %d | %d |\n' "${class}" \
        "${class_counts[${class},Complete]:-0}" "${class_counts[${class},Partial]:-0}" \
        "${class_counts[${class},Missing]:-0}" "${class_counts[${class},N/A]:-0}"
    done
    printf '\n## Evidence Strength\n\n'
    printf 'Counts are row claims carrying each explicit evidence kind; a row can appear more than once.\n\n'
    printf '| Evidence kind | Rows | What it establishes |\n'
    printf '|:--|--:|:--|\n'
    printf '| Source/unit proof | %d | Source contracts and CPU/unit behavior only |\n' "${strength_counts[source-unit]:-0}"
    printf '| Compile to code object | %d | Target-specific compilation only |\n' "${strength_counts[compile-code-object]:-0}"
    printf '| Local hardware execution | %d | Execution on the named local lane only |\n' "${strength_counts[local-hardware]:-0}"
    printf '| Remote hardware execution | %d | Execution on the named remote lane only |\n' "${strength_counts[remote-hardware]:-0}"
    printf '| Negative/adversarial testing | %d | Rejection or robustness behavior only |\n' "${strength_counts[negative-adversarial]:-0}"
    printf '| Verus proof | %d | The named source-level mathematical model only |\n' "${strength_counts[verus-proof]:-0}"
    printf '| Machine-code refinement | %d | Verified source/IR-to-machine-code correspondence only |\n' "${strength_counts[machine-code-refinement]:-0}"
    printf '\n## N/A Rows\n\n'
    printf '| ID | Feature | Gate | Why this is not an AMD parity blocker |\n'
    printf '|:--|:--|:--|:--|\n'
    for ((index = 1; index <= 109; index++)); do
      id="$(row_id_at "${index}")"
      [[ "${ROW_STATUS[${id}]}" == N/A ]] || continue
      printf '| %s | %s | %s | %s |\n' "${id}" \
        "$(markdown_escape "${ROW_FEATURE[${id}]}")" "${ROW_GATES[${id}]}" \
        "$(markdown_escape "${ROW_ACCEPTANCE[${id}]}")"
    done
    printf '\n## Claim Details\n\n'
    printf 'The deterministic machine-readable row ledger, including implementation paths, commands, lanes, toolchains, and limitations, is [cuda-oxide-parity-dashboard.tsv](cuda-oxide-parity-dashboard.tsv). No row is Complete at this evidence snapshot.\n'
  } >"${output}"
}

main() {
  local command="${1:-}"
  local status_file="${DEFAULT_STATUS}"
  local matrix_file="${DEFAULT_MATRIX}"
  local claims_file=""
  local markdown_file="${DEFAULT_MARKDOWN}"
  local tsv_file="${DEFAULT_TSV}"
  local signed_promotions_file="${DEFAULT_SIGNED_PROMOTIONS}"
  local promotion_baseline=""
  local row_evidence_archive=""
  local row_evidence_manifest=""
  local row_evidence_trusted_root=""
  local row_evidence_trust_policy=""
  local row_evidence_trusted_policy=""
  local row_evidence_candidate_policy=""
  local generated_markdown
  local generated_tsv

  [[ -n "${command}" ]] || {
    usage >&2
    return 2
  }
  shift
  if [[ "${command}" == claims ]]; then
    (($# == 0)) || die 'claims accepts no options'
    parse_status "${DEFAULT_STATUS}"
    emit_default_claims
    return 0
  fi
  case "${command}" in
    check | update | validate) ;;
    -h | --help | help)
      usage
      return 0
      ;;
    *)
      usage >&2
      die "unknown command: ${command}"
      ;;
  esac

  while (($# > 0)); do
    (($# >= 2)) || die "$1 requires a value"
    case "$1" in
      --status) status_file="$2" ;;
      --matrix) matrix_file="$2" ;;
      --claims) claims_file="$2" ;;
      --repo) REPO_ROOT="$2" ;;
      --markdown) markdown_file="$2" ;;
      --tsv) tsv_file="$2" ;;
      --signed-promotions) signed_promotions_file="$2" ;;
      --promotion-baseline) promotion_baseline="$2" ;;
      --row-evidence-archive) row_evidence_archive="$2" ;;
      --row-evidence-manifest) row_evidence_manifest="$2" ;;
      --row-evidence-trusted-root) row_evidence_trusted_root="$2" ;;
      --row-evidence-trust-policy) row_evidence_trust_policy="$2" ;;
      --row-evidence-trusted-policy) row_evidence_trusted_policy="$2" ;;
      --row-evidence-candidate-policy) row_evidence_candidate_policy="$2" ;;
      *) die "unknown option: $1" ;;
    esac
    shift 2
  done
  [[ -d "${REPO_ROOT}" ]] || die "repository root does not exist: ${REPO_ROOT}"

  TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-parity-dashboard.XXXXXX")"
  parse_status "${status_file}"
  if [[ -z "${claims_file}" ]]; then
    claims_file="${TEMP_ROOT}/claims.tsv"
    emit_default_claims >"${claims_file}"
  fi
  parse_matrix "${matrix_file}"
  parse_claims "${claims_file}"
  parse_signed_promotions "${signed_promotions_file}"
  validate_claim_coverage

  if [[ -n "${promotion_baseline}${row_evidence_archive}${row_evidence_manifest}${row_evidence_trusted_root}${row_evidence_trust_policy}${row_evidence_trusted_policy}${row_evidence_candidate_policy}" ]]; then
    [[ -n "${promotion_baseline}" && -n "${row_evidence_archive}" &&
      -n "${row_evidence_manifest}" &&
      -n "${row_evidence_trusted_root}" &&
      -n "${row_evidence_trust_policy}" &&
      -n "${row_evidence_trusted_policy}" &&
      -n "${row_evidence_candidate_policy}" ]] ||
      die 'promotion validation requires every protected signed-evidence input'
    "${SCRIPT_DIR}/parity-row-evidence.sh" gate \
      --repo "${REPO_ROOT}" \
      --archive-root "${row_evidence_archive}" \
      --trusted-root "${row_evidence_trusted_root}" \
      --trust-policy "${row_evidence_trust_policy}" \
      --manifest "${row_evidence_manifest}" \
      --trusted-policy "${row_evidence_trusted_policy}" \
      --candidate-policy "${row_evidence_candidate_policy}" \
      --baseline-status "${promotion_baseline}" \
      --candidate-status "${status_file}"
  fi

  if [[ "${command}" == validate ]]; then
    printf 'parity claims are valid: 109 rows, %d evidence records\n' "${#EVIDENCE_PATHS[@]}"
    return 0
  fi

  generated_markdown="${TEMP_ROOT}/dashboard.md"
  generated_tsv="${TEMP_ROOT}/dashboard.tsv"
  emit_markdown "${generated_markdown}"
  emit_tsv "${generated_tsv}"

  if [[ "${command}" == check ]]; then
    [[ -f "${markdown_file}" ]] || die "generated Markdown is missing: ${markdown_file}"
    [[ -f "${tsv_file}" ]] || die "generated TSV is missing: ${tsv_file}"
    if ! cmp -s -- "${markdown_file}" "${generated_markdown}"; then
      printf 'parity dashboard: generated Markdown drift; run scripts/parity-dashboard.sh update\n' >&2
      diff -u --label "${markdown_file}" --label generated "${markdown_file}" "${generated_markdown}" >&2 || true
      return 1
    fi
    if ! cmp -s -- "${tsv_file}" "${generated_tsv}"; then
      printf 'parity dashboard: generated TSV drift; run scripts/parity-dashboard.sh update\n' >&2
      diff -u --label "${tsv_file}" --label generated "${tsv_file}" "${generated_tsv}" >&2 || true
      return 1
    fi
    printf 'parity dashboard is current: 109 evidence-gated rows\n'
    return 0
  fi

  mkdir -p -- "$(dirname -- "${markdown_file}")" "$(dirname -- "${tsv_file}")"
  cp -- "${generated_markdown}" "${markdown_file}"
  cp -- "${generated_tsv}" "${tsv_file}"
  printf 'updated deterministic parity dashboard: %s and %s\n' "${markdown_file}" "${tsv_file}"
}

main "$@"
