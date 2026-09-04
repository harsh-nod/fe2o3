#!/usr/bin/env bash

set -Eeuo pipefail
umask 077

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../.." && pwd)"
readonly REPO_ROOT
readonly MATRIX_SCRIPT="${REPO_ROOT}/scripts/kernel-compile-matrix.sh"
TEST_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-kernel-compile-matrix-test.XXXXXXXX")"
readonly TEST_ROOT
readonly FAKE_BIN="${TEST_ROOT}/bin"
readonly FAKE_ROCM="${TEST_ROOT}/rocm"
readonly GFX950_DEVICE_LIBS="${TEST_ROOT}/gfx950-device-libs"
readonly GFX950_MANIFEST="${TEST_ROOT}/gfx950-ocml-rocm-7.2.1.manifest"
readonly GFX950_MANIFEST_724="${TEST_ROOT}/gfx950-ocml-rocm-7.2.4.manifest"
readonly GFX950_MANIFEST_UNREVIEWED="${TEST_ROOT}/gfx950-ocml-rocm-7.2.5.manifest"
readonly GFX950_MANIFEST_BAD_DIGEST="${TEST_ROOT}/gfx950-ocml-bad-digest.manifest"
readonly CHECKED_GFX950_MANIFEST_724="${REPO_ROOT}/examples/gfx950_low_precision/gfx950-ocml-rocm-7.2.4.manifest"
readonly LOG="${TEST_ROOT}/commands.log"
trap 'rm -rf -- "${TEST_ROOT}"' EXIT

mkdir -p -- \
  "${FAKE_BIN}" \
  "${FAKE_ROCM}/llvm/bin" \
  "${FAKE_ROCM}/amdgcn/bitcode" \
  "${GFX950_DEVICE_LIBS}" \
  "${TEST_ROOT}/sysroot/lib" \
  "${TEST_ROOT}/tmp"
for library in \
  ocml.bc \
  oclc_isa_version_942.bc \
  oclc_unsafe_math_off.bc \
  oclc_finite_only_off.bc; do
  : >"${FAKE_ROCM}/amdgcn/bitcode/${library}"
done
for library in \
  ocml.bc \
  ockl.bc \
  oclc_daz_opt_off.bc \
  oclc_unsafe_math_off.bc \
  oclc_finite_only_off.bc \
  oclc_correctly_rounded_sqrt_on.bc \
  oclc_wavefrontsize64_on.bc \
  oclc_isa_version_950.bc \
  oclc_abi_version_600.bc; do
  printf 'fixture %s\n' "${library}" >"${GFX950_DEVICE_LIBS}/${library}"
done

cat >"${FAKE_BIN}/rustup" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
printf 'rustup' >>"${KERNEL_MATRIX_TEST_LOG}"
printf ' %q' "$@" >>"${KERNEL_MATRIX_TEST_LOG}"
printf '\n' >>"${KERNEL_MATRIX_TEST_LOG}"

