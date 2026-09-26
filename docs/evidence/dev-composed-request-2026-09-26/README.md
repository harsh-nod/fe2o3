# Composed Request Custody Foundation

Base: signed `4301798ef051c896b732e88206cd84fb78193aa4`.
This packet implements typed accounting minting and custody, not an enabled
Context/native profile or complete MEM-DOM, A1/A2 or HIP/HSA acceptance.

## Production Contract

`Gfx942ComposedBackingRootV1` mints one canonical device parent, one session and
three sibling request/N1/N2 leaves. It uses the existing four-level accounting
arena. Every child is constructed inside canonical admission before publishing
a new device entry; failed construction drops all unpublished children. Domain
capacity includes root, device slots and four domains per concurrent session.
The minimum is `max_devices + 5`, not a limit of one simultaneous session.

Separate composed budgets name requested, host and device byte ceilings and an
explicit combined-record ceiling. Each class retains its own byte/record limits.
Existing native-only budget meanings and default behavior are unchanged. The
registry's metadata row consumes a generic record but no AllocationRecords unit.
Each logical request and each native backing charge separately consumes one of
both kinds of record; logical allocations are not native residency readings.

The request account owns the exact typed registry, session, leaf and checked
device identity/generation. Private minting establishes actual sibling edges;
the coherence check validates captured bindings, not a general ancestry theorem.
There is no public raw-account/token extraction or native-only downgrade.
The typed account can be cloned, but reservations and retained credits cannot.
Every token holds that same account owner. Generic token fields precede account
fields so cancellation/quarantine occurs before final typed-owner destruction.
Consuming retain/release methods keep the owner live through the transition.

Cold reservations and retained credits therefore survive external root/admission
Drop. Clean cancellation, rejection and disposal reclaim the typed registry when
the final owner disappears. Retained Drop or explicit quarantine preserves it,
including its metadata debit. Final-inner defense also anchors on outstanding
request records, nonzero request usage, poison or session quarantine. It does
not confuse the root's permanent metadata row with outstanding request custody.

Batch admission checks the shared roster bound before allocating charges and
uses the existing atomic ancestor planner. Its exact-size/fused owning iterator
wraps the already-allocated generic token array without a second output array or
post-commit allocation. Unconsumed members cancel before its registry owner drops.
Zero-byte accounting requests still cost a record; this is not permission to
create a zero-sized native allocation. Wrapper/Arc/allocator payloads and adapter
charge-array storage remain outside the existing arena/bootstrap byte charge.

## Verification

All thirteen focused CPU groups pass. They
exercise bootstrap/budget boundaries; failed minting at every child; immutable
canonical parents; root/session/generation substitution; all six mixed-class
record orders; class/device/root/generic-record ceilings; cold lifetime; panic;
partial batches; and native-sibling quarantine with no request usage. Three
compile-fail examples reject cloning, double refund and raw-token extraction.
Native-class charges in these tests use private CPU ledger fixtures, not device
allocation or actual Context shutdown.

Broad libraries report 1,034 model, 70 accounting, 1,334 KFD and 1,482 runtime
passes (3,920 total), with the same four socket failures: KFD
`target_debug_telemetry_v2::tests::credential_bound_channel_accepts_typed_failure_before_publication`
at `SocketAdmission`, and runtime
`authorized_execution::tests::cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`authorized_execution::tests::failed_session_end_is_explicit_and_terminal`, and
`authorized_execution::tests::pre_native_telemetry_failure_is_returned_and_poisoned`
at `InspectSocket(EPERM)`. Library exit is 101. The broad run filters 296 KFD
construction cases and preserves 19 model/28 runtime ignores. This is not full
CPU qualification. All 69 selected construction/custody tests pass (67 KFD,
two runtime), as do 107 doctests (31 KFD, three accounting, 46 runtime, 27 model)
and strict all-feature/all-target Clippy. No-default-feature checking and
workspace formatting also pass. The validation driver is terminal with exit 1
because the library stage returned 101; all other stages returned zero.
Overlapping selections are not additive distinct coverage.

The initial focused run records nine passes and three fixture failures. Two
fixtures selected child record limits larger than their deliberately small root
record arena. The third incorrectly expected generic quarantine to poison the
ledger and forbid spare-capacity sibling reservations. Fixtures were corrected,
not production accounting semantics. Quarantine retains debit and ancestor
pressure; native session sealing is a separate consumer contract.

Two deliberate Rust negative controls reject on lifetime assertions, not on
compilation/infrastructure. Removing the final typed-registry anchor makes all
three selected quarantine tests fail. Reordering reservation fields to drop the
account before cancellation makes the selected cold-cancellation test fail
because the clean registry is permanently anchored. Both runs return 101. Their
source captures/logs are retained; the original source digest was restored and
checked before final qualification. These are regression mutations, not formal
proof obligations.

The shared accounting/model/proof bodies and source pins are unchanged from the
retained-charge packet; the source manifest recheck passes. Its prior
planner/arena proofs do not prove this new minting transaction, Arc lifetime
adapter, mutex composition or native behavior.
No new formal-verification or native-performance result is claimed.

## Remaining Work

The runtime does not yet install the composed request account automatically or
carry a mandatory borrowed request witness to native allocation. No composed
native constructor consumes this admission yet. Next work must adopt the exact
leaf without adding a fifth level; preserve typed request custody in runtime
account/reservation/retained variants; install a complete device-bound admission
roster at Context open; and require exact witnesses before backend/native effects.
Generated rosters need witness-bound plans before infallible metadata commit.
All SDMA-first, compute-first, generated-first and ordered XGMI paths, direct
backend calls, backend recovery/shutdown and forwarding adapters need coverage.
Worker protocols without witness transport must not advertise this profile.

Frozen source-file lists for the four tested packages plus workspace Cargo inputs
match before/after validation and before commit. All four actual library test
executables have recorded SHA-256 digests and passed rechecks before cleanup.
The terminal validation session and cleanup receipts are under `raw/`: only the
owned 712,728 KiB build target was removed, followed by a separate absence check.

MI300X access still fails resolving `sharkmi300x-1` before remote entry. No shared
machine artifacts or native results were created. Accepted milestones and full
parity claims do not move; delivery is recorded separately against the signed
commit to avoid self-referential evidence.
