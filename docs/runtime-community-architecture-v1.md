# Runtime Community Architecture V1

## Status

This document defines the community-facing runtime ownership boundaries. The
legacy protected Worker V3 dispatch remains supported, but it is no longer the
shape applications or backend adapters should extend.

## Dependency Direction

The runtime stack has one inward dependency direction:

1. `fe2o3-runtime-model` owns pure executable specifications, invariant
   vocabulary, and model-only identities. It owns no native authority.
2. `fe2o3-kfd-uapi`, `fe2o3-aql`, and native HSA bindings own wire mechanisms.
3. KFD and HSA adapters own native resources and refine model transitions.
4. `fe2o3-runtime` owns the public context, capability, handle, stream, memory,
   module, typed launch, event, completion, peer-copy, and backend-error API.
5. `fe2o3-service-host` composes persistent services above the public runtime
   boundary; it must not create a competing general-purpose runtime API.

`fe2o3-host-api` remains the canonical runtime-neutral orchestration schema and
is re-exported as `fe2o3_runtime::contract`. It describes operations and
commitments; `fe2o3-runtime` is responsible for executing them.

## Target Process Ownership

The scalable target is for one runtime process context to own each native
runtime enablement and VM domain, with devices and queues as children. Creating
another stream or selecting another admitted device must not reacquire a
process-global singleton or consume a one-shot GPU-specific VM token. The
current concrete adapters implement only the subsets listed below.

Every public identity is context-local and nonzero. Backends retain the native
handle behind a numeric sealed SPI handle. Applications cannot manufacture raw
GPU addresses, queue pointers, HSA signals, or KFD resource owners. Backend
handles must be nonzero and unique among live resources of the same kind; a
violation seals the context as terminal while retaining the affected logical
record for cleanup reporting.

## Terminal Native Failure

An error is classified as one of:

- `Rejected`: no device-visible mutation occurred.
- `Quiescent`: mutation occurred, but every referenced resource is conclusively
  quiescent and may be reclaimed.
- `Terminal`: native state or quiescence is ambiguous; resources remain retained
  and the backend context cannot be used again.

Community applications should host KFD/HSA backends in
`RuntimeWorkerBackendV1`. Its public handshake verifies protocol compatibility;
it does not authenticate the executable, module, or host. The caller must
select a trusted worker and provide any required artifact authority, sandbox,
or operating-system isolation. The worker may abort for terminal ambiguity
without terminating the application. The parent treats timeout, EOF, malformed
frames, and worker abort as terminal backend loss.

## Current Implementation

| Backend | Devices and queues | Memory | Unsupported |
| --- | --- | --- | --- |
| KFD | The base backend owns one admitted `gfx942:xnack-` device, one reusable compute queue, multiple serialized logical streams, and one pending compute launch. `KfdMultiDeviceRuntimeBackendV1` admits all devices before queue creation and routes one independent child per device. The lower-level gfx942 SDMA engine owns one additional classic copy queue per child with at most 64 in-flight copies. | Persistent compute storage is unchanged. The SDMA profile adds move-only host-coherent and HBM buffers plus per-device best-fit pools with explicit recycle and trim. The routed facade peer copy is bounded, synchronous host staging. | Same-device concurrent compute dispatches, native XGMI peer copy, integrated runtime-SPI async copy, atomics, collectives |
| HSA | One HIP-correlated gfx942 or gfx950 HSA device with persistent per-stream queues | Host-visible allocations only | Device-local allocation, peer copy, multi-device, atomics, collectives |

The V1 facade's multi-device KFD router advertises peer copy only because it
implements the complete facade contract through host staging. A single-device
KFD child and the HSA adapter do not advertise it. Atomics and collectives are
capability vocabulary only; V1 defines no general atomic or collective
operation. These rows are not HIP/HSA parity.

The KFD adapter validates and owns a module once at load, caches selected
kernel metadata at resolution, and shares those immutable bytes and descriptors
across launch preparation. A completed and recycled same-shape dispatch can be
resubmitted without detaching or rebuilding code, kernarg, or data storage.
Host writes update an attached coherent allocation before launch, and exact
native-dirty extents remain authoritative until facade readback or a later
host-authority requirement. Staging-budget or host-allocation exhaustion is a
pre-publication `Capacity` rejection.

The feature-gated gfx942 qualification lane is intentionally outside production
authority. It re-hashes and loader-validates one repository-owned COV6 object,
then a private KFD gate accepts only that artifact's fixed typed ABI,
metadata-declared effects, deterministic buffer images, and geometry. The gate does not implement
`KfdRuntimeLaunchAuthorityV1` and supplies no compiler-lineage or Worker V3
authentication. Its HSA lane relies separately on the reviewed backend's unsafe
construction contract after admitting the same fixture.

## Asynchronous Operations

Typed launches associate a Rust argument type with an application-supplied,
nonzero 32-byte signature. This creates a stable identity; it does not prove the
native kernarg ABI or the completeness of declared memory effects. Assurance
comes from the KFD launch authority or the HSA backend's unsafe-construction
contract. The argument value produces an address-free kernarg image and
allocation-relative memory effects. Launch dependencies name exact events from
the same device. Submissions are nonblocking and may be polled or waited against
a monotonic deadline.

