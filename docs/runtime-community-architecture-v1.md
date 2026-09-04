# Runtime Community Architecture V1

## Status

This document defines the community-facing runtime ownership boundaries. The
bounded protected Worker V3 dispatch remains an internal production direction,
but it is not yet a supported public application path. New backend work extends
the direct-KFD boundary; HIP/HSA paths are deprecated qualification-only code.

## Dependency Direction

The runtime stack has one inward dependency direction:

1. `fe2o3-runtime-model` owns pure executable specifications, invariant
   vocabulary, and model-only identities. It owns no native authority.
2. `fe2o3-kfd-uapi` and `fe2o3-aql` own production wire mechanisms. Native HSA
   bindings are retained only for deprecated qualification.
3. The KFD adapter owns production native resources and implements corresponding
   transitions that are checked separately against the model. The HSA adapter
   exercises the same SPI only in explicit legacy qualification builds.
4. `fe2o3-runtime` owns the public context, capability, handle, stream, memory,
   module, typed launch, event, completion, peer-copy, and backend-error API.
5. `fe2o3-runtime-machine-adapter` is an integration-only join between the
   move-only authenticated analyzer receipt and an exact runtime prepared
   dispatch. The host runtime does not depend on compiler-analysis objects.
6. `fe2o3-service-host` composes persistent services above the public runtime
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

Community applications should host native KFD backends requiring operational
isolation in negotiated `RuntimeWorkerBackendV4`; its exact profile carries
execution-capability discovery, flush, same-device async copy, cancellation,
and deadline-bounded drain. The canonical V4 server requires the backend to
implement all three additive SPIs; the direct, multi-device, and native-XGMI
KFD owners satisfy that bound. Typed atomic/collective process transport uses additive
`RuntimeWorkerBackendV5`, which retains every V4 operation and requires the two
semantic SPIs. Direct and multi-device KFD carry exact contracts; copy-only
native XGMI satisfies the server bound only through explicit pre-custody
`Unsupported` rejections and false semantic capabilities. Frozen
`RuntimeWorkerBackendV1` remains for backends that explicitly opt
into immediate progress. The versioned transports share one subprocess owner
and reject cross-version handshakes. V4 and V5 cache capability records only
for the latest successfully enumerated roster and fail closed before enumeration,
after a failed replacement, and for unknown handles. Runtime Worker transport
versioning is separate from compiler/proof Worker V3. The same isolation can
contain deprecated HSA qualification code, but that is not a public runtime
route. Its public handshake verifies protocol compatibility;
it does not authenticate the executable, module, or host. The caller must
select a trusted worker and provide any required artifact authority, sandbox,
or operating-system isolation. The worker may abort for terminal ambiguity
without terminating the application. The parent treats timeout, EOF, malformed
frames, and worker abort as terminal backend loss.

## Current Implementation

| Backend | Devices and queues | Memory | Unsupported |
| --- | --- | --- | --- |
| KFD | The direct runtime backend owns one admitted `gfx942:xnack-` device, exactly two reusable native compute lanes, directional SDMA queues, and at most 65,536 logical streams with bounded caller-driven FIFO scheduling. The lower-level `fe2o3-kfd` compute session additionally supports an even 2-through-16 striped SDMA set, which is not yet wired into `KfdRuntimeBackendV1`. At most one dispatch occupies each compute lane, and concurrent native work must use disjoint allocations. `KfdMultiDeviceRuntimeBackendV1` admits every selected device before queue creation and routes one child per device. `KfdNativeXgmiRuntimeBackendV1` is a separate exact two-device, copy-only facade backend. Exact atomic/collective contracts can use a separate unsafe semantic-authority constructor; ordinary constructors remain fail-closed. | Logical allocations retain pooled native host-coherent or HBM SDMA buffers. Device-local buffers are zero-initialized before publication and scrubbed before recycle; explicit shutdown trims the pool. Fixed-dispatch compute storage remains separate and is synchronized lazily. Generic peer copy remains bounded host staging; the XGMI backend retains reusable PUBLIC-HBM mappings to the exact two-GPU roster and publishes ready copies in batches of at most 63. Lower-level production striped submission prepares every bounded shard before publication and reports exact partial custody without rollback. | Native queue-side dependency packets, more than two in-flight compute dispatches, runtime-facade striped SDMA integration, unified persistent compute/SDMA/XGMI storage, unified compute plus native XGMI, a concrete production semantic authority, broader atomic/collective profiles, formal native refinement, clean hardware evidence |
| HSA, deprecated qualification only | One HIP-correlated gfx942 or gfx950 HSA device with persistent per-stream queues | Host-visible allocations only | Production use, device-local allocation, peer copy, multi-device, atomics, collectives |

