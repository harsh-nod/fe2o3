# Gfx950 Native Admission Work Order

Status: **model, read-only planning and numeric queue-output observations
implemented and scoped-qualified** as of 2026-10-06 UTC. Native gfx950 device, memory, queue and SDMA
capabilities remain unimplemented. This document separates the completed code
boundaries from the next native implementation slices.

The [gfx950 foundations checkpoint](evidence/dev-gfx950-foundations-2026-10-05/README.md)
owns the current topology, loader and machine-analyzer results and their limits.
Those foundations grant neither native gfx950 queue/dispatch authority nor
conditional-fill machine semantic refinement. Existing gfx942 capabilities,
manifest identities and evidence scopes must remain unchanged.

The [model/planning checkpoint](evidence/dev-gfx950-model-planning-2026-10-05/README.md)
records CPU tests, 35 model proof obligations, 22 affected negative controls and
read-only planning on all eight MI350 GPUs. Its broad Verus attempt stopped at
a baseline reader-proof source-pin mismatch and its requalification timed out.
The October 6 scoped reader campaign now passes both positives and all 16
unchanged mutations after isolating two existing proof queries. Its source pin
is refreshed and all 776 source checks match. The broad proof suite has not been
rerun; no broad-suite pass or whole-adapter refinement is claimed.

The [parallel A1/A2/A3 checkpoint](evidence/dev-a1-a2-a3-parallel-2026-10-06/README.md)
adds independent gfx950 queue/doorbell output observations and an inert join to
the resource plan. The join rejects different caller-retained full GPU IDs,
including 16-bit mmap-hash collisions, and keeps both profile identities.
Neither observation nor plan authenticates a live device or a successful queue
creation. The independent C oracle, UAPI tests, KFD regression, target-separation
doctests and strict UAPI/KFD lint checks pass. No native queue was created.

## Retained Facts And Missing Inputs

The [host observations](evidence/dev-gfx950-foundations-2026-10-05/host-observations.json)
record MI350 PCI device `0x75a0`, target `90500`, wavefront64, SIMD1024,
SIMD-per-CU4, XCC8, LDS160KiB, array-count32, arrays-per-engine1,
max-waves-per-SIMD8 and 24 compute queues. They also record compute firmware41,
SDMA firmware12, two ordinary and 14 XGMI SDMA engines, eight queues per engine,
module source version `703B1127E578BC5D4BD6615`, and `mes=0`, `sched_policy=0`,
`cwsr_enable=1`.

Fresh observation-only KFD/DRM ioctls confirm KFD 1.18, DRM 3.64.0, acceleration
enabled, PCI `0x75a0`, chip revision 0, external revision 80, PCI revision 0 and
family 141 on all eight render nodes. Fresh sysfs observations confirm kernel
`6.8.0-124-generic`, module `6.16.13`, source version
`703B1127E578BC5D4BD6615`, SPX/NPS1 and the driver parameters above. The probe
does not observe process XNACK mode, apertures, reset subscriptions or native
admission/currentness. The gfx950 model's disabled-XNACK premise remains an
explicit untrusted input, not a hardware result.

The source directory `/usr/src/amdgpu-6.16.13-2303411.24.04` was observed on MI350.
The independent gfx950 queue manifest now retains reviewed KFD queue/topology,
MQD/UAPI and ROCr 7.2.1 resource/header source identities. Those source files and
the module source version do not authenticate the loaded module or ROCr binary.

The new read-only geometry derives CWSR from observed CU/XCC/LDS inputs and
reviewed target policy. It covers independent positive kernel context/control
size precedence; the observed deployed sysfs exports neither property, so the
public closed planner admits only the derived fallback. Geometry/oracle
agreement is not native allocation or CWSR execution evidence. Complete the
doorbell, event and gfx950 trap/currentness review before native admission.

## Current Code Boundaries