`RuntimeLaunchGeometryV1::grid` is the global work-item extent published in the
AQL grid-size fields. `workgroup` is the per-group extent. For COV6 implicit
arguments, each block count is `grid / workgroup` and the corresponding
remainder is `grid % workgroup`; resource admission still uses the ceiling
number of workgroups when accounting for a partial final group. The pure
`fe2o3-aql` geometry value derives these implicit dispatch values once; KFD,
the legacy runtime transition, and the HSA adapter only encode that shared
result into their owned kernarg storage.

Peer copies require two distinct peer-capable devices, an exact destination
stream, equal nonempty source/destination ranges, and explicit event
dependencies. Each copy retains a model peer-transfer contract identity. The
current KFD router completes the staged copy before returning its submission
handle; `poll` and `wait` therefore report success immediately. This is not a
claim of asynchronous native peer DMA.

The gfx942 SDMA API is backend-specific so the frozen Worker V1 protocol does
not silently change. Batch submit is nonblocking, tickets bind queue slot and
generation plus the non-reused queue occurrence, buffers remain owned by the
queue until exact completion is
observed, and batch wait uses one operational-currentness envelope. Queue-full
and structural prepublication validation failures return the move-only buffers
after a successful currentness check. Counter or generation divergence,
currentness failure, and any uncertainty after preflight terminally poison the
session and retain native custody.

## Performance Rules

- `fe2o3-completion` transitions update only direct successors. No transition
  may rebuild the complete dependency graph.
- `fe2o3-runtime-model` production lifecycle state uses persistent path-copy
  AVL journals and key indexes with local inductive checks. Public slices are
  materialized lazily and cached. Full invariant scans are audit/debug
  operations, not hot transitions.
- Completion waits use deadlines and a bounded spin/backoff policy. Poll counts
  are not timeout units.
- SDMA batch submission and batch completion are linear in batch depth, bounded
  by 64. Currentness validation is constant per batch rather than per packet;
  ring reservation, packet publication, and doorbell notification remain one
  bounded operation per submitted copy.
- KFD device, VM, allocation, mapping, and queue lifecycle transitions use the
  full contracted topology/aperture currentness composite. Active mapped-memory
  and queue operations use the retained process, reset-event, descriptor, UAPI,
  XNACK, and DRM-loss operational fence. Packet atomics execute within explicit
  owner pre/post fence scopes rather than recursively rescanning host topology.
- `fe2o3-host` alias admission uses allocation-aware interval indexes. It must
  not scan all arguments of all in-flight launches for every new argument.
- `fe2o3-hsa-runtime` indexes pending accesses by allocation, stream, and byte
  interval and carries sparse causal frontiers. Admission must not scan every
  pending submission or walk transitive event ancestry.
- Worker request writes and response reads share one parent-process deadline. A
  dedicated writer owns child stdin so a worker that stops reading cannot block
  the runtime thread past that deadline. Worker-backed completion waits encode
  only a relative child duration and reserve response grace inside the caller's
  deadline; parent and child never assume a shared monotonic-clock epoch.

Scale benchmarks cover the maximum completion graph and large lifecycle
journals. Regressions in asymptotic behavior are release blockers.

The gfx942 runtime qualification runner compares only like-named measurement
scopes. KFD persistent execution, HSA host-visible execution, HIP staging,
synchronized launch/wait, and HIP device-event intervals are reported
separately. Results from unlike scopes must not be converted into parity
ratios; even the KFD/HSA/HIP synchronized rows retain different currentness,
allocation, signal, and readback policies.

## Backend Selection

Native HSA support is selected explicitly by Cargo feature. A stub build is
deterministic and has no ROCm link dependency. Enabling the native feature
requires a configured ROCm development installation and fails the build when
the required headers or libraries are absent.

## Verification Boundary

The R7 Verus file proves eight theorems over the abstract pool/lease/copy model:
in-flight and quarantined blocks are not reusable, completion-gated release
advances generations, stale leases cannot submit reused storage, distinct
retained blocks cannot alias, dependency frontiers gate publication, and peer
copies retain exact device coordinates. Expected-negative mutations demonstrate
that generation reuse and source-device execution invalidate named theorems.

Executable Rust tests cover the pool model, router route-exhaustion preflight
and terminal latching, packet bytes, range checks, queue admission, and custody
transitions. The frozen SDMA manifest pins the reviewed ROCr revision and packet
sources. KFD ioctl results, MMIO, coherence, kernel and firmware behavior,
hardware completion, progress, liveness, and performance are not formally
proved. Native claims require a retained result artifact from an identified
MI300X run. The R7 copy qualification record names the exact measured commit,
devices, software stack, validation policy, load boundaries, raw output, and
limitations in
[`benchmarks/runtime_gfx942/results/async-copy-mi300x-2026-09-01.md`](../benchmarks/runtime_gfx942/results/async-copy-mi300x-2026-09-01.md).
