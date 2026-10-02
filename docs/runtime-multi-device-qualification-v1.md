# Multi-Device Runtime Qualification

## Scope

The `hardware-qualification` feature exposes
`KfdMultiDeviceRuntimeBackendV1::open_gfx942_vecadd_qualification_v1` for two
to eight distinct, nonzero device unique IDs. It admits only the existing
repository-owned exact gfx942 vecadd fixture, not arbitrary application kernels.
All selected devices are admitted before a child creates a VM or queue.

The `gfx942-runtime-multi-device-smoke` example:

1. Allocates and initializes the exact 4 MiB vecadd fixture on each device.
2. Publishes and flushes every producer before waiting for any producer.
3. Checks every output byte, then explicitly releases producer submissions.
4. Copies the first selected device's result to separate HostVisible
   destinations on the other selected devices using **host-staged** transfers.
5. Checks every copied byte, releases copy submissions, drains the completed
   group and explicitly shuts down both logical and native resources.

The final `PASS` line is emitted only after successful explicit cleanup.
This is replicated compute, not sharding. Publication before waiting does not
prove physical overlap. The native witness drains an already-completed group;
it does not qualify draining outstanding heterogeneous GPU work.

## Running

Use the hexadecimal device **unique IDs**, not GPU ordinals or PCI addresses:

```sh
cargo run --locked -p fe2o3-runtime --features hardware-qualification \
  --example gfx942-runtime-multi-device-smoke -- "$GPU0_UID" "$GPU1_UID"
```

Each argument must have the form `0x0123456789abcdef`. The host must expose
compatible `gfx942:xnack-` devices and permit KFD access. On a shared host,
inspect process attachments, engine activity and memory use immediately before
execution and again after cleanup. An idle percentage alone is insufficient:
another process may retain allocations or be between dispatches. A point-idle
check is not an exclusive reservation. Never reset devices or terminate other
users' processes to run this example.

## Qualification Status

CPU qualification of the unsigned 18-path overlay on `dd5e61712` passed
1,928 runtime tests, with 32 existing hardware tests ignored, no failures and
no filtering. Strict Clippy, the no-default library check, the single CLI test
and the normal example build also passed. The result composes seventeen
accepted original gates with five completion commands. The original campaign
remains rejected for its singular-test-list parser defect; archive collection
defects did not rerun or alter the test commands. This is not a new single-pass
campaign or a hardware result. The [CPU evidence packet](evidence/dev-multi-device-cpu-2026-10-01/README.md)
includes complete command records and a standalone readback auditor. The tested
source tree was recovered byte-for-byte after a local session restart; only
documentation and the evidence packet were added or updated afterward.

The new CPU regressions exercise roster validation, routed copies on two,
three and eight mock devices, opposite-direction group drain, retained results,
failed/cancelled work, deadline expiry, and contradictory-tail rejection.
The stream cleanup fix permits destruction of a completed cooperative tail
without releasing its result or event early. Pending work remains busy;
contradictory custody terminalizes the backend without destroying the child.

Native validation of this example remains pending. Earlier standalone XGMI
copy measurements do not qualify this combined compute-and-transfer path.
The implementation is not a machine-code-refined or fully formally verified
multi-GPU runtime, and no HIP/HSA parity or performance claim follows.

## Shared-VM Queue Attachment

`ComputeAqlQueueSessionV1::create_native_xgmi_queue_with_peer_v1` now creates
an owned `Gfx942ComputeXgmiQueueV1` using two existing compute VMs, with no
second VM or public raw-session callback. The caller supplies a
`Gfx942ComputeXgmiQueueCreationRootV1` to retain uncertain native creation.
The paired operation attempts both foundation restorations even when one fails;
uncertainty poisons both endpoints while retaining native ownership. The first
panic is preserved through subsequent restoration or poison failures.

Both compute owners retain the same private attachment certificate, bound to
their stable session identities, directional route and native queue ID. Explicit
`destroy_native_xgmi_queue_with_peer_v1` checks both certificates and clears them
only after native retirement and both successful restorations. Teardown cannot
release either compute VM while the attachment remains. Ordinary compute stays
available; additional peer, auxiliary or ordinary SDMA queue creation is refused
while attached. Create any required ordinary SDMA queues before attachment.

This interface permits one peer attachment per compute owner. Its original
attachment/retirement checkpoint did **not** route runtime peer copies through
XGMI, expose peer-mapped compute buffers, or establish native success. Nineteen
focused CPU regressions pass, including eleven new settlement/attachment tests;
strict all-feature KFD Clippy passes. Native creation/retirement and real-engine
failure injection still require hardware qualification. The separate full
1,858-test KFD run exceeded its 900-second bound; it is not a full-suite pass.
After integration, the all-feature runtime library suite passed 1,928 tests
with 32 existing hardware ignores, no failures and no filtering. All 32 existing
source-CI commands also passed. The [attachment CPU packet](evidence/dev-compute-xgmi-attachment-cpu-2026-10-01/STATUS.md)
separates the completed checks from the KFD timeout. Source-guard metadata was
rebound with all 22 associated proof-closure files unchanged; no new formal
verification claim follows.