[[ "${1:-}" == run && $# -ge 3 ]] || exit 90
shift 2
case "${1:-}" in
  rustc)
    [[ " $* " == *' --print sysroot '* ]] || exit 91
    printf '%s\n' "${KERNEL_MATRIX_TEST_SYSROOT}"
    ;;
  cargo)
    shift
    case "${1:-}" in
      build)
        mkdir -p -- "${CARGO_TARGET_DIR}/debug/deps"
        printf '#!/usr/bin/env bash\nexit 0\n' \
          >"${CARGO_TARGET_DIR}/debug/fe2o3-rustc-extract"
        chmod 700 "${CARGO_TARGET_DIR}/debug/fe2o3-rustc-extract"
        ;;
      check)
        [[ -n "${FE2O3_EXTRACT_CRATE_BINDING_PATH_V1:-}" ]] || exit 92
        printf '%064d\n' 0 >"${FE2O3_EXTRACT_CRATE_BINDING_PATH_V1}"
        chmod 600 "${FE2O3_EXTRACT_CRATE_BINDING_PATH_V1}"
        if [[ -n "${FE2O3_EXTRACT_GFX942_LLVM_PATH_V1:-}" ]]; then
          printf '%s\n' 'target triple = "amdgcn-amd-amdhsa"' \
            >"${FE2O3_EXTRACT_GFX942_LLVM_PATH_V1}"
        elif [[ -n "${FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1:-}" ]]; then
          feature=
          while (($# > 0)); do
            if [[ $1 == --features && $# -ge 2 ]]; then
              feature=$2
              break
            fi
            shift
          done
          [[ -n $feature ]] || exit 93
          symbol=
          fp4_mfma=0
          fp8_mfma=0
          mixed_mfma=0
          bf16_mfma=0
          transpose=
          transpose_calls=0
          ocml_calls=0
          lds=0
          extra_target_function=0
          case "$feature" in
            kernel-fp4-gemm)
              symbol=gfx950_fp4_gemm_rust; fp4_mfma=1 ;;
            kernel-fp8-gemm)
              symbol=gfx950_fp8_gemm_rust; fp8_mfma=1 ;;
            kernel-fp4-attention)
              symbol=gfx950_fp4_attention_rust; fp4_mfma=1
              transpose=llvm.amdgcn.ds.read.tr4.b64.v2i32
              transpose_calls=2; ocml_calls=4; lds=4096
              extra_target_function=1 ;;
            kernel-fp8-attention)
              symbol=gfx950_fp8_attention_rust; fp8_mfma=1
              transpose=llvm.amdgcn.ds.read.tr8.b64.v2i32
              transpose_calls=4; ocml_calls=4; lds=8192 ;;
            kernel-kda-decode | kernel-kda-decode-baseline-v1)
              symbol=gfx950_kda_decode ;;
            kernel-kda-prefill | kernel-kda-prefill-baseline-v1)
              symbol=gfx950_kda_chunkwise_prefill ;;
            kernel-content-sparse-attention | kernel-content-sparse-attention-reciprocal-reuse-v1)
              symbol=gfx950_content_sparse_attention; fp8_mfma=1
              transpose=llvm.amdgcn.ds.read.tr8.b64.v2i32
              transpose_calls=4; ocml_calls=1; lds=8192 ;;
            kernel-deepseek-sparse-attention)
              symbol=gfx950_deepseek_sparse_attention; ocml_calls=1 ;;
            kernel-compressed-hybrid-attention | kernel-compressed-hybrid-attention-division-baseline-v1)
              symbol=gfx950_compressed_hybrid_attention; fp8_mfma=1
              transpose=llvm.amdgcn.ds.read.tr8.b64.v2i32
              transpose_calls=4; ocml_calls=1; lds=8192 ;;
            kernel-attnres-aggregate | kernel-attnres-aggregate-explicit-reuse-v1)
              symbol=gfx950_attnres_aggregate; ocml_calls=1 ;;
            kernel-four-branch-residual | kernel-four-branch-residual-explicit-v1)
              symbol=gfx950_four_branch_residual; ocml_calls=1 ;;
            kernel-mhc-sinkhorn-mix | kernel-mhc-sinkhorn-mix-scalar-v1)
              symbol=gfx950_mhc_sinkhorn_mix; ocml_calls=1 ;;
            kernel-moe-route)
              symbol=gfx950_moe_route_fp4_t16_e4_k2_v1; ocml_calls=1 ;;
            kernel-moe-expert-rank | kernel-moe-expert-rank,ablation-expert-serial)
              symbol=gfx950_moe_expert_rank_fp4_fp8_v1; mixed_mfma=3
              ocml_calls=1 ;;
            kernel-combine-expert-ranks)
              symbol=gfx950_combine_expert_ranks_v1 ;;
            kernel-speculative-transaction | kernel-speculative-transaction,ablation-speculative-recompute-prefix)
              symbol=gfx950_speculative_transaction_v1 ;;
            kernel-qwen-ngram-gather | kernel-qwen-ngram-gather,ablation-ngram-reverse-probe)
              symbol=gfx950_qwen_ngram_gather_v1 ;;
            kernel-stage-gradient-shard)
              symbol=gfx950_stage_gradient_shard_v1 ;;
            kernel-muon-update | kernel-muon-update,ablation-muon-broadcast16)
              symbol=gfx950_muon_update_4x4_v1 ;;
            kernel-gpt-oss-decode | kernel-gpt-oss-decode-router-serial | kernel-gpt-oss-decode-held-fragments | kernel-gpt-oss-decode-interleaved-stores)
              symbol=gfx950_gpt_oss_120b_decode_megakernel_v1
              fp4_mfma=4; bf16_mfma=4; ocml_calls=1 ;;
            kernel-gpt-oss-router-component)
              symbol=gfx950_gpt_oss_120b_router_v1 ;;
            kernel-gpt-oss-attention-component)
              symbol=gfx950_gpt_oss_120b_attention_v1
              bf16_mfma=4; ocml_calls=1 ;;
            kernel-gpt-oss-expert-component)
              symbol=gfx950_gpt_oss_120b_expert_v1; fp4_mfma=4 ;;
            *) exit 94 ;;
          esac
          {
            printf '%s\n' 'target triple = "amdgcn-amd-amdhsa"'
            printf 'define amdgpu_kernel void @%s() #0 {\n' "$symbol"
            if [[ $feature == kernel-kda-* ]]; then
              printf '  %%kda = call i32 @llvm.amdgcn.ds.bpermute(i32 0, i32 0)\n'
            fi
            for ((index = 0; index < bf16_mfma; index++)); do
              printf '  %%b%s = call <4 x float> @llvm.amdgcn.mfma.f32.16x16x16bf16.1k()\n' "$index"
            done
            for ((index = 0; index < fp4_mfma; index++)); do
              printf '  %%f4%s = call <4 x float> @llvm.amdgcn.mfma.scale.f32.16x16x128.f8f6f4.v8i32.v8i32(i32 4, i32 4, i32 0, i32 0, i32 0, i32 0)\n' "$index"
            done
            for ((index = 0; index < fp8_mfma; index++)); do
              printf '  %%f8%s = call <4 x float> @llvm.amdgcn.mfma.scale.f32.16x16x128.f8f6f4.v8i32.v8i32(i32 0, i32 0, i32 0, i32 0, i32 0, i32 0)\n' "$index"
            done
            for ((index = 0; index < mixed_mfma; index++)); do
              printf '  %%fm%s = call <4 x float> @llvm.amdgcn.mfma.scale.f32.16x16x128.f8f6f4.v8i32.v8i32(i32 4, i32 0, i32 0, i32 0, i32 0, i32 0)\n' "$index"
            done
            for ((index = 0; index < transpose_calls; index++)); do
              printf '  %%t%s = call <2 x i32> @%s(i32 0)\n' "$index" "$transpose"
            done
            for ((index = 0; index < ocml_calls; index++)); do
              printf '  %%e%s = call float @__ocml_exp_f32(float 0.0)\n' "$index"
            done
            if ((lds > 0)); then
              printf '@lds = internal addrspace(3) global [%s x i8] undef, align 64\n' "$lds"
              printf '  call void asm sideeffect "s_barrier", ""()\n'
            fi
            printf '  ret void\n}\n'
            printf 'attributes #0 = { "target-cpu"="gfx950" "target-features"="-wavefrontsize32,+wavefrontsize64,-xnack" }\n'
            if ((extra_target_function > 0)); then
              printf 'define internal float @decode_fp4_e2m1(i8 %%bits) #1 {\n'
              printf '  ret float 0.0\n}\n'
              printf 'attributes #1 = { "target-cpu"="gfx950" "target-features"="-wavefrontsize32,+wavefrontsize64,-xnack" }\n'
            fi
            if ((fp4_mfma + fp8_mfma + mixed_mfma > 0)); then
              printf 'declare <4 x float> @llvm.amdgcn.mfma.scale.f32.16x16x128.f8f6f4.v8i32.v8i32()\n'
            fi
            if [[ -n $transpose ]]; then
              printf 'declare <2 x i32> @%s(i32)\n' "$transpose"
            fi
            if ((ocml_calls > 0)); then
              printf 'declare float @__ocml_exp_f32(float)\n'
            fi
          } >"${FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1}"
        else
          exit 95
        fi
        ;;
      run | test)
        printf '%s\n' 'hardware/host execution was reached in compile-only mode' >&2
        exit 96
        ;;
      *) exit 97 ;;
    esac
    ;;
  *) exit 98 ;;
