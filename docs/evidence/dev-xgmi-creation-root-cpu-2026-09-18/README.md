# XGMI Creation Roots: CPU Development

CPU qualification passed: all 21 recorded commands exited zero. GNU and musl each
passed all 30 selected KFD tests and all 17 runtime XGMI tests, with no failures
or ignored tests in those rosters. Both public example test targets compiled and
passed with zero tests. Strict Clippy, no-default checks, formatting, diff checks,
six verifier calibrations and unchanged-source checks passed. Unsafe-source policy
passed five tests with one explicitly ignored maintenance test. The independently
reviewed archive contains 111 files plus its seal. This packet covers caller-rooted
XGMI creation custody, not HIP/HSA parity or native performance acceptance.

The source cohort starts at `3813f93d3b67a627d52741c1100c82a6185f1544`
plus the exact 5,562 per-file identities in `raw/source-before.log`.
Before/after Cargo, toolchain, source and release-contract inventories must match.
Evidence and build products are excluded from the source selector and the archive
itself is separately sealed. Old evidence archives remain unchanged.

## Implementation

The public XGMI constructor now requires a mutable, caller-owned
`Gfx942NativeXgmiSdmaQueueCreationRootV1`. The root is move-only and inline,
with a tested 2 KiB ceiling (4 KiB for the runtime's pair). It is vacant before
pure preflight and after successful creation. Arming installs the exact route
before opening route validation. Terminal states retain no-queue custody, an
opaque memory-operation marker, the exact prepared native attempt, or a confirmed
owner. The ioctl mutates arguments inside the borrowed attempt; the mapped
doorbell is retained before closing currentness. A confirmed owner remains rooted
through closing route validation and gate disarm. Extraction occurs only on success.

Both session quarantines and global poisoning are independently unwind-caught.
An original creation panic wins over secondary panics; secondary panic payloads
are forgotten without invoking their destructors. When an operation returns an
error, the first terminalizer panic is resumed after all terminalizers run.
Occupied-root retry is inert and occupied-root Drop aborts. No cleanup or recovery
authority is added. Diagnostic errors contain no native owners and can be dropped
or formatted without releasing the root's custody.

The runtime embeds one root per direction, checks them before queue reuse or
shutdown, and aborts Drop before sibling queue teardown if a root is occupied.
Its production-shared settlement helper marks the backend terminal before error
formatting or resuming the original panic. The runtime deliberately preserves
its conservative policy: even a lower-level retryable preflight error terminalizes
the backend. The public example and exports migrate to the required root API.
The shared raw lower constructor leaves attempts borrowed; ordinary constructors
retain their existing owner-bearing wrapper and behavior.

## Test Scope

GNU and musl use optimization level 1 with debug assertions and overflow checks
explicitly enabled. The selected KFD roster includes the new XGMI driver tests,
existing XGMI contract tests, creation gate and manifest tests, and all ordinary
SDMA creation regressions. Both targets also run the selected runtime XGMI tests
and compile the migrated public example. Strict all-feature/all-target Clippy,
no-default checks, unsafe-source policy, formatting and diff checks are required.
Six verifier calibration tests reject malformed rosters/outcomes, invalid receipt
commands/statuses/times, wrong membership, symlinks and overwritten seals.

The XGMI fixtures use the actual private creation driver called by the public
API, admitted synthetic topology routes in both directions, real local memory
fixtures and the shared native-attempt driver with scripted native operations.
They cover preflight reuse, route/key/lower/opaque errors and panics, nine native
prefix failures, closing validation, disarm panic, success extraction, inert
retry and subprocess Drop for every occupied phase. Observations include route,
queue key/engine/id, allocation identities, record-vector storage and occupancy,
mutable native arguments, doorbell identity, zero generations and no uncertain
ticket. All seven hostile terminalizer masks run with an original error or panic
at no-queue, opaque, mapped-attempt and confirmed phases. A hostile diagnostic
formatter cannot consume confirmed custody.

Runtime tests exercise both directions in the settlement helper with move-only
stand-ins, preserve the sibling direction, retain the exact original panic
payload, and verify terminal latching before hostile formatting. Actual backend
root wiring, liveness, shutdown and Drop ordering are source-guarded and reviewed,
not behaviorally qualified with real two-device sessions. The example's successful
path explicitly destroys both queues; its preexisting partial-creation error-exit
behavior is not a general cleanup guarantee.

Development compilation caught test use of `unwrap_err` on a non-Debug queue.
An exploratory run also exposed a fixture doorbell encoding that lacked the
source GPU ID hash and therefore failed before the intended closing fault. That
encoding was corrected; hostile test payloads are now held in `ManuallyDrop` so
failed assertions cannot trigger destructor-abort cascades. These exploratory
attempts are not accepted qualification receipts.

## Limits

This is bounded CPU development evidence. The real `Sessions` adapter, native
ioctl/mapping panic behavior, and constructed-runtime Drop are not dynamically
qualified by these fixtures. Pre-preparation memory custody remains opaque;
arbitrary consuming-memory and doorbell-mapping primitive unwind refinement is
not established. Keeping both sessions and the root alive after terminal failure
is a documented contract, not a lifetime-enforced relationship. No Rust/native
formal refinement, GPU acceptance, copy-performance improvement or full HIP/HSA
parity follows from this packet.

The remaining retirement path has a separate custody gap: runtime shutdown takes
the queue out of its slot before fallible teardown, and public teardown consumes
the owner before fallible resource release. Exact partial-release custody is not
guaranteed there. A separately rooted retirement protocol and in-place cleanup
remain open work; this creation packet does not fix or qualify that path. The
public teardown documentation no longer claims otherwise.

A read-only MI300X inventory at 2026-09-18 21:58 UTC found only GPU 1 apparently
unoccupied; GPU 0 held a live allocation and GPUs 2-7 were busy. There was no free
two-GPU XGMI pair at that time. Two read-only snapshots at 22:39:30 and 22:39:37
UTC subsequently found GPUs 1-7 at baseline VRAM use with no mapped KFD processes;
GPU 0 remained occupied. The user permits observed-free devices, but idleness
is not an exclusive reservation and must be rechecked immediately before use.
No remote files or GPU workloads were created for this CPU packet, and no
existing jobs or resources were altered. The inventory commands left no
persistent remote jobs. These observations are scheduling context, not GPU
qualification or benchmark evidence.

```sh
python3 -B docs/evidence/dev-xgmi-creation-root-cpu-2026-09-18/verify.py
python3 -B docs/evidence/dev-xgmi-creation-root-cpu-2026-09-18/verify.py --live
```

`--live` reruns only the SHA-pinned source selector. The verifier never executes
archived qualification commands. The recorder refuses receipt overwrite or
appending to a sealed archive.
