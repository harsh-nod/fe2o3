# gfx942 Wave/LDS V1

This document records one bounded wave64 and static-LDS reduction slice. It is
evidence for Partial parity only. It is not a claim of general collective
support, compiler refinement, or authenticated Rust-source-to-HSACO execution.

## Candidate identity

- Base commit: `4bd1be0d6325d3946075904d653222aa9c81eebd`.
- Base tree: `c813c06a8897599bd5144c82d2dc93d72394ec53`.
- Reviewed implementation and proof commit:
  `fbc06915ffe170925952ae1844dbd341fd1ed3f5`.
- Reviewed implementation and proof tree:
  `bfc745023c7100bc4eee353ac2ebf09e908fee5f`.
- Target: exactly `gfx942:xnack-`, wave64, and a `256x1x1` workgroup.

The later documentation commit does not change the implementation, hardware
runner, or proofs identified above.

## Rust contract

`Gfx942Collectives::wave64_reduce_sum_active_u32(active_flag, value)` has this
exact contract:

- `active_flag == 0` contributes zero;
- every nonzero `active_flag` contributes `value`;
- addition wraps modulo 2^32;
- all 64 physical lanes must execute the call convergently, even when a lane is
  logically inactive; and
- the result is returned to every physical lane.

The compiler emits one wave64 ballot to preserve the logical mask, selects zero
for inactive contributions, and emits a six-stage XOR reduction at offsets
32, 16, 8, 4, 2, and 1.

`Gfx942Collectives::static_lds_u32x256()` returns a private
`Gfx942StaticLdsU32x256` capability. The capability:

- represents exactly 256 `u32` slots, 1,024 bytes, alignment 4, in AMDGPU
  address space 3;
- exposes no pointer or safe constructor; and
- is not `Copy`, `Clone`, `Send`, or `Sync`.

`workgroup256_reduce_sum_active_u32` consumes that exact capability. Each of
the 256 work-items initializes its identity slot, including a zero write from
logically inactive work-items. The reduction uses offsets 128 through 1 and
exactly 18 full-workgroup acquire-release barriers: one after initialization,
two around each of eight read/write stages, and one after the final shared
read.

All three operations are unsafe because the Rust type system does not prove
physical EXEC convergence or launch identity. The backend recognizes only
trusted diagnostic identities, authenticates the exact target and launch
profile, and rejects an ordinary local substituted for the LDS capability.
Host execution panics closed.

## Compiler and LLVM evidence

The genuine Rust fixture is
`crates/rustc-codegen-fe2o3/tests/fixtures/memory-v1-compiler/src/bin/gfx942_wave_lds_v1.rs`.
It reached a verified one-kernel compiler module for `gfx942:xnack-`; the paired
`gfx1100` mutation failed closed. Finalization deliberately stopped at:

```text
requires a complete compiler FFI envelope
```

The exact compiler and dialect checks require:

- one `llvm.amdgcn.ballot.i64` call;
- six `llvm.amdgcn.ds.bpermute` calls;
- one `[256 x i32] addrspace(3)` global aligned to 4;
- exactly 18 `llvm.amdgcn.s.barrier` calls;
- `amdgpu-flat-work-group-size="256,256"`; and
- `target-features="-wavefrontsize32,+wavefrontsize64"`.

An under-aligned LDS mutation is rejected with
`UnsupportedWorkgroupMemory` before LLVM output is accepted. UI tests reject
private-field construction and calls outside an unsafe block.

## MI300X execution

The implementation/proof commit above was validated on SSH host `mi300x` with
ROCm 7.2.4 and its gfx942 GPU. Heavy Rust builds used the pinned nightly:

```sh
export PATH=/home/harsh/.rustup/toolchains/nightly-2026-04-03-x86_64-unknown-linux-gnu/bin:$PATH
export LD_LIBRARY_PATH=/home/harsh/.rustup/toolchains/nightly-2026-04-03-x86_64-unknown-linux-gnu/lib:${LD_LIBRARY_PATH:-}
export CARGO_TARGET_DIR=/home/harsh/fe2o3-targets/gfx942-wave-lds-v1
```

The focused dialect and hardware commands were:

```sh
cargo test --locked -p dialect-amdgcn --test gfx942_wave_lds_v1 -- --nocapture
cargo test --locked -p dialect-amdgcn --test gfx942_wave_lds_v1 \
  gfx942_xnack_minus_hardware_executes_masked_wave_and_lds_reductions \
  -- --ignored --exact --nocapture
```

The first command passed two tests with the hardware case ignored by default.
The second command passed the exact hardware test. Inside that test, this
command invokes the direct LLVM/LLD route:

```sh
/opt/rocm/llvm/bin/clang --target=amdgcn-amd-amdhsa \
  -mcpu=gfx942:xnack- -nogpulib wave_lds.ll -o wave_lds.hsaco
```

COMGR is not used for linking.

Before launch, the test verifies `ds_bpermute_b32`, `ds_write_b32`,
`ds_read_b32`, and `s_barrier` in assembly. `llvm-readobj` verifies
`EF_AMDGPU_FEATURE_XNACK_OFF_V4` and `.group_segment_fixed_size: 1024`. A HIP
module runner launches 256 work-items with zero and noncanonical nonzero (`7`)
activity flags. Every lane's wave reduction and workgroup reduction matched
the host oracle and printed:

```text
PASS gfx942 wave/LDS V1
```

This hardware result starts from independently constructed, verified Kernel IR.
It must not be described as execution of the genuine Rust source fixture.

## Verus evidence

`examples/verus_vecadd/verus/gfx942_wave_lds_v1.rs` models and proves:

- exact wave64 and workgroup256 extents;
- zero/nonzero logical activity and modulo-`u32` reduction;
- independence from values in logically inactive lanes;
- full physical participation despite logical inactivity;
- exact address-space-3, 1,024-byte, four-byte-aligned LDS shape;
- in-bounds stage partners and disjoint identity-slot writes;
- complete participation in all 18 barrier rounds; and
- legal initialized LDS reads after the barrier transfer.

The required run was:

```sh
VERUS=/home/harsh/.local/bin/verus VERUS_TIMEOUT_SECONDS=120 \
  examples/verus_vecadd/run-verus.sh --require
```

It passed six positive proof harnesses and rejected all 26 adversarial
fixtures. The two new mutations reject wave63 as wave64 and reject a barrier
round with one missing participant. No `admit`, `assume(false)`, or
`external_body` is present in the exact proof file.

## Explicit boundary

The following join is missing:

```text
genuine Rust source
  -> verified compiler module / Kernel IR       established
  -> authenticated general Worker V2 envelope  missing for this profile
  -> finalized and admitted HSACO               missing for this profile
  -> GPU execution                              therefore not source-proven
```

Separately, verified Kernel IR to LLVM/LLD HSACO and numerical MI300X execution
is established by the dialect hardware test. Neither the LLVM/assembly checks
nor the Verus model prove compiler correctness. A future candidate must join
the genuine source artifact to the direct LLVM/LLD finalization path and bind
the Verus result to that exact artifact before claiming source-to-HSACO or
machine-code refinement.

The remaining parity gaps include broader element types and operations,
wave32 and target breadth, partial physical EXEC masks, general launch sizes,
dynamic LDS admission, scans in this exact proof/hardware lane, production
artifact authentication, and compiler refinement.
