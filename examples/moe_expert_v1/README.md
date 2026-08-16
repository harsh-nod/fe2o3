# Exact host-scheduled MoE expert compute V1

This standalone crate starts the bounded expert-compute slice for the public
`T8/E4/K2/C4` router. It consumes the router's route IDs, stable permutation,
inverse map, expert offsets, capacity, and drop sentinel. The routing ABI does
not produce gating weights, so this slice makes them an explicit finite,
nonnegative input in token-major/rank-minor route-ID order.

The host compacts accepted token activations into four zero-padded `16x16`
BF16 tiles, schedules four independent exact `16x16x16` BF16/F32 GEMMs, packs
active expert rows back into routing-slot order, and combines each token's two
route rows in rank order. Dropped routes contribute zero without
renormalization.

`src/kernel.rs` contains ordinary attributed Rust `#[kernel]` definitions for
the expert GEMM and deterministic combine. It contains no `macro_rules!`
kernel facade. These are exact source definitions only: authenticated
MIR-to-Kernel-IR profiles, upstream LLVM/LLD finalization, typed host/runtime
authority, protected gfx942 execution, source/model-to-machine refinement,
generalized memory safety or race freedom, and numerical correctness remain
open and must fail closed.