## Bounded Native Transfer

`Gfx942ComputeXgmiQueueV1::copy_recycled_data_full_extent_with_peer_v1` adds a
synchronous transfer of exact recycled fixed-dispatch DATA between those two
compute VMs. Both inputs must be fully initialized PUBLIC device allocations,
with matching complete requested extents no larger than `0x003fffe0` bytes
(one native copy packet). The exact detached ordinals, generations, storage
variants, endpoint attachments and native queue identity are checked before
either input is consumed.

The queue retains both data owners through local unmapping, exact two-device
mapping, copy submission/completion, peer unmapping and owner-local remapping.
Both VM models must be restored before either output or detached ledger update
is published. Outputs retain initialization but not stale content digests.
Preflight rejection leaves the caller's slots intact. After admission, any
error or panic, including timeout, is terminal: partial native mappings and
copy tickets remain owned, both endpoints are poisoned, and teardown cannot
release their VMs. This API does not provide retry, asynchronous completion,
multi-packet transfers or runtime `peer_copy` routing.

CPU tests exercise the actual borrowed mapping driver with injected native
prefix errors, malformed results, currentness failures and panics, plus
transfer ordering and paired model restoration. They do not yet execute the
entire concrete two-session adapter as one composed test. Native execution
and machine-code refinement remain unqualified.

On the final source, 19 compute-XGMI, seven paired-restoration and 53
PUBLIC-related CPU tests pass. Strict combined all-feature Clippy, the runtime
no-default library check and all 32 source-CI commands pass. The independent
full runtime run has 1,929 passes, 32 existing hardware ignores and three
failures: the unchanged telemetry fixture's socket inspection is denied with
`EPERM` in this environment. That is not a full-suite pass. The
[DATA/PUBLIC CPU check packet](evidence/dev-compute-xgmi-data-cpu-2026-10-01/README.md)
records scope, raw failures and the separate broad KFD result. Source-hash
updates leave 75 associated executable proof files unchanged and do not
establish formal verification of the new transfer.
The full optimized KFD run also remains incomplete: both the build-and-test
attempt and the cached-build retry exceed their 1,800-second bounds, without
a reported assertion failure before timeout. Focused results do not replace
that missing full-suite result.

## Peer-Visible Runtime Storage

The original vecadd authority still requires HostVisible memory and exact
initial digests. The existing separate DeviceLocal R57 N3 V2 authority can
support the native pipeline without relaxing that fixture. The new
`KfdMultiDeviceRuntimeBackendV1::open_gfx942_r57_n3_qualification_v2` constructor
admits an independent unchanged R57 V2 authority for each selected device;
it does not share authority state or local allocation identities. Five focused
CPU tests pass, covering roster rejection, admission failure, routing and
independent authority advancement with identical local handles on three GPUs.
This constructor does not by itself enable native peer routing or PUBLIC storage.

The separate opt-in
`KfdMultiDeviceRuntimeBackendV1::open_gfx942_r57_n3_peer_qualification_v2`
selects genuine PUBLIC DeviceLocal allocations before lazy child queue or
allocation creation. Ordinary constructors retain private allocations, and
HostVisible behavior is unchanged. Pooled checkout matches exact allocation
flags as well as kind, extent and alignment; private and PUBLIC leases cannot
be substituted or relabeled. Both classes use the existing shared accounting
and capacity classification. This constructor also admits directional XGMI routes
from the checked devices before their compute sessions consume those admissions.
Ordinary constructors do not enable this route.

## Persistent Native Route

R57 DeviceLocal launches use persistent SDMA allocations, not recycled
fixed-dispatch DATA. The new
`Gfx942ComputeXgmiQueueV1` persistent transfer adapter
retains the original persistent owners, directional attachments, identities and
pool generations. It copies equal complete logical extents up to `0x003f_ffe0`
bytes, while preserving independently sized physical pool extents. Both buffers
must already be fully initialized PUBLIC storage with retired use frontiers.
The shared rooted mapping/copy/remapping sequence restores both VM models before
restoring either persistent output. This is not a demote/re-promote conversion.

The opt-in runtime selects this transport only for ordinary, full-range,
single-packet DeviceLocal copies between distinct children whose persistent
storage is already initialized and normalizable at admission. Submission preserves
the existing stream, dependency, event and allocation-retain indexes and reserves
restoration shells, but no host payload staging or SDMA scratch. Uninitialized,
demoted, in-flight, partial, directed and larger copies retain host staging.
Poll and wait do not advance a native copy.

