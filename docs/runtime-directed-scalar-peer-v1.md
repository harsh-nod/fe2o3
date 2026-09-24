# Directed Scalar Peer-Copy Backend Contract

This is the native contract prerequisite for Context pending-producer reads,
not their admission or A1/A2 acceptance. It adds an explicit backend SPI without
changing legacy peer-copy, ordered-copy or Worker contracts.

## Exact Identity

`RuntimeDirectedScalarPeerCopyBackendV1` accepts an untrusted
`BackendDirectedScalarPeerCopyV1`: the destination stream, both backend device
handles, both allocation-relative regions (including access), and the original
ordered event/expected-producer pairs. The native XGMI implementation checks
these against its own stream, allocation and event records before calling the
existing scalar admission path. That path retains its depth, overlap, owner,
queue and capacity checks. No caller-supplied digest grants execution authority.

Every dependency must itself have retained directed-profile metadata. This
includes completed producers: the old completion record cannot distinguish a
legacy scalar copy from an ordered sequence. Duplicate producers, including
different events naming one producer, reject before admission. Event identities
need not remain live after successful admission.

The retained root contains the complete route, original allocation extents,
ordered dependency bindings and provisional/admitted phase. Its expected handle
is the exclusively borrowed backend's next handle. Storage is reserved and the
root installed before entering shared scalar admission. The returned handle and
installed scalar record must agree before the root is accepted. A definite
rejection removes only the provisional root. Terminal failure, unexpected
quiescent admission or unwind seals the backend and retains that root; original
error payloads and panic payloads are preserved. A quiescent error is promoted
to terminal rather than granting cleanup authority for an unreturned handle.

## Progress and Lifetime

`BackendDirectedScalarProgressV1` repeats the exact route and ordered producer
submission IDs. Every coordinate must match before a progress action. Active
roots also match live allocation/stream records and the scalar owner; completed
roots match retained provenance and the terminal record, without requiring
already-released events, allocations, streams or predecessors to exist.

The existing native selector performs at most one publication (up to 63 scalar
copies), one ticket observation or one failed-dependent settlement. Selection
has at most 256 dependency levels and 256 dependencies per node. It does not
wait, sleep, recursively poll or create a background task. A driver syscall has
no new hard wall-clock bound. A returned terminal status belongs only to the
requested submission, not to a progressed dependency or an unrelated FIFO
blocker. Unexpected quiescent progress errors are conservatively terminal;
they do not establish the requested operation's completed state.

Success gating uses the native producer result. Every dependency must have
`Succeeded` before consumer publication; failure and cancellation do not qualify.
Ordinary flush/poll/wait and incidental FIFO progress remain valid. A host journal
may lag actual device completion: the forthcoming Context adapter must retain
the consumer's backend result and reconcile producer outcomes before publishing
Context success. Suppressing native readiness until host observation is not
required for that invariant and would unnecessarily change strict flush.

Cancellation and completion preserve directed provenance. Ordinary submission
release retains its active/event/dependency guards and validates directed
metadata before removing any completed record. Successful release removes the
metadata with the completion/depth records. Shutdown and Drop account for both
admitted and provisional roots; an unresolved root is not silently discarded.

## Bounds and Evidence

Each root has at most 256 dependencies and the root map has at most the existing
submission-count limit. Additional storage is O(S + E) for retained submissions
and dependency entries. This is a finite count bound, not an aggregate byte
budget: the largest configured count profile can still consume substantial
memory. Admission validates immediate producer rosters, not the transitive
graph; worst-case work is O(D squared log D) with D bounded by 256. Exact
progress matching adds O(D log D) validation to the existing bounded selector.
No whole-submission-map scan or new background worker is added.

Completed-route matching relies on immutable private provenance, not redundant
independent copies or arbitrary-memory-corruption authentication. CPU fixtures
exercise shared admission/rooting/matching/failure logic using scripted leaves.
They are not native mapping, currentness, ticket or physical-copy evidence and
do not establish executable/formal correspondence or HIP/HSA performance.

## Next Integration

Context must add a distinct directed-copy kind, exact pending-writer/member and
event bindings, producer-read reservations alongside stable readers, and
producer-first bounded progress. Every completion ingress, including ordinary
poll/wait/events/drain/cleanup, must retain an early consumer terminal observation
until exact producer reconciliation permits logical settlement. Release,
cancellation, Unknown disposal, generated-domain exclusion, accounting and
callbacks must preserve those roots. Async integration must avoid an extra
generic flush in the same targeted-progress quantum. Native chain, canary,
fault and owned-cleanup campaigns follow; none is supplied by this SPI alone.

The generic Context observation paths need no new backend trait bound or stored
capability function pointer. A private planner can select the requested
observation, one ordinary poll of an exact retained producer on another stream,
or local finalization. Retained terminal observations must precede fallible
journal settlement. Producer chains require iterative, bounded reconciliation,
including shared predecessors; repeated observations must agree with the first
terminal fact. Native success paired with an incompatible retained producer
result must not mint logical success. Previously discarded producer results
remain unknown, even if the consumer later reports native success. Definite
cancellation and other exact quiescence exits must still release consumer input
reservations without requiring unrelated producers to finish. This is the next
implementation design, not behavior enabled by this backend-only checkpoint.
