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
the canonical representation and verifier owner. New production compilations
use the single closed Pliron-backed V2 entry point. The fixed pass order is:

1. sparse conditional constant propagation;
2. control-flow simplification;
3. select canonicalization;
4. dead-code elimination;
5. conservative same-block pure common-subexpression elimination;
6. dead-code elimination;
7. control-flow simplification.

The optimizer transforms a fresh private Pliron candidate, verifies the
admitted input and every changed checkpoint, and publishes a result only when
the entire pipeline succeeds. Unsupported import, optimization, or export
fails the production transaction; there is no legacy or unoptimized fallback.
Calls, memory operations, wave operations, assembly, synchronization,
potentially trapping arithmetic or casts, pointer arithmetic, and operations
without complete pure effect summaries are retained.

The production transaction retains per-pass structural work, changed-pass
epochs, and typed bridge identities. KIR-to-LLVM replay V4 reconstructs target
binding and the same fixed optimizer from the original canonical KIR, then
requires exact optimized KIR, optimizer accounting, and LLVM identity.
Historical V3 records are decoded by inert replay-local structures and cannot
select the live optimizer. These are reproducibility and structural-validity
properties; no optimizer semantic-refinement theorem exists yet.

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
- Add loop canonicalization and GPU-specific scheduling behind explicit
  legality and cost-model contracts.
- Expand MIR coverage for aggregates, enums, calls, panics, atomics, and
  pointer/provenance behavior; add 128-bit KIR constants before enabling
  128-bit switches.
- Add target-independent legalization, cost models, pass-pipeline selection,
  differential fuzzing, and performance evidence across supported AMD
  targets.
