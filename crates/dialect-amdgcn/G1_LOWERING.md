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
- conditional and ordinary branches, typed integer switches, reducible loops,
  and scalar, global-pointer, and global-slice block arguments materialized as
  LLVM phi nodes;
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

The convergence marker is necessary but not sufficient for emission. G1
independently accepts barriers only on an acyclic unconditional chain from the
kernel entry. It rejects barriers reached through a conditional edge, in a
cycle, in an unreachable block, or in a function containing an unsummarized
call. This deliberately does not infer uniformity from branch value types;
future accepted control flow must arrive with separately bound analysis or
proof evidence.

Static LDS lowers to an internal address-space-3 array. The one dynamic form is
an external zero-length address-space-3 array, matching LLVM AMDGPU dynamic LDS
semantics. Both forms require explicit operation-covering capability
declarations before emission. Element alignment must be at least its AMDGPU
natural alignment, and the declaration sequence must remain representable in
the 32-bit LDS address space. Exact processor LDS capacity and dynamic launch
bytes remain target and host-admission obligations; this textual lowering does
not grant them.

Parameter-carrying critical edges are split into deterministic synthetic LLVM
blocks. Multiple logical edges from one source to one parameterized target are
also split, so every LLVM phi incoming has a distinct physical predecessor.
Natural backedges remain direct when they are not critical. Before emission,
the backend computes dominators, removes dominance backedges, and rejects any
remaining cycle as irreducible control flow.

G1 rejects declarations, non-void entries, 2D/3D domains, missing or oversized
workgroup sizes, unsupported capabilities, unsafe unquoted symbols, entry or
predecessorless block arguments, irreducible control flow,
private/constant/generic memory, unsupported scalar types, legacy barriers
without convergence evidence, unproved barrier control flow, missing LDS or
barrier capability declarations, under-aligned or unaddressable LDS, scoped
atomics, ambiguous workgroup `Alloca`, and every operation not explicitly
listed above. It does not select a GPU processor, invoke LLVM, produce an
artifact, or grant launch authority.

## Inert compiler-module construction

`lower_compiler_module_to_llvm_ir` reuses the same structured preflight and body
lowering to produce one textual LLVM module from one verified kernel-IR
`Module`. The opt-in `kernel-ir-worker-v2` producer invokes this path after
binding its fixed G1 launch and target wave contracts, then publishes the text
only as an attempt-scoped inert compiler handoff. The legacy default emitter
and the specialized `kernel-ir-v1` path remain separate.

The module path emits kernel entries in canonical kernel-ID order, emits every
non-entry definition and external declaration once in canonical function-ID
order, and preserves verified block and operation order inside each definition.
Roles are explicit in kernel IR: `InternalHelper` definitions use LLVM internal
linkage, `DeviceFfiExport` definitions remain externally visible,
`ExternalImport` functions become declarations, and only `KernelEntry`
functions referenced by kernel records use the AMDGPU kernel calling
convention. Visibility is never inferred from reachability or symbol spelling.
AMDGPU intrinsic declarations are deduplicated across entries. Every kernel has
its own workgroup attribute and metadata node.

Before body lowering, the module path constructs a bounded helper call graph
and iterative SCC decomposition. Each kernel's exact wave32/wave64 mode is
propagated through its reachable helper SCCs. A helper SCC reachable from both
modes is rejected rather than cloned. A helper without a local claim inherits
the unique effective caller mode, while a non-kernel-reachable root SCC must
declare an exact mode before that mode can propagate to its callees. The
effective mode is attached to helper definitions, so LLVM lowers their
branches, phi nodes, and control masks under the same wave contract as callers.

This slice supports calls between ordinary device functions and external
declarations with void or one scalar/global-or-workgroup-pointer result. Kernel
slice ABIs remain supported only at kernel entries. Calls to kernel entries,
multi-result and slice helper ABIs, duplicate output symbols, multiple exports
of one entry definition, and kernel-context intrinsics, LDS, barriers, or wave
operations in shared helpers fail closed.

The result is textual LLVM IR only. Emission uses a private 16 MiB
capacity-limited writer; crossing the limit returns an error and exposes no
partial text. It is not bitcode, a linked module, a code object, compiler
provenance, or publication/load/launch authority. The emitted text binds only
the AMDGPU target triple. A target data layout, exact processor, and code-object
version are deliberately unbound and remain required blockers before artifact
construction. The ignored `gfx1151` toolchain probe demonstrates that an
external Clang invocation can compile and disassemble current fixtures; that
probe is test evidence, not target binding or production integration. The
bounded rustc-facing wrapper in `kernel_ir_codegen.rs` additionally limits all
input graph dimensions before this path can be wired to collection.

`lower_device_module_to_gfx942_llvm_ir` admits the same verified helper and
device-export subset without inventing a kernel entry. Every definition must
carry an exact wave32 or wave64 mode. This is the backend test boundary used by
the branching-fill, integer-match, and nested-loop gfx942 goldens. Those tests
use manually transcribed kernel-IR modules corresponding to the reviewed MIR
fixtures. They assert only that the source fixtures remain identifiable; there
is no structured adapter or content hash tying the transcription to MIR. The
MIR-to-kernel-IR adapter remains frontend work and cannot be added in this lane
without changing the shared workspace dependency graph.

The crate has no in-process LLVM target-machine/code-object API. Consequently
these control-flow tests stop at exact, target-bound LLVM text. Code-object
construction, object validation, Worker transport, and hardware dispatch remain
separate wiring obligations; no COMGR or command-line tool is used here.