The V1 facade's multi-device KFD router advertises peer copy through host
staging. The separate `KfdNativeXgmiRuntimeBackendV1` implements the same public
peer-copy SPI with native XGMI for one exact pair, but exposes no compute or
same-device copy operations. A single-device KFD child and the deprecated HSA
adapter do not advertise peer copy. The V1 facade has typed atomic and
collective launch wrappers with explicit operation, scope, ordering, geometry,
and collective-participation contracts. Compare-exchange additionally binds
weak/strong mode and a legal failure ordering distinct from its success
ordering. They submit through additive contract-preserving backend SPIs; they
do not synthesize a native operation. Direct and multi-device KFD retain the
contract through scheduling and present it to final invocation authority.
Ordinary and qualification KFD constructors keep both capability layers false.
A separate unsafe semantic-authority constructor advertises only its enumerated
non-System-atomic and workgroup-collective profiles, and mismatches reject
before custody. Final invocation authorization still occurs during preparation;
a denial settles the accepted unpublished submission and releases its custody
before native publication. Runtime Worker V5 preserves the exact validated
semantic contract over its bounded wire operation, but provides no concrete
production authority; structural machine evidence alone is insufficient.
These rows are not HIP/HSA parity.

The KFD adapter validates and owns a module once at load, caches selected
kernel metadata at resolution, and shares those immutable bytes and descriptors
across launch preparation. A completed and recycled same-shape dispatch can be
resubmitted without detaching or rebuilding code, kernarg, or data storage.
Host writes update an attached coherent allocation before launch, and exact
native-dirty extents remain authoritative until facade readback or a later
host-authority requirement. Staging-budget or host-allocation exhaustion is a
pre-publication `Capacity` rejection.

The direct adapter's same-device copies are native SDMA submissions. Direct
dependency chains are capped at 256 before ledger mutation, cancellation can
withdraw only work still waiting before publication, and a published copy is
reported as `TooLate` until it is drained. Compute and copy can overlap only
when their allocation sets are disjoint. The separate XGMI facade retains a
successful exact-roster mapping for reuse across copies until host access or
allocation release requires an explicit unmap. For each direction it selects
submissions through a deterministic FIFO readiness queue, publishes at most 63 in one
native reservation and doorbell store, and retains every mapping through exact
completion. Polling a submission beyond the current batch may observe a
published ticket in front of it, so caller-driven observation cannot
indefinitely ignore the batch that must drain first. Neither poll nor wait
publishes deferred work. The additive in-process `flush_stream` operation
snapshots the complete dependency-ready directional set and publishes it in FIFO
prefixes of at most 63. It synchronously completes each non-final prefix before
publishing the next, then returns with the final prefix outstanding so subsequent
host work can overlap DMA. First-prefix allocation failure rejects before
mutation; a recoverable later-prefix failure is quiescent because prior prefixes
have completed and the remaining custody is retryable. Flush creates no
background thread. Frozen Runtime Worker V1 has no flush request; negotiated
Runtime Worker V4 and V5 expose the operational SPI profile with request-timeout
and caller-drain-deadline bounds.

