# Tiled GEMM V1 MI300X Observation

This record captures one non-authoritative hardware observation of the exact
direct-global Tiled GEMM V1 profile. It is not a protected receipt and grants no
compiler, publication, loading, launch, verification, or parity authority.

## Bound inputs

The run used:

| Input | Observed value |
|---|---|
| UTC date | `2026-08-14` |
| SSH host label | `mi300x` |
| physical target required by the harness | `gfx942:xnack-` |
| repository commit under test | `6d35aea57b13ac24cdb05147da3b34bc410b16f4` |
| repository tree | `ee865d36a5de4eb0264e3a69c5fd48af427ee3bf` |
| artifact size | `6672` bytes |
| artifact SHA-256 | `681077be1108c57d9d887f94afdd0ec3700ed2c86d73e66d2b229d6b418d0c66` |
| artifact export | `tiled_gemm_v1` |
| observed LLVM tool | `AMD LLVM version 22.0.0git` |
| `llvm-objdump` SHA-256 | `e5bf27bb6ba178b4de94ac0d5da760b628672cd00d2ffeb40a4372fa6ad25140` |
| test executable SHA-256 | `cffd8cafcd8ee4bcfd361f26dc7b5f297c85e6d52caed07420cc70f2313e986a` |

The canonical LLVM tool path during the run was
`/opt/rocm-7.2.4/lib/llvm/bin/llvm-objdump`. The test first required exact COV6,
`gfx942:xnack-`, WG64, wave64, 320-byte kernarg, zero-LDS, one-entry metadata and
descriptor facts. Its bounded disassembly policy required exactly one retained
`v_mfma_f32_16x16x16_bf16`, at least one global load and global store, and no
admitted branch, call, atomic, flat, buffer, image, scratch, or LDS instruction
forms.

## Command

The passing command was equivalent to the following. The absolute toolchain,
artifact, and target-directory paths identify this observation; they are not a
portable installation recipe.

```bash
FE2O3_RUN_GFX942_TILED_GEMM_V1_HARDWARE=1 \
FE2O3_GFX942_TILED_GEMM_V1_HSACO=/home/harsh/fe2o3-tiled-gemm-f494.hsaco \
FE2O3_GFX942_TILED_GEMM_V1_SHA256=681077be1108c57d9d887f94afdd0ec3700ed2c86d73e66d2b229d6b418d0c66 \
FE2O3_GFX942_TILED_GEMM_V1_KERNEL_SYMBOL=tiled_gemm_v1 \
FE2O3_LLVM_OBJDUMP=/opt/rocm-7.2.4/lib/llvm/bin/llvm-objdump \
FE2O3_LLVM_OBJDUMP_SHA256=e5bf27bb6ba178b4de94ac0d5da760b628672cd00d2ffeb40a4372fa6ad25140 \
cargo test --locked -p fe2o3-hsa-runtime --features hardware-test-hooks \
  --test tiled_gemm_v1_hardware \
  gfx942_tiled_gemm_v1_one_tile_raw_hardware_evidence \
  -- --ignored --exact --nocapture
```

Result:

```text
test gfx942_tiled_gemm_v1_one_tile_raw_hardware_evidence ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 40.92s
```

The test compared all 256 output elements bitwise with an independent dyadic
host oracle. It also checked that A, B, and C remained bitwise unchanged, that
prefix and suffix canaries around all four allocations remained intact, that
the dispatch completed synchronously, and that the exact executable was
unloaded.

The compact, non-authoritative console receipt is committed at
[`docs/receipts/tiled-gemm-v1-mi300x-2026-08-14.txt`](receipts/tiled-gemm-v1-mi300x-2026-08-14.txt).

## Exclusions

The supplied artifact is not committed and its producer is not authenticated by
this observation. The run does not show that the Rust source produced those
bytes. The admitted tile performs direct global loads and stores and has zero
LDS; it is not the production LDS-tiled, multi-phase GEMM described by the
lesson. The finite canaries do not detect beyond-guard accesses,
value-preserving writes, same-value races, or output-inert reads.

This observation therefore does not establish source-to-HSACO causality,
compiler or machine-code refinement, general GEMM shapes, edge handling,
protected publication/load/launch authority, memory safety, race freedom, or
the floating-point semantics of all BF16 inputs.
