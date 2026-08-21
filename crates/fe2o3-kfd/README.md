# fe2o3-kfd

Owned syscall adapters for the direct-KFD fe2o3 runtime. The initial slice is
deliberately limited to opening `/dev/kfd`, querying its UAPI version, and
producing checked admission evidence for the exact reviewed schema in
`fe2o3-kfd-uapi`. The R1 topology slice additionally provides strict,
read-only discovery of the kernel-owned KFD sysfs tree for the initial `gfx942`
profile. It bounds every read, node/property count, and parsed field; rejects
symlinks, non-regular inputs, duplicate identities, malformed values, and
unknown property keys; and records topology generation plus filesystem and
platform provenance. Default-host discovery additionally records a strictly
parsed boot UUID, bounded kernel release, optional `amdgpu` module version
identities, and opaque KFD firmware-version observations, then correlates every
render minor to a kernel sysfs link below `/sys/devices`.
KFD unique ID, PCI domain/location, vendor/device ID, PCI revision, render
device number, and typed compute/memory partition observations are captured or
must agree. The initial admission layer can require the exported `SPX/NPS1` V1
partition constant without losing the observed values. The fixed
kernel-owned render and PCI symlinks are resolved deliberately; symlinks in
the KFD topology tree and regular-file inputs remain prohibited.

The public safe API does not expose file descriptors or raw ioctl arguments.
The R1 composition path consumes an explicitly selected unique ID and returns a
non-cloneable `CheckedGfx942XnackMinusDevice`. It retains `/dev/kfd` and the
exact correlated render descriptor plus a prospective KFD whole-GPU reset-event
descriptor, owns a process-global fe2o3 admission lease, requires KFD 1.18 and
AMDGPU DRM 3.64.0, compares the DRM identity prefix with topology/sysfs,
establishes a disabled-XNACK no-queue barrier, checks the complete bounded
process-aperture inventory, and repeats process, descriptor, topology, XNACK,
aperture, and reset-event observations before committing the token. The
`DEVICE_ADMISSION_PROFILE_MANIFEST_V1` digest binds the exact checked profile
and claim boundary. Retired model history is retained across admissions in the
same process and observation domain; a poisoned history fails closed. Each
successful admission also retains a solver-neutral `DeviceProjectionRecordV1`
covering platform, module-filesystem, and process provenance, both descriptors
and UAPI schemas, the selected topology/DRM profile fields, the explicit
bounded full-GPU identity inventory, firmware and selected capacity
observations, the initial wrapping VRAM-loss counter, the complete process
aperture inventory, and explicit reset-subscription, event-mask, `CLOEXEC`,
post-subscription DRM equality, and initial/final clear-fence facts. These are
contracted currentness observations, not an all-reset generation or proof.
Projection history is
updated atomically with identity history and links each admission generation to
its exact predecessor. R1 deliberately retains, rather than compacts, at most
`MAX_MODEL_DEVICE_ADMISSIONS_V1` admissions for the process lifetime. After the
first admission, any observation-domain change fails closed with
`ModelDomainChangedWithActiveHistory` or
`ModelDomainChangedWithRetainedHistory`; it never replaces retained history.
The sixty-fifth bind fails with `ProjectionHistoryExhausted`. Restarting the
process is the only supported way to create an empty history. This reviewed
availability bound avoids silently discarding substitution evidence. The
`kfd-device-identity` example performs this no-queue admission.

`check_observable_currentness(&mut self)` sandwiches a complete reobservation
between checks of the retained reset-event descriptor. Any event or error
permanently poisons later checks. It also compares the wrapping DRM
`VRAM_LOST_COUNTER`, but never treats that counter or KFD topology generation as
an all-reset generation. Under the pinned driver contract this detects
subscribed whole-GPU resets, VRAM-loss resets, and all changes visible through
the admitted identity, process, descriptor, XNACK, aperture, and topology
queries.