| Boundary | Exact code and required change |
| --- | --- |
| Native device admission | [device.rs](../crates/fe2o3-kfd/src/device.rs): `DEVICE_ADMISSION_PROFILE_MANIFEST_V1`, `bind_gfx942_xnack_minus`, `validate_platform_provenance`, `validate_render_profile`, `model_profile` and `model_admission` pin gfx942, PCI `0x74a1`, firmware192/25, SIMD1216 and the MI300X platform. Add an independent exact gfx950 profile and separately branded capability; do not broaden `CheckedGfx942XnackMinusDevice`. |
| Device model | [device_identity.rs](../crates/fe2o3-runtime-model/src/device_identity.rs): implemented closed `DeviceAdmissionTargetProfileV1`, separate gfx950 constructor, and retained target through model-only correlation/admission. Caller digests do not substitute for the target. |
| Projection contract | [device_projection.rs](../crates/fe2o3-runtime-model/src/device_projection.rs): implemented separate exact platform, firmware, capacity and DRM tuples; selected-device/inventory/aperture correspondence remains checked. This is not native observation authentication. |
| Queue model | [queue_lifecycle.rs](../crates/fe2o3-runtime-model/src/queue_lifecycle.rs): implemented gfx950 variant and mandatory target/profile equality in shared plan validation and lifecycle invariant replay. |
| Resource geometry | [gfx950_queue_resources.rs](../crates/fe2o3-kfd/src/gfx950_queue_resources.rs): implemented separate read-only `Gfx950AqlQueueResourcePlanV1`, checked CWSR/header/debug/shadow offsets and independent manifest/oracle. Old [queue_resources.rs](../crates/fe2o3-kfd/src/queue_resources.rs) types and bytes remain unchanged. |
| CWSR layout | [queue_submit.rs](../crates/fe2o3-kfd/src/queue_submit.rs): `gfx942_cwsr_header_bytes` and `initialize_gfx942_cwsr_headers` fix XCC, context and debug offsets. [queue_linux.rs](../crates/fe2o3-kfd/src/queue_linux.rs): `CwsrShadowPlanV1` and shadow ownership independently fix mapping sizes and 24 shadow pages. All must consume the same target-bound geometry; changing only the planner is insufficient. |
| Native ownership | [memory_linux.rs](../crates/fe2o3-kfd/src/memory_linux.rs) retains `CheckedGfx942XnackMinusDevice`; [shared_memory.rs](../crates/fe2o3-kfd/src/shared_memory.rs) exposes acquisition from that token. [currentness/full.rs](../crates/fe2o3-kfd/src/currentness/full.rs) reobserves through default gfx942 discovery. Add exact-target custody and currentness without converting gfx950 into a gfx942 token. |
| Queue construction | [construction_primary.rs](../crates/fe2o3-kfd/src/queue_live/construction_primary.rs) consumes `Gfx942AqlQueueResourcePlanV1`; [queue_live.rs](../crates/fe2o3-kfd/src/queue_live.rs) constructs a gfx942 model queue plan. The new path must join matching device, memory, geometry, queue and output profiles before publication. |
| Queue UAPI outputs | [gfx950_queue_outputs.rs](../crates/fe2o3-kfd-uapi/src/gfx950_queue_outputs.rs): implemented separately branded numeric output observations, independent source/profile manifest and C oracle. [KFD planning join](../crates/fe2o3-kfd/src/gfx950_queue_outputs.rs) retains resource/output profiles and rejects full-GPU-ID mismatch. Native successful-call/input-preservation and device/VM/custody joins remain; numeric observations alone are not admission. |

## Mandatory Queue-Target Safety Gate

**Implemented for the model; still required at every future native join.**
`validate_plan_shape` checks queue target and admitted device profile together,
in addition to schema, domain, device key and resource ownership.

The model rejects a gfx942 queue plan carrying a gfx950 admission and the
reverse, even when all domain, VM, resource and generation identifiers
otherwise agree. The discriminator is retained in correlated/admitted device
facts; an arbitrary caller-supplied profile digest cannot substitute for it.
The same check applies during lifecycle invariant validation, not only at one
constructor. Reciprocal admission and invariant-replay tests cover both targets.
The native adapter must additionally match its device token, loader profile,
geometry profile and native queue outputs. Existing gfx942 loader guards remain
closed to gfx950.

## Independently Reviewable Slices

### 1. Model And Read-Only Planning

This slice is implemented and scoped-qualified. It adds no native token,
ioctl, allocation, mmap, doorbell store or dispatch entry point.

1. Closed gfx950 model/profile discriminator and exact projection admission;
   gfx942 constructors and manifest bytes preserved; queue-target safety gate
   shared by initial admission and invariant replay.
2. `Gfx950AqlQueueResourcePlanV1` with checked geometry and a separate
   source/profile manifest. A distinct C oracle checks expected bytes against
   reviewed source/header identities. The
   [gfx942 oracle](../crates/fe2o3-kfd-uapi/tests/oracles/kfd_gfx942_queue_resources_1_18.c)
   remains unchanged as a regression oracle, not a gfx950 witness.