The feature-gated gfx942 qualification lane is intentionally outside production
authority. It re-hashes and loader-validates one repository-owned COV6 object,
then a private KFD gate accepts only that artifact's fixed typed ABI,
metadata-declared effects, deterministic buffer images, and geometry. The gate
does not implement `KfdRuntimeLaunchAuthorityV1` and supplies no
compiler-lineage or Worker V3 authentication. Its deprecated HSA oracle relies
separately on the legacy backend's unsafe construction contract after admitting
the same fixture.

## Asynchronous Operations

Typed launches associate a Rust argument type with an application-supplied,
nonzero 32-byte signature. This creates a stable identity; it does not prove the
native kernarg ABI or the completeness of declared memory effects. Assurance
comes from the KFD launch authority. Deprecated HSA qualification separately
relies on that adapter's unsafe-construction contract. The argument value
produces an address-free kernarg image and
allocation-relative memory effects. Launch dependencies name exact events from
the same device. Submissions are nonblocking and may be polled or waited against
a monotonic deadline. `RuntimeCompletionStatusV1` is the central typed state:
`Pending`, `Succeeded`, a typed backend-code or cancellation failure, or
`QuiescentWithoutResult` when native references are gone without an observed
execution result. Submission and event queries read that same state; event poll
and wait update it through the same transition, so an event cannot disagree
with its source submission. Completion callbacks are removed before invocation
and run exactly once at the first conclusive transition across poll, wait,
event observation, cancel, drain, cleanup, or release. Callback panics are
contained and counted. Terminal backend ambiguity is not completion and does
not discharge callbacks.

An optional executor-neutral async engine owns one context on exactly one
thread and provides cloneable cross-thread handles plus standard event futures.
Command and waiter capacity, commands per tick, event polls per tick, and poll
interval are bounded. Polling uses a stable cyclic event-identity cursor;
dropping a future abandons observation without cancellation or release.
Command and executor-waker panics are contained, worker-thread reentry rejects,
and consuming shutdown returns the context while waking retained futures as
stopped. The original `spawn` mode remains background completion observation
only. Additive `spawn_with_progress` permits a bounded, independently rotating
set of move-only stream registrations; each tick may flush at most its declared
budget of registered streams that still have pending work. Retryable failures
are retained, terminal ambiguity seals the engine, and dropping a registration
does not cancel, release, or finally flush work. This is explicit host-driven
progress on the owner thread, not native queue-side scheduling or a hardware
liveness proof. Only Send-capable backends such as Worker V4/V5 can cross the
engine thread boundary; direct KFD owners remain deliberately thread-affine.

`query_stream` aggregates every retained submission by typed status and reports
the first failure deterministically by submission identity. `synchronize_stream`
waits once for each pending submission using one shared monotonic deadline and
returns the same aggregate observation; it may remain non-quiescent if the
deadline expires. Rejected and quiescent wait errors are remembered while later
pending submissions receive their one observation; terminal ambiguity stops
the operation immediately. These operations do not create independent native
streams: the current direct-KFD backend multiplexes logical streams over exactly
two native lanes, with caller-driven FIFO scheduling and at most one active
dispatch per lane.

`launch_atomic` and `launch_collective` match their contracts against the
argument type before admission. Compare-exchange binds its success order,
required failure order, and weak mode. A failure order cannot contain release
semantics or be stronger than its success order; non-CAS operations require no
failure order and `weak = false`. Collective participant count must equal the
selected workgroup or grid geometry. Every collective grid dimension must be
at least its workgroup dimension and divide exactly by it; partial tail
workgroups reject before submission. Atomic launches retain base geometry
validation and may use a partial final workgroup. System scope is rejected
because one stream does not identify a cross-device membership set. Both
wrappers require the stable and execution-detail backend capabilities and an
additive semantic submission SPI. KFD can advertise an exact profile only when
constructed with a separate unsafe semantic authority, and carries the
contract into final authorization. This is executable admission and custody,
not proof of kernel semantics, compiler preservation, firmware behavior, or
native hardware refinement. Ordinary KFD constructors advertise neither
capability. Runtime Worker V4 deliberately does not encode semantic launches;
the additive V5 profile preserves the exact validated contract across the
process boundary without creating native semantic or proof authority.

