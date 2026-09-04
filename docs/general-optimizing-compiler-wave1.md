# General Optimizing Compiler Wave 1

Status: implemented compiler infrastructure. This is not a claim that fe2o3 is
a general-purpose or formally verified compiler.

## Production flow

```text
Rust/rustc semantic MIR
  -> ranked recipe projection
  -> checked recipe normalization
  -> immutable nine-stage Pliron verification
  -> verified target-neutral Kernel IR
  -> composed formal memory admission
  -> AMDGPU target binding
  -> fresh-session Pliron optimization
  -> verified canonical Kernel IR V10 snapshot
  -> AMDGPU LLVM lowering
```

The ranked verifier remains an analysis boundary. No pass mutates live ranked
Pliron between its nine analyses. Position-preserving normalization runs on the
owned recipe before graph construction, and target optimization runs only after
formal memory admission and target binding.

## Existing normalization and legalization

The production frontend and target-neutral lowerer already provide:

- unreachable semantic-block pruning and deterministic CFG construction;
- switch, assertion, and guard expansion;
- loop induction and mutable scalar/fragment promotion to block-argument SSA;
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

General mem2reg, aggregate flattening, critical-edge splitting, loop tiling,
and MFMA synthesis do not belong in the ranked recipe. The first two already
run during target-neutral lowering. The latter transformations require new
coordinate remapping, legality, and cost-model contracts.

## Production optimization

`fe2o3-kernel-opt` owns one closed V2 policy. It imports target-bound KIR V10
into a fresh owner-aware Pliron session and runs:

1. sparse conditional constant propagation;
2. control-flow simplification;
3. `select(c, x, x)` canonicalization;
4. dead-code elimination;
5. conservative same-block pure common-subexpression elimination;
6. dead-code elimination; and
7. control-flow simplification.

There is no pass selector and no unoptimized fallback in the production
transaction. Each changed checkpoint is recursively verified. The final graph
is exported as KIR V10, decoded, semantically verified, and retained with
bounded pass accounting, mutation epochs, endpoint digests, and surviving
coordinate correspondence.

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

The next proof gate is a per-pass refinement relation with explicit
retained/replaced/merged/eliminated source and IR coordinates. Global CSE,
alias-driven load elimination, loop transforms, GPU mapping, and scheduling
must remain disabled until their effect, convergence, provenance, numerical,
and resource contracts are explicit.

## Regression gates

- Dialect verifier, folding, branch-interface, DCE, and local-CSE tests.
- Exact KIR V9 and V10 bridge round trips, including V10 memory intrinsics.
- Deterministic V2 pass order, limits, epoch, and fail-closed tests.
- Ranked recipe replay, hostile mutation, fixed-point, and tree-work tests.
- Production compiler source-order checks and rustc extraction matrices.
- Compiler-side kernel regression runners for the kernels represented by the
  fe2o3-kernels documentation site.
