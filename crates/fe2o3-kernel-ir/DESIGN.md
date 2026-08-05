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
- Source-level logical launch queries use typed `IntrinsicOperation` nodes.
  Each node carries an explicit result type, while `IntrinsicKind` owns the
  canonical type, memory effects, and required target capabilities.
- Capabilities describe requirements; they do not encode a particular GPU.
  Backends decide whether a target satisfies them.

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
respectively. This change does not add intrinsic forms of memory access or
extend barrier and atomic semantics.

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

The verifier does not prove bounds, race freedom, barrier convergence,
functional correctness, or target support. Those require later analyses and
Verus proof artifacts. Keeping those concerns separate lets this verifier
remain deterministic, fast, and usable after every transformation pass.

## Extension Rules

New operations should define their SSA operands, result constraints, memory
effects, and required capabilities together. Vendor-specific operations belong
in a later extension dialect or target-lowering layer. Unknown semantics must
never be represented as unstructured strings in this core IR.
