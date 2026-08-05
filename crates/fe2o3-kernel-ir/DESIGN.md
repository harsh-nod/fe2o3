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
- Capabilities describe requirements; they do not encode a particular GPU.
  Backends decide whether a target satisfies them.

## Verification Boundary

`verify_module` checks structural and local semantic well-formedness. It
checks identities, signatures, SSA definitions and dominance, CFG targets and
block arguments, operation/result types, memory access metadata, barriers,
atomics, launch domains, and capability metadata. Diagnostics are sorted by
location and code for deterministic output.

The verifier does not prove bounds, race freedom, barrier convergence,
functional correctness, or target support. Those require later analyses and
Verus proof artifacts. Keeping those concerns separate lets this verifier
remain deterministic, fast, and usable after every transformation pass.

## Extension Rules

New operations should define their SSA operands, result constraints, memory
effects, and required capabilities together. Vendor-specific operations belong
in a later extension dialect or target-lowering layer. Unknown semantics must
never be represented as unstructured strings in this core IR.