esac
EOF

cat >"${FAKE_BIN}/cargo" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' 'cargo was invoked outside the rustup fixture' >&2
exit 99
EOF

cat >"${FAKE_ROCM}/llvm/bin/llvm-link" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
output=
input=$1
while (($# > 0)); do
  if [[ "$1" == -o ]]; then
    output=$2
    break
  fi
  shift
done
cp -- "${input}" "${output}"
EOF

cat >"${FAKE_ROCM}/llvm/bin/clang" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
while (($# > 0)); do
  if [[ "$1" == -o ]]; then
    printf '%s\n' object >"$2"
    exit 0
  fi
  shift
done
exit 97
EOF

cat >"${FAKE_ROCM}/llvm/bin/ld.lld" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
while (($# > 0)); do
  if [[ "$1" == -o ]]; then
    printf '%s\n' hsaco >"$2"
    exit 0
  fi
  shift
done
exit 98
EOF

cat >"${FAKE_ROCM}/llvm/bin/llvm-readobj" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
artifact=${!#}
key=${artifact##*/}
key=${key%.hsaco}
symbol=
kernarg=0
workgroup=64
lds=0
case "$key" in
  gfx950-fp4-gemm) symbol=gfx950_fp4_gemm_rust ;;
  gfx950-fp8-gemm) symbol=gfx950_fp8_gemm_rust ;;
  gfx950-fp4-attention)
    symbol=gfx950_fp4_attention_rust; kernarg=64; lds=4096 ;;
  gfx950-fp8-attention)
    symbol=gfx950_fp8_attention_rust; kernarg=64; lds=8192 ;;
  kernel-kda-decode | kernel-kda-decode-baseline-v1)
    symbol=gfx950_kda_decode; kernarg=128; workgroup=256 ;;
  kernel-kda-prefill | kernel-kda-prefill-baseline-v1)
    symbol=gfx950_kda_chunkwise_prefill; kernarg=144; workgroup=256 ;;
  kernel-content-sparse-attention | kernel-content-sparse-attention-reciprocal-reuse-v1)
    symbol=gfx950_content_sparse_attention; kernarg=96; lds=8192 ;;
  kernel-deepseek-sparse-attention)
    symbol=gfx950_deepseek_sparse_attention; kernarg=112 ;;
  kernel-compressed-hybrid-attention | kernel-compressed-hybrid-attention-division-baseline-v1)
    symbol=gfx950_compressed_hybrid_attention; kernarg=80; lds=8192 ;;
  kernel-attnres-aggregate | kernel-attnres-aggregate-explicit-reuse-v1)
    symbol=gfx950_attnres_aggregate; kernarg=48 ;;
  kernel-four-branch-residual | kernel-four-branch-residual-explicit-v1)
    symbol=gfx950_four_branch_residual; kernarg=64 ;;
  kernel-mhc-sinkhorn-mix | kernel-mhc-sinkhorn-mix-scalar-v1)
    symbol=gfx950_mhc_sinkhorn_mix; kernarg=48 ;;
  kernel-moe-route)
    symbol=gfx950_moe_route_fp4_t16_e4_k2_v1; kernarg=96; workgroup=256 ;;
  kernel-moe-expert-rank)
    symbol=gfx950_moe_expert_rank_fp4_fp8_v1; kernarg=88 ;;
  kernel-combine-expert-ranks)
    symbol=gfx950_combine_expert_ranks_v1; kernarg=48; workgroup=256 ;;
  kernel-speculative-transaction)
    symbol=gfx950_speculative_transaction_v1; kernarg=144 ;;
  kernel-qwen-ngram-gather)
    symbol=gfx950_qwen_ngram_gather_v1; kernarg=96 ;;
  kernel-stage-gradient-shard)
    symbol=gfx950_stage_gradient_shard_v1; kernarg=32 ;;
  kernel-muon-update)
    symbol=gfx950_muon_update_4x4_v1; kernarg=48 ;;
  kernel-gpt-oss-decode | kernel-gpt-oss-decode-router-serial | kernel-gpt-oss-decode-held-fragments | kernel-gpt-oss-decode-interleaved-stores)
    symbol=gfx950_gpt_oss_120b_decode_megakernel_v1; kernarg=208 ;;
  kernel-gpt-oss-router-component)
    symbol=gfx950_gpt_oss_120b_router_v1; kernarg=48 ;;
  kernel-gpt-oss-attention-component)
    symbol=gfx950_gpt_oss_120b_attention_v1; kernarg=80 ;;
  kernel-gpt-oss-expert-component)
    symbol=gfx950_gpt_oss_120b_expert_v1; kernarg=96 ;;
  *) exit 99 ;;