`RuntimeLaunchGeometryV1::grid` is the global work-item extent published in the
AQL grid-size fields. `workgroup` is the per-group extent. For COV6 implicit
arguments, each block count is `grid / workgroup` and the corresponding
remainder is `grid % workgroup`; resource admission still uses the ceiling
number of workgroups when accounting for a partial final group. The pure
`fe2o3-aql` geometry value derives these implicit dispatch values once. The KFD
adapter, protected KFD transition, and deprecated HSA qualification adapter
only encode that shared result into their owned kernarg storage.

Peer copies require two distinct peer-capable devices, an exact destination
stream, equal nonempty source/destination ranges, and explicit event
dependencies. Each copy retains a model peer-transfer contract identity. The
current KFD router returns before child allocation access. Poll and wait are
observation-only and do not issue deferred child range operations. The additive
in-process `flush_stream` operation advances the oldest dependency first, then
performs cooperative child range operations of at most 64 KiB. A child may first
reconcile allocation-wide native-dirty or copy-on-write state, so flush is
potentially blocking and the range size is not a strict host-work or latency
bound. Pending staging is capped at 1 GiB per router and released at conclusive
completion. Overlapping
copies require an exact dependency, live copies retain both allocations, and
ambiguous child failure terminally seals the router without releasing the
retained state. This is cooperative host progress, not asynchronous native peer
DMA. The executable Rust tests check these implementation contracts; the R7
Verus proof does not cover this router state machine.

The gfx942 SDMA API is backend-specific so the frozen Worker V1 protocol does
not silently change. Batch submit is nonblocking, tickets bind queue slot and
generation plus the non-reused queue occurrence, buffers remain owned by the
queue until exact completion is
observed. Split batch submit and wait each use one operational-currentness
envelope; the checked combined form uses one envelope around preparation,
publication, and observed completion. Queue-full and structural prepublication
validation failures return the move-only buffers after a successful currentness
check. Counter or generation divergence, currentness failure, and any
uncertainty after mutation terminally poison the session and retain native
custody.

The production striped submission operation creates one deterministic bounded
plan over an admitted even set of 2 through 16 queues and at most 1,008
requests. It prepares every per-queue shard before publishing any shard and
uses a production-checked publication mask. A later failure reports confirmed
published shards, an optional indeterminate failing shard, and untouched
indexed requests separately; there is no rollback or device-transaction
atomicity claim. Terminal results expose addressless audit observations only and
must remain retained until process teardown; only no-effect preflight returns
retryable requests. Selection advances only after complete publication plus
closing currentness. Preparation and publication failures execute the shared
production algorithms through safe callbacks. Closing-currentness failure uses
the shared cursor/state transition after a successful test publication, not the
outer live-session currentness/poison path. There is no live hardware fault
injection.

The R9 native XGMI variant additionally binds a generation-retained
directional type-11 link, same-hive endpoints, the exact XGMI engine inventory,
and the link's one-bit recommended engine mask. PUBLIC device-local allocation
owners map one canonical ascending two-GPU KFD array. Map/unmap interruption is
tracked as an absolute cumulative prefix: errno at a full map prefix remains
cleanup-only, while errno at a full unmap prefix quarantines without minting
free authority. Nonblocking queue tickets retain both mapping owners until an
exact completion is acquired. Full topology checks bracket lifecycle and batch
scopes; prospective reset and operational checks remain on the publication and
completion path. This API still requires process teardown after terminal native
ambiguity.

## Performance Rules

- `fe2o3-completion` transitions update only direct successors. No transition
  may rebuild the complete dependency graph.
