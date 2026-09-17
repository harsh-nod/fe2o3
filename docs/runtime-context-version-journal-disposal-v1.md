# Ordinary Unknown Writer Disposal

Development extension of the [ordinary async journal](runtime-context-version-journal-async-v1.md).
This closes its ordinary Submission Unknown allocation-owner disposal gap, not
V6, A1/A2, issue #182 or HIP/HSA parity. Accepted checkpoints remain Native R125
CPU/test, Admission R118B C1-C3 and Resources R116/V3. See the separately sealed
[CPU qualification archive](evidence/dev-v6-unknown-disposal-2026-09-17/README.md).

## Retained Identity

Before the original submission enters the backend, its Context-owned writer root
retains the sorted, deduplicated destination IDs, original allocation records and
canonical model members. The root remains independent of any returned backend
handle, public submission token or released submission metadata. Read-only
bindings are not writer destinations.

Only an exact Unknown Submission writer enters this disposal path. Pending
membership still rejects before backend entry. Before the first release, Context
checks the complete original roster against the journal, allocation phases,
facade records, backend indexes and configured request-credit ownership. A
surviving submission must have the exact writer marker and conclusive quiescence.
Its absence is valid after a no-handle failure or successful metadata release.
Missing or substituted custody seals Context rather than authorizing disposal.

## Per-Allocation Release

`release_allocation(id)` releases only that allocation, never its siblings.
Only backend release `Ok` supplies the SPI allocation-owner disposal premise.
Context records the successful member receipt before final bookkeeping, removes
that exact facade handle and backend index, and retains the original member as
Disposed. Subsequent reads, writes, launches and releases through the disposed ID
reject. Cleanup iterates only the remaining live IDs, in its existing order.

Every journal allocation/member and all of the writer's requested-byte/record
credits remain charged until the complete original roster is disposed. Successful
subsets neither publish content lineage nor return writer or allocation slots.
Cleanup therefore remains incomplete even after some facade handles disappear.

Rejected or Quiescent release errors retain the attempted member and original
diagnostic for retry; neither class is a disposal receipt. Terminal failure or
panic seals Context, quarantines the entire writer's request credits including
disposed members, preserves the original diagnostic/payload, and forbids further
backend cleanup. Existing synchronous singleton disposal is unchanged.

## Whole-Writer Commit

After every member has its own successful receipt, Context calls the existing
model `dispose_unknown` with the complete canonical roster. This atomically
retires all model allocations, membership and writer without producing recovered
content. Context then clears phases, refunds the exact retained request credits,
clears any surviving submission marker, and removes the writer root. Completion
status and callback delivery do not change.

A post-disposal model/bookkeeping failure retains the success receipts in a
terminal Context. If the model already retired but credit settlement fails, the
root explicitly records that state and remains included in the retained writer
count. No successful member is passed to the backend again. Final whole-writer
commit does not touch disposed backend handles: those numeric handles may
already belong to new Context allocations with different journal identities.

## Bounds And Limits

The retained records and disposal flags add O(k) storage for k destinations, all
acquired before submit. Canonicalization is O(b log b) for b writable bindings.
First disposal validates the complete roster in O(k); subsequent member lookup
is O(log k), with expected amortized hash lookup. Final commit is O(k). No disposal
step allocates a new receipt or roster. Retained-writer reporting also scans the
root index to include any post-model finalization failure. This is not an
aggregate byte-budget or throughput guarantee.

Backend release consumes allocation ownership under the existing **Contracted**
SPI; pooled KFD backing may remain resident. This does not prove native unmapping,
zero VRAM residency, input/read leases, alias safety, cross-run versions, generated
protected execution, ordered overlapping writers, content recovery, Rust/native
refinement or a performance improvement. Synthetic model-only non-Submission
multi-member writers remain unsupported. CPU fault injection and test-only
bookkeeping rejection do not qualify native device failures or prove the adapter.
