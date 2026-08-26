# General Kernel Check Pipeline V1

Status: mandatory target-neutral rejection gate at the MIR-to-Kernel-IR
boundary. The reports are descriptive analysis results and grant no source,
compiler-refinement, artifact, publication, load, or launch authority.

## Placement

The compiler runs checks over one structurally verified, immutable Kernel IR
module before any transformation, schedule selection, target legalization,
Pliron projection, LLVM emission, or artifact publication:

```text
optimized MIR and authenticated frontend facts
    -> canonical Kernel IR
    -> general kernel checks (this pipeline)
    -> fixed algorithm-contract checks
    -> schedule and target transformations
    -> AMDGPU lowering and artifact stages
```

These are compiler analysis passes, but they are not LLVM optimization passes.
Running them after lowering would lose source-independent memory regions,
launch structure, address spaces, and synchronization intent. They are also not
generic mutating Pliron `Pass` implementations yet: generic Pliron mutation
remains withheld until owner-aware operation handles land under issue #140.
The target-neutral `kernel.*` dialect now applies MLIR-style local `Verify`
methods and exposes a closed, non-mutating `kernel-memory-bounds-v1` function
stage for ranked-memory `FuncOp`s. `require_pliron_ranked_bounds_before_lowering_v1`
is the fail-closed lowering boundary: static violations, unresolved dynamic
bounds, malformed CFG, and resource-limit failures are errors and cannot fall
back to unchecked lowering. The Kernel IR runner and Pliron function stage are
both closed and do not accept caller-registered executable passes.

The ranked PLIRON path has one fixed workload-neutral V2 sequence:
`tensor-layout -> memory-bounds -> atomic-legality -> race-freedom ->
hierarchical-ownership -> barrier-convergence -> workgroup-memory ->
semantic-refinement`. Tensor layout runs first because later reference
refinement consumes its exact propagated result-layout facts. Atomic legality
runs before race analysis because race analysis may classify two atomic effects as compatible
only after explicit ordering, scope, memory-space, and target-capability
requirements have been checked. Missing or invalid ordering/scope is
`Rejected`; an absent matching target capability or unauthenticated
system-coherent allocation is `Incomplete`.

## Fixed Pass Order

`fe2o3-kernel-analysis::GENERAL_KERNEL_CHECK_PASS_ORDER_V1` defines the only V1
order:

1. `kernel-structural-v1` checks types, SSA, operations, capabilities, and
   module/kernel structure once and returns a private-constructor borrowed
   token.
2. `kernel-control-flow-v1` derives reachable blocks, predecessors, dominance,
   and reducibility.
3. `kernel-tensor-layout-v1` validates each target-owned instruction ABI,
   propagates fragment layouts through compiler-derived value roots retained by
   authenticated source projection, and checks every tensor consumer against
   its producer fact. Equal producer facts for one root join. Conflicting facts
   and incompatible operand or accumulator uses are compile-time errors. A
   checked conversion or reload can create a new root only through source
   projection; a detached PLIRON layout assertion grants no authority.
4. `kernel-memory-bounds-v1` derives formal accesses and the exact runtime
   extents still needed to close each access. Its Pliron ranked-memory form
   constant-folds static dimensions and runs a forward must-analysis: a strict
   `index < extent` relation is usable only when it holds on every incoming CFG
   edge. Dynamic extents remain bound to the exact ranked view SSA value (or
   its exact runtime-shape operand), so a guard for another view cannot prove
   the access. A shared bounded Presburger query discharges finite affine and
   constant-modulus relations and reports the first unconditional invocation
   witness for an affine violation as `FE2O3-BOUNDS-004`.
5. `kernel-race-freedom-v1` reuses the same formal-memory extraction to report
   alias requirements and possible inter-invocation write conflicts. It first
   uses sparse rank/injectivity rules, then the shared Presburger map engine to
   prove empty cross-invocation intersections. Unsupported relations fall
   through to the established exact trace; a solver limit is never Clean.
6. `kernel-barrier-convergence-v1` rejects barriers controlled by values that
   are too varying for the participating scope.
