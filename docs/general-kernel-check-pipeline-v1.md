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

## Fixed Pass Order

`fe2o3-kernel-analysis::GENERAL_KERNEL_CHECK_PASS_ORDER_V1` defines the only V1
order:

1. `kernel-structural-v1` checks types, SSA, operations, capabilities, and
   module/kernel structure once and returns a private-constructor borrowed
   token.
2. `kernel-control-flow-v1` derives reachable blocks, predecessors, dominance,
   and reducibility.
3. `kernel-memory-bounds-v1` derives formal accesses and the exact runtime
   extents still needed to close each access. Its Pliron ranked-memory form
   constant-folds static dimensions and runs a forward must-analysis: a strict
   `index < extent` relation is usable only when it holds on every incoming CFG
   edge. Dynamic extents remain bound to the exact ranked view SSA value (or
   its exact runtime-shape operand), so a guard for another view cannot prove
   the access.
4. `kernel-race-freedom-v1` reuses the same formal-memory extraction to report
   alias requirements and possible inter-invocation write conflicts.
5. `kernel-barrier-convergence-v1` rejects barriers controlled by values that
   are too varying for the participating scope.
6. `kernel-workgroup-memory-v1` performs a forward must-analysis. A load is
   accepted only when every incoming path has written and published the same
   LDS region. Every new store starts a new epoch and invalidates the prior
   publication until another workgroup-memory barrier.

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

## General Checks And Algorithm Contracts

Safety mechanics are kernel-independent. The shared pipeline does not know
what GEMM, softmax, attention, or convolution means:

| Existing failure class | General owner |
|---|---|
| out-of-bounds loads/stores | memory-bounds pass plus a frontend-supplied dimensional description |
| duplicate lane/workgroup writes and LDS write conflicts | race-freedom pass |
| missing publish, read-before-wait, or stale/reused LDS data | workgroup-memory pass |
| divergent barriers | barrier-convergence pass |

Algorithmic equations are intentionally not generalized by name. Accumulator
carry, tail values, and alpha/beta epilogues belong in fixed contract checkers
selected by a declared algorithm schema after the general passes. No arbitrary
callback or plugin API is added here. When those contract checkers enter the
unified production route, integration must select them from a closed enum and
must not let them weaken or skip the general sequence. A convolution contract
can then describe its own halo or reduction equation without teaching the
bounds pass that the kernel is a convolution.

The source-level diagnostic vocabulary is also shared. For example, a frontend
may describe a two-dimensional read as `row < height` and `column < width`.
The formatter identifies each failed and proven dimension without encoding a
GEMM-specific variable name or source pattern.

## Efficiency

The production composition verifies each module once, even when it contains
multiple kernels. Bounds and race analysis share one formal-memory extraction.
Control-flow successors are built once, and workgroup-memory facts use a
descending worklist that revisits only successors of changed blocks.

The Pliron ranked-memory stage performs one structural traversal, one CFG
construction, and one descending fixed point. It indexes at most one strict
relation per block and stores must-facts in dense bitsets; predecessor
intersection is word-wise rather than cloning hash sets. Its deterministic
worklist revisits only successors whose input changed. Rank is bounded at 8,
functions at 1,024 blocks and 65,536 operations, and every rejected access is
reported once in block/operation/dimension order. The production recipe
requires dense local IDs and validates and materializes them with indexed
vectors, so recipe work is linear in blocks, operations, and operands; sparse
IDs cannot force proportional allocation.

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
`ProductionPlironSessionV1` now constructs bounded ranked recipes internally,
retains the exact function privately, runs the bounds pass, and issues a
move-only lowering input only from the verified typestate. The existing
detached Pliron-to-GPU lowering does not yet consume ranked-memory functions,
and `production-v1` still stops before canonical semantic-MIR-to-ranked-memory
projection. Those two missing stages must consume the closed result; they may
not add a second unchecked route or reconstruct a clean report.
Dynamic launch sizes and unmodeled effects remain `Incomplete`; they are not
silently accepted as proven. The general GEMM optimized-MIR mutation oracle
continues to issue diagnostic-only source findings and cannot mint positive
correspondence. Re-enabling positive source authority requires a complete
closed root/helper verifier that produces the canonical Kernel IR and retained
frontend facts consumed here.
