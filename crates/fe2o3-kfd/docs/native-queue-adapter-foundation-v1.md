# Native queue adapter foundation V1

This R4 slice executes the compute-AQL queue lifecycle against a private
backend and projects every attempted lifecycle operation into the existing
bounded `QueueLifecycleStateV1`. It does not expose a production queue API.
The current memory owner cannot yet mint the exact retained mapping
capabilities required by CREATE_QUEUE, so constructing a Linux backend would
fabricate authority from numeric addresses.

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
address. DISABLE uses the reviewed null-ring encoding. DESTROY is allowed only
after a confirmed DISABLE. Any changed write-only/in-out request record is
indeterminate.

The future Linux backend must translate success to `Succeeded` and every errno
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

A production constructor remains blocked until the R2 memory owner can consume
its retained checked device/VM session and provide one unforgeable authority
with all of these properties:

- exact current device, VM, process incarnation, allocation generation,
  mapping key, and queue generation bindings;
- an AQL ring mapping with the admitted power-of-two size, 4096-byte base
  alignment, packet geometry, required double-map/wrap policy, and GPU/CPU
  visibility;
- distinct, live read/write control-pointer mappings with 8-byte alignment and
  sufficient mapped length;
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

The existing host-visible coherent allocation session does not satisfy these
requirements. In particular, the reviewed queue resource observations name
USERPTR/SVM and VRAM policies that R2 deliberately does not admit.

## Missing doorbell boundary

Successful CREATE currently retains only the validated numeric output. The
next doorbell slice must implement all of the following before dispatch:

1. Map exactly 8192 bytes from the retained KFD file description using the
   validated `encoded_process_slice_offset`; clearing only a 4096-byte page
   mask is incorrect for offsets in the second half of the process slice.
2. Establish an owned, DONTFORK, lifetime-bound mapping and derive the selected
   8-byte doorbell location only from the checked in-slice offset.
3. Bind that mapping to the exact active queue ID, process incarnation, device
   generation, and CREATE observation. No raw pointer or general MMIO write may
   escape.
4. Pin and test the required CPU/device ordering, store width, value encoding,
   cache attributes, teardown order, fork behavior, and reset/currentness
   policy. The current source pins validate geometry, not these executable
   mmap/store semantics.

## Missing packet publication boundary

`fe2o3-kfd-uapi` has no AQL packet or ring-publication schema. R4 still needs a
single-producer reservation token, monotonic read/write index rules, checked
wraparound, complete 64-byte packet definitions, and a typestate sequence that
initializes every non-header field before publishing a valid header with the
reviewed release ordering. Kernarg, code object, mapping, queue, dispatch, and
completion generations must be embedded in that token. Publication must prove
slot uniqueness and initialization-before-validity; the subsequent write-index
and doorbell operations must preserve the required host-to-device ordering.
Fault injection is required before and after reservation, field initialization,
header publication, write-index publication, and doorbell store. A doorbell
failure after publication is not rollback evidence.

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

The existing Verus queue proofs establish properties of the pure lifecycle
model. This adapter adds an executable projection discipline and hostile fake
backend tests, but there is no Verus proof of this Rust implementation, no
syscall-to-model refinement, and no proof of the kernel, firmware, mappings,
atomics, or hardware memory model. A later refinement must relate each concrete
request/observation pair to the exact `Begin*`/`Observe*` history edge and prove
that all concrete uncertain outcomes select `Indeterminate`.