7. `kernel-workgroup-memory-v1` performs a forward must-analysis. A load is
   accepted only when every incoming path has written and published the same
   LDS region. Every new store starts a new epoch and invalidates the prior
   publication until another workgroup-memory barrier.

The ranked V2 sequence uses the same tensor, bounds, race, barrier, and
workgroup stages. It additionally places `kernel-atomic-legality-v1` before
race analysis, `kernel-hierarchical-ownership-v1` after race analysis, and
`kernel-semantic-refinement-v1` last. Semantic refinement compares GPU output
coordinates, guards, values, effects, and numerical policy with the safe Rust
reference contract. For tensor results it also requires the reference
obligation to name the exact result root produced by layout dataflow, with the
same component count and scalar contract.

## Shared Presburger Analysis

`pliron-presburger-v1` is an immutable analysis service cached once per
function by `PlironAnalysisManagerV1`; it is not a ninth policy pass. Bounds,
race, and hierarchy ownership query it from their existing fixed positions.
This keeps pass order and diagnostic ownership stable while avoiding three
independent implementations of integer-set reasoning.

The admitted fragment is finite integer boxes, conjunctions of signed affine
equalities and inequalities, constant-modulus congruences, and affine or
remainder maps. Queries cover emptiness with a witness, range containment,
injectivity/collision, cross-map intersection, total box coverage, and
pointwise map equivalence. Path-sensitive traces can supply a finite image to
the same coverage query when guards or loops are not affine.

The engine is exact for this bounded fragment. Interval pruning rejects
impossible partial assignments; remaining points are visited in deterministic
lexicographic order, so diagnostics retain a stable first counterexample.
Streaming range and collision queries stop at the first witness. Only coverage
retains an image set. Rank, constraint, result, and work-unit limits are fixed.
An unknown dynamic extent, nonlinear product, unsupported index fact, machine
index overflow, malformed relation, or exhausted budget returns `Incomplete`.
The compiler never substitutes mathematical `i128` arithmetic for an unproved
machine-`u64` operation.

No transformation may run between these passes. A structurally invalid module
stops after pass 1. Otherwise all passes run so independent findings are
reported together.

## Outcomes

Every pass returns one of three typed states:

- `Clean`: the pass found no obligation in its modeled domain.
- `Incomplete`: the compiler lacks an authenticated launch extent, allocation
  size, alias fact, callee summary, or supported operation model. This is never
  treated as success by an authority-bearing consumer.
- `Rejected`: the checked IR contains a concrete safety violation, such as a
  possible same-allocation race, divergent barrier, or LDS read before the
  current epoch was published. The production translator emits a located
  `KernelCheckRejected` compile-time diagnostic and does not lower the module.

A clean report is still not proof of source correspondence or runtime safety.
Those require the same retained frontend owner and authenticated runtime facts.

Every error returned by the unified eight-pass production PLIRON pipeline also
contains at least one `KernelCheckRepairV1`. A repair has a stable action code,
the owning pass, an applicability classification, and an actionable message.
The compiler prints the repair as `help[FE2O3-FIX-*]` at both the raw
production-pipeline and production-session APIs. A suggestion is never applied
silently. Layout dataflow diagnostics include both producer and consumer sites
and profiles;
when no unique edit is sound, the repair is marked `HasPlaceholders` rather
than being presented as machine-applicable.

## General Checks And Algorithm Contracts

Safety mechanics are kernel-independent. The shared pipeline does not know
what GEMM, softmax, attention, or convolution means:

| Existing failure class | General owner |
|---|---|
| incompatible MFMA/tensor producer, CFG join, operand, accumulator, or wave layout | tensor-layout dataflow pass over authenticated value roots |
| out-of-bounds loads/stores | memory-bounds pass plus a frontend-supplied dimensional description |
| invalid atomic ordering/scope or unsupported atomic target requirement | atomic-legality pass plus target/coherence admission |
| duplicate lane/workgroup writes and LDS write conflicts | race-freedom pass |
| missing publish, read-before-wait, or stale/reused LDS data | workgroup-memory pass |
| divergent barriers | barrier-convergence pass |
| GPU output coordinates, values, effects, or numerical policy differ from a safe Rust/Verus reference | semantic-refinement pass joined to bounds, ownership, tensor-layout, and exact proof-boundary evidence |

