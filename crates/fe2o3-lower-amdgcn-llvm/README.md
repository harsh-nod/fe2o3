# fe2o3-lower-amdgcn-llvm

This crate is the bounded #145 typed AMDGPU-to-Pliron-LLVM lowering lane.
It consumes the canonical `Gfx942HandoffV2` emitted by existing scalar and
general-GEMM compiler models, admits the closed gfx942 profile below,
constructs a real owner-bound `pliron_llvm` graph, recursively verifies it, and
retains deterministic canonical graph and non-graph receipts.

## Admitted V1 subset

- gfx942, wave64, XNACK-disabled, code-object V6 policy
- AMDGPU kernel calling convention and void kernel return
- scalar `i1`, `i8`, `i16`, `i32`, `i64`, `f16`, `bf16`, and strict `f32`
- typed pointers in the admitted AMDGPU address spaces, including bounded
  local and constant arrays
- fixed four-lane vectors used by the closed GEMM MFMA profile
- constants, integer and strict-float binary arithmetic, comparisons, casts
- bounded GEP, aligned scalar and four-lane vector loads, and aligned stores
- branches, conditional branches, unreachable, and void return
- phi values lowered to typed Pliron block arguments and successor operands
- direct helper calls and the closed work-item/workgroup, barrier, FMA, square
  root, trap, and gfx942 BF16 MFMA intrinsic set
- exact function attributes, module metadata, origins, and obligations retained
  on the live graph or in a separately hashed bounded non-graph envelope
- fresh owner-borrowing graph export, LLVM serialization, and measured worker
  admission; detached Handoff values cannot mint this receipt

## Authority boundary

The complete closed general-GEMM graph is admitted structurally. Graph export,
serialization, and worker admission are inert evidence only: they grant no
artifact, publication, load, or launch authority. Production use still requires
the rustc-owned final identity join and late-machine verifier binding. Types,
operations, policies, or target profiles outside the closed gfx942 schema return
typed rejection categories.

`pliron-llvm` is built with `default-features = false`. This crate has no
`llvm-sys`, COMGR, shell compiler, shell linker, or printer-authority path.
