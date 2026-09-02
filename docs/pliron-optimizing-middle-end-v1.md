# Pliron Optimizing Middle End V1

## Decision

Pliron is the mutable in-memory IR and pass framework for fe2o3's optimizing
middle end. Canonical Kernel IR remains the Pliron-independent interchange,
identity, replay, and proof-checkpoint format.

The two representations are not concurrent mutable authorities. An
optimization transaction starts from one verified canonical KIR snapshot,
constructs a fresh session-owned Pliron graph, mutates only that candidate,
and publishes a new canonical KIR snapshot only after checked export and both
Pliron and KIR verification succeed. Dropping the candidate session aborts the
transaction.

```text
verified canonical KIR
        |
        v
bounded typed import into a fresh Pliron session
        |
        v
closed Pliron pass pipeline over executable gpu.* SSA
        |
        v
recursive Pliron verification and bounded typed export
        |
        v
verified canonical KIR + transformation and coordinate receipts
```

`fe2o3-kernel-ir` does not depend on Pliron. The owner-aware bridge and pass
executor live on the Pliron side of that dependency boundary.

## IR ownership

- `mir.*` represents admitted Rust semantics.
- `kernel.*` represents structured algorithm and indexing semantics.
- `schedule.*` represents scheduling decisions.
- `tile.*` represents distributed tiles and layouts.
- `gpu.*` represents executable target-neutral SSA, control flow, memory, and
  synchronization.
- `amdgcn.*` represents AMD-specific legalized operations.
- `llvm.*` is the final typed lowering dialect.
- canonical KIR snapshots bind stable identities and evidence between stages;
  they are not the mutable optimization data structure.

## Pass execution boundary

The Pliron-backed V2 optimizer accepts only a closed, versioned pass plan. It never accepts a
caller-provided `Pass`, callback, `Context`, or `Ptr<Operation>`. The executor
authenticates the session and root handle, constructs audited pinned-Pliron
passes internally, and returns no graph pointer or mutable context. A future
production switch must consume the graph through a move-only typestate
transition and a new replay evidence version.

Every pass records its stable kind, input and output graph work, change status,
and deterministic resource charge. A changed graph is recursively verified
before the next pass. Erased descendants invalidate all corresponding
owner-aware handles. A panic, invalid graph, accounting mismatch, or failed
post-mutation check poisons and discards the candidate session.

Initial generic pass order is:

```text
sccp
simplify-cfg
canonicalize
dce
pure-cse
dce
simplify-cfg
```

The existing Kernel IR optimizer V1 and its replay format remain frozen. The
Pliron pipeline uses a new versioned report and replay contract; it must not
reinterpret the V1 pass roster or receipts.

## Dialect legality interfaces

Executable dialect operations implement pinned Pliron interfaces for constant
folding, branch folding, and dead-code removal. Potentially trapping,
convergent, memory-accessing, and unknown operations are excluded by a closed
eligibility policy. An absent interface always weakens optimization.

The first CSE pass accepts only operations proven to be deterministic, pure,
total, non-convergent, and free of memory dependence. Loads become eligible
only after alias/effect analysis and memory versioning exist. Barriers, fences,
atomics, inline assembly, wave collectives, unknown calls, pointer arithmetic,
and potentially trapping operations are ineligible by default.

## Determinism and resource limits

Optimization uses stable traversal and pass order. Current limits count graph
structure inspected at each boundary, registered handles, pass count, and
canonical bytes. Wall-clock time is not an artifact-affecting input.

Pinned Pliron passes do not currently expose internal budget hooks. Their
inputs and outputs are hard-capped and recursively verified, and the initial
pass set is non-expanding, but structural accounting is not instruction-level
CPU metering. Expanding rewrites require explicit budget hooks before
production admission. Output graph caps are checked after every pass;
canonical-byte caps are checked on typed export.

## Implemented scope

The non-production V2 transaction supports scalar constants, unary and binary
operations, compares, casts, selects, direct calls, slices, pointer arithmetic,
loads, stores, branches, conditional branches, and returns. It executes SCCP,
CFG simplification, select canonicalization, conservative same-block pure CSE,
and DCE. The O0 bridge preserves exact canonical bytes; optimized export binds
input and output identities in a receipt.

Generic address spaces, switch/unreachable terminators, allocation,
synchronization, atomics, guarded memory, matrix/wave operations, inline
assembly, cross-block/global CSE, and production replay V4 remain fail-closed
gates. The frozen production V1 optimizer is not silently replaced.

## Admission tests

The production switch requires all of the following:

1. `KIR -> Pliron -> KIR` at `-O0` is byte-identical for every supported KIR
   operation, terminator, type, attribute, function role, kernel descriptor,
   target capability, and source coordinate.
2. Unsupported or malformed constructs fail closed before mutation.
3. Pliron DCE and CFG simplification are differential-tested against the frozen
   Kernel IR V1 passes over the shared supported corpus.
4. Each mutating pass has negative tests for traps, overflow, memory effects,
   synchronization, convergence, and target-specific operations.
5. Replay reconstructs the exact pass plan and canonical output identity.
6. Pre-to-post coordinate receipts explicitly represent retained, replaced,
   merged, and eliminated operations.
7. Production never falls back silently to a different optimizer.

After these gates pass, the custom V1 implementation remains only for frozen
replay compatibility. New production compilation uses the Pliron pipeline.
