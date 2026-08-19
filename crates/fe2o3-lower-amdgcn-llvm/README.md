# fe2o3-lower-amdgcn-llvm

This crate is the bounded #145 typed AMDGPU-to-Pliron-LLVM lowering lane.
It consumes the canonical `Gfx942HandoffV2` emitted by existing scalar and
general-GEMM compiler models, admits a closed scalar subset, constructs a real
owner-bound `pliron_llvm` graph, recursively verifies it, and retains the exact
typed source in a deterministic canonical receipt.

## Admitted V1 subset

- gfx942, wave64, XNACK-disabled, code-object V6 policy
- AMDGPU kernel calling convention and void kernel return
- scalar `i1`, `i8`, `i16`, `i32`, `i64`, and strict `f32`
- global-address-space `f32` pointers
- constants, integer and strict-float binary arithmetic, comparisons, casts
- one-index scalar GEP, aligned scalar load, and aligned scalar store
- branches, conditional branches, unreachable, and void return
- phi values lowered to typed Pliron block arguments and successor operands
- exact function attributes, module metadata, origins, and obligations retained
  by the canonical typed source rather than unchecked Pliron strings

## Explicit V1 gaps

The full general-GEMM graph is not admitted yet. LDS and constant globals,
fixed vectors, vector loads, direct calls, AMDGPU intrinsics including barrier
and MFMA, helper functions, source spans, device libraries, `f16`, `bf16`, and
`f64` return stable typed rejection categories. No path in this crate compiles,
links, loads, publishes, or executes an artifact.

`pliron-llvm` is built with `default-features = false`. This crate has no
`llvm-sys`, COMGR, shell compiler, shell linker, or printer-authority path.
