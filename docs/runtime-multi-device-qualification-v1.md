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
allocation creation. Existing non-opt-in constructors retain private allocations, and
HostVisible behavior is unchanged. Pooled checkout matches exact allocation
flags as well as kind, extent and alignment; private and PUBLIC leases cannot
be substituted or relabeled. Both classes use the existing shared accounting
and capacity classification. This constructor also admits directional XGMI routes
from the checked devices before their compute sessions consume those admissions.
Non-opt-in constructors do not enable this route.

The production-authority equivalents are available without hardware-qualification
features:

- `KfdMultiDeviceRuntimeBackendV1::open_default_with_native_peer_copy_v1`
- `KfdMultiDeviceRuntimeBackendV1::open_default_with_semantic_authorities_and_native_peer_copy_v1`

Both validate the complete bounded, distinct, nonzero UID roster before native
opens, admit every device before constructing children, and require every
ordered XGMI route before enabling PUBLIC DeviceLocal storage. An unsupported
route fails construction; no partially enabled backend escapes. Caller launch
authorities and semantic profiles are retained unchanged. Peer visibility grants
no additional kernel authority. Composed/accounted-profile constructors have no
new opt-in in this increment. Existing private defaults, PUBLIC/private pool
separation, HostVisible behavior and ineligible-copy staged fallback remain.
`completed_compute_xgmi_copies_v1` is now a default-feature read-only observation.

The copy-only public-API witness deliberately denies all kernel launches and
advertises no atomic or collective profiles. It performs two changed-content
8,388,581-byte copies using the same four allocations, checks every source and
destination byte, and explicitly closes logical and native resources:

```sh
cargo +nightly-2026-04-03 run --locked -p fe2o3-runtime \
  --no-default-features --example gfx942-runtime-peer-copy-smoke \
  -- 0xSOURCE_UNIQUE_ID 0xDESTINATION_UNIQUE_ID
```

Leading `--semantic-authority` selects the semantic constructor;
`--staged-default` selects the corresponding unchanged private/staged constructor.
These flags combine. Native runs require two logical native completions;
staged runs require zero. This witness does not qualify compute authorization.

Cold copy-only sessions need no warm-up dispatch. Their primary AQL owner must
be Ready with zero write/read history, releasable completion, no dispatch,
clear unpublished state, empty detached ownership ledgers and initial persistent
generation. The endpoint admits this initial state in addition to its existing
quiescence rule; the shared auxiliary and detached-dispatch rules are unchanged.
Previously this initial state incorrectly failed peer queue creation.

The shared cold-predicate body has a bounded Verus proof of its exact Boolean
conditions and preservation of previously accepted states. Two positive contracts,
nine one-condition negative mutations and six controller controls pass. The
native fact observers are source-bound, not formally refined: this is not proof
of Linux currentness, ownership restoration, DMA or the complete runtime adapter.

## Persistent Native Route

R57 DeviceLocal launches use persistent SDMA allocations, not recycled
fixed-dispatch DATA. The new
`Gfx942ComputeXgmiQueueV1` persistent transfer adapter
retains the original persistent owners, directional attachments, identities and
pool generations. It copies equal complete logical extents in up to 4,096
ordered packets, each at most `0x003f_ffe0` bytes, while preserving independently
sized physical pool extents. The runtime's separate 256 MiB allocation cap still
applies; the packet planner's larger arithmetic limit does not raise it. Both buffers
must already be fully initialized PUBLIC storage with retired use frontiers.
The shared rooted mapping/copy/remapping sequence restores both VM models before
restoring either persistent output. This is not a demote/re-promote conversion.

The opt-in runtime selects this transport only for ordinary, full-range,
bounded DeviceLocal copies between distinct children whose persistent
storage is already initialized and normalizable at admission. Submission preserves
the existing stream, dependency, event and allocation-retain indexes and reserves
restoration shells, but no host payload staging or SDMA scratch. Uninitialized,
demoted, in-flight, partial, directed and over-plan copies retain host staging.
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
`progress_persistent_data_full_extent_with_peer_v1` and
`finish_persistent_data_full_extent_with_peer_v1`. A progress call samples one
fence or publishes the next packet, without a GPU wait loop. The separate lower
`poll_persistent_data_full_extent_with_peer_v1` remains observation-only and never
publishes the next packet. Pending retains the exact ticket, mapping owners and
both child reservations across calls; it does not advance the cooperative
progress generation. Both model foundations are retaken before each return.
Intermediate completions and next-packet publication advance the progress
generation, but do not release either child or count a completed logical copy.
Mappings remain rooted across packets, with at most one ticket in flight.
Ready requires the complete extent and retains custody until explicit finish
restores the original owners.
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
concurrency guarantee. The native completion counter increases only
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
that an expired public drain deadline is rejected without completing the copy,
then drives the outstanding copy through `RuntimeContextV1::drain`. It checks
the native completion counter changes from zero to one and
performs 13 full-buffer readbacks before explicit logical and native shutdown.
Both the baseline and packetized successor now pass the selected two-GPU gate.
Primary-session SSH and
ROCm process discovery succeeded on 2026-10-02 at 03:47 UTC. Earlier failed DNS
checks were from the worker execution namespace and do not establish a host
outage. Device occupancy must be rechecked immediately before each run.

