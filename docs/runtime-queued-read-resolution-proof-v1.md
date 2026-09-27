# Queued Read Resolution Proof Boundary

This packet covers one shared production transformation, not the complete queued
owner, Context release composition, GPU ordering or HIP/HSA parity.

`context_queued_writers/reads.rs::resolve_reads` invokes
`read_resolution_body.rs` with empty Rust proof hooks. The Verus root
`context_queued_read_resolution_v1.rs` invokes that same body over a selected
storage projection and proves its result against an independent, identity-selected
update. The logical selection does not follow the executable attachment traversal.

## Premises

- The exact writer's attached list has valid occupied slots, unique membership,
  complete next/previous links, the stated count, and Pending/unversioned readers.
- Membership is complete: every Pending reservation for that exact writer occurs
  in the list. `validate_attached_reads` checks the reachable chain but does not
  establish this global construction invariant by itself.
- Live and captured roots agree on writer identity and attachment head/count.
  Their phase may differ: `mark_unknown` changes the live phase before resolution.
- Success has correctly represented retained allocation snapshots with no pending
  writer and equal settled epoch/lineage. The projected lookup validates full
  allocation reference identity; correspondence to the actual inner journal lookup
  remains a separate obligation.
- Outcome authenticity, caller admission and fail-stop ownership are external.
  The proof permits and preserves `disposal_terminal == true`; it does not require
  an inner writer identity that successful settlement may already have retired.

## Guarantees

Only Pending reservations for the exact producer change. Success records actual
snapshot versions. NoEffect/Unknown produce no successful version. Requests,
allocation/device/range bindings, producer and consumer identities, incarnations
and occupied slots remain unchanged. Processed readers detach from the producer;
only the selected live root's attachment head/count clear. Unrelated roots and
reservations, free slots, reader counts, next incarnation and fail-stop state are
preserved within the projection.

The projection does not represent the remaining outer member/index arenas or
prove a full-owner representation relation. There is no admission, activation,
attachment-validator, cancellation, unlink/refund, disposal, panic or native effect
refinement claim. In particular, this is not the active-then-queued Context reader
release proof. CPU frozen-baseline tests additionally compare full executable
state, storage addresses/capacities and indexed inner accesses across outcomes.

## Qualification

The dedicated checker uses the existing authenticated diagnostic classifier and
pinned verifier closure, a clean signed source, relocated positive verification,
production-body-only mutations and closing source/tool continuity checks:

```sh
python3 -I -B crates/fe2o3-runtime-model/verus/check-queued-read-resolution.py \
  --verus /absolute/path/to/pinned/verus --output /new/absolute/path/outside/repo
```

Its 16 negative controls cover skipped/truncated traversal, lost next cursor,
wrong status, missing/fabricated successful versions, invented failure versions,
retained links, wrong root, uncleared head/count, altered incarnation, early free
slot refund and cleared terminal state. Parser/compiler errors, resource limits,
timeouts and foreign diagnostics are not accepted logical failures. Empty,
singleton and noncontiguous executable witnesses exercise terminal outcomes,
different consumers/versions, unrelated and already-resolved records, and the
captured/live phase discrepancy. These are projection witnesses, not a proof
that all such states are reachable from the whole owner constructor.

Historical pins/checkers remain unchanged. Qualification results belong in a
separate signed evidence packet; the checker source alone is not run evidence.
