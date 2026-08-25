# Native queue adapter foundation V1

This R7 slice executes the compute-AQL queue lifecycle against a private
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

## Fixed packet publication boundary

The production composition now depends canonically on `fe2o3-aql` and retains
one internal, non-`Clone` single-producer submission owner. Before CREATE,
every logical 64-byte ring slot contains the exact little-endian `u32` INVALID
header value `1`; an all-zero slot would encode VENDOR_SPECIFIC and is rejected
by the initialization contract. Each slot header is explicitly initialized as
an `AtomicU32`, and both control counters are explicitly initialized as
`AtomicU64`, before GPU mapping.

Submission acquire-loads the actual shared write and read counters, requires
the write observation to equal the retained model, and applies
`AqlSingleProducerRingModelV1` with the additive V2 transition to reserve one
through 8192 packets while
rejecting over-capacity, full, insufficient, replayed, regressed, impossible,
or exhausted observations. After a second PID/device-currentness check it
performs exactly one acquire-release fetch-add by the complete batch count on
the actual write pointer. It selects every exact masked slot, copies all
complete still-INVALID packet bodies before publishing any packet, and then
release-stores the aligned little-endian `u32` full headers in packet order. A
one-dimensional packet publishes exactly `0x00011402`. A final currentness
check precedes release fence, x86 SFENCE, and volatile little-endian `u64`
doorbell stores for every packet ID in monotonic order.

Any error after the pure reservation or any possible shared-memory or MMIO side
effect permanently poisons the submission owner. A notification failure may
leave the complete published batch device-visible, so no packet-prefix execution
authority is inferred. Counter mismatch, read regression,
impossible distance, exhaustion, or currentness loss also poison it; only an
ordinary full or insufficient-space observation before the actual write
reservation remains retryable. The mapped production path never creates an
encompassing Rust slice or exposes a raw mapping pointer. Its private Linux
backend performs only checked exact atomic/copy operations on the retained
`NonNull` mapping. `Drop` performs no counter, packet, doorbell, ioctl, unmap,
or free operation. The callable methods are crate-private and expose no
counter reference, address, pointer, ring slice, or general MMIO primitive.
They accept inert packet values, not code, kernarg, allocation, dispatch, or
completion authority.

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

The pre-dispatch composition now owns the process-global RUNTIME_ENABLE
transition before creating an event or queue. It admits one first-internal-page
auto-reset signal event with ID 1 through 255. Exactly eight owned PROT_NONE
reservation pages at the gfx942 XCC stride are replaced by private anonymous
pages, marked DONTFORK before read/write access, and initialized with the same
exact 40-byte header as the executable-GTT BO. Every header names the event and
one aligned zero error-reason word in shadow page zero. Addresses, fd values,
and event authority remain private.

Cleanup is typed and ordered: confirmed queue DESTROY, event DESTROY, runtime
disable, then doorbell and complete CWSR/resource release. Drop performs none
of those effects. A bounded wait is one-shot and terminal because a timeout and
zero payload are only a racy snapshot. No later publish or in-process cleanup
is admitted after any wait attempt or ambiguity.

This still is not full CWSR equivalence. CPU-visible debug suspend and
checkpoint/wave-state control-stack `copy_to_user` paths observe the shadow or
remaining PROT_NONE range and stay unsupported. Ordinary hardware CWSR
preemption and restore use the GPUVM BO and remain Contracted, not excluded.
The live evidence creates and destroys the prepared queue with zero packets and
MMIO stores; it does not inject a fault or prove actual exception delivery.
Foreign KFD clients in the same process are also outside the crate-global
runtime ownership claim.

The public addressless fixed-dispatch layer consumes one through 32 inspected
code envelopes, zero-pointer kernarg images, and the exact complete set of
mapped device-data authorities. Access effects come from inspected metadata;
read access requires sealed full-byte initialization. Exact-set admission
permits queue-model transfer only when every retained device lease is
represented once. The queue owns those resources through publication,
completion, and signal recycle; generation keys are substitution checks rather
than ownership.
A doorbell failure after publication is not rollback evidence and remains
process-teardown-only poison.

After one exact dispatch generation reaches completion and signal recycle, a
detach transition releases code and kernarg while keeping the queue, ring,
completion arena, event, runtime, and doorbell live. A later fixed batch may use
a different packet/program cardinality, geometry, scalar kernarg bytes, and
device-data set. The detached-lease ledger forbids queue destruction until all
data is rebound or explicitly released. Fully initialized state survives this
transition, but no pre-publication content digest is restored as current.

After DESTROY is confirmed, return is all-or-terminal. Any later
event/runtime/doorbell/CWSR/queue/code/kernarg/completion release or model
restoration error produces no partial returned value. The consumed session's
no-effect drops retain all possibly live native resources until process
teardown; cleanup and retry are not admitted.

## Private completion-signal boundary

The owned KFD event remains only the queue-exception route. Dispatch completion
uses a separate exact 512 KiB host-coherent allocation with 8192 unique 64-byte
ROCr user signals. Each signal object is initialized to pending before GPU map.
The arena and its address facts stay crate-private and linearly owned by the
queue session; no KFD wakeup or public address is introduced.

One fixed batch binding reserves a distinct signal per packet and retains the
exact queue, signal-allocation, code-mapping, kernarg-mapping, and dispatch
generations.
It publishes through the existing one-reservation/monotonic-doorbell primitive.
Bounded polling acquires every signal value and distinguishes pending, all-zero
ready, unexpected-value fault, and timeout. Only all-zero evidence permits a
release reset and slot-generation advance. A live or completed-but-unrecycled
batch prevents queue resource release; timeout, invalid poll bounds,
generation exhaustion, currentness loss, observation ambiguity, and partial
reset poison the session and require process teardown.

The relationship between a firmware signal write, dispatch completion,
system-scope coherence, visibility of device result writes, and dispatch
quiescence is **Contracted**. Host mocks exercise the lifecycle and every
effect boundary, but there is no concrete Verus refinement or hardware run.
The public layer exposes no packet template, signal, pointer, address, or MMIO
capability. Implicit-kernarg construction is rejected. Concrete kernel
alias/effect refinement and hardware quiescence remain unimplemented
obligations rather than properties inferred from generation keys.

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