```sh
cargo +nightly-2026-04-03 run --locked -p fe2o3-runtime \
  --features hardware-qualification --example gfx942-runtime-compute-xgmi-smoke \
  -- 0xSOURCE_UNIQUE_ID 0xDESTINATION_UNIQUE_ID
```

Run only after fresh shared-host endpoint admission. This command is a correctness
witness, not a performance benchmark or an independently verified machine-code
refinement. No new formal-verification claim follows from existing source guards.

The optional leading `--queued-consumer` flag enables the version journal and
admits the destination's exact typed consumer before progressing the copy:

```sh
cargo +nightly-2026-04-03 run --locked -p fe2o3-runtime \
  --features hardware-qualification --example gfx942-runtime-compute-xgmi-smoke \
  -- --queued-consumer 0xSOURCE_UNIQUE_ID 0xDESTINATION_UNIQUE_ID
```

That mode releases the public copy event after admission, checks consumer poll
and zero-time wait remain Pending and expired drain is rejected, then drains
only the consumer until its backend and version-journal reconciliation finish.
It does not separately drive the copy. The original four exact launches,
sentinel, native completion count of one, 13 full-buffer readbacks and explicit
cleanup remain required. Both modes pass on MI300X GPUs 6 -> 7 in the scoped
campaign below. Run the default explicit-copy-drain mode first, then this
additional dependency gate.

The optional leading `--packetized-copy` flag adds two auxiliary full-buffer
peer copies after the four exact launches and their readbacks. The extents are
4,194,273 and 8,388,581 bytes: two and three packets with one-byte and 37-byte
tails. Absolute-offset-derived source bytes and complementary destination
sentinels distinguish no-op, omitted-tail and repeated-first-packet errors.
Each case verifies both initial buffers, unchanged source and copied destination.
It requires one native completion per logical copy, not per packet. Successful
cleanup reports three total logical peer copies, 21 readbacks and 18 total
allocations. These auxiliary allocations do not expand kernel launch authority.
The flag can be combined with `--queued-consumer`; the large copies themselves
have no kernel consumer. This additional hardware mode passes in both directions
between the selected GPUs; combined queued-consumer/packetized mode also passes
in the original direction. These results do not qualify a general large-buffer
kernel consumer. They also do not cover the separate ring-reuse extension below.

```sh
cargo +nightly-2026-04-03 run --locked -p fe2o3-runtime \
  --features hardware-qualification --example gfx942-runtime-compute-xgmi-smoke \
  -- --packetized-copy 0xSOURCE_UNIQUE_ID 0xDESTINATION_UNIQUE_ID
```

The leading `--ring-reuse` extension performs two 268,433,409-byte copies
through a single 64-slot physical queue per logical copy. Each plan contains
65 packets with a one-byte tail; the allocation cap stays at 256 MiB. Four
auxiliary allocations are reused, and every source byte changes on the second
round. Initial source/sentinel and final unchanged-source/copied-destination
checks cover every byte, using at most one packet-sized readback scratch buffer.
The logical native completion count advances exactly once per completed copy.
The reported packet count is source-bound plan evidence, not raw ring telemetry;
queues are not cached between logical copies. No large compute authority is added.

```sh
cargo +nightly-2026-04-03 run --locked -p fe2o3-runtime \
  --features hardware-qualification --example gfx942-runtime-compute-xgmi-smoke \
  -- --queued-consumer --ring-reuse 0xSOURCE_UNIQUE_ID 0xDESTINATION_UNIQUE_ID
```

`--ring-reuse`, `--queued-consumer` and `--packetized-copy` may be combined.

The [production-peer packet](evidence/dev-multigpu-production-peers-2026-10-02/README.md)
qualifies all seven native/staged and ring combinations selected by its controller
on MI300X GPUs 6/7. The large upload explicitly continues across two directional
windows (63+2 packets), while each ring-reuse peer copy retains one native queue across all
65 packets. Rejected cold-start and upload-helper attempts remain in the packet;
only the final immutable source and complete seven-case run are accepted.

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