esac
if [[ $key == kernel-* ]]; then
  workgroup=256
fi
cat <<EOF_METADATA
Format: elf64-amdgpu
Machine: EM_AMDGPU
Flags [
  EF_AMDGPU_MACH_AMDGCN_GFX950
  EF_AMDGPU_FEATURE_XNACK_OFF_V4
]
amdhsa.target: 'amdgcn-amd-amdhsa--gfx950:xnack-'
.name: ${symbol}
.symbol: ${symbol}.kd
.kernarg_segment_size: ${kernarg}
.kernarg_segment_align: 8
.group_segment_fixed_size: ${lds}
.wavefront_size: 64
.reqd_workgroup_size:
- ${workgroup}
- 1
- 1
.max_flat_workgroup_size: ${workgroup}
.uses_dynamic_stack: false
amdhsa.version:
- 1
- 2
EOF_METADATA
EOF

cat >"${FAKE_ROCM}/llvm/bin/llvm-objdump" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
artifact=${!#}
key=${artifact##*/}
key=${key%.hsaco}
symbol=
kind=scalar
case "$key" in
  gfx950-fp4-gemm) symbol=gfx950_fp4_gemm_rust; kind=fp4 ;;
  gfx950-fp8-gemm) symbol=gfx950_fp8_gemm_rust; kind=fp8 ;;
  gfx950-fp4-attention) symbol=gfx950_fp4_attention_rust; kind=fp4_attention ;;
  gfx950-fp8-attention) symbol=gfx950_fp8_attention_rust; kind=fp8_attention ;;
  kernel-kda-decode | kernel-kda-decode-baseline-v1)
    symbol=gfx950_kda_decode; kind=kda ;;
  kernel-kda-prefill | kernel-kda-prefill-baseline-v1)
    symbol=gfx950_kda_chunkwise_prefill; kind=kda ;;
  kernel-content-sparse-attention | kernel-content-sparse-attention-reciprocal-reuse-v1)
    symbol=gfx950_content_sparse_attention; kind=fp8_attention ;;
  kernel-deepseek-sparse-attention) symbol=gfx950_deepseek_sparse_attention ;;
  kernel-compressed-hybrid-attention | kernel-compressed-hybrid-attention-division-baseline-v1)
    symbol=gfx950_compressed_hybrid_attention; kind=fp8_attention ;;
  kernel-attnres-aggregate | kernel-attnres-aggregate-explicit-reuse-v1)
    symbol=gfx950_attnres_aggregate ;;
  kernel-four-branch-residual | kernel-four-branch-residual-explicit-v1)
    symbol=gfx950_four_branch_residual ;;
  kernel-mhc-sinkhorn-mix | kernel-mhc-sinkhorn-mix-scalar-v1)
    symbol=gfx950_mhc_sinkhorn_mix ;;
  kernel-moe-route) symbol=gfx950_moe_route_fp4_t16_e4_k2_v1 ;;
  kernel-moe-expert-rank)
    symbol=gfx950_moe_expert_rank_fp4_fp8_v1; kind=mixed ;;
  kernel-combine-expert-ranks) symbol=gfx950_combine_expert_ranks_v1 ;;
  kernel-speculative-transaction) symbol=gfx950_speculative_transaction_v1 ;;
  kernel-qwen-ngram-gather) symbol=gfx950_qwen_ngram_gather_v1 ;;
  kernel-stage-gradient-shard) symbol=gfx950_stage_gradient_shard_v1 ;;
  kernel-muon-update) symbol=gfx950_muon_update_4x4_v1 ;;
  kernel-gpt-oss-decode | kernel-gpt-oss-decode-router-serial | kernel-gpt-oss-decode-held-fragments | kernel-gpt-oss-decode-interleaved-stores)
    symbol=gfx950_gpt_oss_120b_decode_megakernel_v1; kind=gpt_oss ;;
  kernel-gpt-oss-router-component)
    symbol=gfx950_gpt_oss_120b_router_v1 ;;
  kernel-gpt-oss-attention-component)
    symbol=gfx950_gpt_oss_120b_attention_v1; kind=gpt_oss_attention ;;
  kernel-gpt-oss-expert-component)
    symbol=gfx950_gpt_oss_120b_expert_v1; kind=gpt_oss_expert ;;
  *) exit 99 ;;