- `fe2o3-runtime-model` production lifecycle state uses persistent path-copy
  AVL journals and key indexes with local inductive checks. Public slices are
  materialized lazily and cached. Full invariant scans are audit/debug
  operations, not hot transitions.
- Completion waits use deadlines and a bounded spin/backoff policy. Poll counts
  are not timeout units.
- Direct KFD logical-stream creation does not lease a native lane. Accepted
  compute work is retained in bounded per-stream FIFOs, with O(1) head/tail
  operations and O(stream depth) interior cancellation. The scheduler scans
  exactly two physical lanes. Implicit stream-tail and explicit dependency
  chains are capped at depth 256. Poll is observation-only; submit may publish
  immediately ready work; wait is observation-only; and explicit in-process
  stream flush may enter the potentially blocking dirty-buffer reconciliation
  and publication path. Progress is caller-driven. Frozen Runtime Worker V1 does
  not expose flush; negotiated Runtime Worker V4 and V5 do.
- SDMA batch submission and batch completion are linear in batch depth, bounded
  by 63 so one of the 64 physical ring slots remains empty. Currentness
  validation is constant per batch rather than per packet; packet construction
  is linear, while visible write-pointer publication and final doorbell
  notification are each one release operation per batch.
- Native XGMI facade batches are direction-local and FIFO readiness ordered,
  reuse exact-roster mappings after successful completion, and use the same
  63-ticket ring bound. Ready dequeue and publication selection are O(batch),
  with `batch <= 63`; focused in-flight selection is O(log batch), while
  completed-ticket removal is O(batch) within that fixed bound and does not
  inspect the ready backlog. Dependency
  wakeup is O(waiters for the completed dependency times the bounded 256-entry
  dependency roster). Prepublication cancellation may remove an arbitrary
  ready entry in O(ready), and allocation-overlap admission remains O(active).
  A poll focused beyond the published batch may observe its earliest published
  predecessor but does not publish deferred work. Explicit flush remains the
  publication mechanism and synchronously drains fixed-size prefixes when the
  entry snapshot exceeds one native batch.
- KFD device, VM, allocation, mapping, and queue lifecycle transitions use the
  full contracted topology/aperture currentness composite. Active mapped-memory
  and queue operations use the retained process, reset-event, descriptor, UAPI,
  XNACK, and DRM-loss operational fence. Packet atomics execute within explicit
  owner pre/post fence scopes rather than recursively rescanning host topology.
- `fe2o3-host` alias admission uses allocation-aware interval indexes. It must
  not scan all arguments of all in-flight launches for every new argument.
- The deprecated `fe2o3-hsa-runtime` qualification adapter indexes pending
  accesses by allocation, stream, and byte interval and carries sparse causal
  frontiers. Qualification admission must not scan every pending submission or
  walk transitive event ancestry.
- Worker request writes and response reads share one parent-process deadline. A
  dedicated writer owns child stdin so a worker that stops reading cannot block
  the runtime thread past that deadline. Worker-backed completion waits encode
  only a relative child duration and reserve response grace inside the caller's
  deadline; parent and child never assume a shared monotonic-clock epoch.

Scale benchmarks cover the maximum completion graph and large lifecycle
journals. Regressions in asymptotic behavior are release blockers.

The gfx942 runtime qualification runner compares only like-named measurement
scopes. KFD persistent execution, deprecated HSA host-visible qualification,
deprecated HIP staging oracles, synchronized launch/wait, and HIP device-event
intervals are reported separately. Results from unlike scopes must not be converted into parity
ratios; even the KFD/HSA/HIP synchronized rows retain different currentness,
allocation, signal, and readback policies.
The XGMI facade benchmark likewise emits separate `remap-per-round` and
`persistent-hot` rows. The persistent-hot row primes each direction once,
performs no host allocation access between timed repetitions, and validates
payloads and canaries only after the timed sequence. Mapping-reuse conclusions
must use that row rather than the remap row.

