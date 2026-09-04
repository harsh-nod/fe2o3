# General Optimizing Compiler Wave 1

Status: implemented compiler infrastructure. This is not a claim that fe2o3 is
a general-purpose or formally verified compiler.

## Production flow

```text
Rust/rustc semantic MIR
  -> semantic ownership and bounded generic SSA planning
  -> ranked recipe projection
  -> checked recipe normalization
  -> immutable nine-stage Pliron verification
  -> verified target-neutral Kernel IR
  -> composed formal memory admission
  -> AMDGPU target binding
  -> fresh-session Pliron optimization
  -> verified canonical Kernel IR V10 or V11 snapshot
  -> AMDGPU LLVM lowering
```

The ranked verifier remains an analysis boundary. No pass mutates live ranked
Pliron between its nine analyses. Position-preserving normalization runs on the
owned recipe before graph construction, and target optimization runs only after
formal memory admission and target binding.

`ProductionSemanticSsaOwnerV1` is the production custody boundary between
semantic ownership and ranked projection. It runs the shared
`fe2o3-mir-model` planner for every semantic function and retains the exact
source owner, per-function plan identities, resource reports, and replay
policy. The `SsaSemanticMirStage` typestate prevents every production entry
from bypassing this step.

This is mixed SSA, not a requirement that every Rust local become a value.
Promotable locals receive sparse block parameters and edge arguments;
address-observable locals remain memory. The planner uses compact liveness
bitsets plus inverse definition rows and generation-marked sparse IDF
worklists. A linear semantic-variable lookup maps into a compact
promoted-variable domain, and all block bitsets and value-resolution arrays use
that compact index. The planner and semantic adapter charge checked limits for
input, output, storage, and work, including conservative bounds for adapter
scratch and partial-state cloning. Those deterministic bounds participate in
the production identity. Undefined uses and transports are rejected instead
of manufacturing an `undef` value.

The semantic adapter also certifies Rust-specific details that a generic CFG
planner cannot infer:

- a path-sensitive partial-move analysis tracks fixed fields, constant array
  indices, and enum payload fields through joins and loop fixed points;
- exact reinitialization clears only the reinitialized path, while parent/child
  reuse, union fields, dynamic indices, and missing type evidence fail closed;
- a borrow is transparent only when its reference has exactly one direct use
  in the exact accepted argument of a registered compiler intrinsic; and
- an implicit entry definition is permitted only for the exact ambient,
  inhabited zero-sized `WorkgroupLdsScope` temporary whose producer rustc may
  erase and whose uses are all certified transparent scope consumers.

The ambient exception is recorded in the plan identity and is checked again by
KIR lowering; it is not a general zero-sized-value synthesis rule. KIR replays
the semantic plan, materializes parameters only at pruned merge sites, and
references dominating definitions directly elsewhere. Compiler-issued
capabilities retain their exact type, index-space, disjointness, availability,
and pointer metadata through SSA edges. Exact whole-value aliases and matching
enum payload paths, including nested `Option`, `Result`, and `ControlFlow`
wrappers emitted by rustc desugaring, are resolved with bounded cycle checks;
ambiguous or type-changing paths fail closed. Exact SSA-keyed enum facts are
renamed across certified edge arguments, intersected at joins, invalidated by
unknown definitions, and bounded independently for work and storage.

## Existing normalization and legalization

The production frontend and target-neutral lowerer already provide:

- unreachable semantic-block pruning and deterministic CFG construction;
- switch, assertion, and guard expansion;
- generic liveness, dominance, pruned merge placement, and edge-argument SSA
  planning before ranked projection;
- loop-induction recognition and an index-only Ranked V1 block-argument
  overlay;
- by-value aggregate ABI decomposition;
- explicit views, address spaces, bounds guards, atomics, barriers, and
  execution layout;
- explicit GPU intrinsic, wave, tensor-layout, and MFMA operations; and
- construction into the closed set of registered production dialects.

The ranked preverification pipeline adds only position-preserving forms that
are safe before proof analysis: canonical empty CFG/SSA edge spellings,
explicit global memory space for legacy views, checked index constant folding,
and low-bit constant folding for unsigned index casts. Every rewrite has an
independent replay validator and preserves block count, operation count,
coordinates, result identities, and bounded tree work.

General scalar promotion policy does not belong in the ranked recipe. The
shared planner handles arbitrary reachable semantic CFGs, including loops,
duplicate and critical edges, and irreducible regions. The semantic adapter
uses a mixed value/memory model: address-taken, aliased, atomic, volatile,
projection-mutated, and drop-observed locals remain memory rather than being
forced into SSA. Production KIR materializes retained locals as private slots
when their layout is exactly representable as a scalar or metadata-free
32/64-bit pointer. A bounded must-initialization dataflow controls every slot
load and models moves, storage lifetime events, and call destinations; slot
state is intentionally excluded from SSA block transport. Dereferenced
external memory and authenticated LDS operations remain supported. Aggregate,
fat-pointer, dynamically sized, and cyclic-entry private-slot materialization
remain target-neutral lowering work. Loop tiling and MFMA synthesis require new
coordinate remapping, legality, and cost-model contracts.