esac
printf '0000000000000000 <%s>:\n' "$symbol"
case "$kind" in
  fp4)
    printf '%s\n' 'v_mfma_f32_16x16x128_f8f6f4 cbsz:4 blgp:4' ;;
  fp8)
    printf '%s\n' 'v_mfma_f32_16x16x128_f8f6f4' ;;
  fp4_attention)
    printf '%s\n' 'ds_read_b64_tr_b4' 'ds_read_b64_tr_b4'
    printf '%s\n' 'v_mfma_f32_16x16x128_f8f6f4 cbsz:4 blgp:4' ;;
  fp8_attention)
    printf '%s\n' 'ds_read_b64_tr_b8' 'ds_read_b64_tr_b8' \
      'ds_read_b64_tr_b8' 'ds_read_b64_tr_b8'
    printf '%s\n' 'v_mfma_f32_16x16x128_f8f6f4' ;;
  kda)
    printf '%s\n' 'ds_bpermute_b32' ;;
  mixed)
    printf '%s\n' \
      'v_mfma_f32_16x16x128_f8f6f4 cbsz:4' \
      'v_mfma_f32_16x16x128_f8f6f4 cbsz:4' \
      'v_mfma_f32_16x16x128_f8f6f4 cbsz:4' ;;
  gpt_oss)
    for _ in 1 2 3 4; do
      printf '%s\n' 'v_mfma_f32_16x16x16_bf16'
    done
    for _ in 1 2 3 4; do
      printf '%s\n' 'v_mfma_f32_16x16x128_f8f6f4 cbsz:4'
    done ;;
  gpt_oss_attention)
    for _ in 1 2 3 4; do
      printf '%s\n' 'v_mfma_f32_16x16x16_bf16'
    done ;;
  gpt_oss_expert)
    for _ in 1 2 3 4; do
      printf '%s\n' 'v_mfma_f32_16x16x128_f8f6f4 cbsz:4'
    done ;;
  scalar)
    printf '%s\n' 's_endpgm' ;;