The native runtime profiler also samples process-local monotonic time after an
accepted AQL publication and after runtime completion processing. Samples
commit only with exact retained runtime events and are returned through an
opaque runtime-owned bundle. A fresh `getrandom` occurrence is part of every
clock identity, making accidental aliasing across reused caller scope bytes and
distinct `Instant` epochs cryptographically negligible. Empty captures report
no observed intervals. This provides host observation bounds, not GPU
start/end or device time.

The low-level KFD queue can sample GPU, CPU, and system counters with one
`GET_CLOCK_COUNTERS` observation bracketed by currentness checks. That sample
is a clock-domain calibration input only. It does not identify dispatch
publication, start, or completion and therefore is not a per-dispatch device
timestamp. Runtime profiling continues to report such timestamps unavailable.

## Deprecated Qualification Backend

The default `fe2o3-hsa-runtime` crate is an inert compatibility marker with no
ROCm link dependency. Its API is restored only by the explicit
`qualification-legacy-hsa-runtime` feature; `native-hsa` additionally enables
the native legacy implementation. That implementation requires a configured
ROCm development installation and fails the build when required headers or
libraries are absent. Neither feature creates a production runtime fallback.

## Verification Boundary

The R7 Verus file proves eight theorems over the abstract pool/lease/copy model:
in-flight and quarantined blocks are not reusable, completion-gated release
advances generations, stale leases cannot submit reused storage, distinct
retained blocks cannot alias, dependency frontiers gate publication, and peer
copies retain exact device coordinates. Expected-negative mutations demonstrate
that generation reuse and source-device execution invalidate named theorems.

The additive R8 Verus file proves ten theorems over a separate whole-resource
execution model: deferred reservation, dependency-gated exact publication,
conflict-free admitted overlap, aligned abstract fetch-add binding and
old-value return, unique collective membership, idempotent duplicate arrival,
and publication only after every named member arrives. Eleven R8 mutations
exercise eager and dependency-inverted publication, identity/generation/epoch
substitution, conflicting overlap, atomic alignment/coherence/return, and
early or duplicate collective arrival. The model has no byte ranges or
physical-alias relation and does not refine the executable Rust router.

The additive R9 Verus file proves fourteen abstract theorems over canonical
absolute mapping prefixes, successful compensation, exact directional route
currentness, native-copy custody, evidence equality, and publication gating.
Fifteen R9 mutations cover noncanonical/duplicate devices, prefix and
compensation errors, reversed or stale routes, inactive mappings, artifact and
receipt substitution, stale dispatch state, reset-fence loss, and premature
release after uncertain completion. The cumulative suite has 81 proved
obligations and 60 mutations. Errno-at-full-prefix behavior is Checked in the
Rust state machine; it is not a consequence of the successful-compensation
Verus theorem. There is no Rust-to-Verus refinement proof.

Executable Rust tests cover the pool model, router route-exhaustion preflight
and terminal latching, bounded dependency-chain progress, packet bytes, range
checks, queue admission, and custody transitions. The separate executable
kernel-semantic model checks the reviewed gfx942 integer-atomic and collective
roster against exact runtime resources and rejects misaligned or overlapping
atomic objects and invalid geometry; its coherence and convergence facts are
caller premises and it remains `ModelOnly`. The frozen SDMA manifest pins the
reviewed ROCr revision and packet sources. KFD ioctl results, MMIO, coherence,
compiler-to-code-object semantic correspondence, kernel and firmware behavior,
hardware completion, progress, liveness, and performance are not formally
proved. Native claims require a retained result artifact from an identified
MI300X run. The R8 copy qualification record names the exact measured commit,
devices, software stack, validation policy, load boundaries, raw output, and
limitations in
[`benchmarks/runtime_gfx942/results/async-copy-mi300x-2026-09-02.md`](../benchmarks/runtime_gfx942/results/async-copy-mi300x-2026-09-02.md).