This crate checks userspace schema admission and encapsulates descriptor
ownership. Verus proves the pure canonical-record projection and abstract
generation/history relations. The executable validator checks the same record,
but there is not yet a Verus proof of the Rust implementation or a syscall-to-record
refinement proof, nor a
`ProductionDeviceAuthorityV1` implementation. No R1 API grants VM, allocation,
mapping, queue, event, code, or dispatch authority. It does not enumerate
cache, memory-bank, or link subtrees or prove their reported counts. The
process-global lease excludes other fe2o3 R1 admissions, not arbitrary raw KFD
users in the process. Ancestor traversal, mount-namespace integrity, sysfs
truth, cross-file snapshot semantics, KFD/DRM ioctl behavior, firmware meaning,
and absence of an ABA reset remain named external contracts. KFD does not expose
a sequence snapshot for the prospective subscription, does not report every
engine/per-queue reset through that stream, and creates its anonymous event fd
with an empty mask and without an atomic `CLOEXEC` option. A reset can therefore
occur between descriptor creation and mask enablement. The adapter sandwiches
that enablement between DRM identity/VRAM-counter observations, sets `CLOEXEC`
immediately, and never drains the complete event after detecting its first byte,
but a VRAM-preserving reset in the enablement gap can remain unobservable. It
also cannot close the concurrent
fork/exec inheritance window or exclude interference from arbitrary raw KFD
users in the process. A retained-device, nonwrapping counter incremented for
every reset class plus an atomic create/mask/CLOEXEC operation, or an atomic
generation-snapshot/event handshake, is required for an all-reset currentness
proof. Successful kernel responses and node metadata are checked or Contracted
observations, not proof of the kernel or hardware implementation.

The separate `scripts/runtime-identity-oracle.sh` hardware lane compares the
`kfd-device-identity --all` evidence with bounded output from an isolated
`/opt/rocm/bin/rocminfo` subprocess. A match is recorded only as `Measured` with
`authority=none`; oracle output is never passed to this crate and cannot create
device, VM, memory, queue, dispatch, or proof authority. The exact comparison,
evidence schema, CI separation, and limitations are documented in
`docs/runtime-identity-oracle-v1.md`.
The evidence marks contracted currentness and the VRAM-loss counter as
pure-Rust-only observations; neither is represented as an HSA differential
match.

## R2 host-visible memory slice

CheckedGfx942XnackMinusDevice::acquire_host_visible_memory_session consumes
the selected device and makes one irreversible ACQUIRE_VM attempt for the
process. A successful HostVisibleMemorySession owns the retained KFD/render
files and admits one ordinary, single-device, host-visible coherent GTT
allocation. The adapter rounds a nonzero requested length to a checked
4096-byte footprint, obtains a temporary anonymous address reservation, checks
the entire half-open interval against the selected process GPUVM aperture, and
passes that fixed GPU VA to ALLOC_MEMORY_OF_GPU. It rejects any mutation of
the input fields, zero handle/offset, unaligned offset, overflow, or profile
flag mismatch.

GPU VA, the opaque allocation handle, and the CPU VMA remain separate private
authorities. After successful ALLOC, the temporary address reservation is
unmapped. The BO is then mapped through the retained selected render file at a
kernel-selected CPU address with MAP_SHARED and PROT_NONE. MADV_DONTFORK must
succeed before mprotect enables read/write access and before any safe
closure-scoped byte borrow can be formed. Failed madvise or mprotect setup is
synchronously unmapped; failed cleanup is process-fatal rather than returning
an ambiguously inheritable VMA.

The mmap-to-DONTFORK step is not atomic. Absence of an external raw fork or
clone during that interval is Contracted; this API does not claim atomic
no-inheritance. Every borrow requires mutable session authority and checks the
opener PID and observable currentness before and after the closure. A reset
concurrent with the closure remains Contracted. CPU borrows are unavailable
while the BO is mapped to the GPU. Native KFD handles, GPU virtual addresses,
and descriptors remain private. Safe byte borrows cannot escape their closure,
but safe code can observe and retain a raw address derived from a slice;
dereferencing it outside the borrow requires unsafe code and is an external
contract.