Every retained slot remains a `ReadWrite` allocation because compiler-generated
initialization and later mutable uses require that capability. A shared borrow
or immutable address-of emits KIR V11 `RestrictPointerAccess`, which preserves
the pointee, address space, allocation provenance, and pointer identity while
narrowing only `ReadWrite` to `ReadOnly`. The verifier rejects widening,
identity relabeling, write-only substitution, and address-space or pointee
changes. General Rust reference helpers still require provenance-keyed
address-space specialization: a reference to private retained storage cannot
be silently typed as a global helper parameter. That specialization is not
implemented here, so this wave does not claim general reference-through-helper
support or all-Rust lowering.

The cyclic-entry boundary applies to generic admitted Semantic MIR, not valid
Rust input. The pinned rustc requires `START_BLOCK` to have no predecessors,
and the production importer independently scans all successors under its
validation-work budget before constructing semantic blocks. The shared planner
still supports an explicitly modeled external definition for cyclic entries so
its target-independent contract does not depend on rustc's structural rule.

## Production optimization

`fe2o3-kernel-opt` owns one closed V2 pass policy with two exact transport
endpoints. `optimize_production_kernel_ir_module_v2` retains the frozen KIR V10
endpoint for production V8/V9 modules. The additive
`optimize_production_kernel_ir_module_v3` imports and exports exact KIR V11 for
production V11 modules. V3 returns `OptimizedKernelIrModuleV3` with a
`VerifiedCanonicalKernelIrV11`, but deliberately reuses
`KernelIrPlironOptimizationLimitsV2` and `KernelIrPlironOptimizationReportV2`;
it does not define a new pass policy. Both endpoints use a fresh owner-aware
Pliron session and run:

1. sparse conditional constant propagation;
2. control-flow simplification;
3. `select(c, x, x)` canonicalization;
4. dead-code elimination;
5. conservative same-block pure common-subexpression elimination;
6. dead-code elimination; and
7. control-flow simplification.

There is no pass selector and no unoptimized fallback in the production
transaction. Each changed checkpoint is recursively verified. The final graph
is exported as the version-selected KIR V10 or V11 endpoint, decoded,
semantically verified, and retained with bounded pass accounting, mutation
epochs, endpoint digests, and surviving coordinate correspondence. Independent
V11 structural replay uses `admit_production_kernel_ir_structural_replay_v3`;
it establishes exact closed-policy replay and structural well-formedness, not
semantic preservation or compiler-refinement authority.

Only operations proven deterministic, pure, total, non-convergent, and
memory-independent participate in local CSE or DCE. Memory operations,
barriers, atomics, wave and matrix operations, inline assembly, unknown calls,
pointer arithmetic, and potentially trapping computations remain conservative.

The executable direct-KIR V1 optimizer is absent. Versioned V1 names that
remain in dialect or bridge internals identify data/API formats, not an
alternate production optimizer.

## Proof boundary

The optimizer report proves deterministic structural replay and successful IR
verification. It is not a semantic-refinement theorem. A mutating optimizer
does not inherit the formal status attached to the pre-optimization KIR.

The semantic SSA report has the same deliberate limit: exact reconstruction of
planner input, plan identity, resources, and replay establishes a structural
invariant, not a proof that promotion preserves Rust semantics. The
partial-move, transparent-borrow, ambient-scope, and component-witness checks
are executable certificates inside the compiler's trusted implementation; they
are not mechanized proofs of the compiler. Ranked V1 is still an index-only
overlay, and the production frontend continues to reject Rust constructs that
its semantic importer or target-neutral lowerer does not support. The shared
planner is general over the modeled semantic CFG, but this wave does not claim
that every Rust feature can be imported, lowered, or executed on a GPU.

The next proof gate is a per-pass refinement relation with explicit
retained/replaced/merged/eliminated source and IR coordinates. Global CSE,
alias-driven load elimination, loop transforms, GPU mapping, and scheduling
must remain disabled until their effect, convergence, provenance, numerical,
and resource contracts are explicit.

## Regression gates

- Dialect verifier, folding, branch-interface, DCE, and local-CSE tests.
- Exact KIR V9, V10, and V11 bridge round trips, including V10 memory
  intrinsics and V11 pointer-access restriction.
- Deterministic V2 policy order, limits, epoch, fail-closed tests, and V3 exact
  V11 transport/replay tests.
- Ranked recipe replay, hostile mutation, fixed-point, and tree-work tests.
- Sparse SSA lit fixtures plus focused semantic partial-move, transparent
  borrow, implicit-scope, resource-limit, and KIR entry/transport tests.
- Production compiler source-order checks, rustc extraction matrices, and an
  executable compiler-produced KIR regression for nested loops and switches.
- Compiler-side kernel regression runners for the kernels represented by the
  fe2o3-kernels documentation site.
