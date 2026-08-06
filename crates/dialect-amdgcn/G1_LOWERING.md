# AMDGPU G1 lowering

`lower_kernel_to_llvm_ir` is a target-neutral experiment, not the production
fe2o3 emitter. It verifies the complete kernel IR module before selecting one
exact `KernelId`, then performs a fail-closed AMDGPU-specific preflight before
emitting text.

G1 accepts:

- a 1D launch domain with either a static or dynamic total extent;
- a required static workgroup size `(x, 1, 1)` with `x <= 1024`;
- scalar and global slice kernel parameters;
- global-X invocation index, slice length/data, constants, selected scalar
  arithmetic and casts, integer comparisons, GEP, aligned global load/store,
  and optional volatile memory access;
- conditional and ordinary branches, including scalar, global-pointer, and
  global-slice block arguments materialized as LLVM phi nodes;
- global and workgroup pointer memory access, explicit static or dynamic LDS,
  scoped fences, and convergence-bearing workgroup barriers;
- workgroup-memory/barrier capabilities and exact wave32 or wave64 function
  attributes;
- void return and unreachable.

The global index is computed in `i64` from AMDGPU workitem and workgroup IDs
using the declared workgroup X size. The total launch extent remains a host
launch contract and is intentionally absent from the LLVM IR. G1 does not add
`inbounds` to GEPs because the source IR does not carry that assertion.

Workgroup barriers lower to release/acquire fences around the convergent
`llvm.amdgcn.s.barrier` intrinsic. A selected address-space subset may lower to
a conservatively stronger LLVM fence because LLVM fences do not carry the
Kernel IR address-space set.

G1 rejects declarations, non-void entries, 2D/3D domains, missing or oversized
workgroup sizes, unsupported capabilities, unsafe unquoted symbols, entry or
predecessorless block arguments, duplicate edges into a block with arguments,
private/constant/generic memory, unsupported scalar types, legacy barriers
without convergence evidence, scoped atomics, ambiguous workgroup `Alloca`,
and every operation not explicitly listed above. It does not select a GPU
processor, invoke LLVM, produce an artifact, or grant launch authority.

## Inert compiler-module construction

`lower_compiler_module_to_llvm_ir` reuses the same structured preflight and body
lowering to produce one textual LLVM module from one verified kernel-IR
`Module`. It is additive and is not wired into the production emitter.

The module path emits kernel entries in canonical kernel-ID order, emits every
non-entry definition and external declaration once in canonical function-ID
order, and preserves verified block and operation order inside each definition.
AMDGPU intrinsic declarations are deduplicated across entries. Every kernel has
its own workgroup attribute and metadata node. The complete module is
preflighted before the private output string is returned.

This slice supports calls between ordinary device functions and external
declarations with void or one scalar/global-or-workgroup-pointer result. Kernel
slice ABIs remain supported only at kernel entries. Calls to kernel entries,
multi-result and slice helper ABIs, duplicate output symbols, multiple exports
of one entry definition, and kernel-context intrinsics, LDS, barriers, or wave
operations in shared helpers fail closed.

The result is textual LLVM IR only. It is not bitcode, a linked module, a code
object, compiler provenance, or publication/load/launch authority. The bounded
rustc-facing wrapper in `kernel_ir_codegen.rs` additionally limits all graph
dimensions and the final text size before this path can be wired to collection.