Once dependencies succeed, both children must be physically quiescent. Existing
cache-release operations retire all retained compute references; unrelated dirty
materialized caches may be reconciled by those existing release operations.
The selected endpoints cannot have materialized dirty extents. Dirty persistent
SDMA shadows are allowed and are not downloaded for the transfer. After checking
both normalized storage slots, the submission roots both owners, creates an
ephemeral peer queue and publishes the copy. Later progress calls sample its
completion, restore local mappings, retire the queue and restore both allocations.
Success is published only after restoration. Any uncertain error or unwind
poisons both children and retains the occupied root, without staged fallback.

The runtime uses `begin_persistent_data_full_extent_with_peer_v1`,
`poll_persistent_data_full_extent_with_peer_v1` and
`finish_persistent_data_full_extent_with_peer_v1`. Each sample reads the fence
once, without a GPU wait. Pending retains the exact ticket, mapping owners and
both child reservations across calls; it does not advance the cooperative
progress generation. Both model foundations are retaken before each return.
Ready retains custody until explicit finish restores the original owners.
The synchronous lower adapter remains available for its existing callers.

Both entire child backends are reserved before owner extraction. Conflicting
native allocation, compute, SDMA, readback, cache release and teardown cannot
enter either child until restoration. Stored observations and host-only logical
bookkeeping remain available, and disjoint pairs may progress. Cancellation
before publication remains possible; a started transfer is TooLate to cancel.
No uncertain native prefix falls back to host staging.

Drain checks its deadline between phases and before starting new progress.
Native mapping, topology/currentness, creation and retirement calls remain
synchronous: this is not a hard syscall deadline or general same-VM compute/copy
concurrency guarantee. The qualification-only completion counter increases only
after an actual native transfer, queue retirement and both restorations succeed;
scripted CPU execution does not increase it.

The new `gfx942-runtime-compute-xgmi-smoke` witness runs `A+B -> C` on each GPU,
releases both producers, overwrites the destination's existing C allocation with
a full-buffer `-1.0` sentinel, and verifies that sentinel before the peer copy.
Copying the source C
back into that same destination allocation must restore every expected byte;
both children then run their existing `C+B -> D` second phase. This preserves
the authority's exact local C/B identities while ensuring a no-op copy fails.
Transfer completion must dirty the destination shadow, clear both its content
digest and last-host-write evidence, and restore initialized native custody.
Ordinary runtime SDMA DeviceLocal allocations remain private; the new peer
qualification constructor supplies PUBLIC backing explicitly. The direct
fixed-dispatch initializer was already PUBLIC. Neither path retags private
allocations.

The witness requires pre-flush poll and zero-time wait to remain Pending, checks
the native completion counter changes from zero to one, and
performs 13 full-buffer readbacks before explicit logical and native shutdown.
It is implemented but has not run on GPUs. The latest SSH attempt failed hostname
resolution before executing a remote command; no current device pair is admitted.

```sh
cargo +nightly-2026-04-03 run --locked -p fe2o3-runtime \
  --features hardware-qualification --example gfx942-runtime-compute-xgmi-smoke \
  -- 0xSOURCE_UNIQUE_ID 0xDESTINATION_UNIQUE_ID
```

Run only after fresh shared-host endpoint admission. This command is a correctness
witness, not a performance benchmark or an independently verified machine-code
refinement. No new formal-verification claim follows from existing source guards.

At the preceding synchronous checkpoint `d2ff52f63`, CPU testing passes 12
scripted runtime route tests, two example tests, and the focused KFD
initialization, transfer, paired-restoration and allocation-policy regressions.
The full runtime run records 1,941 passes,
three unchanged baseline socket-inspection permission failures and 32 ignores;
the broad KFD suite remains incomplete. All 32 source-control commands pass,
with 76 associated executable proof files unchanged. Strict combined Clippy
passes and the runnable witness builds; neither is a hardware result. The
[persistent runtime CPU packet](evidence/dev-compute-xgmi-persistent-cpu-2026-10-02/README.md)
records exact commands, rosters, executable identities and coverage limits.

The asynchronous successor passes all 17 runtime route tests, both example
tests and six focused KFD filters, including eight one-shot sampler tests.
The full runtime run records 1,946 passes, the same three socket-inspection
permission failures and 32 ignores; broad KFD qualification remains incomplete.
Strict combined Clippy, the runnable witness build and all 32 source-control
commands pass, with 76 executable proof files unchanged. The
[async CPU packet](evidence/dev-compute-xgmi-async-cpu-2026-10-02/README.md)
records final-source executable hashes, earlier rejected checks and the exact
boundary between scripted, mapped-arena and unexecuted native coverage.

## Next Dependencies

Native hardware qualification, complete composed native fault coverage,
hardware validation of the asynchronous custody path, persistent peer mappings
and multi-packet copies remain open. The current route does not qualify a
general native runtime pipeline.

Only after full-byte native compute/transfer pipelines pass should qualification
expand to real workload partitioning, all admitted devices, partial failures,
physical-overlap measurement and matched HIP/HSA scaling benchmarks. The
[current milestone tracker](runtime-a1-a2-swarm-current.md) retains those exit
gates explicitly.
