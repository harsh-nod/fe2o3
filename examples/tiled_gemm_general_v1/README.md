# Dynamic strided GEMM

This example is an end-to-end safe Rust GPU kernel for

```text
C = alpha * A * B + beta * C
```

`M`, `N`, `K`, `lda`, `ldb`, `ldc`, `alpha`, and `beta` are runtime values.
One global invocation owns one physical slot in `C`; padding columns and the
rounded-up grid edge return without a memory access. Every active invocation
runs the full dynamic K loop and applies the epilogue once.

The kernel is deliberately the scalar correctness baseline. It exercises the
general compiler path for dynamic control flow and memory safety without
special-casing matrix multiplication in the compiler. LDS/MFMA scheduling is a
separate optimization of the same verified Kernel IR, not a condition for this
kernel to compile or execute.

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

## Safety boundary

The library containing the kernel uses `#![forbid(unsafe_code)]`. Ordinary Rust
slice indexing and `DisjointSlice::get_mut` remain visible to the compiler, so
generic bounds and ownership analysis can verify them. The host binary contains
the two required documented unsafe operations: loading external machine code
and launching it with an exact physical ABI.

The resulting HSACO is qualification output. Protected release publication and
artifact-currentness admission remain a separate, fail-closed pipeline.
