# Charged Generated Results V1

R73 implements GEN-2R: owned generated data with result-peak admission. It does
not implement generated invocation authority, native publication or completion.
Those remain GEN-2A/B and the compiler's exact protected-verification and
semantic-to-machine handoff. The legacy GEN-1 data API remains available and
unchanged; its bare `Box<[T]>` result state never receives charged-path data.

## Public Boundary

- `GeneratedRuntimeResultBudgetV1` supplies byte and member admission. Cloning
  this handle shares one account; it does not create fresh capacity. Usage is
  an inert observation, not completion or residency evidence.
- `GeneratedRuntimeReadWriteSlice::new_charged` and
  `GeneratedRuntimeWriteSlice::new_charged` consume an owned typed seed and
  return a separate `GeneratedRuntimeChargedResultV1<T>` observer. Read-only
  arguments continue to use `GeneratedRuntimeReadSlice`.
- `AuthenticatedWorkerV3ExecutableV1::prepare_generated_runtime_arguments_charged`
  validates current compiler publication and the exact generated packing plan,
  preflights every member, reserves the complete roster and then encodes inputs.
  The result is `GeneratedRuntimeChargedArgumentsV1`, a data owner exposing only
  inert kernel, footprint and packing observations.
- `GeneratedRuntimeChargedResultV1::try_take` returns `None` when data is not
  currently extractable, including transient slot-lock contention. It never
  waits for that mutex. Once committed and uncontended, extraction returns one
  move-only `ChargedTypedResultV1<T>`; later extraction or abandonment reports
  unavailability. Poisoned custody fails closed.
- `ChargedTypedResultV1` exposes a borrowed slice, length and emptiness. It has
  no `Clone`, raw `Box`/`Vec` extraction or public constructor. It retains its
  charge independently of the observer and producer lifetimes.

There is no public charged decoder, caller-provided completion authorizer or
launch operation. Creating charged wrappers before preparation is not result
admission. Caller storage is charged on transfer into the prepared result state,
not retroactively before the caller allocated it.

## Accounting

Each memory member consumes one R70 ledger owner slot, including zero-length
members. It does not consume the unrelated `AllocationRecords` vector coordinate.
Scalar-only invocations have no memory roster and bypass the nonempty batch API.

| Member | Reserved `ReplyBytes` | Refund boundary |
| --- | --- | --- |
| Read-only, `b` bytes | `b` | Disposal of its returned encoded storage, or abandonment with input storage disposed |
| Typed output, `b` bytes | `2 * b` | Disposal of the charged typed result, or abandonment with all covered storage disposed |

Typed destinations reuse the original seed `Box<[T]>`; decoding allocates no
second typed destination. The output debit conservatively covers the encoded
and typed peak and remains whole after the encoded bytes are disposed. It is
not split or reduced after decoding. Overflow rejects before reservation.

Complete-roster preflight reserves every member before input encoding. Binding
then checks the same ordered scalar, extent, access and output-slot identities.
Mixed legacy/charged routes, repeated output slots, reordered slots and
equal-width scalar substitution reject. Each read or output member binds the
same private readiness gate; equal-capacity accounts are not interchangeable.

The bound is result storage, not total invocation/process memory. Explicit
kernarg bytes, descriptors, `Arc`/mutex/ledger metadata, allocator overhead,
caller construction and native executable/control storage are excluded. The
existing runtime preparation path may also make read-only initialization copies;
GEN-2 integration must account for those separately. MEM-DOM/3/4/5 still own
aggregate native/host accounting, bootstrap headroom and retained metadata.

## Decode And Disposal

The private decoder validates the complete nonempty-buffer count, exact lengths,
capacities and accesses, plus every output/read gate binding before writing any
typed result. It decodes sealed primitive scalar types in place, disposes all
returned encoded buffers, then publishes one shared Release/Acquire ready gate.
No observer can extract an earlier result from a partially decoded roster.

Errors and unwinding dispose returned buffers before decoder-owned credits.
Uncommitted output abandonment removes the slot state under its mutex, releases
the lock, then disposes storage before refund. Poison recovery is used for
disposal only; it does not make a poisoned observer usable. Failed core refund
transitions retain/quarantine their debit. Dropping an observer is not producer
cancellation; the producer retains the prepared storage until its own disposal.

The GEN-2A/B bridge must retain the private decoder with exact invocation and
operation custody until return-buffer disposal or transfer into the decode
transaction. A read-result credit does not independently own those buffers.
The crate-private input/decoder tuple must not be detached while covered storage
can remain live. Charged readiness is data commit, never GPU-completion evidence.
This observer has no Future or wake-registration implementation. A future
adapter must arrange wake/retry when contention makes extraction temporarily
unavailable; a single completion notification is not sufficient by itself.

## Complexity

The generated ABI bounds the memory roster at `MAX_ABI_FIELDS` (64). Complete
duplicate-output validation is quadratic only in that bounded roster, with at
most 4,096 comparisons per scan; encoding and decoding are linear in bytes.
All destinations exist before decoding. The one readiness commit does not
require per-output publication allocation or polling threads. These are
algorithmic bounds, not measured HIP/HSA performance results.

## Proof And Test Boundary

Production calls the R73 cost and exact-shape guards. Their Verus counterparts
prove typed two-copy/read-only one-copy costs, overflow rejection, zero-length
acceptance, zero unrelated coordinates and complete shape/access checks. Rust
and Verus correspondence is reviewed, not mechanically extracted. R70 supplies
the separate batch-reservation arithmetic properties.

Twenty charged-path host tests cover seed identity, both routes, preflight
exhaustion, partial-binding error/unwind, lower packing failure, exact scalar
and output identities, empty/scalar-only arguments, read-credit gate mismatch,
mapped WriteOnly output, complete-result validation, concurrent publication,
poisoned-slot disposal, nonblocking slot polling, independent result disposal
and observer/producer drops. The polling test's ten-second guard bounds a
blocking regression and releases all threads before asserting; it is not a
performance threshold or speedup measurement.
All ten sealed scalar types include integer boundaries and floating-point bit
patterns. Three model tests exercise cost/shape guards. Four generated fixture
negatives and four compile-fail doctests prohibit relevant ownership escapes.

Synthetic returned buffers exercise the production private packing/decoder
helpers after original inputs are disposed. They do not authenticate a compiler
deployment or native completion. Injected `Error::Allocation` is not an allocator
failure test; the after-output panic is not a sealed-scalar decoder panic; a
joined ordinary producer thread is not `RuntimeAsyncEngine` shutdown. Mutex/Box
ownership adapters and the full native executor are not proved by the seven
R73 obligations. Exact validation logs and remaining boundaries belong in the
[local evidence record](evidence/local-r73-charged-results-2026-09-10/README.md).
