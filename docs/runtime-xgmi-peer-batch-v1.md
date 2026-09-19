# Explicit XGMI Peer-Copy Batches V1

This development API amortizes a native currentness scope across an explicit
roster of ordinary peer-copy submissions. It does not change ordinary `wait`,
`poll`, stream flush, event identity, or per-submission journal ownership.
It is not encoded by Worker V1-V5 and is not a HIP/HSA parity claim.

## Contract

`RuntimeContextV1::wait_peer_copy_batch` takes 1..=63 distinct pending peer-copy
handles from one Context and one destination device. The optional
`RuntimePeerCopyBatchBackendV1` SPI receives their backend IDs and one absolute
deadline established before preparation. Expiry permits one completion scan;
mandatory closing currentness may extend beyond the deadline.

The native XGMI implementation accepts exactly the complete ready roster in a
direction with no in-flight work, or exactly the complete already-published
in-flight roster. It validates scheduling indexes and ownership before effects.
Ready work follows existing FIFO order; retry follows the existing in-flight
index. Dependency-blocked successors remain outside the native batch and keep
their dependency, stream and allocation custody. Caller subsets are rejected.

- Ready execution maps owners, opens one full currentness scope, publishes one
  batch/doorbell, waits for all fences, then performs a full closing observation.
- Pending retains every ticket and native mapping, including already-ready
  members. Retrying never republishes the batch or retires a ready prefix.
- Every mapping is restored before any backend Success is recorded. Context
  validates the whole journal roster before entry and before settlement, then
  uses ordinary per-submission settlement and callbacks in caller order.
  Callback delivery is not an atomic group operation.
- Recoverable post-preparation failure settles the entire quiescent roster
  without a result. It never reports initial no-effect rejection after mapping
  effects. A secondary journal failure cannot replace the backend diagnostic.
- Ambiguous publication or a failed closing observation terminalizes the
  backend and quarantines queue/mapping/session custody. The process-global KFD
  runtime gate is irreversibly poisoned. No Success or reusable native owner is
  returned from these paths.
- Unwinding through native preparation or execution aborts the process, without
  formatting diagnostics or dropping the panic payload in the abort handler.
  This is an explicit process-exit profile, not resumable unwind recovery.

Single-packet host-attribution diagnostics do not cover aggregate calls. An
aggregate invalidates an enabled capture rather than silently omitting calls.

## Measurement

The existing `gfx942-runtime-xgmi-peer-benchmark` accepts the opt-in exclusive
flag `--aggregate-peer-batch`. Default behavior and result format are unchanged.
Aggregate depth is bounded at 63; ordinary benchmark depth remains bounded at 32.
The aggregate output has a distinct schema and progress/timing labels. Both
paths validate payloads and canaries and explicitly tear down native resources.

Measure ordinary and aggregate modes from the same signed source, binary,
devices, payload, depth and run controls. A maximum-depth correctness run is not
a matched performance comparison. Shared-host endpoint checks do not establish
exclusive reservation or continuous absence of interference.

## Evidence Boundaries

Executable tests exercise Context journals, callbacks, failure classes, native
operation ordering with move-only fixtures, and the actual lower fence-wait
algorithm with mapped-memory fixtures. These are not machine-code refinement,
native driver failure injection, or a proof of complete HIP/HSA behavior.
The lower aggregate path still performs allocation and currentness work; there
is no allocation-free, GPU-latency, or orders-of-magnitude speedup claim.