esac
EOF

chmod 700 \
  "${FAKE_BIN}/cargo" \
  "${FAKE_BIN}/rustup" \
  "${FAKE_ROCM}/llvm/bin/llvm-link" \
  "${FAKE_ROCM}/llvm/bin/clang" \
  "${FAKE_ROCM}/llvm/bin/ld.lld" \
  "${FAKE_ROCM}/llvm/bin/llvm-readobj" \
  "${FAKE_ROCM}/llvm/bin/llvm-objdump"

write_fixture_manifest() {
  local rocm_version=$1
  printf '%s\n' \
    'schema=fe2o3-gfx950-ocml-closure-v1' \
    "rocm_version=${rocm_version}" \
    'llvm_version=22.0.0git' \
    "canonical_device_library_dir=${GFX950_DEVICE_LIBS}"
  for library in \
    ocml.bc \
    ockl.bc \
    oclc_daz_opt_off.bc \
    oclc_unsafe_math_off.bc \
    oclc_finite_only_off.bc \
    oclc_correctly_rounded_sqrt_on.bc \
    oclc_wavefrontsize64_on.bc \
    oclc_isa_version_950.bc \
    oclc_abi_version_600.bc; do
    printf '%s=%s\n' "$library" "$(sha256sum -- "${GFX950_DEVICE_LIBS}/${library}" | awk '{ print $1 }')"
  done
  printf 'clang-22=%s\n' \
    "$(sha256sum -- "${FAKE_ROCM}/llvm/bin/clang" | awk '{ print $1 }')"
  printf 'lld=%s\n' \
    "$(sha256sum -- "${FAKE_ROCM}/llvm/bin/ld.lld" | awk '{ print $1 }')"
}

write_fixture_manifest 7.2.1 >"${GFX950_MANIFEST}"
write_fixture_manifest 7.2.4 >"${GFX950_MANIFEST_724}"
write_fixture_manifest 7.2.5 >"${GFX950_MANIFEST_UNREVIEWED}"
sed \
  's/^ocml\.bc=.*/ocml.bc=0000000000000000000000000000000000000000000000000000000000000000/' \
  "${GFX950_MANIFEST_724}" >"${GFX950_MANIFEST_BAD_DIGEST}"

[[ ! -L ${CHECKED_GFX950_MANIFEST_724} && -f ${CHECKED_GFX950_MANIFEST_724} ]]
[[ $(wc -l <"${CHECKED_GFX950_MANIFEST_724}") -eq 15 ]]
[[ $(sha256sum -- "${CHECKED_GFX950_MANIFEST_724}" | awk '{ print $1 }') == \
  330a4190d140bfd9b9eeeefa97302241c9e422e140e0044d4cb9f49d0c24696e ]]

