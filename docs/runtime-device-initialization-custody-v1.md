# Device Initialization Custody V1

R107/N3-D extends the existing device initializer with one owning source/lease
root. The [retained local campaign](evidence/local-r107-device-initialization-custody-2026-09-12/README.md)
accepts its named CPU/shared-sequence contract and runtime stack regression.
It does not establish successful Linux/KFD initialization, formal adapter
refinement or performance.

## Ownership Boundary

`shared_memory/device_initialization.rs` owns the original arbitrary byte box
or repeated-byte recipe, the original returned device lease, stage, per-call
native admission and returned GPU-map progress. The public standalone entry
uses the same complete-entry helper as the CPU/shared-sequence fixture.

The in-place core borrows this root and never moves it into the engine's
terminal slot. A later live-lane owner can therefore keep it through its own
retake/commit sequence. N3-L must implement and qualify that composition; the
lower initializer alone does not close live insertion or replacement.

The standalone wrapper retains admitted errors and panics in the original
engine, quarantines it and preserves the original error or panic payload.
Failed roots cannot yield completed output or restart initialization. Existing
unrelated allocations, foundation state and exact accounting charges remain
owned by their original fixture/session.

## Ordered Sequence

1. Authenticate source length/content while borrowing the original byte box;
   construct its validated witness without copying it or hashing it twice.
2. Validate allocation admission and reserve configured backing charges using
   the existing allocation path. Mark this call's native attempt immediately
   before `reserve_va`, not from unrelated prior engine activity.
3. Keep the actual returned lease while checking CPU currentness, mapping,
   preparing, writing, verifying arbitrary bytes, unmapping and checking closing
   CPU currentness.
4. Borrow the lease through final GPU mapping and its opening/closing
   currentness checks. Record the returned result/prefix before classification.
5. Only after success, retag the original lease and return initialized content.

The existing algorithms are preserved: one preflight hash, the existing full
copy and exact readback for arbitrary bytes, and repeated-byte fill without a
second readback. Allocations retain the `DEVICE_LOCAL_PUBLIC` profile. There
are six currentness checks across allocation, CPU initialization and GPU map.

A genuine lower-entry lease is authenticated and admitted before fallible
source metadata checks. A metadata rejection must not discard that native
input. Foreign coordinates/accounts reject without claiming custody over an
unrelated lease. An ambiguous reservation attempt retains its source even if
no lease returns; the root never fabricates an unreturned lease.

Host-only source/layout/capacity rejection leaves the standalone slot empty.
The existing first-currentness policy remains distinct: an unconfigured
pre-effect panic permits a healthy retry, while configured unwind behavior
retains the allocator's quarantine policy.

## Bounded Terminal Storage

The first inline-root candidate reproducibly overflowed the stack of an
existing runtime qualification test with no runner stack override. The
replacement stores the root in a
private `TerminalInitializationSlotV1` with one logical slot.

`SharedMemoryEngine::acquire` fallibly reserves its vector storage before
session-ID issuance and backend currentness/VM acquisition. Reservation failure
returns a typed capacity error. `try_reserve_exact(1)` guarantees capacity of at
least one, not physically exactly one. No removal or growth API is exposed;
retention checks vacancy and spare capacity before the sole push.

This adds one pre-effect allocation per engine, not per initializer or failure.
It avoids enlarging the engine by the complete inline root. The new storage is
not yet part of an aggregate host/native memory ceiling; M1-M4 must account for
it alongside bootstrap, journals, replies and quarantined owners. Pointer and
capacity checks are not a global allocation-count measurement.

## Verification Scope

The eleven new test functions cover both accounting modes, arbitrary/repeated
inputs, boundary lengths, currentness errors/panics, native failures, partial
GPU-map outcomes, malformed allocation outputs, lower-entry identity, external
root custody, host-only rejection/retry and occupied-slot refusal. Tests compare
original source identity/content/metadata, lease coordinates, native arguments,
mapping prefixes, account domains/charges and unrelated retained owners.

The configured capacity case saturates bytes and records together; it is not
independent coverage of each limit. Snapshots cover the declared ownership and
progress fields, not every private implementation bit. Slot allocation failure
and constructor-before-native ordering are source-reviewed, not dynamically
fault-injected. No CPU mapping-address substitution qualification is added.

Acceptance must preserve the original inline candidate's failed GNU campaign
and isolated stack-abort reproduction, then independently check the corrected
candidate's unchanged-command stack regression, full GNU/musl suites, focused
regressions, auxiliary audits and compiled behavioral negatives. The accepted
source must be exactly restored after every mutation. Proof inventory is not a
solver run, and CPU/shared-sequence results are not native or benchmark results.
The original ambient stack limits were not captured; unchanged commands and
runner-controlled settings do not retroactively establish those limits.

## Next Live Integration

The first N3-L slice is `initialize_fixed_dispatch_data` and
`insert_initialized_fixed_dispatch_data` in `queue_live/fixed_dispatch.rs`.
Keep a shared outer insertion owner outside `with_live_queue_memory_model`;
expose the existing in-place initializer through a narrow shared-memory adapter
and return only unit across the model-loan callback. Source and incomplete or
completed output must survive failed retake, and output extraction follows the
identity/count commit rather than preceding it.

Reserve `detached_data_identities` capacity before effects; the current
post-retake `Vec::insert` can allocate. Preserve the existing distinction between
append, remembered-hole replacement and explicit middle insertion, which shifts
occupied entries. Reuse existing loan settlement and facade-deferred parent
transport so auxiliary restoration precedes whole-parent transport. Avoid
another large inline session field.

Coherent and uninitialized insertion are later slices. R106's coherent
initializer retains internal transitions but does not expose a composite
external owning root for the complete live retake sequence. Neither its public
wrapper nor this device-only contract closes that separate gap. N4 release and
destruction also retain independent acceptance requirements.
