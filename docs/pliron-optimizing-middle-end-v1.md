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
- target-bound canonical KIR carries AMD-specific legalization; the
  `fe2o3-amdgcn-model` implementation lowers it directly to deterministic LLVM
  text. `dialect-amdgcn` is currently an API facade, not an `amdgcn.*` Pliron
  dialect, and there is no `llvm.*` dialect.
- canonical KIR snapshots bind stable identities and evidence between stages;
  they are not the mutable optimization data structure.

## Pass execution boundary

The Pliron-backed V2 optimizer accepts only a closed, versioned pass plan. It never accepts a
caller-provided `Pass`, callback, `Context`, or `Ptr<Operation>`. The executor
authenticates the session and root handle, constructs audited pinned-Pliron
passes internally, and returns no graph pointer or mutable context. New
production compilation invokes this exact pipeline through a fixed entry point
with no optimizer selector or fallback. Production replay V4 independently
reconstructs the same transaction.

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

The executable Kernel IR optimizer V1 has been removed. Historical V3 replay
records retain self-contained inert wire types and cannot invoke or select the
live optimizer. The Pliron pipeline uses its own versioned report and V4 replay
contract; it does not reinterpret the historical pass roster or receipts.

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

The production V2 transaction gives scalar arithmetic, compares, casts,
selects, direct calls, slices, pointer arithmetic, loads, stores, branches,
conditional branches, and returns executable dialect operations. The remaining
verified KIR V9 operation and terminator families cross the bridge through
typed, effectful preservation carriers: their SSA operands, result types, and
CFG successors remain first-class while their exact versioned payload remains
owned by the private bridge transaction. This lets SCCP, CFG simplification,
select canonicalization, conservative same-block pure CSE, and DCE rewrite the
surrounding graph without treating opaque GPU effects as pure.

The O0 bridge is byte-exact. Optimized export binds input/output identities and
a deterministic digest of every surviving bridge coordinate. Unrecognized or
malformed graph nodes fail closed. Cross-block/global CSE is not implemented,
and the live transaction never falls back to a historical or unoptimized path.

## Admission tests

The production admission is maintained by the following regression gates:

1. `KIR -> Pliron -> KIR` at `-O0` is byte-identical across the verified KIR V9
   operation, terminator, type, attribute, function role, kernel descriptor,
   target capability, and source coordinate.
2. Unsupported or malformed constructs fail closed before mutation.
3. Conservative carrier tests cover allocation, guarded memory, atomics,
   synchronization, matrix/MFMA, wave, inline assembly, and switch families.
4. Mutating-pass tests retain potentially trapping and effectful operations and
   exercise live SSA rewrites into preserved operations.
5. Replay V4 reconstructs the fixed production plan and exact-compares the live
   accounting, bridge identities, correspondence digest, optimized KIR, and
   LLVM output.
6. Production has one fixed optimizer-policy entry point and no legacy or
   unoptimized fallback.

Historical V3 compatibility is implemented in the replay layer using inert
wire records rather than retaining executable V1 optimizer authority.
Semantic-refinement proofs and richer retained/replaced/merged/eliminated
coordinate outcomes remain future work; the current receipt proves deterministic
structural replay, not semantic equivalence.