MAP and UNMAP always use an immutable one-element `[selected_gpu_id]` array.
The returned n_success is cumulative and must satisfy old <= new <= 1.
Only ioctl success plus the full prefix commits a phase transition. An errno
with n_success == 1, malformed output, or a failed currentness check after
any mutation permanently quarantines the session. Cleanup requires successful
UNMAP, then CPU munmap, then exactly one FREE attempt. Any FREE error is
terminal because the pinned driver removes validation-list state before all
interruptible failure points. Drop performs no memory ioctl, munmap, FREE, or
retry; normal Rust ownership still closes the retained descriptors and invokes
driver process teardown.

HOST_VISIBLE_MEMORY_PROFILE_MANIFEST_V1 composes the frozen KFD memory schema
with the R1 device profile, active module digest, 4096-byte page profile, and
the reviewed transitive driver-source closure. The source-to-loaded-binary
relationship and kernel behavior remain Contracted. The completion-only model
journal records only fully committed adapter transitions. It is not a history
of every concrete syscall side effect: a
quarantined ALLOC, MAP, or UNMAP path can have unmodeled kernel effects. The
journal is model-only evidence, not production authority or a Verus/concrete
refinement proof.

For this one-allocation compatibility API, AQL, executable, kernarg, VRAM,
USERPTR, peer-device mapping, multiple allocations, retry, queues, and dispatch
are rejected or absent. The
default-feature `kfd-host-visible-memory-policy` example links and reaches the
complete production memory adapter without enabling process/fork support. CI
builds and ELF-audits that executable under the pure-Rust runtime policy so
dead-code elimination cannot hide the production syscall closure.

The `live-validation` feature is non-production only. It enables the
`kfd-host-visible-memory` example and a single-threaded fork/mincore negative
that verifies the DONTFORK VMA is absent in the child. The example always
launches the selected-GPU transaction in an isolated subprocess and creates no
queue or reset.

## R2 shared typed GTT memory slice

`CheckedGfx942XnackMinusDevice::acquire_shared_gtt_memory_session` is the
bounded multi-allocation successor to the one-BO foundation. It still performs
one irreversible process `ACQUIRE_VM` and retains the selected KFD and render
files, but one session can own at most 64 simultaneous allocations and at most
8 GiB of admitted GPU VA. Each allocation is represented by a non-cloneable,
redacted `SharedGttAllocationV1<Profile, State>` token. Tokens expose sizes and
the exact reviewed flag profile, never native handles, GPU VAs, CPU addresses,
or descriptors.

Four marker profiles are constructible: ordinary host-visible coherent GTT,
kernarg GTT, AQL queue GTT, and host-visible executable GTT. Ordinary requests
are checked and page-rounded. AQL requests must be a power of two from 4096
through 2 GiB. The active driver allocates one physical ring while reserving
and mapping two consecutive GPU copies, so the typed layout records distinct
logical/CPU bytes and a checked doubled GPU-VA span. The CPU VMA covers only
the physical copy. All four raw flag values are private constants supplied by
the typed profile; callers cannot introduce other bits.

Every live allocation retains its original anonymous `PROT_NONE` VMA as a GPU
VA guard. CPU BO mappings are separately kernel-selected. Guards prevent the
host VMA allocator from recycling an address that KFD still owns, and the
session independently checks half-open ranges for overlap. A guard is unmapped
only after CPU munmap and successful `FREE_MEMORY_OF_GPU`.

Every safe CPU view is closure-scoped and requires exclusive mutable access to
the session. This prevents simultaneous safe aliases across allocations as
well as escaping borrows. Mutable views exist only in `GttCpuWritableV1`.
Executable construction begins in that state and must pass
`seal_executable`, which changes the complete CPU VMA to read-only before the
token can enter the executable GPU-map path. This is CPU/VMA immutability, not
global content immutability: the reviewed executable UAPI profile includes GPU
write permission, and concurrent GPU writes remain outside this foundation.

