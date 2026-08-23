# Dynamic strided GEMM

This example is an end-to-end safe Rust GPU kernel for

```text
C = alpha * A * B + beta * C
```

`M`, `N`, `K`, `lda`, `ldb`, `ldc`, `alpha`, and `beta` are runtime values.
Each wave64 workgroup owns one 16x16 output tile and executes
`V_MFMA_F32_16X16X16_BF16` for every 16-element K phase. Checked edge loads
contribute BF16 zero; checked tiled output witnesses suppress stores outside
logical M and N. Every active output applies the dynamic alpha/beta epilogue
once.

The matrix instruction is exposed through the target-neutral `DeviceMatrix`
capability. Bounds, uniformity, convergence, ranked indexing, and disjoint
output ownership are ordinary compiler analyses shared with every other kernel;
none of those passes recognizes GEMM or grants it a special case.

## Run on gfx942

From this directory:

```bash
./run-gfx942.sh
```

The script performs the complete qualification flow:

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

It runs packed, fully strided/edge, multi-workgroup dynamic-K, and zero-K epilogue
cases against an independent CPU reference. Temporary AMD Cargo output, LLVM,
and object files are deleted on exit. The final HSACO is retained under
`target/fe2o3-gfx942/`.

The generated gfx942 disassembly contains
`v_mfma_f32_16x16x16_bf16`. Run `./run-benchmark.sh` for an event-timed
comparison with the directly equivalent HIP kernel.

## Safety boundary

The library containing the kernel uses `#![forbid(unsafe_code)]`. Ordinary Rust
slice indexing and `DisjointSlice::get_mut` remain visible to the compiler, so
generic bounds and ownership analysis can verify them. The host binary contains
the two required documented unsafe operations: loading external machine code
and launching it with an exact physical ABI.

The resulting HSACO is qualification output. Protected release publication and
artifact-currentness admission remain a separate, fail-closed pipeline.