3. Target-bound identity/projection/queue contracts in
   [device_identity_generation_v1.rs](../crates/fe2o3-runtime-model/verus/device_identity_generation_v1.rs),
   [device_projection_refinement_v1.rs](../crates/fe2o3-runtime-model/verus/device_projection_refinement_v1.rs)
   and [queue_lifecycle_v1.rs](../crates/fe2o3-runtime-model/verus/queue_lifecycle_v1.rs),
   with explicit target-mismatch negative controls. Proofs of model predicates
   are not authentication of the running kernel or whole-native-adapter refinement.

The planner's source-profile digest is not a native device-admission identity.
No caller-supplied model digest or read-only plan can create a native token.

### 2. Native Device And Barrier-Only Queue

After the first slice and source review, introduce a separately branded proposed
`CheckedGfx950XnackMinusDevice`, preserving the existing admission/currentness
ordering, process-global history and fail-stop rules. Share only private,
explicitly target-bound custody mechanics; do not introduce a conversion to a
gfx942 authority or duplicate the entire public runtime surface.

Extend native VM/memory ownership, exact queue-output admission, CWSR
initialization/shadow custody and queue creation to retain the same checked
gfx950 profile. First qualify a bounded barrier-only create, publish, completion
and confirmed destroy/release probe. That probe must expose no arbitrary kernel
dispatch, fixed compute or SDMA authority. Timeout or process exit is not proof
of native settlement; retain existing quarantine/termination semantics.

Proposed next swarm split, not implemented in the read-only checkpoint:

- Device/currentness: factor private target-bound retained custody beneath
  separate gfx942/gfx950 wrappers. Reobserve the retained target instead of
  default gfx942; keep directional gfx942 XGMI checks specialized.
- Memory: reuse `MemoryBackend` and `SharedMemoryEngine` behind private
  target-typed custody. Preserve the gfx942 session API; expose only the memory
  operations needed by the gfx950 barrier probe.
- Queue/UAPI: make private construction, header and shadow ownership consume
  checked geometry; join it to the admitted device, current VM and exact owned
  reservation before any mapping. Review doorbell/PQM/mmap contracts and add
  native admission over the now-separate gfx950 numeric output/doorbell
  observations, not gfx942 output relabeling. Numeric checks alone do not
  establish successful syscalls, preserved inputs or exact queue custody.
- Primary integration: check device/queue/geometry/output target agreement
  before effects; run constructor/currentness/failure-custody controls, then
  qualify one GPU's barrier/completion/destroy before two independent GPUs.

### 3. One- And Two-GPU Application Join

A barrier probe is not a fill launch. One-GPU application execution additionally
requires separate gfx950 conditional-fill machine semantic refinement, exact
loader/ABI binding, runtime authority and the protected source/service campaign.

The two-GPU application additionally requires gfx950-branded directional route
admission, peer-memory ownership/currentness and SDMA queue/packet authority.
The observed engine counts alone do not admit these paths. Keep all existing
gfx942 route and dispatch rejection tests. Rejoin the
[multi-GPU application work order](runtime-multi-gpu-critical-path.md#current-work-order)
only after these prerequisites; preserve exact output and explicit cleanup gates.

## Required Qualification

- Cross-target device/profile, projection, inventory and queue/device
  substitution tests, including matching identities with the wrong retained
  target. Keep gfx942 manifest/identity golden tests.
- Independent gfx950 geometry-oracle agreement; wrong LDS, SIMD, XCC, partition,
  source profile, driver parameters and ring geometry must fail closed. Cover
  checked arithmetic, divisibility, overflow, alignment and exact mapping size.
- Byte-exact CWSR headers for every XCC, disjoint in-bounds shadow pages, debug
  region offsets and exact event/payload binding. Reject old gfx942 geometry in
  the gfx950 path and the reverse.
- Compile-fail or equivalent public-boundary tests preventing gfx950 device,
  queue and output capabilities from entering gfx942-only APIs.
- Scripted currentness changes, VM failures, partial queue creation, invalid
  returned outputs, destroy ambiguity and quarantine custody before live tests.
  No wrong-profile rejection may issue a native mutation or publication.
- Fresh occupancy checks and bounded barrier-only hardware qualification on one
  selected device, followed by explicit inspected release and owned-file cleanup.
  No reset or destructive fault injection on shared hosts.

Completion of these slices is not full HIP/HSA parity, an A3/A7 milestone exit,
or performance qualification. No order-of-magnitude speedup is claimed.
