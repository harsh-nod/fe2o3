# General Optimizing Compiler Wave 1

Status: implemented compiler infrastructure, not a claim that fe2o3 is a
general-purpose or formally verified compiler.

## Production Flow

```text
Rust/rustc
  -> authenticated semantic MIR
  -> ranked Pliron verification
  -> verified target-neutral Kernel IR
  -> bounded Formal Compiler V3 classification
  -> AMDGPU target binding
  -> deterministic target-KIR optimization
  -> canonical target-KIR identity
  -> AMDGPU LLVM lowering and exact replay evidence
```

The formal V3 owner is revalidated before target lowering and then dropped.
Its diagnostic status remains bound to the original target-neutral KIR. The
optimizer is deliberately later in the transaction, and a mutating pass does
not inherit that formal status.

## Analysis Ownership

`fe2o3-kernel-analysis` owns immutable whole-function facts. Its analysis
manager binds cached results to a module identity and mutation epoch and
provides deterministic control-flow, post-dominance, and SSA-liveness queries.
A consuming invalidation transition advances the epoch; resource ceilings
cover the analysis work and fail with typed errors. Uniformity remains outside
this manager until it can reuse the cached prerequisites under equivalent hard
limits.

Post-dominance uses a virtual exit and reports may-exit post-dominance for
regions that can reach an exit. Consumers requiring termination or
reconvergence must establish that premise separately.

## Transformation Ownership

`fe2o3-kernel-opt` depends on KIR definitions, while `fe2o3-kernel-ir` remains
the canonical representation and verifier owner. The current passes need no
whole-function analysis; later passes should depend on
`fe2o3-kernel-analysis` only when they consume its stamped immutable reports.
The closed V1 pass order is:

1. remove unreachable blocks;
2. eliminate transitively dead operations from a conservative pure-operation
   whitelist.

Each pass receives independent work and mutation budgets. It transforms a
single private candidate, verifies the admitted input and every changed
checkpoint, and publishes a result only when the entire pipeline succeeds.
Calls, memory operations, wave operations, assembly, synchronization,
potentially trapping arithmetic or casts, pointer arithmetic, and operations
without complete pure effect summaries are retained.

The production transaction retains per-pass work, mutation, and epoch data.
KIR-to-LLVM replay reconstructs target binding and the same fixed optimizer
from the original canonical KIR, then requires exact optimized KIR and LLVM
identity. These are reproducibility and structural-validity properties. No
optimizer semantic-refinement theorem exists yet. When a pass mutates KIR,
exact source-to-ISA correlation is explicitly unavailable until a canonical
pre-to-post optimization coordinate map is carried alongside the replay.

## Rust Semantic Coverage

Production MIR lowering now converts non-Boolean `SwitchInt` into KIR's typed
integer switch rather than the legacy unsigned switch. It validates raw rustc
bits against the selector width, interprets signed values exactly, orders
cases canonically, rejects normalization collisions, and preserves edge
arguments and source correspondence. KIR currently has no 128-bit constant
variants, so `i128` and `u128` switches are rejected explicitly.

## Next Gates

- Prove or validate per-pass semantic refinement and carry a canonical
  optimization receipt and coordinate map from proof-bearing neutral KIR to
  optimized KIR.
- Add dominator-frontier, loop-forest, alias/effect, divergence, and
  reconvergence analyses with the same epoch and budget discipline.
- Add constant propagation, sparse conditional constant propagation, CFG
  simplification, common-subexpression elimination, loop canonicalization,
  and GPU-specific scheduling behind explicit legality contracts.
- Expand MIR coverage for aggregates, enums, calls, panics, atomics, and
  pointer/provenance behavior; add 128-bit KIR constants before enabling
  128-bit switches.
- Add target-independent legalization, cost models, pass-pipeline selection,
  differential fuzzing, and performance evidence across supported AMD
  targets.
