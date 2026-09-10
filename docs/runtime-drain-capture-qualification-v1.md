# Copy-First Outstanding-Work Drain Qualification V1

This is the DRN-2A copy-only qualifier, not a compute qualifier, performance
benchmark, or proof of physical overlap. The implementation consists of
`crates/fe2o3-runtime/examples/gfx942-runtime-drain-capture.rs`, the immutable
CopyOnly qualification observer in
`crates/fe2o3-runtime/src/kfd_backend/qualification_drain_capture.rs`, and
`benchmarks/runtime_gfx942/check_drain_capture.py`. The signed-source runner is
`benchmarks/runtime_gfx942/run-drain-capture-mi300x.py`; it inherits the existing
owner runner's selected-device guards, binary audit and owned-stage cleanup.

Source implementation and synthetic checker tests do not establish a successful
native campaign. Hardware acceptance additionally requires the signed-source,
actual-binary, selected-device, execution-log, census and cleanup evidence of the
owner runner. No such result is asserted by this protocol document. GEN-2
generated executable authority is not needed for these copies and is not supplied
by their successful qualification.

## Frozen Cells

The eight ordered cells are the product of:

1. Cutoff with all work queued, or with the first H2D copy natively retained.
2. One stream using three standalone owned copy operations, or two streams using
   one exclusive five-node graph.
3. Retained or dropped operation/graph result observers.

The capture future is retained in every cell. The single-stream and graph
matrices use separate Contexts because graph reservation excludes independent
operations. There is one runtime owner thread at a time; the eight Contexts and
native sessions are opened, drained and explicitly shut down sequentially.

The two-stream graph is H2D, host event record, host event wait, D2H body, D2H
whole-device verification. Its edges intentionally order the work. These host
joins are not an HSA native-event or GPU-concurrency claim. Each copy carries an
actual native receipt when it is published; graph completion also checks all
five exact node identities and the graph execution identity when that observer
is retained.

## Workload And Cutoff

Let `BODY = 1024 * 1024 + 257`, `DATA = BODY + 514`,
`VERIFY_OFFSET = DATA + 193`, and `CAPTURE = VERIFY_OFFSET + DATA + 211`.
Each cell allocates exactly three Context allocations: a coherent HostVisible
input of `DATA` bytes, a DeviceLocal buffer of `DATA` bytes and a coherent
HostVisible output of `CAPTURE` bytes. The Context request account admits exactly
`2 * DATA + CAPTURE` requested bytes and three allocation records. These are
request credits, not padded backing, device-pool, process-residency, or allocator
overhead measurements.

Before the cutoff, two completed and released async warmup copies materialize
real directional native backing and the queue. The whole device buffer then
contains `0xa5`. CPU writes reset the input and output; the whole output is
`0xd3`. Input bytes outside the source body are `0xc7`. For cell ordinal `o`,
body-relative byte `i` is `(i * 29 + i / 257 + o * 17) % 251`, with integer
division. The complete three-copy accepted prefix is:

| Copy | Source | Destination | Bytes |
| --- | --- | --- | --- |
| H2D | Input offset 131 | Device offset 137 | `BODY` |
| D2H body | Device offset 137 | Output offset 139 | `BODY` |
| D2H verification | Device offset 0 | Output offset `VERIFY_OFFSET` | `DATA` |

The final transfer is a pre-admitted part of the workload, not a verification
download added after admission closes. The single capture covers the complete
output, including its leading/trailing guards and gaps, and the embedded complete
device buffer including both device guards. Python independently constructs the
expected body, device and output and compares all three SHA-256 hashes. Odd
offsets and the non-aligned tail are fixed profile coordinates.

The source is registered before admission closes and the caller allocates the
capture destination before `begin_drain_with_capture`. That call reserves the
exact destination slice-byte capacity and a reply cell. It does not claim to
reserve caller memory before the caller allocated it or measure reply metadata
bytes.

An initial bounded callback pause establishes the empty native warmup frontier.
For queued cells all work is accepted while this owner pause holds, and then
capture closes admission. For native-retained cells the owner is first released
to publish the H2D. The forwarding observer pauses the owner immediately after
the actual backend call has produced a retained directional native receipt,
before returning to the owner loop. This can occur inside `copy_async_v1` before
Context receives its backend ID, or inside flush; the accepted owner command
and native receipt are retained in either case. The main thread then accepts any remaining
standalone operations and closes admission with capture. It never waits for the
first operation result while the publication pause is held. Both pauses use
bounded channel handshakes, not elapsed sleeps as evidence of publication.

This pause stops only host observation/progress. The GPU may already have
completed a retained copy at cutoff. Native retention means the exact native
record and its endpoint custody still exist, not that a GPU engine is physically
busy. Accepted-prefix work may publish after cutoff. Submission acceptance or a
`Pending` value alone is never treated as publication.

