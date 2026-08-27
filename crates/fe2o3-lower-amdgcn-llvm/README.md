# fe2o3-lower-amdgcn-llvm

This crate is the bounded typed AMDGPU-to-Pliron-LLVM lowering lane. It consumes
canonical `Gfx942HandoffV2`, admits the workload-neutral gfx942 profile below,
constructs a real owner-bound `pliron_llvm` graph, recursively verifies it, and
retains deterministic canonical graph and non-graph receipts.

## Admitted V1 subset

- gfx942, wave64, XNACK-disabled, code-object V6 policy
- AMDGPU kernel calling convention and void kernel return
- scalar `i1`, `i8`, `i16`, `i32`, `i64`, and strict `f32`
- typed pointers in the admitted AMDGPU address spaces, including bounded
  local and constant arrays
- bounded fixed vectors; four lanes remain mandatory only for the four-lane
  load and the exact gfx942 BF16 MFMA intrinsic
- constants, integer and strict-float binary arithmetic, comparisons, casts
- bounded GEP, aligned scalar and four-lane vector loads, and aligned stores
- branches, conditional branches, unreachable, and void return
- phi values lowered to typed Pliron block arguments and successor operands
- direct helper calls and the closed work-item/workgroup, barrier, FMA, square
  root, trap, and gfx942 BF16 MFMA intrinsic set
- exact function attributes, module metadata, origins, and obligations retained
  on the live graph or in a separately hashed bounded non-graph envelope
- fresh owner-borrowing graph export, LLVM serialization, and exact LLVM/LLD
  build-policy admission at the worker boundary; detached Handoff values
  cannot mint this receipt, and this admission does not authenticate a worker
  measurement

## Authority boundary

The admitted graph is classified only by structural features: scalar
straight-line code, scalar control flow, or vector/local-memory operations.
Graph export, serialization, and worker admission are inert evidence only: they
grant no artifact, publication, load, or launch authority. Production use still
requires the rustc-owned final identity join and late-machine verifier binding.
Types, operations, policies, or target profiles outside the bounded gfx942
schema return typed rejection categories.

`pliron-llvm` is built with `default-features = false`. This crate has no
`llvm-sys`, COMGR, shell compiler, shell linker, or printer-authority path.
