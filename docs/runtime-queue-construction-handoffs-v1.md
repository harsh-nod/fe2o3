# Queue Construction Handoffs V1

R91 implements NATIVE-2A.1 above signed planning checkpoint
`b66e52d3246a88d76eb30252b9cfe465fa2c9882` and R90 bind settlement. This is a
private, production-used lower-handoff contract, not complete constructor
retention. Local acceptance and attempts are recorded in the
[R91 evidence](evidence/local-r91-queue-handoffs-2026-09-11/README.md).

## Ownership

`queue_live/construction.rs` retains ring construction in one of these phases:

```text
CPU -> Mapped -> Retained -> Transferred
CPU/Mapped -- admitted failure --> InSession (exact R88 terminal custody)
```

The three backing variants remain distinct: production AQL GTT, executable
probe GTT and USERPTR probe. Each carries the existing move-only typed token;
there is no new allocator, queue planner, resource authority constructor or
native backend. The private `RingMemoryV1` interface only forwards borrowed
prechecks and the existing map/role-retention primitives. It allows the same
sequencer to execute against the existing fake-native R88 fixture.

Before a consuming handoff, a borrowed precheck requires an active session,
no existing terminal transition, and the exact token/record phase. This is
necessary because R88 does not retain inputs rejected before its admission.
Precheck failure leaves the actual token in the caller's ring stage. With the
session exclusively borrowed and no intervening effects, the admitted token
moves into R88 with an exact `InSession` identity marker in the caller's stage.
Successful mapping and role retention are each stored before the next step.
An admitted error or panic leaves the matching actual precursor or successor
in R88 terminal custody; the marker alone never supplies authority or cleanup.

`QueueResourcePrefixV1` owns ring, control, EOP and context-save authorities,
and a separate completed-output slot. The existing resource planner now borrows
these authorities. VM, extents, profiles, control subranges, digest inputs,
queue identity allocation, UAPI values and final mapping/publication validation
keep their existing order. Every validation completes before any owner is
taken. The final transfer only moves fields into an empty output slot; it has
no callback, allocation or native operation. Repeated attempts and occupied
outputs reject without consuming the retained prefix. The completion-signal
arena remains a sibling resource, not part of this four-resource aggregate.

`NativeQueueEngineInitializationV1` retains the backend before its opener and
foundation callbacks, then retains a returned foundation before authentication.
An attempt is one-shot, including after failure or panic. Successful field
extraction follows authentication and model construction. `admit_in_place`
borrows the caller's authority slot until resource/model checks complete. The
existing revision-exhaustion path still consumes that authority into the
poisoned engine; other precommit rejections leave the caller's slot occupied.
Legacy consuming `new`/`admit` wrappers remain compatible but do not provide
external failure custody by themselves.

Both primary and auxiliary Linux construction paths use the new ring and
resource-prefix helpers. Primary initialization and both admission paths use
the in-place engine handoffs. Native operation order, public signatures and
frozen V1 manifest identities are unchanged.

## Validation Boundary

Ten new CPU test functions exercise the production-used handoffs:

- All three ring backing profiles preserve actual token identity/layout through
  mapping and retention; transfer is one-shot.
- Foreign and closed-session prechecks retain the actual CPU/mapped token and
  perform no native operation.
- Map errno, native panic, opening/closing currentness error/panic and
  projection/commit error/panic retain the exact R88 precursor or successor,
  native records and existing N1/N2 charges. Original panic payloads survive.
- Resource-prefix tests cover all missing input slots, an occupied destination,
  wrong sizes/VM/geometry and queue-identity exhaustion. Rejection preserves
  the present owners without native calls; success transfers the exact roster.
- Borrowed view validation rejects independent mapping/publication substitution
  in every resource coordinate.
- Engine tests cover opener/take/authentication errors and panics, repeated
  initialization, resource-view errors/panics, malformed/model-invalid plans,
  exact success and revision-exhaustion retention.

Ring/resource fixtures use the actual fake-memory engine, R88 transitions and
typed role authorities. The existing geometry planner supplies the reviewed
sizes; a larger test VM aperture accommodates the real CWSR extent. These tests
do not touch or validate CWSR payload contents. Engine fixtures use model-only
resource authorities, not Linux allocations. Synthetic publication IDs can be
fresh at model admission; exact native publication/facts matching belongs to
the native resource validator and is tested there.

## Still Open

The constructor-local holders still drop on a later returned error or unwind.
NATIVE-2A.2 must retain the whole original memory session, accounts, preparation,
control stages, runtime/event/shadows, engine and completed session outside all
covered fallible work. NATIVE-2A.3 supplies its integrated failure acceptance;
auxiliary and replacement/insertion settlement remain NATIVE-2B/C. R91 does not
retain a value consumed internally by an arbitrary callback and never returned.

Preserve the USERPTR terminal threshold, explicit process-gate poisoning,
runtime-admission ordering and unpublished CWSR payload-cleanup contract during
that integration. The primary constructor takes its foundation once and has no
closing live-model retake. Do not add such a retake to its claimed trace.

No new backing charge is added for ring, USERPTR control, EOP or context-save.
Existing completion Host and retained device charges are observed, not refunded
or reconstructed. Terminal retention is not bounded aggregate accounting.
Generated adoption/publication/completion, new executable-refinement proofs,
Linux qualification, hardware overlap and HIP/HSA performance comparisons remain
open. CPU tests and mutation rejection do not prove those properties.
