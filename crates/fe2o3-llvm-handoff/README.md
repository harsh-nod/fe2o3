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

Schema V2 additively embeds the exact canonical V1 record and carries an
authenticated executable-module graph. Its closed typed families cover scalar
globals, intrinsic references, helper and kernel functions, calling
conventions, parameter and function attributes, basic blocks, bounded SSA
values, arithmetic, comparisons, casts, scalar memory operations, calls,
terminators, module flags, named metadata, and V1-bound origin and obligation
references. Canonical module and V2 handoff identities use separate SHA-256
domains; changing valid-looking module semantics without changing the bound
module identity fails decoding.

V2 is deliberately not arbitrary LLVM IR. Unsupported types, instructions,
intrinsics, calling conventions, metadata, attributes, malformed graphs, and
unbounded inputs are rejected with typed diagnostics. The crate remains an
inert schema: it does not parse or emit LLVM text, invoke a toolchain, link or
load code, or grant artifact, execution, or publication authority.