The next lifecycle increment composes the production persistent transfer,
paired model loans, native-leaf mapping outcomes and SDMA publication/fence
algorithms in CPU tests. Seven test functions cover 85 loop cases, including
errors and unwinds; the public Linux endpoint adapter and actual GPU payload
movement remain outside that fixture. Six runtime lifecycle tests add expired
and resumed drain, opposite directions, disjoint outcomes, owned shutdown and
pending ordinary-copy consumer rejection. Final-source checks pass 47 KFD
compute-XGMI tests, the broader memory/SDMA filters, 23 runtime transfer tests,
seven multi-group tests and both example tests. The full runtime has 1,952
passes, the same three socket-inspection permission failures and 32 ignores.
Strict combined Clippy and the no-default check pass; the witness builds.
All 32 source-control commands pass with 76 executable proof files unchanged.
The [lifecycle CPU packet](evidence/dev-multigpu-async-lifecycle-cpu-2026-10-02/README.md)
retains complete commands and scope limits. None of these checks qualifies
hardware, a full KFD suite, physical overlap or performance.

## Fixed-Total Transfer Shards

The production-default `gfx942-runtime-sharded-peer-copy-smoke` example accepts
2..8 explicit distinct GPU unique IDs. It partitions one 64 MiB + 37-byte global
payload into balanced contiguous shards and copies each shard to a separate
incoming allocation on the next GPU in the ring. All edges are enqueued through
one current-thread owned engine before driving progress. Two changed rounds
reuse the allocations; full-byte readbacks and global digests distinguish actual
sharding from replicated per-device work. Neighboring edges may serialize.

The [sharded evidence packet](evidence/dev-multigpu-shards-2026-10-02/README.md)
qualifies 2/3/5/7-device rings and the reversed seven-device ring on MI300X. GPU 0
is occupied by foreign work, so eight-device coverage remains CPU-only. Each
case checks exact native logical counts, full contents and explicit cleanup.
All 2,011 runtime tests pass with 32 existing hardware ignores; all 19 example
tests, strict lint/build checks and all 32 source controls pass. The unchanged
KFD test executable reuses the authenticated prior 1,925-pass campaign.

Single-range Context-authorized host capture is now forwarded by the multi-device
router after terminal and live-custody checks across every child. Exact handle
translation delegates to the native host-buffer reader, with no shadow fallback
or implicit progress. Completed result/event handles may remain retained. Six
new CPU tests qualify that routing boundary; no native capture run is added.

These changes do not qualify compute sharding, pending-peer group drain,
physical overlap, performance scaling or whole-adapter formal refinement.

## Pending Peer Readback And Capture

`copy_async` can now admit a same-device DeviceLocal `Read` to HostVisible
`Write` behind an exact pending ordinary native peer event. The read must be
covered by that peer's destination span. The multi-device backend authenticates
the event and global/child-local routes using retained metadata, without reading
detached allocation slots. It reuses the existing cooperative copy owner and
does not acquire child execution custody until dependencies succeed and the
peer restores its original owners. Existing staging/dependency bounds,
cancellation, failure and uncertain-custody rules remain in force. Readback uses
native D2H into bounded scratch followed by host staging and a HostVisible write;
this is not a zero-staging copy-performance optimization.

With the version journal enabled, a distinct same-device-copy root retains
canonical producer identities and the exact producer-read lease independently
of public events. Backend consumer success is retained until Context observes
the peer parent and validates the producer lease, including epoch and lineage,
before committing the output. Other backends do not gain this profile unless
they explicitly implement the default-false `supports_pending_peer_readback_v1`
contract. Ordinary pending-reader checks, graph copy admission, scalar peer
identity and existing kernel authorities are unchanged.

`begin_drain_with_capture_group` accepts one caller-owned boxed roster of up to
16 registered HostVisible ranges and one exact-size concatenated destination.
`with_drain_capture_group_byte_capacity` opts into an aggregate budget of at
most 128 MiB. It replaces, rather than adds to, the existing one-record account;
the original single-range API and configuration setter retain their 64 MiB
limits. Rejection returns the exact roster and storage without closing admission.
After the accepted prefix becomes quiescent, Context revalidates every logical
source before the first read, then the router validates native host backing.
Any read error discards the whole result; terminal failure or unwind preserves
existing poisoning. Result storage remains charged until disposal, including
when it outlives its future or owner. Quiescence and captured bytes alone do not
establish successful operations; individual completions must also be checked.

