# fe2o3 LLVM handoff

`fe2o3-llvm-handoff` defines the canonical, inert record passed from reviewed
AMDGPU lowering to an isolated LLVM worker. It does not depend on Pliron, LLVM,
LLD, COMGR, HIP, HSA, or rustc-private crates, and it does not emit LLVM IR or
grant artifact, execution, or publication authority.

The first schema is intentionally closed around `gfx942:xnack-`, wave64,
AMDHSA code-object V6 kernel entries. It carries:

- the exact target triple, data layout, CPU, feature states, optimization,
  relocation, code-model, and code-object policy;
- bounded kernel signatures, AMDGPU kernel calling convention, parameter
  attributes, and reviewed function attributes;
- reviewed module flags and named metadata plus exact device-library content
  identities;
- semantic, schedule, and target-plan identities, stable source origins, and
  machine-boundary obligations; and
- a versioned canonical binary encoding and domain-separated SHA-256 handoff
  identity.

Order-insensitive collections are sorted during checked construction. Parameter
order remains significant. Duplicate, conflicting, oversized, dangling,
unknown, and noncanonical values fail closed.

This slice does not model arbitrary helper bodies, globals, instruction graphs,
intrinsics, or opaque LLVM strings. A later schema must add each such semantic
family as typed data before the worker may accept it.