Map/unmap/release consume and return typed state tokens. CPU access is absent
while GPU-mapped. A started native transaction, malformed output, range or
identity collision, partial/errno ambiguity, currentness loss, sealing error,
or destructive cleanup error quarantines the whole shared session. Previously
issued tokens then grant no further operation. Preflight size and capacity
rejections are failure-atomic and leave the session active. CPU munmap precedes
the one permitted `FREE_MEMORY_OF_GPU` attempt; Drop performs no ioctl, munmap,
FREE, or retry.

The crate-private queue bridge can consume mapped tokens into distinct,
non-Clone ring, control, EOP, context-save, completion-signal, dispatch-code,
and dispatch-kernarg role
capabilities. Each retains
the exact private GPU VA span, model mapping key, and proposed publication key;
validated subranges are computed with checked bounds and alignment. Numeric
addresses and bridge constructors are not public, and the bridge neither
publishes a mapping nor grants queue authority. The eventual native queue
adapter retains the four CREATE_QUEUE role capabilities plus the separate
completion arena and admits their exact fe2o3 backing policy before use.

The completion journal projects exact profile kinds, allocation generations,
non-overlapping GPU-VA spans, and successful map/unmap/release order into
`fe2o3-runtime-model`. It does not model the CPU VMA, AQL physical-versus-double
mapping relation, executable `mprotect`, or failed native side effects. There
is no Verus theorem connecting this unsafe Linux adapter to the abstract model.
Those relations, loaded-kernel behavior, reset exclusion during a borrow, and
the mmap-to-DONTFORK fork gap remain Contracted. The manifest and hostile tests
are evidence, not refinement proof.

The default-feature `kfd-shared-gtt-memory-policy` example links the complete
production closure for dependency and ELF auditing. The `live-validation`
`kfd-shared-gtt-memory` example runs all four profiles, AQL double mapping,
executable sealing, GPU map/unmap, CPU content checks, and explicit release in
an isolated subprocess. It additionally exercises exact 4096-byte EOP and
186019840-byte gfx942 context-save footprints through fe2o3's executable GTT
profile. This measures that bounded GTT policy only; it does not claim ROCr
backing equivalence or queue acceptance. This slice performs no queue ioctl,
doorbell mapping, packet publication, dispatch, wait, VRAM, USERPTR/SVM, or
peer mapping.

## C3 gfx942 device-memory leases

The shared KFD VM session can additionally own at most 64 writable
device-local VRAM/HBM allocation records and at most 192 GiB of retained
device memory. `Gfx942DeviceMemoryLeaseV1<State>` is non-Clone and exposes only
checked requested/backing size, alignment, and the exact `0x80000001` UAPI
profile. It retains the exact admitted device generation and VM identity
privately. The unmapped and mapped typestates are consumed by explicit
map/unmap/release methods. Even a mapped lease exposes no handle, descriptor,
pointer, or numeric GPU address.

Size rounding, capacity totals, power-of-two alignment through 4096 bytes,
aperture ends, range overlap, handle identity, and mmap-offset identity use
checked arithmetic and exact comparisons. Mapping targets only the selected
GPU; peer arrays are absent. Every native transition is surrounded by the
existing contracted device-currentness fence. Once an allocation ioctl has
been attempted, any errno, malformed output, partial progress, trailing
currentness failure, unmap ambiguity, FREE ambiguity, or VA-release ambiguity
quarantines the entire shared session. The reservation and every possible
handle remain retained, and Drop performs no cleanup or retry.

The ordinary queue path still rejects live device-memory leases. The private C5
dispatch path can transfer model ownership only when it consumes an exact,
complete, distinct set representing every live mapped C3 lease. That bridge
retains the real lease and keeps its address facts private. It does not turn an
initialization declaration into copy evidence or expose a numeric address.
The dedicated bounded lease journal is not projected into the runtime memory
model and has no Verus-to-Rust or syscall refinement. C3 itself still grants no
CPU mapping, initialization, sync or async copy, alias, quiescence, public
kernel launch, or hardware-completion authority.

## R4 queue-resource observations

