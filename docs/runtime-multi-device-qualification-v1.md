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

This initial interface permits one peer attachment per compute owner and exposes
attachment/retirement only. It does **not** yet route runtime peer copies through
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

## Next Dependencies

The next bounded data-path increment is a full-extent transfer of exact recycled
PUBLIC device data: retain both data owners, transition local mappings into
two-device peer mappings, copy with the existing native XGMI engine, restore
owner-local mappings, then return data only after both foundation restorations.
Partial mappings, timeout tickets and uncertain results must remain owned.
Initialization may survive the transfer; stale content-digest authority must not.

The original vecadd authority still requires HostVisible memory and exact
initial digests. The existing separate DeviceLocal R57 N3 V2 authority can
support the native pipeline without relaxing that fixture. The new
`KfdMultiDeviceRuntimeBackendV1::open_gfx942_r57_n3_qualification_v2` constructor
admits an independent unchanged R57 V2 authority for each selected device;
it does not share authority state or local allocation identities. Five focused
CPU tests pass, covering roster rejection, admission failure, routing and
independent authority advancement with identical local handles on three GPUs.
This constructor does not by itself enable native peer routing or PUBLIC storage.

The planned native witness runs `A+B -> C` on each GPU, releases both producers,
overwrites the destination's existing C allocation with a full-buffer `-1.0`
sentinel, and verifies that sentinel before the peer copy. Copying the source C
back into that same destination allocation must restore every expected byte;
the destination then runs its existing `C+B -> D` second phase. This preserves
the authority's exact local C/B identities while ensuring a no-op copy fails.
Transfer completion must dirty the destination shadow, clear both its content
digest and last-host-write evidence, and restore initialized native custody.
Ordinary runtime SDMA DeviceLocal allocations are not PUBLIC, whereas the
direct fixed-dispatch initializer already is; runtime integration must address
that difference explicitly, not retag private allocations.

Async submission custody, runtime routing and dependency readiness follow the
typed data transition; host-staged transfers remain the fallback.

Only after full-byte native compute/transfer pipelines pass should qualification
expand to real workload partitioning, all admitted devices, partial failures,
physical-overlap measurement and matched HIP/HSA scaling benchmarks. The
[current milestone tracker](runtime-a1-a2-swarm-current.md) retains those exit
gates explicitly.
