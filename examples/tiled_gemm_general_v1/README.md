# Dynamic strided GEMM

This example is an attributed safe Rust GPU kernel candidate for

```text
C = alpha * A * B + beta * C
```

`M`, `N`, `K`, `lda`, `ldb`, `ldc`, `alpha`, and `beta` are runtime values.
Each wave64 workgroup owns one 16x16 output tile and executes
`V_MFMA_F32_16X16X16_BF16` for every 16-element K phase. Checked edge loads
contribute BF16 zero; checked tiled output witnesses suppress stores outside
logical M and N. Every active output applies the dynamic alpha/beta epilogue
once.

The K loop keeps current and next MFMA operand fragments live as a two-buffer
register pipeline and performs one speculative, zero-filled prefetch. This is
distinct from the target-neutral `kernel.pipeline` PLIRON protocol, which
verifies shared workgroup-storage ring lifecycles. The source frontend does not
yet synthesize that workgroup protocol from this Rust loop.

The matrix instruction is exposed through the target-neutral `DeviceMatrix`
capability. Bounds, uniformity, convergence, ranked indexing, and disjoint
output ownership are ordinary compiler analyses shared with every other kernel;
none of those passes recognizes GEMM or grants it a special case.

## Run on gfx942

From this directory:

```bash
./run-gfx942.sh
```

The script requests the complete qualification flow:

```text
safe Rust
  -> semantic MIR
  -> ranked PLIRON verification
  -> Kernel IR
  -> formal memory admission
  -> gfx942 LLVM
  -> HSACO
  -> fe2o3-host launch
```

The current compiler stops before code generation with `FE2O3-RACE-002`
because the generic checked-tiled source capability is not yet joined to the
dynamic-launch race proof. This is a fail-closed source-to-PLIRON handoff gap,
not pipeline-protocol authority. The ordinary Rust, UI, and independent CPU
reference suites remain runnable with `cargo test`.

Once that handoff is implemented, qualification must also confirm that gfx942
disassembly contains `v_mfma_f32_16x16x16_bf16` before performance claims are
made.

## Safety boundary

The library containing the kernel uses `#![forbid(unsafe_code)]`. Ordinary Rust
slice indexing and `DisjointSlice::get_mut` remain visible to the compiler, so
generic bounds and ownership analysis can verify them. The host binary contains
the two required documented unsafe operations: loading external machine code
and launching it with an exact physical ABI.

Any resulting HSACO is qualification output. Protected release publication
and artifact-currentness admission remain a separate, fail-closed pipeline.
