# Owned Host Drain Capture V1

DRN-1A adds one optional coherent HostVisible capture to the existing async
owner drain. Native validation, owned result credits and cutoff integration are
one implementation packet. This is not active-work hardware qualification,
generated-kernel authorization or complete A1/A2 acceptance. The
[swarm plan](runtime-a1-a2-swarm-plan.md) tracks those separate gates.

## Public Surface

- `RuntimeAsyncEngineConfigV1::with_drain_capture_byte_capacity` opts into a
  per-engine destination ceiling of at most 64 MiB. Zero, the default, disables
  capture. Account construction precedes owner startup; reported construction
  errors return a typed spawn error without losing an existing Context. This
  does not promise recovery from every allocator failure.
- `RuntimeContextV1::prepare_host_drain_capture_v1` registers one positive
  HostVisible allocation range. The sealed source records exact logical
  Context/device/allocation identity and extent without a native call.
- `RuntimeAsyncProgressHandleV1::begin_drain_with_capture` takes that source,
  a preallocated `Box<[u8]>` of exactly the registered length, and the existing
  cooperative drain tick budget. It returns a standard command future.
- `RuntimeAsyncDrainCaptureAdmissionFailureV1::into_parts` returns a fixed
  rejection reason, the source and the original unadmitted destination.
- `RuntimeAsyncDrainCaptureReportV1` separates the ordinary historical drain
  report from the capture result. `RuntimeAsyncCapturedBytesV1` exposes borrowed
  bytes, not an uncharged owned buffer or native authority.
- `RuntimeAsyncEngineHandleV1::drain_capture_bytes_in_use` is an inert byte
  observation; clones share the same account, including after owner shutdown.

Source preparation can occur before transferring a Context to the engine or
through an ordinary pre-cutoff owner command. Owned-engine startup returns the
actual initialized Context generation; a descriptor from another Context
rejects before closing admission. Allocation release invalidates a source even
if a later native handle or allocation extent happens to match.

## Cutoff And Capture

Admission preallocates the fixed reply/result metadata and reserves slice bytes,
one accounting-owner record and an existing reply-cell permit. The same admission
mutex orders command acceptance, ordinary drain and capture registration.
Only the winner installs the lifecycle request and closes admission. Failed
registration restores unadmitted destination custody and temporary credits;
it does not close an otherwise open engine. Separate byte/reply reservations
are not advertised as a multi-account atomic transaction.

Previously accepted commands may legitimately publish after cutoff. The owner
first drains that finite prefix using the existing command, graph, operation,
waiter and native-submission machinery. Only its conclusive branch can create
the private capture witness. Context validation rechecks live logical identity,
HostVisible kind, exact range/destination length, graph reservation and retained
submission quiescence before entering the additive backend SPI.

Native backing is resolved at capture, not at registration. Accepted D2H work
may create or promote that backing. KFD requires initialized native Host
storage, no pending/custodied operations, exact queue/pool affiliation and a
current mapped SharedGtt token. A stale Arc shadow after completed D2H is not a
failure: the copy reads current native host bytes without repairing the shadow.
Synthetic storage, DeviceLocal storage and synchronization/download fallback
are not admitted capture sources. Other backends inherit a fixed unsupported
response unless they implement the contract explicitly.

The CPU read-into checks operational currentness before and after the copy.
It performs no completion polling, flush, GPU publication, synchronization,
native allocation or boxed readback. Required observation-only reset-fence
readiness polling and DRM loss-counter observation remain enabled. Queue model
loan/retake preserves the existing native owner and its fail-closed panic path.
Already-poisoned queues report terminal uncertainty before copying.

The lower mapped-read helper authenticates storage identity, not independent
queue-wide quiescence. Production capture composes that helper with the private
Context witness, complete runtime pending/custody checks and move-only native
buffer ownership. A model loan or public drain report is not a substitute.

## Outcomes And Ownership

| Outcome | Delivery and retention |
| --- | --- |
| Quiescent, capture succeeds | Deliver the complete owned bytes before cleanup; the result keeps its byte credit after leaving the reply cell and after engine shutdown |
| Quiescent, logical source or structural preflight rejected | Deliver a quiescent drain report plus fixed capture error, no bytes; cleanup remains separate |
| Native observation/read/retake failure or already-terminal queue | Seal Context, discard the private destination and return `CaptureFailed`; never publish a quiescent capture report or release ambiguous native custody |
| Capture panic or owner interruption | Existing owner containment returns `EngineStopped`, not partial bytes or a fabricated drain report; possibly live native ownership is retained |
| Drain tick budget exhausted | Return the budget-exhausted drain report and `CaptureIncomplete`; no capture occurs and unresolved native ownership is quarantined |

Pre-copy rejection leaves the private destination untouched. Failure after a
partial CPU write discards the destination; it cannot become an accepted result.
Dropping a future abandons observation, not the accepted drain. The result's
destructor disposes its known boxed storage before returning its retained byte
credit. Reply-cell credit is independent and follows reply-cell lifetime.

The ceiling measures the actual owned slice extent, not a caller cost estimate.
The caller's earlier allocation is outside this admission operation. Account
bootstrap, allocator overhead, fixed metadata backing, arbitrary wakers and
generic results are not included in this slice-byte measurement. MEM-5 must
account for them before any aggregate runtime-memory bound is claimed.

## Verification Boundaries

R69 supplies a production-used range predicate, a corresponding Verus source
and five named mutations covering zero length, short/long destination, overflow
and out-of-allocation access. Its four obligations describe exact range
acceptance and exclusion, not native currentness, witness issuance, Rust
ownership or whole-executor refinement. The Rust/Verus source correspondence
is reviewed, not mechanically linked.

CPU coverage includes logical registration, opt-in limits, atomic cutoff races,
late backing, abandoned observers, result extraction/shutdown lifetime,
pre-entry rejection, terminal partial writes, panic, exhausted drain, exact
native token coordinates and already-poisoned queues. Allocation-counting tests
cover production adapter helpers with bounded injected readers and ordinary
rejection paths. They are not live Linux syscall or full-owner zero-allocation
qualification, and do not bound panic machinery or kernel-internal allocation.

The [local validation record](evidence/local-r69-host-capture-2026-09-10/README.md)
records executed gates separately from these implementation contracts.
DRN-2 still requires signed outstanding-work captures with complete independent
output/padding checks. Production-generated cells need matching compiler and
machine evidence. No new hardware acceptance, overlap or HIP/HSA performance
claim follows from this implementation.