Algorithmic equations are intentionally not generalized by name. Accumulator
carry, tail values, and alpha/beta epilogues belong in fixed contract checkers
selected by a declared algorithm schema after the general passes. No arbitrary
callback or plugin API is added here. The production transaction may select a
contract checker from a closed schema enum, but that enum does not select a
compiler implementation and cannot weaken or skip the general sequence. A
convolution contract can then describe its own halo or reduction equation
without teaching the bounds pass that the kernel is a convolution.

The source-level diagnostic vocabulary is also shared. For example, a frontend
may describe a two-dimensional read as `row < height` and `column < width`.
The formatter identifies each failed and proven dimension without encoding a
GEMM-specific variable name or source pattern.

## Efficiency

The production composition verifies each module once, even when it contains
multiple kernels. Bounds and race analysis share one formal-memory extraction.
Control-flow successors are built once, and workgroup-memory facts use a
descending worklist that revisits only successors of changed blocks.

The PLIRON tensor-layout analysis performs one bounded operation traversal,
one deterministic producer join per result root, and three consumer checks per
tensor site. Facts are stored in an ordered root map; conflicted roots are
tracked separately. Work is linear in tensor sites and dataflow edges apart
from ordered-map lookup, and is capped at 16,384 roots and 65,536 edges.

The PLIRON ranked-memory stage performs one structural traversal, one CFG
construction, and one descending fixed point. It indexes at most one strict
relation per block and stores must-facts in dense bitsets; predecessor
intersection is word-wise rather than cloning hash sets. Its deterministic
worklist revisits only successors whose input changed. Rank is bounded at 8,
functions at 1,024 blocks and 65,536 operations, and every rejected access is
reported once in block/operation/dimension order. The production recipe
requires dense local IDs and validates and materializes them with indexed
vectors, so recipe work is linear in blocks, operations, and operands; sparse
IDs cannot force proportional allocation.

The shared Presburger engine caps a relation at 16 variables, 256 constraints,
16 outputs, and 1,048,576 work units. Range and collision checks stream the
finite domain; cross-map race queries retain at most two owners for each first
map image; total coverage retains one deduplicated image set. The old exact
invocation trace remains the bounded fallback for non-affine guards and loops.

Structural verification and formal-memory extraction each traverse the relevant
IR once. Control-flow and barrier algorithms have explicit storage/work limits.
For `V` reachable blocks, `E` edges, and `R` modeled LDS regions, the
workgroup-memory worklist is bounded by the finite `(block, region)` fact set
and revisits only successors of changed blocks. Race comparison is quadratic in
the number of modeled access families in the worst case because every possibly
conflicting pair must be considered; inputs are bounded and future affine
partitioning may reduce that candidate set without changing semantics. No pass
performs an unbounded fixed point.

## Current Boundary

The production MIR-to-Kernel-IR translator invokes the Kernel IR pipeline for
every translated kernel and stops on `Rejected`. The ranked-memory Pliron
operations, local verifiers, and pre-lowering bounds gate are implemented and
tested as the target-neutral projection contract. The closed
`ProductionPlironSessionV1` constructs bounded ranked recipes internally,
retains the exact function privately, runs the bounds pass, and issues a
move-only lowering input only from the verified typestate. The sole production
transaction now consumes that ranked-memory result before target-neutral KIR
and gfx942 lowering. Detached lowering remains a migration/conformance surface;
it cannot publish and may not become a second unchecked compiler path or
reconstruct a clean report.
Dynamic launch sizes and unmodeled effects remain `Incomplete`; they are not
silently accepted as proven. The general GEMM optimized-MIR mutation oracle
continues to issue diagnostic-only source findings and cannot mint positive
correspondence. Re-enabling positive source authority requires a complete
closed root/helper verifier that produces the canonical Kernel IR and retained
frontend facts consumed here.
