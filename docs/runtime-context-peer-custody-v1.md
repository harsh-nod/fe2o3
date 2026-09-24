# Context Scalar Peer-Copy Custody

This is a prerequisite for Context-directed dependency progress and pending-source
admission. It is not their completion, a new backend execution guarantee, or
HIP/HSA parity.

## Ownership

Every ordinary scalar `RuntimeContextV1::peer_copy` keeps a private Context root
containing its original stream, backend stream, source/destination allocation
records and exact regions. Its dependency snapshot contains each original event,
backend event, producer submission, backend submission, device and stream. The
snapshot is canonicalized by producer/event for linear grouped accounting; each
entry retains its original ordinal. The backend receives the original event order.

The root map and complete roster are reserved before journal Begin or backend
entry. The root and producer retain increments are installed before the backend
can accept work. A returned zero or duplicate backend handle still has its own
Context record and root before protocol sealing. A terminal error or panic can
leave a provisional root without a returned handle; cleanup reports that root.

Every event edge retains its producer's Context submission independently of the
public event. Multiple events aliasing one producer are preserved and counted
together, without changing the concrete backend's right to reject them. Releasing
events or destroying producer streams does not erase the snapshot. Producer
completion alone does not release the consumer's holds.
These metadata roots create no new allocation or native lease authority; buffer
custody remains with the existing optional journal and concrete backend.

Public submission release rejects with `SubmissionRetainedByDependency` while
internal retains remain. The direct cleanup release loop checks the same count.
Successful consumer completion, cancellation, conclusive quiescence or definite
initial rejection discharges its holds once. All grouped decrements are validated
before mutation. Original backend errors keep precedence over secondary settlement
failures. Terminal errors, detected malformed custody and backend panics seal
Context and retain unresolved roots; panic containment also applies without the
optional journal.

Completed roots remain as immutable operation provenance until submission release.
Their old allocation/event descriptions are metadata, not live authority, and need
not remain in the public registries. Cleanup requires all scalar roots, including
provisional ones, to be gone before it reports complete.

Admission costs O(D log D), with D at most 256 dependency events. Retain installation,
validation and discharge cost O(D) expected hash-map work. Root storage is O(S + E)
for retained scalar submissions and their event edges. Existing submission/event
limits remain; this is not an end-to-end byte budget or a bound on native resources.
Validation checks the touched roster and sufficient per-producer counts, not a
global recomputation of every count or arbitrary-memory-corruption authentication.

## Journal And Execution Boundaries

Context now owns the existing `ContextProducerReadJournalV1`. Its immutable stable
projection and forwarding operations preserve existing stable-input behavior.
No second version journal is introduced. The producer arena and stable arena share
the configured reader budget, but this packet acquires only stable leases.

Pending source allocations still reject before a submission identity or backend
call. Ordinary `peer_copy` is not a success-gated dependency contract. Neither
the scalar marker nor the public transfer digest grants that guarantee. Ordered
segments retain their separate path and do not acquire scalar provenance. Generated
submissions are excluded as dependencies while their owner-only attempt is live.
Worker wire protocols, backend poll/wait and strict nonwaiting flush are unchanged.

The next integration needs an explicit scalar directed-route success-gating
contract, exact producer reservations, producer-first Context reconciliation and
one bounded backend progress action per async budget quantum. Consumer completion
must never manufacture producer success. Native chain, canary, fault and cleanup
qualification follow that integration.

## Evidence Boundary

The [CPU checkpoint](evidence/dev-context-peer-custody-cpu-2026-09-23/README.md)
records GNU/musl regressions, static checks, unchanged source inputs and retained
development failures. It adds no native or performance measurement.

Changing `context.rs` changes an inherited input to the historical constructor-origin
owner-lifecycle proof packet. That packet remains evidence for its captured source,
not a live-tree proof of this integration. No checker pins or old records are
rewritten. Context event binding, physical storage/unwind and native execution
refinement remain separate proof obligations.