## Observation And Result Contract

The hardware-qualification-only CopyOnly constructor denies all compute launch
authority. It neither accepts an unsafe caller authorizer nor changes the R66
R26/persistent-compute gate. Each immutable observation checks the complete
bounded copy roster against the actual native queue, receipts and retained
endpoint identities. Ready rows are runtime custody, not native publication.
The observer never polls for completion or advances the queue.

The wrapper forwards original backend outcomes unchanged. Its trace storage is
preallocated before owner startup: at most two stream IDs, three allocation IDs,
three copy calls and three retained native receipts. Capture entry/return
observations use fixed-size snapshots and do not grow heap storage. Output JSON
and hashes are constructed after owner shutdown. The wrapper rejects its own
evidence if an ordinary read or write occurs after setup, if receipt collection
overflows, or if repeated observations change the receipt for the same copy.

The frozen backend observation has at most eight active copies, eight dependencies
per copy, one native-published copy at a time and 32 lifetime async-publication
IDs. The qualifier requires exactly two warmup IDs followed by the three exact
work submission IDs. Each work ID must also have its independently collected
native receipt, runtime membership identity, stream, source/destination IDs,
offsets, length, dependencies and packet count. A publication-history counter
alone is insufficient. The history excludes synchronous copies and is not a
physical-completion history.
Only successful native publication transitions append history; restoring an
index after a pending poll or wait does not. Pending copy state is validated
against the active owner/custody indexes, with any conflicting terminal-table
entry rejected. These corrections also repair the R66 membership observer's
former pending-table assumption without changing native receipt/digest checks.

Capture entry and return must have identical empty active-copy rosters and
identical complete publication histories. The drain must be quiescent within 128
ticks, exhaust queued commands, and leave no active graph or owned operation.
Standalone completion records remain until owner cleanup; graph records are
released before successors run. Thus the final retained submission roster is
three succeeded records for standalone cells and empty for graph cells.

The result remains charged for `CAPTURE` bytes through future extraction and
owner shutdown. Explicit successful owner cleanup must leave no allocation
credit records. Dropping the captured bytes must refund their slice-byte debit;
after all observers and the owner have been disposed, reply and snapshot usage
must also be zero. A stopped owner or a dropped observer is not completion proof.
Failures produce no success record for a partial campaign.

## Independent Checker

The output is exactly one UTF-8 JSON line with a final newline, schema
`fe2o3.runtime.drain-capture-copy.v1`, at most 262144 encoded bytes and exactly
eight ordered cases. The reader limits its file read before parsing. Duplicate
fields, unknown fields, non-integer number tokens including finite/overflowing
floats, nonfinite values, numeric booleans, invalid encodings and duplicate or
relabelled cells reject. Exact resource-role, geometry, dependency, receipt,
history, drain, result-credit and cleanup contracts are independently checked.
The runtime membership digest is recomputed from the native receipt and seven
little-endian `u64` coordinates: submission, stream, source, destination, source
offset, destination offset and length, using the existing R66 membership domain
and `copy` tag. A stale membership digest cannot accompany relabelled coordinates.

The checker is intentionally a consistency checker. It cannot cryptographically
authenticate an arbitrary JSON producer or infer native membership from a
digest-shaped string. Coherently relabelled synthetic evidence with recomputed
unkeyed membership digests can remain
internally consistent; a CPU test documents that boundary. Actual production
observers plus the signed-source/actual-binary owner runner supply the separate
execution-provenance acceptance layer. Synthetic fixtures cannot replace it.

The `physical_overlap` and `performance` fields must both be `unmeasured`.
Neither wall-clock intervals nor native retention are accepted as overlap,
throughput, HIP/HSA parity, full async-drain proof, or generated compute evidence.

## Validation Gates

- Local all-targets compilation for the feature-gated example and focused native
  observer mutation tests, followed by the normal runtime regression gates.
- Synthetic checker acceptance plus rejection of malformed/bounded-input cases,
  one-coordinate workloads, incomplete receipts, wrong cutoff phases, history
  changes, incomplete drains, premature byte refund and incomplete cleanup.
- Review that the capture-stage observer is immutable and bounded and that native
  receipt acceptance remains rooted in real lower-level retention.
- Separate signed native execution of all eight cells on a selected idle gfx942
  device, independent checker success and confirmed runner cleanup. This remains
  a hardware gate, not a consequence of local tests or abstract proofs.
- Existing DRN-1A capture and resource-credit proofs continue to have their own
  authenticated source rosters. This qualifier does not add a proof of the
  complete engine, GPU completion, machine code, or compiler handoff.
