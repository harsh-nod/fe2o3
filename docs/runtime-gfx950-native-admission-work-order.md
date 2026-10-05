# Gfx950 Native Admission Work Order

Status: **not implemented or qualified** as of 2026-10-05. This document maps
the next device/model and queue/CWSR implementation slices. Proposed types and
tests below are work orders, not existing capabilities or accepted evidence.

The [gfx950 foundations checkpoint](evidence/dev-gfx950-foundations-2026-10-05/README.md)
owns the current topology, loader and machine-analyzer results and their limits.
Those foundations grant neither native gfx950 queue/dispatch authority nor
conditional-fill machine semantic refinement. Existing gfx942 capabilities,
manifest identities and evidence scopes must remain unchanged.

## Retained Facts And Missing Inputs

The [host observations](evidence/dev-gfx950-foundations-2026-10-05/host-observations.json)
record MI350 PCI device `0x75a0`, target `90500`, wavefront64, SIMD1024,
SIMD-per-CU4, XCC8, LDS160KiB, array-count32, arrays-per-engine1,
max-waves-per-SIMD8 and 24 compute queues. They also record compute firmware41,
SDMA firmware12, two ordinary and 14 XGMI SDMA engines, eight queues per engine,
module source version `703B1127E578BC5D4BD6615`, and `mes=0`, `sched_policy=0`,
`cwsr_enable=1`.

The source directory `/usr/src/amdgpu-6.16.13-2303411.24.04` was observed on MI350.
Its presence and the module source version do not establish a reviewed source
closure or prove which source built the loaded module. Before admitting a native
profile, retain the exact kernel/module and source identities, KFD/DRM UAPI
observations, PCI revision, and full DRM device tuple. The current host-observation
JSON does not retain the kernel release or full DRM tuple. Do not infer these
from the gfx942 profile or accept arbitrary versions.

Review the deployed kernel queue, MQD, doorbell, event and gfx950 CWSR/trap paths
and the matching ROCr queue-resource derivation. The existing gfx942 queue
manifest's source hashes do not qualify those paths. In particular, derive CWSR
from reviewed target policy and observed CU/XCC/LDS inputs, including any
kernel-reported size precedence; do not copy MI300X constants or claim an
unreviewed calculated size as hardware evidence.

## Current Code Boundaries

