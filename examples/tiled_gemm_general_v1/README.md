# General tiled GEMM safe-source fixture

This standalone crate contains the positive ordinary-Rust kernel source for
issue #138. The kernel accepts dynamic `M`, `N`, `K`, `lda`, `ldb`, and `ldc`,
uses 16x16x16 BF16/F32 phases across a 2D grid, zero-fills guarded operand
tails, carries its accumulator across all phases, and applies `alpha`/`beta`
only to valid output coordinates.

The kernel crate uses `#![forbid(unsafe_code)]`. Compiler-only operations are
exposed through the sealed linear `Gfx942TiledGemmWave64V1` typestate in
`fe2o3-device`. That capability hides wave identity, two separate XOR4 LDS
tiles, publish/reuse barriers, MFMA state, the accumulator, phase epochs, and
disjoint output addressing. Its only phase sequence is:

```text
Ready -> Staged -> Published -> Consumed -> Ready
```

## Current boundary

This is a compile-tested source contract, not GPU execution authority. The
safe compiler operations are panic stubs under host rustc and have no current
MIR-to-Kernel-IR or LLVM lowering. A proof-required build must reject this
kernel before emitting an artifact until general source import, semantic proof
discharge, gfx942 lowering, protected publication, and runtime launch are all
joined. The existing exact `tiled_gemm_v1` Slice 1 source and identities are
unchanged.
