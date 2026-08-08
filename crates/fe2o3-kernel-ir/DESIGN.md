# fe2o3 Kernel IR

This crate is the target-neutral semantic boundary between Rust kernel
extraction and vendor lowering. It has no dependency on rustc, LLVM, ROCm,
CUDA, or a serialization framework.

## Invariants

- `Module` owns stable function and kernel identities. A `Kernel` names a
  defined entry function and adds launch-domain and capability contracts.
- Function bodies use SSA values and explicit basic-block parameters. There
  are no implicit fallthrough edges or implicit phi nodes.
- Pointer and slice types retain address space and access mode. Memory
  operations repeat address-space and alignment metadata so verification and
  lowering do not need to infer target-sensitive facts.
- Barriers and atomics carry explicit scope and ordering information. Their
  memory effects are queryable without inspecting a target backend.
- `Fence` separates memory ordering from execution synchronization.
  `WorkgroupBarrier` additionally requires a uniform-workgroup convergence
  claim. The verifier checks the claim's scope; uniformity analysis or a proof
  artifact must establish that the claim is true.
- `WorkgroupMemory` distinguishes fixed element counts from the one dynamic LDS
  base allowed in a function. Dynamic LDS requires both the dynamic and base
  workgroup-memory capabilities. The legacy `Alloca` form remains
  representable.
- `WaveWidth` expresses an exact wave32 or wave64 lowering requirement without
  overloading target-neutral subgroup sizes.
- Source-level logical launch queries use typed `IntrinsicOperation` nodes.
  Each node carries an explicit result type, while `IntrinsicKind` owns the
  canonical type, memory effects, and required target capabilities.
- Capabilities describe requirements; they do not encode a particular GPU.
  Synchronization over workgroup memory derives `WorkgroupMemory` in addition
  to its execution capability. `Module::effective_capabilities` and
  `Function::effective_capabilities` close explicit declarations over all
  operation-derived requirements. Backends decide whether a target satisfies
  that closure.

## Initial Intrinsic Set

The first portable source subset contains `global_id_1d` and
`launch_extent_1d`. The former has the single canonical IR representation
`IntrinsicKind::InvocationIndex { kind: Global, axis: X }`; there is no
parallel invocation-index operation. Both queries return the target's `Index`
type and are pure: querying launch geometry does not read program-visible
memory. An intrinsic axis must belong to the kernel's launch domain.

Core launch queries derive no optional target capabilities. A target that
cannot provide invocation coordinates and launch geometry cannot lower the
core kernel IR, so treating those queries as optional features would only add
redundant capability declarations. `verify_module_with_capabilities` remains
available for operations and declarations that do require optional target
features. A backend may implement launch queries using its physical
workgroup, local-invocation, and dispatch geometry, but those details do not
enter this IR.

Global memory semantics continue to use the existing typed `Load` and `Store`
operations. Their effect summaries report `Read(Global)` and `Write(Global)`
respectively. Synchronization and LDS use dedicated operations rather than
intrinsic forms of memory access.

## Verification Boundary

`verify_module` checks structural and local semantic well-formedness. It
checks identities, signatures, SSA definitions and dominance, CFG targets and
block arguments, operation/result types, memory access metadata, barriers,
atomics, launch domains, and capability metadata. Diagnostics are sorted by
location and code for deterministic output.

Intrinsic verification additionally checks the explicit result type and
result arity against canonical metadata. Kernel verification rejects intrinsic
axes outside the declared launch domain. Callers that know a target's
capability set can use `verify_module_with_capabilities` to reject unsupported
requirements.

Synchronization verification rejects relaxed fences/barriers, private or
constant synchronization, scopes that cannot observe the selected address
space, malformed atomic orderings and operands, unsupported atomic widths,
multiple dynamic LDS bases, and inconsistent wave-width requirements. Target
atomic capabilities are subsuming: support at a wider legal scope authorizes a
narrower requested scope with the same width and address space.

The uniform-workgroup marker remains an explicit claim rather than an inferred
property. A backend must bind it to accepted control-flow analysis or proof
evidence before emitting a convergent target operation. The first AMDGPU
lowering accepts only barriers on an acyclic unconditional entry chain and
rejects conditional, cyclic, unreachable, and unsummarized interprocedural
placements.

The verifier does not prove bounds, race freedom, the truth of a barrier's
convergence claim,
functional correctness, or target support. Those require later analyses and
Verus proof artifacts. Keeping those concerns separate lets this verifier
remain deterministic, fast, and usable after every transformation pass.

## Extension Rules

New operations should define their SSA operands, result constraints, memory
effects, and required capabilities together. Vendor-specific operations belong
in a later extension dialect or target-lowering layer. Unknown semantics must
never be represented as unstructured strings in this core IR.

### Semantic operation boundary

The semantic_operations module owns a versioned identity registry and the
SemanticOperation contract hook. Its stable families cover memory intrinsics,
collectives, debugging, launch queries and constraints, and matrix operations
without selecting a vendor dialect.

A family identity is not executable authority. Adding an operation requires:

1. A closed family-local opcode and strongly typed payload.
2. Explicit operands, result types, memory effects, and capabilities.
3. Payload-specific structural and type verification.
4. A closed OperationKind admission path.
5. A new module wire version if the operation must be serialized.
6. An explicit backend lowering and target-capability check.

Unknown versions, families, and family-local opcodes fail closed. The existing
launch intrinsics implement the contract to exercise the boundary while
preserving their original representation and behavior.

## Canonical Wire Format

`Module` has bounded deterministic V1 and V2 binary representations documented in
[`WIRE_FORMAT.md`](WIRE_FORMAT.md). The format covers every stored IR node,
operation, terminator, type, capability, and enum variant currently reachable
from `Module`. Derived query results such as `IntrinsicMetadata`,
`MemoryEffect`, and `MemoryEffectSummary` are intentionally recomputed rather
than serialized.

V1 remains byte-for-byte frozen. V2 adds tags for fences, convergent workgroup
barriers, explicit workgroup memory, exact wave widths, and typed integer
switches. V2 integer-switch cases use typed constants and are strictly
increasing, making duplicate and reordered case encodings noncanonical. Its
decoder accepts both versions so readers can migrate before writers. Exact
floating-point contracts use closed, verifier-reserved intrinsic declarations
and ordinary call records, so the frozen operation tags remain unchanged. Wire
decoding is a parser boundary, not a semantic trust decision. A decoded module
can still contain undefined SSA values, invalid types, missing terminators, bad
barriers, or other frontend errors. Every consumer must run `verify_module` or
`verify_module_with_capabilities` before lowering or executing decoded IR.
