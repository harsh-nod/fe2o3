# Source Memory Proof Checkpoint

Status: detached implementation, not mounted or centrally compiled. Thirteen
standalone tests pass against cached libraries (no dependency rebuild). Priority
has moved to the actual23c/mixed24 numerical-issuance root failure; do not mount
this memory child as a production acceptance path without the joins below.

Ready child files: `../pliron_semantic_memory_v1.rs`, `collect.rs`, `state.rs`,
`state_tests.rs`, `tests.rs`, and this note. Standalone harness and binary:
`/tmp/pauli-source-memory.8lcGn2/{check.rs,check}`. The harness includes the real
inventory, invocation-trace and provenance modules, and links cached analysis,
dialect and PLIRON libraries. It does not mock their algorithms or inject KIR
evidence. Test filter: `pliron_semantic_memory_v1 --test-threads=1`.

## Common API For Parent And Lagrange

`prove_live_pliron_semantic_memory_v1(&Context, &FuncOp)` returns a privately
constructed `LivePlironSemanticMemoryProofV1` or an exact rejection. It takes no
KIR, canonical identity, receipt, effect roster, or caller-selected read list.

`proof.with_live_reads(context, function, |reads| ...)` checks context identity,
function pointer, mutation epoch, structural graph identity, and exact pointer
rosters before and after inspection. Each `PlironProvedSemanticReadV1` exposes
the actual access and SSA producer, result, view, index Values, allocation,
exact `SemanticTypedScalarV1`, and per-invocation cell/version facts.

`PlironSemanticMemoryVersionV1` is closed:

- `Initial`: this cell's function-entry memory, not zero or a free symbol.
- `AfterWrite { invocation, event, block, operation }`: the last exact write
  to this cell in the same invocation. This does not assert a stored value or
  equality with an initial-memory CPU reference.

This API proves source-graph memory stability/noninterference, not physical ABI
provenance, output equivalence, or launch authority. Lagrange's Global/BF16
consumer must keep its independent reference and source-to-final correspondence.
BF16 and other scalars retain exact kinds and widths; there is no bit-width-only
equivalence or implicit scalar conversion. No free-symbol conversion is exposed.

## Exact Next Parent Hooks (Approval Required)

1. Mount the analysis child/export after the diagnostic window, then run its
   graph and differential component tests before enabling any acceptance path.
2. Ranked materialization must create `SemanticTypedReadOp` immediately after
   the existing matching `RankedAccessOp(Read)` at the original source read
   site. The pair denotes one effect, with one actual SSA result. Keep the
   existing ranked observation for bounds/provenance/race/effect analyses.
3. Retain an exact source `(block, operation)` to read-result table. Expression
   `Load` materialization looks up that result and checks all retained scalar,
   allocation, view and index facts; it does not emit a new read or TypedSymbol
   at the consuming expression site. Do not infer volatility or predicates.
4. Shared semantic-expression reconstruction must consume the live source proof
   before admitting reads. Continue rejecting every reserved free TypedSymbol,
   including unused ones. Expression commitments alone are not memory evidence.
   Checked input bindings must remain attached during scalar and effect equality;
   a raw reserved-symbol tree must never stand in for that relation.
5. Bounds' operation whitelist needs the checked typed producer observation,
   not a second unvalidated access path. Production materializer replay and
   source-contract export must reproduce the same producers and operation sites.

The detached proof currently supports one straight-line block, a nonempty
explicit static launch, and unpredicated nonvolatile unordered Global accesses.
It uses the shared sparse-index/provenance/exact-invocation-trace analyses.
Unknown effects, atomics, synchronization, dynamic/unsupported CFG, unresolved
aliases/indices, out-of-bounds accesses, missing/duplicate read producers and
cross-invocation overlap involving writes reject. No limit is raised.

## Remaining Value-Fixture Work

The existing value fixture has KIR address `i*i` but ranked address `i`. A full
memory-proof positive must align those addresses; retain the nonlinear case as
an explicit incomplete/rejection test. Preserve `memory_effects == 2` and
`value_expressions == 1`, and exact operator/constant/executable-drift failures.

Add actual independent CPU expression roots and exact effect contracts tied to
the retained output view/indices/write site/ownership contract. Do not create
self-equal contracts or use caller-policy staging as compiler proof authority.
An `AfterWrite` read cannot satisfy an initial-memory CPU load merely because
the address or symbol label matches. These contracts and the shared expression
consumer remain pending; the detached memory proof is not a full PASS claim.