plan_gfx942_aql_queue_resources turns one selected, correlated topology
observation into bounded resource geometry for the exact gfx942,
SPX/NPS1 topology profile. It checks every topology field used by the active
KFD/ROCr CWSR formula, the 4096-byte host page, a conservative
ROCr-compatible power-of-two ring range, exact EOP and context-save sizes,
counter mapping geometry, and non-MES doorbell geometry. The plan requires
read-only module-parameter observations mes=0, sched_policy=0, and
cwsr_enable=1; missing or changed values fail closed. Queue ID zero is
explicitly valid: the pinned KFD process queue manager allocates the first zero
bit from a zero-initialized 1024-slot bitmap.

The plan also names the exact reviewed ROCr 7.2.4 backing-policy expressions.
On the reviewed branches, ring and control produce fine-grained USERPTR
profiles, EOP produces executable coarse VRAM, and CWSR requests anonymous host
SVM attributes with a USERPTR fallback. The manifest pins the queue call sites,
runtime allocator dispatch, KFD driver flag translation, KMT allocation
translation, the header definitions of page and huge-page alignment, and
CWSR/EOP expressions needed to derive those values. This is an exact
expression set, not a transitive ROCr policy implementation closure or
evidence that an invocation selected a particular branch. These queue-resource
backing observations are not allocations accepted by the current fe2o3 queue
authority. USERPTR, SVM, executable coarse-VRAM resource binding, queue
creation through this planning API, doorbell mmap, and doorbell stores remain
unsupported. The generic writable device-memory lease has no queue-resource
binding or numeric-address export. The topology does not export CWSR sizes on
the admitted host, so
the plan uses and tests the exact pinned fallback formula. The read-only
kfd-queue-resources example validates the topology-derived facts on every
visible MI300X without opening /dev/kfd or creating a queue.
This topology-only result does not observe process XNACK mode. Its embedded R1
device-profile digest is only a compositional prerequisite identifier, not
evidence that R1 admission occurred. A future queue authority must pair the
plan with a live checked device token that establishes XNACK-disabled
admission and currentness.

## R4 native queue adapter foundation

The crate now contains a crate-private process-level CREATE/UPDATE/DISABLE/
DESTROY engine and narrow private Linux ioctl shims. Every lifecycle ioctl is
surrounded by opener-PID and contracted device-currentness checks, enters the
existing queue model's pending phase before the call, validates immutable and
output fields, and projects the observation before the trailing check. Linux
errno, malformed output, projection failure, process change, and currentness
loss fail closed. Queue ID zero remains valid; process-global unknown-create
poison and known-ID collision behavior come from the shared queue model.

The backend-specific resource authority is private and linearly retained by
the adapter through every phase that may have a native queue. Model
publications are returned only by an explicit non-syscall release after
confirmed DESTROY. Engine Drop performs no queue ioctl or retry. Scripted tests
cover success, every per-operation
failure/ambiguity class, hostile CREATE outputs, request mutation,
currentness/process loss, multi-queue collisions, global create poison, and
no-Drop-call behavior.

The first production composition consumes one checked gfx942:xnack- device and
creates a redacted, non-Clone queue session. It allocates one exact 4 KiB AQL
ring with the required doubled GPUVA, one exact 4 KiB control mapping with
distinct aligned write/read counters in the same page, a 4 KiB EOP mapping,
and the exact 0xb167000-byte CWSR mapping. EOP and CWSR use the separately named
fe2o3 executable-GTT policy; this is not ROCr policy equivalence. All four
linear role authorities and the shared model owner transfer into the queue
engine and remain there until confirmed direct DESTROY.

CREATE returns an admitted process-local queue ID, including zero, and the
adapter maps the exact complete 8192-byte KFD process doorbell slice. It checks
the encoded returned offset, installs MADV_DONTFORK before enabling the VMA,
and exposes neither an address, pointer, fd, handle, nor public MMIO store. The
private submission foundation initializes every ring header to exact INVALID
type 1 and the two control counters as atomics before GPU mapping. It uses the
canonical `fe2o3-aql` single-producer model, the actual acquire/read counters,
and an inert batch bound of one through 256 packets. One batch performs one
acquire-release write-pointer fetch-add by the full count, copies all INVALID
packet bodies before any aligned release header, publishes headers in packet
order, and performs one release-fenced x86-SFENCE volatile `u64` doorbell
store of the last packet ID. Counter divergence/regression and every possible
side-effect failure poison the non-Clone owner; only full or insufficient
space before the actual reservation is retryable. The private publication
path revalidates the live process-global runtime transition, event, all shadow
headers, payload, and currentness before publication. There is still no public
launch API. The C5 path below is the only private producer that can compose
code, kernarg, and data-allocation liveness.

