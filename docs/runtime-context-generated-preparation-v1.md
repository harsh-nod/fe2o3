# Context Generated Preparation V1

R76 implements GEN-2B-2's nonexecuting preparation boundary. It builds on R74's
[owned generated custody](runtime-owned-generated-invocation-v1.md) and R75's
[owner-local drivers](runtime-owner-local-operations-v1.md), but does not yet
install generated authority in an operation, publish native work or decode results.

## Retained Device Scope

`with_retained_device_v1` is available on the checked device, shared GTT session
and compute queue session. Each callback receives only an immutable checked-device
borrow. Its result cannot borrow or replace that device. The Linux memory backend
has only a crate-private immutable accessor; no device extraction, second admission,
queue selection or memory-model loan is introduced.

The production-used [scope helper](../crates/fe2o3-kfd/src/retained_device.rs) performs:

1. Validate the owner and full observable currentness.
2. Invoke the callback once with an immutable view.
3. Revalidate full currentness before exposing its result, even if the callback
   returned an error value.

Currentness error or panic poisons the owner before candidate disposal. Callback
panic poisons and resumes the original unwind without a second fallible check.
The candidate remains outside the closing check's unwind boundary. Objects
already unwound inside a callback cannot be recovered, and arbitrary callback or
destructor behavior is not formally verified by this adapter.

For a shared session, active phase, exact retained model admission and VM/device
identity must agree. For a queue, the selected queue must exist, be Active and
have unpoisoned authority; its VM must equal the session VM. Queue/session scope
failure also poisons the existing process runtime gate. None of these checks
claims quiescence, queue-exception absence or successful completion.

Full currentness uses the existing process, descriptor, UAPI, XNACK, DRM/reset,
topology and aperture observation contract, not the reduced operational fence.
It may allocate and perform system calls. Existing reset-wrap/observation-ABA
and external kernel/firmware limitations are unchanged.

## Context And Host

`RuntimeContextV1<KfdRuntimeBackendV1>::with_gfx942_preparation_device_v1`
rejects foreign/unknown devices and terminal Contexts before callbacks. The
backend rejects retired or synthetic owners and routes through its actual
checked device or retained queue. Missing/duplicated ownership and actual
device/description mismatch fail terminally.

The resulting `RuntimeGfx942PreparedV1<T>` privately binds Context generation,
runtime device ID, backend device ID and complete `ModelDeviceAdmissionV1`.
Context generation identifies the private, nonreplaceable backend instance;
there is no additional redundant global counter. The wrapper is non-Clone,
non-Send and non-Sync, offers immutable access only, and has no consuming payload
extraction. An arbitrary wrapped `T` is not execution authority.

The stable binding deliberately does not record initial VM or queue absence.
Lazy creation moves the same checked device into its retained memory/queue
owner. `validate_gfx942_prepared_v1` checks exact Context/native identity and
currentness again without consuming the payload. Actual VM/lane/allocation
incarnations and one-shot operation adoption remain GEN-2B-3 requirements.

Host's `prepare_generated_context_invocation` consumes the authenticated
executable and raw generated arguments through the runtime-defined scope.
Missing protected application evidence rejects before Context access or
argument callbacks. The standalone constructor shares the same private payload
preparation; its final device check now encloses application-authority creation.
Both reuse the existing production-only admission and authority implementation.
There is no synthetic-verifier or qualification fallback.

R77 adds a [checked persistent projection](runtime-persistent-generated-projection-v1.md)
to the Context constructor inside the same currentness scope. It preserves the
original storage and authority identity while rejecting shapes unsupported by
the existing fixed queue. Standalone preparation remains in the one-shot format.

The public `GeneratedWorkerV3ContextInvocationV1` privately retains complete prepared
storage before decoder credits and application authority. It exposes descriptive
metadata and nonexecuting Context validation, not its payload, decoder or device.
It holds no mutable Context borrow and can outlive Context shutdown only as inert
host storage. Revalidation against a retired or different Context rejects.

R73 argument/result credits are neither detached nor reserved twice. The new
binding itself establishes no aggregate byte limit; executable images, hidden
kernargs, read-only policy copies and metadata still need their accounting work.

## Validation And Next Work

Fourteen new CPU tests cover the production-used currentness envelope, candidate
disposal ordering, callback/error/panic paths, 44 queue-state combinations, model
VM/device substitutions, Context identities, synthetic/retired/terminal rejection,
and inert storage after shutdown. Five runtime compile-fail doctests reject
borrow escape, mutable replacement, Send/Sync and owned payload extraction.
Five downstream generated-host negatives reject cloning, Send/Sync, private
storage and execution; a compile-only (`no_run`) host doctest type-checks Context
reuse after preparing, without executing either operation.

The [local record](evidence/local-r76-context-preparation-2026-09-10/README.md)
retains exact-source gates and the intermediate terminal-fixture failure.
These tests do not construct a genuine checked-device/compiler-backed Context
invocation or qualify successful Linux preparation before/after lazy bootstrap.
Those hardware and protected-compiler acceptance cells remain open.

No model/proof source or Verus theorem is added. Existing device-identity and
resource proofs do not establish this callback/owner/Context/host composition.
R77 implements the nonexecuting projection portion of GEN-2B-3. Operation
adoption/publication-time authority, readback/charged result commit,
shared async/blocking execution and graph/drain integration remain open.
No full HIP/HSA parity or performance gain is claimed.