The `gfx942-runtime-pending-peer-capture-smoke` example exercises this path with
the journal enabled and the same fixed 64 MiB + 37-byte ring partition. Every
peer and dependent readback is pending before the owner handoff and capture
cutoff. Public producer events are released before cutoff; exact indexed
completion callbacks retain separate success evidence. The explicit `--round 0|1`
option selects one changed-content round per process. Admission closure is
permanent, and post-`ACQUIRE_VM` device admission is process-lifetime state:
opening another Context on the same physical devices in that process remains
unsupported even after native cleanup. Both rounds therefore require separate
processes; no admission-history reset is used. The witness checks all
initial device source/sentinel bytes and all final captured output bytes, then
requires explicit owned shutdown and capture-credit disposal. It does not read
Context after cutoff, observe the post-copy native counter, or establish
post-copy source preservation, physical overlap or performance acceptance.

These new adapters have not gained whole-runtime formal refinement. The four
shared journal observer bodies and all existing proof contracts remain
unchanged; rebinding their surrounding source identity is not a proof of the
new same-device-copy or group-capture integration.

The [pending-capture campaign](evidence/dev-multigpu-pending-capture-2026-10-02/README.md)
passes ten final-source MI300X runs: rounds 0 and 1 on 2/3/5/7 GPUs and the reverse
seven-GPU ordering. Every run admits all peers/readbacks before cutoff, checks
their exact success receipts and all 67,108,901 captured bytes, explicitly shuts
down native resources, and disposes the capture credit. The controller derives
both complete-payload digests independently. Its local/uploaded/final witness
SHA-256 is `d2789f83643d64849459c3de77dd33ea47ca98e22d21f4d79d015fba805038bc`.
All owned processes/files are gone and selected GPU/process baselines restored.
The two initial rejected hardware campaigns and cleanup receipts are retained.

The complete runtime suite passes 2,036 tests with 32 unchanged hardware ignores;
all 23 example tests, strict Clippy/no-default checks and 32 source controls pass.
The previous 1,925-test KFD qualification is reused through exact source, roster,
executable and authenticated archive identity. It is not a fresh KFD suite.
Nine source-control files change only 18 SHA literals and seven roster counts;
all 76 associated proof files remain unchanged, with no new solver qualification.

## Finite Sharded Compute

The hardware-qualification-only
`open_gfx942_sharded_vecadd_peer_qualification_v1` constructor installs an
independent one-shot authority for every device in an ordered two-to-eight-UID
roster. All recipes and UIDs are checked before native admission. It does not
change the existing vecadd, repeat, or R57 authorities, nor grant general kernel
authority through the production constructors.

The new policy has exactly 70 recipes: each shard of 2 through 8 devices, in
round 0 or 1. Every workload contains 65,537 `f32` elements, independent of the
device count. Uneven logical shards use full page-padded DeviceLocal allocations
and bindings; the 48-byte ABI carries the logical lengths and the grid rounds up
to a 256-thread workgroup. The existing full-extent readiness requirement remains
unchanged. Exact source, policy, object, ABI, geometry, allocation identities and
initial contents are authenticated before an atomic one-shot acceptance.

`gfx942-runtime-sharded-vecadd-capture-smoke` checks independently constructed
HostVisible A/B/C inputs before upload, then joins every exact H2D completion.
It preserves the authenticated H2dReady owners through launch authorization;
precompute native readback would normalize those owners and clear required
digests. All compute launches are admitted before explicit progress, although
ordinary admission may eagerly publish them. The witness joins exact compute
success and owner restoration before admitting the native peer ring, then
queues event-bound D2H reads and closes admission with one group capture.

No expected output is installed from the host after launch. Every logical output
and padding byte is checked, with a global digest of the 262,148 meaningful bytes.
The incoming destination sentinel is checked before peer admission. Individual
compute and transfer completion receipts, quiescent drain, explicit native
shutdown and capture-credit disposal are required before PASS.

```sh
cargo +nightly-2026-04-03 run --locked --offline -p fe2o3-runtime \
  --all-features --profile test \
  --example gfx942-runtime-sharded-vecadd-capture-smoke -- \
  --round 0 "$GPU0_UID" "$GPU1_UID"
```

Use only freshly available devices. Each round is a separate process, not a
reopened Context. This workload does not establish a fully prequeued compute-to-
peer graph, arbitrary kernel authority, output-dependent recurrence, native
partial-failure isolation, physical overlap, performance parity or machine-code
refinement. The post-cutoff native counter and post-copy source preservation
remain unobserved.