R9 adds an authenticated machine-structure receipt for a closed gfx942 subset
of global/LDS integer RMW atomics and collective primitives. It binds the exact
payload, descriptor, entry, reachable instruction bytes, opcode classes, and a
loader-prepared dispatch. Its current collective roster covers exact LDS
read/write/permutation and workgroup-barrier spellings; every `_DPP` spelling
is rejected. The safe structure-required wrapper in
`fe2o3-runtime-machine-adapter` consumes the applied receipt, independent
Worker V3 authority, and a checked device before delegating to the sole
authorized runtime dispatch, and retains the structure in the successful
result. The receipt is Checked structural evidence, not a proof of instruction
semantics, ordering, scope, convergence, compiler preservation, or hardware
behavior, and it grants no load or launch authority. Worker V3 remains the
semantic and launch authority.
The exact claim matrix is in
[`crates/fe2o3-kfd/docs/r9-native-xgmi-machine-structure-v1.md`](../crates/fe2o3-kfd/docs/r9-native-xgmi-machine-structure-v1.md).

R10 adds a closed executable model for simultaneous compute/copy state,
dependencies, atomic batch publication, cancellation, quarantine, pool
generation reuse, peer ownership, the bounded integer atomic vocabulary, and
Wave64 barrier/reduction/scan collectives. Twenty additional Verus obligations
and eleven expected-negative mutations bring the pinned totals to 101 and 71.
Six deterministic public-runtime traces compare the executable facade against
the closed model. These are model and differential checks, not a Rust-to-Verus,
compiler-to-ISA, firmware, or hardware proof.

R11 adds executable abstract models for typed submission/event completion,
exact-once callback discharge, atomic and compare-exchange order/weak contract
matching, collective geometry/membership admission, and persistent-batch mapping
custody. Eighteen additional Verus obligations and eight expected-negative mutations bring the
pinned totals to 119 and 79. The proof covers the abstract model only: it makes
no Rust-to-Verus refinement claim and supplies no compiler, KFD, firmware, or
native execution authority.

R12 adds a bounded abstract multi-queue model for capability admission, queue
occurrences and slot generations, dependency-gated publication, terminal
observation, cancellation, release, currentness quarantine, drain, and queue
recreation. Twenty-three additional obligations and thirteen expected-negative
mutations bring the pinned totals to 142 and 92. R13 adds a bounded abstract
logical-stream scheduler with exactly two lanes, effective implicit and explicit
dependencies, FIFO publication, resource custody, lane-bound terminal events,
tail cancellation, dependent retention, and currentness quarantine. Twenty
additional obligations and eleven mutations bring the totals to 162 and 103.
R14 adds ten obligations and eight mutations for bounded event observation,
exact outcome preservation, abandonment/stop custody, and stable identity
ordering, bringing the totals to 172 and 111. R16 adds twenty-one obligations and
ten expected-negative mutations for reachable already-decoded Worker V5 states,
exact attempted/accepted/indeterminate custody, response sealing, and an ordered
exhaustive sidecar sequence join, bringing the pinned totals to 193 and 121. It
deliberately does not model byte parsing, serde, SHA-256, subprocess execution,
concrete backend invocation counts, Rust refinement, or native execution. These
tranches prove only their abstract finite models; none establishes a
Rust-to-Verus refinement, native behavior, progress, or performance.

The remaining community-launch blockers are material. Direct KFD owns exactly
two compute lanes per child, but has no background native-publication scheduler, queue-side
dependency packets, or more than two in-flight compute dispatches. Native XGMI is owned by a separate
exact two-device, copy-only backend; there is no unified native multi-device
compute owner. The host exposes a typed unsafe boundary and sealed adapter for
a separately reviewed producer of the required Worker V3 semantic-to-machine
refinement receipt, but the repository ships no concrete issue #214 proof
backend or authenticated scalar proof artifact. The Rust
device-language path includes a bounded volatile-load/store bridge rather than
broad Rust language support. Consequently, the runtime is not at HIP/HSA
parity.