The private completion slice owns one separate 16 KiB host-coherent GTT arena
containing exactly 256 distinct aligned `AmdBusyCompletionSignalV1` objects.
All are constructed as exact pending user signals before GPU mapping. A batch
of one through 256 packets receives one unique slot per packet; the private
binding retains the exact queue, signal allocation, code/kernarg mapping, and
dispatch generations without exposing a numeric signal address. The generation
keys detect substitution but do not themselves mint resource ownership,
initialization, alias, or copy authority.

Completion observation performs bounded atomic `i64` acquire loads. A batch is
ready only after every exact signal is zero; pending, unexpected-value fault,
timeout, currentness loss, and native observation ambiguity are distinct.
Fault, timeout, invalid poll bounds, generation exhaustion, ambiguity, or any
partial reset terminally poison the queue and require process teardown.
Completed slots can be recycled only by a checked
release reset to pending, after which their slot generations advance. Queue
destroy refuses any bound, published, or completed-but-unrecycled batch and
releases the completion arena only after confirmed queue destruction.

### C5 private dispatch binding

The private C5 constructor accepts one authenticated
`ValidatedKernelEnvelope` for exact gfx942 COV6 code, one through 256 complete
typed kernarg images, bounded dispatch geometry, and one through 16 device-data
allocation requests with role, valid-byte, initialization, and effect premises.
It validates all identities, sizes, alignments, geometry, pointer-field ranges,
and whole-allocation nonalias structure before native preparation. It then uses
the actual C3 API to allocate and map every device-data lease in the queue's VM.

The authenticated object is materialized exactly into one owned executable GTT
allocation, hashed after materialization, CPU-sealed, and GPU-mapped. The
selected kernel descriptor is resolved by checked subtraction from the loader's
image base and checked addition to the private mapped base. Kernargs occupy one
owned mapped arena with distinct aligned slices per packet. Device pointers are
inserted only inside a closure-scoped CPU initialization borrow; no numeric code,
kernarg, or device address is returned by safe public API.

The queue retains the real code allocation, kernarg arena, and every C3 lease
while C2 publishes the batch and C4 observes its unique per-packet signals. One
nonzero dispatch generation advances from prepared to in-flight to completed to
recycled in lockstep with C4. Ordinary pre-publication ring occupancy can cancel
the inert binding. Any generation divergence, currentness loss, publication or
observation ambiguity, timeout, fault, partial recycle, or teardown ambiguity
poisons the session and requires process teardown. Explicit release occurs only
after every signal was recycled and the queue was confirmed destroyed.

This is not a public safe launch API. No production copy/initialization bridge
can yet mint the data premises, and no generated implicit-kernarg producer is
connected. Per-segment GPU permission behavior for the uniformly mapped code
allocation, concrete effect/alias semantics, CPU/GPU coherence, firmware packet
execution, device-write visibility, and quiescence remain Contracted. The host
state machines and mock fault tests are not a concrete Verus or machine
refinement, and C5 performed no GPU workload.

Before event or queue creation, the composition takes a crate-global linear
owner and executes exact KFD RUNTIME_ENABLE mode 1 with zero debugger address,
capabilities, and TTMP-save. Success is required while the process has no user
queue. It then creates one first-internal-page auto-reset signal event, admits
only IDs 1 through 255, and replaces exactly the eight owned PROT_NONE CWSR
reservation pages at `base + xcc * 0x1621000` with private anonymous pages.
Each page transitions PROT_NONE, MADV_DONTFORK, then read/write. The same exact
40-byte header is read back from the executable-GTT BO and CPU shadow; every
header names the event and one aligned zero reason word in the first shadow.
No mapping address, pointer, fd, event handle, or MMIO capability is public.

