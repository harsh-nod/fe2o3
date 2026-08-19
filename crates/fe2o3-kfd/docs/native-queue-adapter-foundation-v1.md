# Native queue adapter foundation V1

This R4 slice executes the compute-AQL queue lifecycle against a private
backend and projects every attempted lifecycle operation into the existing
bounded `QueueLifecycleStateV1`. The production composition consumes the
checked device and exact shared-GTT capabilities; callers cannot construct a
backend from numeric addresses.

## Executable transition contract

The adapter owns one process-domain identity snapshot, memory model, queue
model, and a backend-specific private resource authority for every admitted
queue plan. The adapter retains that value linearly and never clones it. It
checks the opener PID and contracted device currentness before and after every
CREATE, UPDATE, DISABLE, or DESTROY ioctl. The corresponding
`Begin*` transition is committed before the lifecycle ioctl is issued. The
kernel observation is then projected through `Observe*` before the trailing
currentness check.

CREATE initializes both output fields to sentinels, compares every input field
after the call, and uses `admit_kfd_gfx942_create_queue_outputs` on success.
Queue ID zero is valid. IDs outside 0 through 1023, a retained sentinel,
changed input, wrong doorbell mmap type/GPU hash, an offset outside the full
8192-byte process slice, or non-8-byte alignment becomes `Indeterminate` and
therefore `Ambiguous`. Queue-ID collisions and unresolved creates use the
model's process-global poison rules.

UPDATE and DISABLE are the same KFD request with different typed inputs.
UPDATE can only reuse the retained ring authority; it cannot introduce a raw
address. DISABLE uses the reviewed null-ring encoding. DESTROY accepts exactly
Active or Disabled, stores that source in DestroyPending, and restores it after
a source-justified FailedNoEffect observation. Any changed write-only/in-out
request record is indeterminate.

The Linux backend translates success to `Succeeded` and every errno
to `Indeterminate`: an errno does not prove rollback. `FailedNoEffect` exists
for a future source-justified semantic backend and scripted tests, but no Linux
errno is currently mapped to it. A malformed result or post-call projection
failure globally poisons the concrete adapter. A pure pre-call model rejection
issues no lifecycle ioctl and does not fabricate a quarantine history event.
Process change or currentness loss quarantines retained resources and prevents
later native calls. The engine reserves enough bounded history for the
observation and worst-case global quarantine before issuing an ioctl. Queue
lifecycle requests have no `n_success` field; cumulative state here means the
append-only model history and retained process-wide queue-ID observations, not
invented partial-progress semantics.

Resource publications remain live after successful DESTROY. A separate
non-syscall operation releases the four model publications and returns the
backend authority only from `Destroyed`. Active, disabled, pending, and
ambiguous queues cannot release it. `Drop` performs no queue ioctl, retry, or
publication release. The future backend and capability contract separately
forbids cleanup syscalls from their `Drop` implementations.

## Memory-owner integration contract

The production constructor consumes the R2 checked device/VM session and
requires one unforgeable authority with all of these properties:

- exact current device, VM, process incarnation, allocation generation,
  mapping key, and queue generation bindings;
- an AQL ring mapping with the admitted power-of-two size, 4096-byte base
alignment, packet geometry, required double-map/wrap policy, and GPU/CPU
visibility;
- one exact page control mapping containing distinct 8-byte aligned read/write
  counters; KFD truncates each pointer to its containing GPU page and requires
  that GPUVM mapping to be exactly one page;
- the exact 4096-byte EOP mapping and exact gfx942 SPX/NPS1 CWSR address,
  mapping length, context-save size, and control-stack size derived by the
  reviewed resource plan;
- structural queue-owned publications that prevent generic unmap/free while a
  plan may have a native queue;
- no public fd, handle, GPU address, mapping constructor, or numeric queue-ID
  authority, plus explicit return of the resource capability after confirmed
  destruction;
- a retained `CheckedGfx942XnackMinusDevice` and its KFD file description so
  currentness checks and lifecycle ioctls cannot be redirected to different
  devices;
- no cleanup syscall in capability `Drop`; explicit release remains
  fail-closed and at most once.

The production V1 composition satisfies these ownership requirements with a
separately named fe2o3 GTT-only policy: AQL_QUEUE for the ring,
HOST_VISIBLE_COHERENT for control, and EXECUTABLE GTT for EOP/CWSR. Live MI300X
measurement confirms KFD accepts all four concurrently. This does not claim
equivalence with ROCr's USERPTR/SVM/VRAM policy.

## Doorbell mapping boundary

Successful CREATE maps and owns the exact complete 8192-byte process slice
from the retained KFD file description. It validates the encoded base and
in-slice 8-byte offset, installs MADV_DONTFORK while PROT_NONE, then enables
the VMA. No pointer, address, fd, or store API is public. The live lane checks
that the VMA is absent in an isolated child without reading or writing MMIO.

The private submission owner binds the mapping to the retained active queue,
process, device generation, and CREATE observation. No raw pointer or general
MMIO write escapes. The store width/value and CPU ordering sequence are tested,
but GPU coherence, firmware observation, reset races, and write-combining MMIO
semantics remain Contracted rather than proved.

## Private packet publication boundary

The production composition now depends canonically on `fe2o3-aql` and retains
one crate-private, non-`Clone` single-producer submission owner. Before CREATE,
every logical 64-byte ring slot contains the exact little-endian `u32` INVALID
header value `1`; an all-zero slot would encode VENDOR_SPECIFIC and is rejected
by the initialization contract. Each slot header is explicitly initialized as
an `AtomicU32`, and both control counters are explicitly initialized as
`AtomicU64`, before GPU mapping.

