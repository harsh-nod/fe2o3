# Checked Execution-Capability Erasure

This is an implementation component for #272 M1, not a completed production
vertical slice. The backend still rejects execution capabilities until
authenticated source, instance expansion and operation correspondence are
connected to this component. No tutorial kernel gains qualification from this
change alone.

## Invariant

After validating a complete canonical KIR V15 graph, the lowerer can remove
exactly these lifecycle-only operations:

| Input operation | Required condition | Physical output |
| --- | --- | --- |
| `ContextIssue` | Accepted by the full V15 verifier | None |
| `WorkgroupDerive` | Accepted by the full V15 verifier | None |
| `ScopeEnd` | Accepted by the full V15 verifier; empty discard roster | None |

Every other field remains identical: ordinary operations and their operands,
result identities, calls, memory accesses, terminators, block parameters,
function signatures and roles, launch roots, capabilities and vector order.
`ScopeEnd` is a nominal lifetime boundary, **not a GPU barrier**. Removing it
does not prove synchronization or replace a barrier instruction.

Tile loads, tile/fragment conversions and nonempty descendant disposal require
separate checked lowering rules and currently reject. Execution-free modules
continue through the existing physical pipeline without this rule.

## Ownership And Replay

`VerifiedCanonicalKernelIrModuleV15` admits an exact V15 encoding, freshly
decodes it, runs the full semantic/lifecycle verifier, compares the complete
inverse with its source and hashes the canonical bytes under a V15-specific
domain. It owns both representations immutably and cannot be cloned or split.
It is not a certificate that those bytes came from Rust source.

The lifecycle verifier permits execution values only as operation results in
a same-function graph. It rejects execution parameters, invalid acquisitions,
incompatible incoming states (including loop backedges), escaping/live scopes
at exits and retained calls while a workgroup scope is open. Source callbacks
must therefore be expanded and checked before such a graph can be admitted.

`ProductionExecutionDischargeV29::try_discharge` then:

1. Preflights the complete input for this closed erasure rule.
2. Decodes an independent, explicitly unverified transformation candidate.
3. Removes the admitted lifecycle operations and records their input ordinals.
4. Freshly admits the candidate as the existing physical canonical KIR V12.
5. Independently replays the input/output relation and every erasure row.

Replay does not trust the producer's classification. It walks both graphs in
order, checks each removed operation against the closed rule, compares every
retained operation and field, and rejects missing, reordered or extra rows.
Exhaustive structural destructuring forces a review when IR containers gain
fields. There is no kernel-name or target-name dispatch.

The opaque result retains the exact input identity, immutable verified output
and checked erasure roster. The identity covers erased SSA values as well as
ordinary instructions. Changing an ordinary constant to a different valid
constant must fail replay, even though the changed output independently passes
the physical IR verifier.

## Loop Invariant

Each reachable block has one exact incoming ownership state: every nominal
value is absent, an available context, a borrowed context, its live workgroup,
or a descendant of that workgroup. The entry starts with all values absent.
The first incoming edge establishes a block's state; deterministic operation
transfer checks the block once. Every later incoming edge must reproduce that
state exactly, including backedges and duplicate edges. States are never
widened or replaced, so there is no iterative convergence heuristic.

This is an inductive safety argument: the entry state is valid; each operation
preserves its ownership obligations; and each edge establishes the successor's
invariant. Consequently every finite execution prefix respects those
obligations, however many loop iterations run. The ordinary verifier still
checks SSA dominance, argument types and complete CFG structure. Execution
values cannot be block parameters, escape in containers or flow through an
ordinary operation, so no nominal phi transfer is omitted by the state check.

For example, a context issued before a loop can acquire a workgroup, consume
or explicitly discard its descendants, and end that scope on every iteration.
The backedge then has the same available context and absent workgroup slots
as the initial edge. A workgroup acquired before a loop can instead remain
live throughout it and end after the loop, provided every edge preserves its
exact state. Ordinary loop-carried scalars remain ordinary SSA arguments and
are retained unchanged by erasure.

A context issuer inside a cycle rejects: its backedge cannot restore the
original absent context. An unclosed per-iteration acquisition, leftover
descendant, stale use, live-scope call, or exit with a live workgroup also
rejects. Exact equality is deliberately conservative; this rule does not
prove path feasibility to reconcile different ownership states at a join.

This check does **not** prove loop termination, eventual scope closure on an
infinite execution, barrier convergence or CPU/GPU numerical equivalence.
Erasing a nominal scope does not eliminate, unroll or otherwise rewrite the
loop. Those properties retain their separate production proof obligations.

## Resource Contract

Admission, copying, erasure and replay use one cumulative work/storage ledger.
Traversal and comparison work is charged before it runs. The exact canonical
encoding bounds complete structural comparison. Vector storage is admitted
before allocation; candidate, output and erasure roster coexistence is counted.
Erasure and replay are linear in graph/encoding size apart from the existing
canonical admission and verification algorithms they invoke.

The lifecycle pass queues each reachable block at most once and compares every
edge. For `B` blocks, `E` edges and `R` execution-role definitions, its retained
state uses `O(B * R + B + R)` logical storage and edge comparisons use `O(E * R)`
work, in addition to metered operation and indexed lookup work. A cycle does
not cause repeated state allocation or depend on a runtime iteration bound.

The caller keeps its input reservation live. Each Result path restores the
incoming storage floor without refunding accepted work, observed peak or first
denial history. A successful storage receipt must be reserved before another
allocation while its owner lives. These are conservative logical payload
bounds, not allocator/RSS accounting or an unwind-cleanup guarantee.

## What Remains

This component proves only its local graph transformation. Production wiring
still needs authenticated Rust producer calls, the complete source/SSA view,
per-instance correspondence, checked callback expansion and scope-end
materialization. #275 owns those source/instance producers; #271 owns the fixed
optimized final-graph pipeline. Neither is replaced by this API.

Cross-invocation memory safety, barrier convergence, numerical error bounds,
source and machine refinement, protected evidence admission, generated safe
host launch and target-matched hardware qualification remain separate required
checks. A simulator comparison is useful regression evidence, not a universal
CPU/GPU equivalence proof.

Tests cover context-only and sequential scopes, balanced branches, mixed roots,
same-slot loop reacquisition, persistent outer scopes, nested and irreducible
loops, switch backedges, and an independent bounded lifecycle trace oracle.
Discharged loops with ordinary scalar block arguments and canary-protected
stores are compared with a Rust CPU oracle in the simulator, including zero,
one and multiple iterations. Negative tests retain malformed lifecycle and
unsupported tile/fragment refusal, independently valid altered loop bounds,
edges, arguments, operations and metadata, malformed erasure rosters and
exact/one-short resource budgets. Run the focused libraries and compile-fail
documentation with:

```sh
cargo test --locked -p fe2o3-kernel-ir -p fe2o3-lower-mir-kernel
```