| Boundary | Exact code and required change |
| --- | --- |
| Native device admission | [device.rs](../crates/fe2o3-kfd/src/device.rs): `DEVICE_ADMISSION_PROFILE_MANIFEST_V1`, `bind_gfx942_xnack_minus`, `validate_platform_provenance`, `validate_render_profile`, `model_profile` and `model_admission` pin gfx942, PCI `0x74a1`, firmware192/25, SIMD1216 and the MI300X platform. Add an independent exact gfx950 profile and separately branded capability; do not broaden `CheckedGfx942XnackMinusDevice`. |
| Device model | [device_identity.rs](../crates/fe2o3-runtime-model/src/device_identity.rs): `DeviceAdmissionProfileV1` stores profile/UAPI identities, while `correlate_model_only_v1` hardcodes the single target. Retain an exact profile/target discriminator through correlation and admission. |
| Projection contract | [device_projection.rs](../crates/fe2o3-runtime-model/src/device_projection.rs): `validate_device_projection_model_only_v1` independently pins platform, firmware, capacity and complete inventory to gfx942. Add separate gfx950 premises and preserve selected-device/inventory/aperture correspondence. |
| Queue model | [queue_lifecycle.rs](../crates/fe2o3-runtime-model/src/queue_lifecycle.rs): `ComputeAqlTargetProfileV1` has one variant; `validate_plan_shape` does not compare it with the admitted device target. Apply the safety gate below before adding a second variant. |
| Resource geometry | [queue_resources.rs](../crates/fe2o3-kfd/src/queue_resources.rs): `plan_gfx942_aql_queue_resources` pins the old source/platform/topology profile, and zero-sized `ContextSaveResourcePlanV1` returns fixed gfx942 dimensions. Add a gfx950-branded checked plan carrying its actual geometry and independent manifest. |
| CWSR layout | [queue_submit.rs](../crates/fe2o3-kfd/src/queue_submit.rs): `gfx942_cwsr_header_bytes` and `initialize_gfx942_cwsr_headers` fix XCC, context and debug offsets. [queue_linux.rs](../crates/fe2o3-kfd/src/queue_linux.rs): `CwsrShadowPlanV1` and shadow ownership independently fix mapping sizes and 24 shadow pages. All must consume the same target-bound geometry; changing only the planner is insufficient. |
| Native ownership | [memory_linux.rs](../crates/fe2o3-kfd/src/memory_linux.rs) retains `CheckedGfx942XnackMinusDevice`; [shared_memory.rs](../crates/fe2o3-kfd/src/shared_memory.rs) exposes acquisition from that token. [currentness/full.rs](../crates/fe2o3-kfd/src/currentness/full.rs) reobserves through default gfx942 discovery. Add exact-target custody and currentness without converting gfx950 into a gfx942 token. |
| Queue construction | [construction_primary.rs](../crates/fe2o3-kfd/src/queue_live/construction_primary.rs) consumes `Gfx942AqlQueueResourcePlanV1`; [queue_live.rs](../crates/fe2o3-kfd/src/queue_live.rs) constructs a gfx942 model queue plan. The new path must join matching device, memory, geometry, queue and output profiles before publication. |
| Queue UAPI outputs | [fe2o3-kfd-uapi/src/lib.rs](../crates/fe2o3-kfd-uapi/src/lib.rs): `admit_kfd_gfx942_create_queue_outputs` produces gfx942-branded queue/doorbell facts. Add separately reviewed gfx950 output admission even if numeric fields coincide. |

## Mandatory Queue-Target Safety Gate

**Do not add a gfx950 queue-model variant until queue target and admitted device
profile are checked together.** The current single-variant model makes that
relationship implicit. `validate_plan_shape` currently checks schema, domain,
device key and resource ownership, but not target agreement.

The new model must reject a gfx942 queue plan carrying a gfx950 admission and
the reverse, even when all domain, VM, resource and generation identifiers
otherwise agree. Retain the discriminator in correlated/admitted device facts;
an arbitrary caller-supplied profile digest must not substitute for it. Keep
this condition in lifecycle invariant validation, not only at one constructor.
The native adapter must additionally match its device token, loader profile,
geometry profile and native queue outputs. Existing gfx942 loader guards remain
closed to gfx950.

## Independently Reviewable Slices

### 1. Model And Read-Only Planning

This is the smallest next implementation slice. It adds no native token, ioctl,
allocation, mmap, doorbell store or dispatch entry point.

1. Add an independent gfx950 model/profile discriminator and exact projection
   admission. Preserve gfx942 constructors and manifest bytes. Implement the
   queue-target safety gate before exposing the second queue target.
2. Add a proposed `Gfx950AqlQueueResourcePlanV1` with checked geometry and a
   separate source/profile manifest. Derive its expected bytes using a distinct
   C oracle against the reviewed sources. Keep the
   [gfx942 oracle](../crates/fe2o3-kfd-uapi/tests/oracles/kfd_gfx942_queue_resources_1_18.c)
   unchanged as a regression oracle, not a gfx950 witness.
3. Extend the corresponding identity/projection/queue contracts in
   [device_identity_generation_v1.rs](../crates/fe2o3-runtime-model/verus/device_identity_generation_v1.rs),
   [device_projection_refinement_v1.rs](../crates/fe2o3-runtime-model/verus/device_projection_refinement_v1.rs)
   and [queue_lifecycle_v1.rs](../crates/fe2o3-runtime-model/verus/queue_lifecycle_v1.rs),
   with explicit target-mismatch negative controls. Proofs of model predicates
   are not authentication of the running kernel or whole-native-adapter refinement.

Model/projection and geometry/oracle work can proceed independently, then join
at the exact profile identity and target agreement tests.

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