validate_fixture_manifest() {
  local manifest=$1
  # SC2016: the child shell expands the closure variables after sourcing it.
  # shellcheck disable=SC2016
  env \
    SCRIPT_DIR="${REPO_ROOT}/examples/gfx950_low_precision" \
    CLANG="${FAKE_ROCM}/llvm/bin/clang" \
    LD_LLD="${FAKE_ROCM}/llvm/bin/ld.lld" \
    SHA256SUM=sha256sum \
    FE2O3_GFX950_OCML_MANIFEST="${manifest}" \
    bash -c '
      set -Eeuo pipefail
      source "$SCRIPT_DIR/gfx950-ocml-closure.sh"
      validate_gfx950_ocml_closure
      [[ ${#GFX950_OCML_CLANG_ARGS[@]} -eq 36 ]]
    '
}

validate_fixture_manifest "${GFX950_MANIFEST}"
validate_fixture_manifest "${GFX950_MANIFEST_724}"
set +e
unreviewed_output=$(validate_fixture_manifest "${GFX950_MANIFEST_UNREVIEWED}" 2>&1)
unreviewed_status=$?
set -e
[[ ${unreviewed_status} -eq 1 ]]
grep -F -- 'gfx950 OCML manifest has an unreviewed ROCm version: 7.2.5' \
  <<<"${unreviewed_output}" >/dev/null

set +e
bad_digest_output=$(validate_fixture_manifest "${GFX950_MANIFEST_BAD_DIGEST}" 2>&1)
bad_digest_status=$?
set -e
[[ ${bad_digest_status} -eq 1 ]]
grep -F -- 'gfx950 OCML input digest mismatch: ocml.bc' \
  <<<"${bad_digest_output}" >/dev/null

success_output="${TEST_ROOT}/success.out"
PATH="${FAKE_BIN}:/usr/bin:/bin" \
ROCM_PATH="${FAKE_ROCM}" \
TMPDIR="${TEST_ROOT}/tmp" \
KERNEL_MATRIX_TEST_LOG="${LOG}" \
KERNEL_MATRIX_TEST_SYSROOT="${TEST_ROOT}/sysroot" \
  bash "${MATRIX_SCRIPT}" >"${success_output}" 2>&1

grep -F -- \
  'kernel compile matrix: target=gfx942 mode=compile-only kernels=5 hardware_observed=false' \
  "${success_output}" >/dev/null
grep -F -- \
  'MATRIX PASS target=gfx942 compiled=5 hardware_executed=0 artifacts=temporary' \
  "${success_output}" >/dev/null
grep -F -- \
  'MATRIX LIMITATION gfx950 requires its separate exact ROCm 7.2.1 or 7.2.4 matrix; source-model-only, proof-only, and basic Cargo examples are not covered' \
  "${success_output}" >/dev/null
for name in \
  tiled-gemm \
  row-softmax \
  flash-attention \
  grouped-expert-moe \
  gemm-autoresearch; do
  [[ "$(grep -Fc -- "CASE ${name} target=gfx942 status=PASS hardware_observed=false" \
    "${success_output}")" -eq 1 ]]
done
[[ "$(grep -Fc -- ' cargo check ' "${LOG}")" -eq 5 ]]
[[ "$(grep -Fc -- ' cargo build ' "${LOG}")" -eq 1 ]]
if grep -F -- ' cargo run ' "${LOG}" >/dev/null; then
  printf '%s\n' 'compile-only matrix unexpectedly executed a host runner' >&2
  exit 1
fi
[[ -z "$(find "${TEST_ROOT}/tmp" -mindepth 1 -maxdepth 1 -print -quit)" ]]

: >"${LOG}"
gfx950_output="${TEST_ROOT}/gfx950.out"
PATH="${FAKE_BIN}:/usr/bin:/bin" \
CARGO=cargo \
ROCM_PATH="${FAKE_ROCM}" \
FE2O3_GFX950_OCML_MANIFEST="${GFX950_MANIFEST_724}" \
TMPDIR="${TEST_ROOT}/tmp" \
KERNEL_MATRIX_TEST_LOG="${LOG}" \
KERNEL_MATRIX_TEST_SYSROOT="${TEST_ROOT}/sysroot" \
  bash "${MATRIX_SCRIPT}" gfx950 >"${gfx950_output}" 2>&1 || {
    cat "${gfx950_output}" >&2
    exit 1
  }

grep -F -- \
  'kernel compile matrix: target=gfx950 mode=compile-only kernels=37 hardware_observed=false' \
  "${gfx950_output}" >/dev/null
grep -F -- \
  'MATRIX PREREQUISITE target=gfx950 exact manifest-pinned ROCm 7.2.1 or 7.2.4 Clang/LLD/device-library closure required' \
  "${gfx950_output}" >/dev/null
grep -F -- \
  'MATRIX PASS target=gfx950 compiled=37 hardware_executed=0 artifacts=temporary' \
  "${gfx950_output}" >/dev/null
grep -F -- \
  'MATRIX LIMITATION remaining gfx950 ablations, HIP comparators, hardware behavior, and numerical results are not covered' \
  "${gfx950_output}" >/dev/null
for name in \
  fp4-gemm \
  fp8-gemm \
  fp4-attention \
  fp8-attention \
  kda-decode \
  kda-decode-baseline \
  kda-prefill \
  kda-prefill-baseline \
  content-sparse-attention \
  content-sparse-attention-reciprocal-reuse \
  deepseek-sparse-attention \
  compressed-hybrid-attention \
  compressed-hybrid-attention-division-baseline \
  attnres-aggregate \
  attnres-aggregate-explicit-reuse \
  four-branch-residual \
  four-branch-residual-explicit \
  mhc-sinkhorn-mix \
  mhc-sinkhorn-mix-scalar \
  moe-route \
  moe-expert-rank \
  moe-expert-rank-expert-serial \
  combine-expert-ranks \
  speculative-transaction \
  speculative-transaction-recompute-prefix \
  qwen-ngram-gather \
  qwen-ngram-gather-reverse-probe \
  stage-gradient-shard \
  muon-update \
  muon-update-broadcast16 \
  gpt-oss-decode \
  gpt-oss-serial-router \
  gpt-oss-held-fragments \
  gpt-oss-interleaved-stores \
  gpt-oss-materialized-router \
  gpt-oss-materialized-attention \
  gpt-oss-materialized-expert; do
  [[ "$(grep -Fc -- "CASE ${name} target=gfx950 status=PASS hardware_observed=false" \
    "${gfx950_output}")" -eq 1 ]]
done
[[ "$(grep -Fc -- ' cargo check ' "${LOG}")" -eq 37 ]]
[[ "$(grep -Fc -- ' cargo build ' "${LOG}")" -eq 1 ]]
for features in \
  'kernel-moe-expert-rank\,ablation-expert-serial' \
  'kernel-speculative-transaction\,ablation-speculative-recompute-prefix' \
  'kernel-qwen-ngram-gather\,ablation-ngram-reverse-probe' \
  'kernel-muon-update\,ablation-muon-broadcast16'; do
  [[ "$(grep -Fc -- "--features ${features}" "${LOG}")" -eq 1 ]]
done
if grep -F -- ' cargo test ' "${LOG}" >/dev/null; then
  printf '%s\n' 'gfx950 compile-only matrix unexpectedly executed a hardware test' >&2
  exit 1
fi
[[ -z "$(find "${TEST_ROOT}/tmp" -mindepth 1 -maxdepth 1 -print -quit)" ]]

set +e
PATH="${FAKE_BIN}:/usr/bin:/bin" \
CARGO=cargo \
ROCM_PATH="${FAKE_ROCM}" \
FE2O3_GFX950_OCML_MANIFEST="${TEST_ROOT}/absent-gfx950-manifest" \
TMPDIR="${TEST_ROOT}/tmp" \
KERNEL_MATRIX_TEST_LOG="${LOG}" \
KERNEL_MATRIX_TEST_SYSROOT="${TEST_ROOT}/sysroot" \
  bash "${MATRIX_SCRIPT}" gfx950 >"${TEST_ROOT}/gfx950-prerequisite.out" 2>&1
gfx950_prerequisite_status=$?
set -e
[[ ${gfx950_prerequisite_status} -eq 1 ]]
grep -F -- 'gfx950 OCML manifest is not a regular non-symlink file' \
  "${TEST_ROOT}/gfx950-prerequisite.out" >/dev/null
if grep -F -- 'MATRIX PASS target=gfx950' \
  "${TEST_ROOT}/gfx950-prerequisite.out" >/dev/null; then
  printf '%s\n' 'gfx950 matrix ignored its exact ROCm closure prerequisite' >&2
  exit 1
fi
[[ -z "$(find "${TEST_ROOT}/tmp" -mindepth 1 -maxdepth 1 -print -quit)" ]]

for invocation in \
  'examples/gfx950_low_precision/run-fp4-gemm-gfx950.sh' \
  'examples/gfx950_low_precision/run-fp8-gemm-gfx950.sh' \
  'examples/gfx950_low_precision/run-attention-gfx950.sh fp4' \
  'examples/gfx950_advanced_attention/run-gfx950.sh kernel-kda-decode'; do
  read -r -a command <<<"${invocation}"
  set +e
  FE2O3_EXAMPLE_COMPILE_ONLY=invalid bash "${REPO_ROOT}/${command[0]}" \
    "${command[@]:1}" >"${TEST_ROOT}/invalid-compile-only.out" 2>&1
  invalid_status=$?
  set -e
  [[ ${invalid_status} -eq 2 ]]
  grep -F -- 'FE2O3_EXAMPLE_COMPILE_ONLY must be 0 or 1' \
    "${TEST_ROOT}/invalid-compile-only.out" >/dev/null
done

set +e
PATH="${FAKE_BIN}:/usr/bin:/bin" bash "${MATRIX_SCRIPT}" gfx000 \
  >"${TEST_ROOT}/unsupported.out" 2>&1
unsupported_status=$?
set -e
[[ "${unsupported_status}" -eq 2 ]]
grep -F -- 'unsupported target: gfx000' "${TEST_ROOT}/unsupported.out" >/dev/null

set +e
PATH="${FAKE_BIN}:/usr/bin:/bin" bash "${MATRIX_SCRIPT}" gfx942 extra \
  >"${TEST_ROOT}/extra-argument.out" 2>&1
extra_argument_status=$?
set -e
[[ "${extra_argument_status}" -eq 2 ]]
grep -F -- 'expected at most one target argument' \
  "${TEST_ROOT}/extra-argument.out" >/dev/null

printf '%s\n' 'kernel compile matrix shell tests passed'
