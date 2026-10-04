# Conditional Fill Invocation V1

This is the native host join for the first two-GPU application profile. It does
not establish successful production application admission or hardware execution.

## Admission And Custody

An `Arc<RemoteConditionalFillArtifactV1<K>>` can prepare independent invocations
with `prepare_generated_multi_context_invocation_async`. Every invocation retains
the same original publication token, production compiler audit and authenticated
controller client. Neither the token nor proof is reconstructed or reacquired;
FD195 is not reused. The artifact remains distinct from an unconditional
`AuthenticatedWorkerV3ExecutableV1`.

The existing multi-device async owner selects its original checked child device.
The application can drive it with `RuntimeAsyncCurrentThreadOwnedEngineV1`, without
adding threads to the permanent application sandbox. No new routing or settlement
protocol is introduced.

Preparation requires:

- The exact generated layout, evaluated once, used for both charged packing and
  conditional coverage against the original compiler inputs and handoff.
- Nonempty write-only u32 output, complete coverage, one-dimensional full64 grid,
  workgroup `[64, 1, 1]`, and zero dynamic LDS. N=65/G=128 is supported by the CPU
  preparation test; this is not a GPU execution result.
- The same consumed packed buffers/fixups, selected kernel identity, original
  token's HSACO, complete loader identity, descriptor binding/offset, and retained
  finalizer's output hash/length.
- The mandatory native conditional-fill constraint on the private generated
  packet before constructing its invocation-specific execution authority.

The last constraint additionally checks the actual patched 272-byte kernarg,
whole coherent-host output, original memory-session association and first dispatch
generation during native preparation/binding. A packed coverage check alone does
not discharge those obligations. Device-local output is not added by this change.

Remote revalidation brackets preparation, including argument-callback errors.
Subsequent runtime validation probes the same controller with a fixed caller
deadline. Expiration or controller loss is an error, never evidence of settlement.
The existing runtime retains the complete carrier during native failure or
quarantine. One invocation completing cannot release another invocation's Arc.

## Completion

Conditional and existing unconditional invocations share one private charged
completion carrier. It retains the original readback destinations, decoder, result
gate and budget. Authority survives storage disposal and final decoder commitment.
Only runtime settlement enables this consuming completion callback. Dropping a
preparation or timing out does not publish a result or infer GPU quiescence.

## Qualification Limits

CPU tests exercise actual macro-generated fill arguments and captured compiler /
Worker bytes, including N=65/G=128 preparation, rejection controls, original buffer
continuity and exact credit cleanup. A rejecting test authority exercises shared
carrier lifetime and completion ordering without manufacturing a remote owner.
These tests are not a new formal proof or production two-GPU qualification.

The remaining application gate needs a fresh selected-rustc compilation through
the actual issuer and anchor, the complete installed proof deployment, and the
actual manager/controller/FD195 join. Explicit
[Cargo/supervisor custodian routing](runtime-custodian-supervisor-route-v1.md) is
implemented; its CPU/component qualification is not production proof admission.
The older composed-startup test uses a synthetic carriage and intentionally closes
the audit connection; it cannot satisfy this gate.

After that admission succeeds, qualify two invocations sharing the real remote
artifact, then move their completed bytes through guarded staging, settled H2D,
PUBLIC XGMI and readback in both directions on free MI300X devices. No HIP/HSA
performance or parity claim follows from this CPU checkpoint.