Submission acquire-loads the actual shared write and read counters, requires
the write observation to equal the retained model, and applies
`AqlSingleProducerRingModelV1` to reject full, replayed, regressed, impossible,
or exhausted observations. After a second PID/device-currentness check it
performs one acquire-release fetch-add on the actual write pointer. It selects
the exact masked slot, copies the complete still-INVALID packet body, and uses
`AqlPreparedKernelDispatchV1` to release-store one aligned little-endian `u32`
full header. A one-dimensional packet publishes exactly `0x00011402`. A final
currentness check precedes one release fence, x86 SFENCE, and one volatile
little-endian `u64` doorbell store of the packet ID.

Any error after the pure reservation or any possible shared-memory side effect
permanently poisons the submission owner. Counter mismatch, read regression,
impossible distance, exhaustion, or currentness loss also poison it; only an
ordinary full-ring observation remains retryable. The mapped production path
never creates an encompassing Rust slice or exposes a raw mapping pointer. Its
private Linux backend performs only checked exact atomic/copy operations on the
retained `NonNull` mapping. `Drop` performs no
counter, packet, doorbell, ioctl, unmap, or free operation. The callable method
is crate-private and exposes no counter reference, address, pointer, ring
slice, or general MMIO primitive.

Rust atomic ordering does not specify concurrent GPU accesses, system-scope
AQL publication, GTT cache coherence, write-combining MMIO ordering, or
firmware consumption. Those links are **Contracted**, backed by pinned source
and hostile CPU tests; they are not Verus-proven or Rust-memory-model-proven.

## CWSR initialization and first-launch limit

The context-save allocation is zeroed, then receives exactly eight 40-byte
`HsaUserContextSaveAreaHeader` encodings at
`base + xcc * 0x1621000`. `DebugOffset` is
`(8 - xcc) * 0x1621000`, `DebugSize` is `0x5f000`, and the no-event
`ErrorReason` and `ErrorEventId` fields remain zero. The layout and behavior
are pinned to ROCr 7.2.4 `queues.c`
`b7ead541340ac996c2305b2e9660cb3176edcd61ee509d4880f02659fbb6f32b`
and `hsakmttypes.h`
`fd9e3e9a0874614e70e518ee420aacd2d171452c2755d05b2cf54b55144ec78e`.

Zero event fields are sufficient only because this slice launches no kernel.
The first isolated launch may use them only with a process fail-stop rule: any
timeout, unexpected completion, or currentness loss terminates without queue
destroy, unmap/free, or further KFD use. A KFD event plus writable error payload
and observable queue exceptions is a required production R4 follow-up, not a
current property.

There is also a distinct address-semantics blocker. The headers above are
written through the kernel-selected BO CPU VMA, while the retained GPU VA is a
separate PROT_NONE reservation. Active KFD fault handling uses `get_user` and
`put_user` against the CREATE context-save address. Source review supports a
bounded next step: replace exactly the eight owned header guard pages with
DONTFORK anonymous shadow pages and bind a KFD event ID plus shared aligned
error payload. That would cover queue creation and exception delivery only.
It is not full CWSR equivalence: wave-state/debug/eviction control-stack
`copy_to_user` would observe the shadow or remaining PROT_NONE range and must
stay explicitly unsupported. The active `kfd_process.c` source is pinned as
`d76db8cbb546aa23dffb33b1d04244037e12246b49b752303194c68dd685e409`.

Consequently, the native submission method requires a private fault-policy
admission value for which no constructor exists. Live packet publication
remains structurally gated until that separate shadow/event slice is admitted.

R4 still needs a dispatch authority that binds kernarg, code object, queue,
dispatch, and completion generations. The existing inert packet carries only
numeric address observations. A doorbell failure after publication is not
rollback evidence and remains process-teardown-only poison.

## Missing completion-signal boundary

The current UAPI crate pins only reset SMI events, not dispatch completion. The
completion slice must add and independently oracle the KFD 1.18
`CREATE_EVENT`, `DESTROY_EVENT`, `SET_EVENT`, `RESET_EVENT`, and `WAIT_EVENTS`
request numbers, C layouts, nested event-data arrays, timeout/result values,
event-page offset, event ID/slot/trigger-data outputs, and their mutation and
partial-failure semantics. It must also pin the gfx942/ROCr signal-memory
layout and atomics that bind an AQL completion signal to the KFD event wakeup.

The safe API must own a dedicated generation-bound signal for each admitted
dispatch, initialize it before packet publication, wait against the exact
dispatch identity, distinguish timeout from completion, acquire the device's
result writes before exposing host reads, and destroy/recycle the event only
after no packet can reference it. Neither a changed signal word nor a returned
event ID alone is completion authority.

## Verification boundary

The Verus queue proof now also establishes that direct destroy begins only
from Active or Disabled and failed-no-effect restores the exact retained source
phase; an expected-negative mutation rejects restoring Active as Disabled.
This adapter adds an executable projection discipline and hostile fake backend
tests, but there is no Verus proof of this Rust implementation, no
syscall-to-model refinement, and no proof of the kernel, firmware, mappings,
atomics, or hardware memory model. A later refinement must relate each concrete
request/observation pair to the exact `Begin*`/`Observe*` history edge and prove
that all concrete uncertain outcomes select `Indeterminate`.
