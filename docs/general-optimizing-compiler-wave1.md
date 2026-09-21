# General Optimizing Compiler Wave 1

Status: implemented compiler infrastructure. This is not a claim that fe2o3 is
a general-purpose or formally verified compiler.

## Default production flow

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

This is the existing default transaction in
[`production_pipeline.rs`](../crates/rustc-codegen-fe2o3/src/production_pipeline.rs):
`lower_production_target` retains ranked checks, formal-memory admission, target
binding and the closed V2/V3 optimizer before lowering. The checked continuations
below are separate, nondefault owning APIs, not extra passes silently inserted
into this schedule. Their implementation does not activate a replacement
artifact or launch route.

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

## Nondefault checked continuations

The bounded checked path retains genuine source custody through private-cell
promotion, neutral loop preheaders and LICM. The next rewrites and their
composition use the following graph identities; these letters are explanatory,
not new policy or wire versions:

```text
retained source / ranked / historical checked owners
  -> private-cell promotion -> checked preheaders -> LICM output L
  -> checked induction refinement of L -> R
  -> checked cross-block private forwarding of R -> F
  -> fresh final F source/private/native/formal census
  -> LLVM emission and independent replay from actual F
```

The individual refinement and forwarding continuations are distinct from their
composed implementation. Forwarding directly from L remains a separate bounded
API; it is not evidence that forwarding after refinement ran on R. Composition
must consume the actual refinement owner, retain L and R, and construct F from
that same R object. Its independent L-to-R and R-to-F pair checks stay live
together. The original source, UnitLocal helper erasure, ranked bindings and
historical execution witness remain owned, rather than reconstructed from an
output digest. Integration and qualification status is tracked in
[#271](https://github.com/harsh-nod/fe2o3/issues/271), not implied by this diagram.

Induction refinement replaces only an independently checked nonwrapping
unsigned `CheckedAdd` update with an adjacent `Add` and Boolean false, retaining
both original result IDs and their uses. Its one-to-two origin relation records
the sum's original source statement and a synthetic overflow result without
inventing a Rust span or trap authority. Guarded no-wrap facts bind the taken
body edge and operation order. A symbolic guard distance is not an executed
trip count, and this rewrite is not general unrolling or bounds-check removal.

Private forwarding requires an admitted integer Load, exact allocation/access
geometry, a dominating stored value and independently grounded memory versions
with no intervening disqualifying effect. It replaces the selected Load in
place with `BitOr(value, value)`, preserving its result identity and original
Load source statement, not attributing it to the earlier Store. Unresolved
initialization, aliasing, memory-phi grounding or effect obligations refuse
selection; general memory disambiguation is not claimed.

Refinement does not generally admit scalar `Add`, `Sub` or `Mul` in the native
census. Its private checked allowance names the precise sum/false pair. The
composed path transports that allowance through both live relations to the
actual F inventory, including coordinates shifted by the synthetic false.
Neither an allowance for R nor a historical safety report can admit F. Complete
source lineage and fresh final checks precede first LLVM emission; native
replay emits the same actual final graph and compares its exact text.

The composed native wrapper also checks descriptor evidence against that same
F owner. Original semantic subjects, ordered typed roots, ABI/type ownership,
source launch and N/B target bindings remain borrowed from the retained
history; final physical arguments and allocation/race/bounds obligations come
from F and its fresh reports. The unchanged descriptor validator checks their
exact join. This predicate runs after wrapper preflight/target/ranked checks
and before final native replay; first inert LLVM emission remains inside F's
native preparation. No encoded descriptor, artifact/proof association, worker
handoff, publication or launch authority is created by the predicate.

Every continuation retains exact typed analysis limits and checks their full
identity on replay. Refinement checks output growth before mutation and refuses
a source-cap mismatch rather than clamping limits. Owning additions and actual
backing capacities are accounted on the cumulative active ledger while the
original source and sibling allocations remain live. Typed failure or unwind
drops partial backing before refunding only scope-owned credit. These are the
stated compiler resource domains, not a whole-process memory bound.

## Conditional analysis and open gates

Source259's consuming `ProductionConditionalRankedAnalysisV1::check_pipeline_v1`
retains the original pending owner and runs the shared conditional checks twice
over its authenticated live graph. `ProductionConditionalPipelineAnalysisV1`
keeps reports and invocation accounting, including failure information; it
cannot convert itself into an ordinary lowering input or recover a mutable
session. Selected conditional facts do not erase outstanding pending
obligations. The `internal-proof-staging` coverage is separate from ordinary
coverage and is not a default compiler selector or activation gate.

The checked continuations and this staged consuming checker do not complete
general memory/control-flow admission, all-Rust import, the signed source/proof
join, or default-pipeline migration. Ordinary-rustc entry coverage must retain
the actual authenticated bindings and exercise the consuming method; a
constructed source-owner test cannot substitute for it. Protected qualification
requires the admitted runtime and exact source/tool closure, with no missing-
runtime positive or fallback. Default activation additionally needs the fixed
policy/descriptor/worker handoff and complete final-graph verification contract.
Tutorial-wide compilation, target-matched hardware results and broader loop,
alias, interprocedural and GPU optimization remain separate open gates. No
formal compiler-verification claim follows from these executable checkers.

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

For this historical Wave 1 route, the next proof gate is a per-pass refinement
relation with explicit retained/replaced/merged/eliminated source and IR
coordinates. Separately, the checked native-V12 Policy3 path now includes
dominance CSE with independent coordinate checking; Policy4/5 add bounded
private-memory forwarding. These checked extraction paths do not imply default
or protected activation. See [the checked middle-end architecture](pliron-optimizing-middle-end-v1.md)
for the current schedules and trust boundaries. Broader GVN, alias-driven memory
optimization, loop transforms, GPU mapping, and scheduling still require their
own effect, convergence, provenance, numerical, and resource contracts.

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

The `refined_forwarding_wire_goldens` optimizer integration target adds 13
literal canonical V12 wire graphs using the existing bounded decoder and hex
transport. It runs actual owning L-to-R induction refinement followed by
R-to-F forwarding, keeps the same R endpoint for both independent pair checks,
and compares complete before/R/F dumps, rewrite remarks and origin rows with
literal expected outputs. The cases cover a dynamic loop, diamond, duplicate
edge, two functions, ungrounded phi, step-two update, unchecked Add, global
clobber, trapping cut, volatile Load, alignment mismatch, no-op and distinct
stores. Cases that exclude one rewrite retain that exclusion explicitly.

Additional checks cover malformed input, hostile admitted outputs, lineage,
idempotence and two genuine fresh-process transcripts. Resource tests retain a
live input and sibling allocation, exact measured work/storage success and the
final one-short Work refusal. The initial Storage test refuses the first
private-scope header at the inherited floor and checks typed denial/history;
it does not claim a known private-header extent or an owning-header denial.
These are canonical-wire regressions, not Pliron textual transformation lit or
source/default-pipeline qualification. The textual runner gap, genuine
ordinary-rustc/protected-runtime gates and target-matched hardware results
remain separate; test definitions alone establish no execution or proof claim.