The [sharded-compute campaign](evidence/dev-multigpu-compute-shards-2026-10-02/README.md)
passes ten MI300X cases: both changed input rounds on 2/3/5/7 GPUs and reversed
seven-device ordering. Every run checks complete logical output and padding,
exact compute/transfer receipts and explicit shutdown. The local/uploaded/final
witness SHA-256 is
`b1c83337877a6a235c8219dbc5aaef65a39cc5b02b26202d4e8411618af86cf4`.
All owned remote resources are removed and selected GPU/process baselines
restored; GPU 0's foreign work is untouched. Eight-device coverage is CPU-only.

Final-source qualification passes 2,048 runtime tests with 32 existing hardware
ignores, all 31 example tests, strict Clippy/feature checks and all 32 source
controls. Twelve runtime tests are new, and the previous 1,925-test KFD result is
reused only through exact executable/source/roster and authenticated archive
identity. Ten source-control files update 17 hashes and seven inventory counts;
all 76 proof files remain unchanged. The pinned ROCm 7.2.4 compiler reproduces
the unchanged kernel object byte-for-byte, without proving compiler correctness.

## Next Dependencies

The [packetized campaign](evidence/dev-multigpu-packetized-2026-10-02/README.md)
passes five current-source hardware modes: default, queued consumer, packetized,
combined, and reverse-direction packetized. Every run checks four exact launches,
full byte contents and explicit cleanup. Two earlier baseline runs are retained
separately. Initial/final executable hashes match, selected-device memory use and
the process roster return to baseline, and the two uploaded files plus the owned
scratch directory are removed. These are correctness observations on a shared
host, not performance or exclusive-reservation evidence.

The public-authority opt-in and 65-packet witness are implemented as described
above. Fixed-total transfer sharding and pending peer/readback group drain now
pass on seven selected GPUs. Additional pairs, eight-GPU hardware coverage,
broader compute sharding, reusable live-Context batches, complete native fault
coverage, and peer mappings retained across separate logical copies remain open.
The current route does not qualify a general native runtime pipeline.

Prioritize exact compute-producer-to-native-peer deferred admission and repeated
work in one live Context. Same-process device reopen remains unsupported, but is
a separate device/VM ownership redesign rather than a prerequisite for useful
iterative workloads. General application kernels still require an appropriate
compiler/effects authority; the finite sharded policy does not supply it.

The default two-GPU witness drains and validates the copy before launching
either consumer. The additional producer-aware path now queues an exact typed
consumer as router-owned host metadata until peer queue retirement, both
allocation restorations and endpoint release. It holds no child allocation
custody before that point. It supports covered read ranges and aliases of the
copy destination, or an exact control-only dependency. Public event release
does not release the retained producer. This metadata-only admission is not
permission for concurrent native work within an occupied child.

There is at most one deferred head per stream. Ordinary `launch` still rejects
pending-copy inputs. Exact **completed** deferred-compute events are now valid
producer-aware inputs: a private immutable completion receipt must match the
original child route, global/local stream, dependency depth and successful,
quiescent child record. The original stream may already be destroyed. Independent
global result custody is acquired before child entry and is separate from directed
ancestry. Public event release does not release it; conclusive settlement or
cancellation does, while terminal/uncertain outcomes retain it. Checked dependency
depth still includes the child preflight minimum and the existing maximum.
Pending deferred-compute chains remain unsupported.

Typed CPU tests begin at an already-admitted child handoff, then exercise normal
Context event/result lifetimes through scripted completion. The Write-to-Read
case checks journal bookkeeping and cancellation before readback; it does not
fabricate or qualify GPU-produced bytes. Failure of a never-dispatched consumer does
not invent an observation of its parent's logical result. The changed ordinary
observation-leaf adapter is not covered by the prior completion-reconciliation
proof's exact unchanged-adapter correspondence. These restrictions and the
unchanged caller kernel authorities prevent treating the feature as general
dependency, native runtime or formally verified pipeline parity.

The [queued-consumer CPU packet](evidence/dev-multigpu-queued-consumer-cpu-2026-10-02/README.md)
records 1,980 passing runtime tests, zero failures, 32 existing ignores, all
25 new test functions, strict combined Clippy and three passing witness CLI
tests. It preserves earlier rejected controller and fixture attempts. This
CPU result does not substitute for either native witness mode's hardware gate.

Only after full-byte native compute/transfer pipelines pass should qualification
expand to real workload partitioning, all admitted devices, partial failures,
physical-overlap measurement and matched HIP/HSA scaling benchmarks. The
[current milestone tracker](runtime-a1-a2-swarm-current.md) retains those exit
gates explicitly.
