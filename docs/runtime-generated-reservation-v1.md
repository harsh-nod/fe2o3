# Finite Generated Reservation

R80 implements the local B3-DATA-REP and B4-RESERVE integration after
[R79 preparation](runtime-async-generated-preparation-v1.md). It retains complete
generated source identity, stages charged readback storage and returns a finite
reserved ticket. It does not adopt native allocations, submit a kernel, decode
outputs or expose a GPU-completion future.

## Ownership

The private host carrier implements a runtime-defined safe, data-only interface.
A simultaneous borrowed view names its immutable R77 projection, exact retained
HSACO and existing Worker authority. Runtime checks actual artifact bytes and
length, the existing five-coordinate authority join, authority currentness and
exact Context/native device generation. No new unsafe authorizer is introduced.
Canonical packet, fixup and hidden-field validity remain invariants established
by R77's immutable projection, not independently re-proved by this view.

A fixed sixteen-slot descriptive roster records every original buffer ordinal,
full length, access mode, checked aggregate readback bytes, fixup count and
original dispatch digest. Unused and read-only buffers are included. The original
vectors stay in their carrier; ordinary cropped snapshots and encoded-source
copies are not used. This roster is not a native registration or publication permit.

The host result gate retains a clone of the original shared R73 account. Readback
reserves one additional aggregate byte debit and one record, then allocates the
complete destination roster. Original encoded/typed debits remain unchanged.
Every nonempty decoder expectation must match its original buffer ordinal,
length and access; zero-length expectations retain custody without adding a
native buffer. Each new destination must have exact length and capacity.

For a sixteen-byte read/write output, preparation retains thirty-two bytes and
one record; readback additionally needs sixteen bytes and one record. These are
conservative admitted extents, not measurements of process memory. Metadata,
allocator overhead, artifacts, hidden kernargs, read-only policy copies and
aggregate domains still need complete accounting and MEM-5 composition.

## Transition

`try_reserve_prepared_v1` admits two new bounded reply cells before enqueue:
one reservation acknowledgement and one private eventual-completion cell.
Preparation's already-completed reply is not reused. No new operation slot is
created; the existing parked owner continues to count against its original limit.

The owner looks up exact ticket identity and phase before invoking its typed
adapter. Generic R79 preparations have no adapter and remain discard-only.
Readback is staged under immutable carrier access inside the retained-device
scope. Authority and closing native currentness must succeed before installation
can mutate the carrier. Ordinary failures destroy staged storage and return the
unchanged prepared ticket; a queued command guard also recovers it on Stop.

Success installs readback, retains its descriptive roster and completion producer,
and returns an opaque non-Clone `RuntimeAsyncReservedTicketV1`. Its completion
consumer stays private. `try_discard_reserved_v1` disposes exact host-only custody
on the owner; an old prepared ticket cannot discard or reserve that state again.
Reserved entries remain parked, do not flush and do not count as active work.

An adapter or installation panic terminalizes Context and returns an outer engine
error without a retry ticket. The registry retains the owner for shutdown or
quarantine. Acknowledgement failure cannot roll back installed custody. Dropped
observers or tickets do not dispose the owner; Stop detaches reply producers but
retains payloads until the existing cleanup decision. Discard accepted before
Stop may lose its ticket, as in R79, but never detaches storage from the owner.

## Disposal

Partial construction destroys its destinations before cancelling an unissued
reservation. A constructed readback owner explicitly releases retained credit
only after destroying all destination storage. Invocation field order is original
payload, readback owner, then decoder. Installed storage is never refunded merely
because GPU submission has not happened.

## Evidence Boundary

CPU tests cover the actual charged host storage and allocation loop, exact
command/registry transitions, bounded reply admission, wrong-phase replay, queued
Stop, observer loss, wakeups, panic retention and owner-thread drain/shutdown.
Source tests reuse the existing data-only authority fixture; they construct no
checked device or production compiler evidence. Transaction tests exercise the
production stage/install helper with injected stage failure and installation
panic. A simulated closing failure is not Linux retained-device qualification.

The protected host constructor is wired to the typed bridge, but successful
production construction and actual R73 storage through that constructor inside
the engine still require the compiler evidence handoff and genuine device.
No new Verus theorem or whole-adapter refinement claim follows from these tests.
The [local record](evidence/local-r80-generated-reservation-2026-09-10/README.md)
separates final source gates from native, proof and performance acceptance.

## Next Boundary

B3-DATA-ADOPT must move this same rooted owner into non-discardable active
custody before its first native effect, retain partial native prefixes, and join
lane, graph, drain and cleanup accounting. B3-ISSUE must retain exact authority
through actual deferred publication/retry. B4-COMPLETE must validate the exact
submission and complete returned roster before publishing all typed outputs.
Only their composition enables the public launch API and matched benchmarks.
