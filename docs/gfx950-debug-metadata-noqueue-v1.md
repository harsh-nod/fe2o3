# Gfx950 debug metadata without a queue

This engineering API advances owned cold preparation to actual trap registration,
mode-3 debug-runtime enablement and publication of one actual retained code object.
It does not create a queue, dispatch a kernel, stop a wave, read physical registers,
or discharge debugger milestone V4. One supervised native registration and eight
exact early refusals passed in the [root qualification](evidence/gfx950-debug-noqueue-20260924.md).
That result does not qualify queue execution, sampling or attached-debugger acceptance.

## Caller and custody

The unsafe method `Gfx950DebugColdOwnerV1::enable_debug_metadata_without_queue(self)` consumes the
real checked-device/VM/kernel/trap/metadata owner and returns
`Gfx950DebugMetadataNoQueueOwnerV1`. The caller cannot provide addresses, a fake
Kernel, runtime flags, a load bias, a GPU ID, or a metadata list. The public plain
queue-runtime token and mode-1 UAPI constructors are unchanged.

The successor owns the predecessor without extracting or freeing any component.
Preparation armed process-lifetime retention before its first possible VM effect.
The existing exclusive debug-profile reservation remains retained. Success,
error, panic, ordinary Drop, and process/PID drift cannot release published
metadata, URI bytes, original ELF, trap/code mappings, handles, or device FDs.
Dropping the reservation poisons the in-library process gate. No retry, runtime
disable, trap clear, queue transfer, resource extraction, or teardown acknowledgment
API exists. Deliberate retention is not a cleanup implementation.

The exact transition is:

1. Recheck actual cold resource cardinality, content/mapping identity, trap
   readback and checked-device currentness.
2. Mark metadata storage possibly exposed before attempting SET_TRAP_HANDLER.
3. Register the retained trap mapping on the actual checked GPU with TMA zero.
4. After acknowledgment and currentness, advertise version 11 in the stable root.
5. Enable the runtime with the actual stable root, mode 3, capabilities input 0.
   Reject any unexpected returned fields. The installed driver leaves them intact.
6. Publish the actual link under ADD -> host rendezvous -> map/CONSISTENT -> host
   rendezvous, with fences and currentness boundaries.

The caller has a process-wide unsafe precondition: the process must be isolated
and disposable, with no foreign KFD/ROCr runtime,
queue, or concurrent foreign runtime activity. The in-library process gate does
not prove foreign exclusion. In particular, driver runtime-enable rejection of
existing queues occurs AFTER this operation registers the trap, so EEXIST is not
a safety check for a process that already contains foreign queues.

Runtime enable can block on an attached debugger's runtime-event acknowledgment.
Use an external process supervisor; no retry or in-process rollback is allowed on
timeout/interruption. Process termination, not Rust Drop, ends this tranche's
resource lifetime.

## Trap, TMA and sampling limits

The pinned ROCr handler uses TTMP14:15 as its second-level trap-memory root.
Its host-PC-sampling path reads a buffer pointer at root+0; its stochastic path
reads at root+8. Both continue into GPU atomic/buffer operations. The relevant
paths do not first prove that a zero root is safe.

The selected compute_942 handler is also ROCr's gfx950 family selection, but the
cold compiled-text hash and successful registration alone do not establish correct
first-level CWSR transfer, TTMP setup, stop/resume semantics, sampling safety or
debugger interpretation. CPU read-only trap mapping is not GPU write protection.

This successor cannot create or obtain an owned queue, so it has no owned wave
that can execute the handler. That is the sole in-library reason zero TMA remains
nonexecuting here; it is NOT a proof that PC sampling is disabled. Future queue
support must either establish an exact enforceable sampling exclusion or supply
a fully validated retained TMA/sampling-memory design. A proposed zero-capacity
sampling sink would require its own arithmetic/layout/first-level-ABI proof and
hardware qualification; it is not implemented or assumed here.

## Minimum validation before native registration qualification

Root/integrator must, before activating even this no-queue path:

- Review and pin actual installed KFD/ROCr/ROCdbgapi versions and the selected
  device; reproduce exact compiled trap content and normal COV6 loader admission.
- Reconfirm SET_TRAP_HANDLER's 24-byte input ABI and 0x40184b13 request,
  runtime-enable's 16-byte in/out ABI and 0xc0104b25 request, mode bits 1|2,
  output capabilities behavior, and current driver CWSR/first-level behavior.
- Establish a fresh disposable isolated process with no other runtime, KFD
  clients, queues or PC-sampling tooling; neither a descriptive flag nor the
  process-local gate authenticates that environmental contract. This is a caller
  proof obligation covering the whole process lifetime, including after Drop;
  /proc snapshots are refusal fences, not a substitute for that proof.
- Run root-owned build, CPU failure/lifetime tests, both relevant feature sets
  and lint gates. Keep plain runtime/gate behavior unchanged.
- Use bounded external supervision for the runtime-event wait and preserve
  process/device/artifact pins. If a debugger is attached, validate its genuine
  version-11 metadata parsing, host breakpoint callbacks and original-ELF URI
  reads against the owned mapping, not a fabricated DTO.
- Observe no owned queue/dispatch. Terminate the disposable process at the end;
  do not infer successful teardown from Drop or absence of an error message.

Before future execution, independently validate the TMA/sampling contract,
gfx950 TTMP/CWSR save/restore and per-wave identity; add a consuming queue owner
retaining the same custody; implement ordered queue/event/runtime/trap teardown
acknowledgments; and provide same-stop native capture and stale/cancel controls.
The current gfx942 native resource token must not be cast into gfx950 authority.

## Source basis

The installed driver source inspected on mi350 is
`/usr/src/amdgpu-6.16.13-2303411.24.04`: `kfd_chardev.c` shows registration and
runtime-mode/event behavior; `kfd_process.c::kfd_process_set_trap_handler` stores
second-level addresses in CWSR TMA when present, otherwise binds first-level
addresses; `include/uapi/linux/kfd_ioctl.h` defines the exact wire layouts.

The pinned upstream [ROCr trap handler](https://github.com/ROCm/ROCR-Runtime/blob/820d83572bd8a098ba8366b84943c760ecce8ca5/runtime/hsa-runtime/core/runtime/trap_handler/trap_handler.s)
and [agent selector](https://github.com/ROCm/ROCR-Runtime/blob/820d83572bd8a098ba8366b84943c760ecce8ca5/runtime/hsa-runtime/core/runtime/amd_gpu_agent.cpp)
supply the handler/TMA context. The pinned [ROCdbgapi rendezvous ABI](https://github.com/ROCm/ROCdbgapi/blob/06465e940698e8423d1b629c834c98bdd7753439/src/rocr_rdebug.h)
and [process implementation](https://github.com/ROCm/ROCdbgapi/blob/06465e940698e8423d1b629c834c98bdd7753439/src/process.cpp)
supply metadata/list interpretation. These nominal source relationships are not
a reproducible build proof for installed binaries.

## Milestone intersection

This is a bounded infrastructure increment toward #281 V4, not an accepted exit.
V3 still needs genuine compiler allocation/instruction lineage and authored-vs-
promoted comparisons. V4 still needs distinct gfx950 queue/debug ownership and
actual supported same-stop hardware cells. V5 still needs the hardware lessons,
tiled comparison and full pinned cross-site validation. Existing CPU/recorded
visualizations and accepted milestone counts are unchanged.