Explicit cleanup first requires every completion batch recycled, then confirms
queue DESTROY, event DESTROY, runtime disable, doorbell unmap, and complete
CWSR/queue-resource/completion-arena unmap and FREE. The process-global
guard remains held through resource return. Drop performs none of those native
operations. The isolated `kfd-compute-aql-queue` example confirms this lifecycle
live on the selected MI300X while publishing zero packets and performing zero
MMIO stores. It also forks to confirm the doorbell and all eight shadow VMAs
are absent in the child. `kfd-compute-aql-queue-policy` links the default
production closure for the no-ROCm ELF audit.

The private bounded exception wait is one-shot and terminal. WAIT result and
volatile payload must agree, but timeout plus zero is only a racy observation,
not proof that no exception occurred. Timeout, an exception, disagreement,
unknown reason, reuse, or syscall uncertainty forbids later publication and
in-process cleanup; recovery is process teardown. Foreign KFD clients in the
same process remain outside the crate-global ownership claim.

This is queue-exception preparation, not actual fault-delivery evidence.
CPU-visible debug suspend and checkpoint/wave-state control-stack copies remain
unsupported because only the eight header pages are CPU shadows. Ordinary
hardware CWSR preemption and restore use the GPUVM BO and remain Contracted,
not excluded. Live kernel dispatch, hardware completion evidence, a production
data-copy and implicit-kernarg premise producer, and an injected-fault
observation remain separate gates.

The abstract Verus relation proves Active and Disabled are the only direct
destroy sources and that failed-no-effect restores the exact retained source.
It does not prove the Rust adapter, ioctl/mmap implementation, kernel, firmware,
hardware, CPU/GPU atomic coherence, or concrete-to-model refinement. The
private CPU publication and completion state machines are implemented and
hostile-tested. GPU signal writes, their system-scope coherence, acquire
visibility of device result writes, and dispatch quiescence remain Contracted;
live dispatch and hardware completion evidence remain excluded.

## Direct-KFD semantic observation boundary

`observe_kfd_live_queue_v1` converts the existing detached queue observation
and an optional detached device-binding observation into a fixed-size,
read-only `KfdSemanticObservationReportV1`. The adapter performs no I/O,
device enumeration, allocation, launch, wait, or runtime call. It is
kernel-agnostic: its observed facts describe the queue and its safe resource
geometry, not the behavior of a particular kernel. A matching detached
`ComputeAqlQueueDestroyedV1` can advance a live report to the destroyed
lifecycle.

The caller must supply a nonzero 32-byte observation scope. Domain-separated
SHA-256 identities commit every exact source field, including raw device,
queue, event, offset, and aperture observations, but the report exports only
scoped opaque evidence/device/queue identities. It never exports a raw queue or
event ID, GPU or CPU address, aperture, PCI location, descriptor, handle,
pointer, or MMIO capability. The scope is pseudonymization and correlation
input, not authentication or confidentiality; publishing or reusing it can
make low-entropy source facts guessable. The report itself does not retain or
return the scope.

The fixed capability matrix marks device binding, queue lifecycle, and safe
resource geometry as observed when supplied. Queue exception delivery,
dispatch submission and completion, dispatch timing, artifact/KIR binding,
workgroups, waves, lanes, memory accesses, and registers/values are explicitly
unavailable with typed reasons. In particular, confirmed queue, event, runtime,
doorbell, and allocation teardown is not evidence that a dispatch existed,
completed, or succeeded. The adapter therefore does not emit Semantic Trace V1
and does not depend on the semantic-trace crate. An authenticated dispatch plus
completion observation and exact artifact/KIR binding are required before that
adapter can be added without fabricating execution history.

`KFD_SEMANTIC_OBSERVATION_MANIFEST_V1` binds these inputs, output bounds,
redactions, availability claims, and authority exclusions. Reports and
capability queries are fixed-size and allocation-free. Hostile tests use only
crate-private detached-fact constructors; they perform no KFD, DRM, HIP, HSA,
or ROCm runtime discovery.
