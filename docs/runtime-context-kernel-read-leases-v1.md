# Context Typed-Kernel Read Leases V1

Development toward A1/A2 in #182, extending the [copy-source contract](runtime-context-copy-read-leases-v1.md).
Accepted checkpoints remain Native R125 CPU/test, Admission R118B C1-C3 and
Resources R116/V3. This is not V6, A1/A2, native/formal or HIP/HSA acceptance.

## Input Ownership

The opt-in Context journal now retains independent input batches for ordinary
typed launches, frozen snapshots, atomic/collective launch forwarding and prepared
graph launches. It preserves the exact ordered backend binding roster and pointer
patch offsets. Separately, pure `Read` bindings are sorted and deduplicated by
original logical allocation ID into whole-allocation input leases. Overlapping,
duplicate and disjoint read ranges on one allocation use one lease per consumer.
Any allocation also bound `Write` or `ReadWrite` uses only its exclusive writer
record, avoiding a self-conflicting reader.

Each prepared read retains its original logical ID and exact allocation record.
Before issue, Context revalidates the live record, backend index and applicable
allocation credit. Epoch and lineage are captured at actual issue, not graph
preparation, so a completed predecessor write can supply the next read's version.
Pending and Unknown writers reject read admission; an event or same-stream order
does not authorize premature access. Leases exclude whole-allocation writes and
retirement, even outside the declared input subrange.

Read-only launches need no writer slot. The reader arena's capacity still equals
the constructor's writer capacity, but occupancy is independent. All source
requests, output/reference vectors and ownership-map insertion headroom are
reserved before the submission ID, writer Begin or backend call. Capacity or busy
source rejection leaves the entire batch unacquired.

The original source batch is rooted by the genuine Context submission ID before
model acquisition. A failed acquisition or unwind can therefore retain its
sources even without a returned handle or installed submission record. The
root is independent of the writer and returned token. Active lease count plus
provisional batch-root count is exposed for diagnostics and cleanup completeness;
it is not an available-byte count.

## Completion And Failure

Copy and kernel inputs use the same batch settlement boundary. Initial Rejected
or exact quiescence releases readers. Pending, rejected observations, TooLate,
token drop and metadata lifetime alone do not. Terminal failures, panics and
malformed handles retain input custody and quarantine source credits once per
allocation, including all-read-only submissions and shared sources.

Before releasing any lease, Context checks the installed first-reference/count
marker, complete source/request/reference lengths, exact original consumer,
contiguous fresh incarnations, live allocation records/indexes/credits, model
requests and any coupled writer marker. The model validates and releases the
entire batch atomically. Source release precedes writer settlement, status and
callback notification. Repeated observations cannot release a later consumer.
Unknown-writer disposal rejects either a retained reader root or installed reader
marker before backend disposal and again before final model/credit retirement.

The default `open` profile still has no journal exclusion or initialized-data
authority. It now retains and revalidates explicit `Read` allocation identities
at prepared issue, including a separate Read alias of a writable allocation.
This requires bounded source metadata and can report Capacity before submission.
A lone `ReadWrite` binding remains writer-only and does not acquire this default
profile's Read identity check. Backend aliasing remains a separate boundary.

## Cost And Evidence

The new source/destination roster canonicalization costs O(b log b) for b bindings;
this is not a complexity claim about the complete existing launch validator.
Batch admission/release visits k distinct inputs with expected hash lookup costs,
without scanning unrelated allocations. All batch vectors are allocated before
issue; model acquire/release and reference installation do not grow them.
Terminal quarantine traverses retained reader/writer roots. Cleanup diagnostics
also scan reader roots for provisional batches. This is bounded metadata, not
an aggregate native-memory budget or latency/throughput measurement.

The opt-in deferred kernel mock retains only ordered descriptors at submission.
Modeled successful completion samples their original backing memory and records
exact ordinal, range, patch offset and bytes. Initial Quiescent can sample before
returning failure. Failed/cancelled modeled paths discard descriptors without
sampling. Distinct per-buffer/per-offset patterns catch source or range swaps.
This mock performs no kernel arithmetic; atomic/collective tests establish
contract forwarding and input lifetime, not atomic or collective execution.

Qualification is recorded in the [final kernel-input archive](evidence/dev-v6-kernel-read-leases-final-2026-09-17/README.md).
The private stale-preparation test deliberately interleaves preflight and Begin
to exercise provisional-root failure. It is not a publicly reachable race or
native fault injection. The post-acquisition assertion/panic window is
source-reviewed, not an injected execution path.

## Remaining Acceptance

Input leases preserve custody and version stability, not initialized bytes or
admitted kernel effects. Lineage zero remains legal; nonzero lineage does not
prove complete initialization. Production Worker V3 and machine refinement,
backend aliases, initialized/available-input authority, ordered overlapping
writers, cross-run version/reuse authority, aggregate pools/residency, native
high-depth/overlap/failure campaigns and matched HIP/HSA measurements remain open.
The existing model implementation is reused; no new solver proof or Rust/native
reader-composition proof is supplied by this packet.
