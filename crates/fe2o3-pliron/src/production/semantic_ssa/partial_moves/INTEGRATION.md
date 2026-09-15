# Partial-Move Sparse State Checkpoint

Scope: Qwen ngram gather and its reverse probe. The recorded source9 failures
require 3650722 and 3650766 storage words against the unchanged 2097152 limit.
Stage-gradient-shard's pointer-provenance rejection is unrelated and untouched.

## Required Parent Hook

The failure can occur BEFORE `validate_partial_moves_v1`: the shared parent
`semantic_ssa_auxiliary_resources_v1` reserves
`(blocks + 2) * projected_moves * (max_projection_depth + 8)` storage words and
a quadratic worst-case revisit/clone work envelope. That reservation is enforced
before the solver, and is included in the solver's `base_storage_words` too.
The new sparse solver alone cannot remove that preflight rejection.

Parent: replace ONLY the body of `semantic_ssa_auxiliary_resources_v1` in
`src/production/semantic_ssa.rs` with:

```rust
partial_moves::auxiliary_resources_v1(function, input)
```

Remove the now-unused `projected_local_move_metrics_v1` import in that parent;
the function remains used inside `partial_moves.rs`. No limit change is needed.
No shared parent file was edited by this worker.

The child envelope retains the original adapter rows, statements, events,
edges, definitions, variable indices and their work charges. It removes only
the obsolete full-copy partial-state reservation and worst-case clone work.
The solver charges its incoming-state/queue handles and bounded scratch before
allocation, actual live immutable nodes and full projection paths on allocation,
projection scratch, and each executed statement/edge/operand/projection plus
state traversal/cloning/merging work. Storage releases when the final shared
owner disappears. Work never releases. Overflow and inclusive limits reject.

## Algorithm And Equivalence

Incoming and temporary states hold immutable shared radix roots. Local IDs
select a compressed binary radix branch, with strictly decreasing bits and at
most 32 levels. Leaves keep the same ordered move-path sets as the old solver.
Snapshots are constant-time shared references. Updating/removing a local copies
only its branch; unchanged leaves/subtrees are shared. Empty-destination joins
share the source root; identical roots/leaves skip redundant merges.

The worklist, reachable-edge handling, statement order, exact call-return
destination initialization, move-path canonicalization, unsupported-projection
checks, parent/child overlap checks, and conservative may-move union semantics
are unchanged. StorageLive/whole assignments clear only the corresponding
local; StorageDead/Deinitialize retain whole-local facts. Field writes cannot
repair a moved strict ancestor. No move/lifetime check was removed.

`ProductionSemanticPartialMoveCertificateV1::state_entries()` now reports peak
logical state-storage words (including workspace), not historical insertion
count. Existing module accounting already adds this value to storage. Work and
storage certificate values, module summaries, and SSA replay identities change;
regenerate derived identities, do not weaken replay checks. MIR bytes and schemas
are unchanged. Parent may clarify that accessor's documentation in its module.

## Files And Verification

- `src/production/semantic_ssa/partial_moves.rs`: integration and adapter envelope.
- `src/production/semantic_ssa/partial_moves/state.rs`: shared sparse state/budget.
- `src/production/semantic_ssa/partial_moves/state_tests.rs`: eight mounted tests.
- `src/production/semantic_ssa/partial_moves/standalone.rs`: std-only test harness.
- This note.

Standalone rustc tests exercise the exact production state implementation:
12000 deterministic differential operations against the previous map/set rules,
all observable reads across eight retained snapshots after each operation,
strict-ancestor rejection, cleanup versus call-return state, loop joins,
4096 shared snapshots of 2048 moved locals, released-fact storage reuse with
cumulative work, long paths/high-bit local IDs, and exact storage/work boundaries.
All eight standalone tests passed. Rustfmt checks and the tracked-file diff
check passed. Measured synthetic node/path storage: 2048 moved locals retain
69620 live words across 4096 snapshots; changing one local raises live storage
to 69784 and peak to 69806 words. Snapshot/queue handles are separately reserved
by the production solver. For those synthetic counts, the obsolete dense
formula alone reserves 75534336 words. This is not a measurement of Qwen gather;
crate compilation and real gather/reverse results remain parent-owned.

Central filters: `partial_moves::state::tests`, all existing
`partial_move_certificate_` / `partial_move_constant_index_` regressions, and
SSA inclusive/aggregate resource tests. Run full Pliron tests after the hook,
then recheck gather and reverse under their existing budgets. No Cargo, network,
SSH, source/provider bypass, or limit modification by this worker.
