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
descriptor, runs under a process-serialized admission transaction, requires KFD 1.18 and
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
admission transaction excludes concurrent fe2o3 R1 commits, not other live
checked physical-device tokens or arbitrary raw KFD users in the process.
Ancestor traversal, mount-namespace integrity, sysfs
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
queue or reset. Pass one decimal or `0x` unique ID, or pass `--all` to run one
isolated child for every topology GPU with the same requested byte count.

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

The queue liveness diagnostics have two crate-private ring profiles. One uses
plain executable coherent GTT with a one-times GPU-VA span. It is selectable
only by the consuming executable-ring barrier probe. The other registers one
anonymous DONTFORK `PRIVATE|NORESERVE` VMA at the same CPU/GPU address with the
exact executable, coherent, uncached, no-substitute USERPTR flags
(`0xd6000004`). It is selectable only by the USERPTR barrier probe. Reusable
queues and every dispatch API continue to require the special doubled AQL
profile. These probes isolate ring backing; the USERPTR path deliberately does
not claim full ROCr allocator or visible-GPU mapping-order equivalence.

Every ordinary live allocation retains its original anonymous `PROT_NONE` VMA
as a GPU VA guard. CPU BO mappings are separately kernel-selected. Guards prevent the
host VMA allocator from recycling an address that KFD still owns, and the
session independently checks half-open ranges for overlap. A guard is unmapped
only after CPU munmap and successful `FREE_MEMORY_OF_GPU`. The diagnostic
USERPTR profile instead makes the reserved pages accessible in place, unmaps
them from the selected GPU, frees the KFD BO while the VMA is still live, then
unmaps that same VMA without a second guard release.

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
While a native queue remains live, every detach, rebind, allocation,
initialization, or release mutation temporarily restores that same model
foundation to the shared session and reclaims the updated foundation before
returning to queue operation.
The dedicated bounded lease journal is not projected into the runtime memory
model and has no Verus-to-Rust or syscall refinement. Ordinary C3 leases still
grant no CPU mapping, initialization, sync or async copy, alias, quiescence,
public kernel launch, or hardware-completion authority.

The separate `Gfx942InitializedDeviceMemoryV1` path admits exact
`VRAM | PUBLIC | WRITABLE` flags (`0xa0000001`) without changing ordinary C3
leases. One entry point accepts an owned nonempty byte slice and a content
descriptor, checks their exact length and SHA-256 before allocation, copies the
complete requested extent, and hashes the mapped bytes again. The private
repeated-byte entry point instead precommits the exact nonempty extent, byte,
role, and SHA-256, then fills the complete safe mapping without a second HBM
scan. Large repeated-byte fills are partitioned into at most 16 disjoint safe
slices and written by bounded scoped CPU workers; smaller fills stay serial.
A worker spawn failure quarantines the retained session, and no partial fill
can mint initialized-content authority. Both paths map the returned BO offset
through the retained render file, use the same
`PROT_NONE -> MADV_DONTFORK -> read/write` setup as GTT, explicitly unmap the
CPU VMA, and only then map the allocation to the selected GPU. The result binds
private allocation/device/VM generations to the checked content descriptor and
exposes no native address or mutable view. A descriptor alone cannot construct
it. This is generic KFD/service allocation behavior; it does not encode a
model, inference engine, or workload-specific initialization policy.

The exact checked gfx942 device profile and successful public-VRAM mmap are the
only capability admission currently available. A driver or platform that
rejects the flag profile or CPU mapping fails closed after retaining the native
record in a quarantined session; it does not fall back to GTT or ordinary
uninitialized VRAM. CPU/GPU coherence and device-read visibility remain
contracted hardware behavior rather than proof claims.

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

The plan also names the reviewed ROCr 7.2.4 backing-policy values. On the
reviewed branches, the final ring ioctl is fine-grained USERPTR
(`0xd6000004`), control produces a source-local fine-grained USERPTR profile,
EOP produces executable coarse VRAM, and CWSR requests anonymous host
SVM attributes with a USERPTR fallback. The manifest pins the queue call sites,
runtime allocator dispatch, KFD driver flag translation, KMT allocation
translation, the header definitions of page and huge-page alignment, and
CWSR/EOP expressions needed to derive those values. The ring value includes
the FMM-added no-substitute bit; other values remain source-local expressions.
This is not a transitive ROCr policy implementation closure or
evidence that an invocation selected a particular branch. These queue-resource
backing observations do not grant allocation authority. General USERPTR/SVM,
executable coarse-VRAM resource binding, queue
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
ring with the required doubled GPUVA, one exact 4 KiB control mapping with the
AMD AQL write/read counters at `+0x38`/`+0x80` in distinct cache lines and the
`0x80` read-base-offset field at `+0x88`, a 4 KiB EOP mapping,
and the exact 0xb167000-byte CWSR mapping. EOP and CWSR use the separately named
fe2o3 executable-GTT policy; this is not ROCr policy equivalence. All four
linear role authorities and the shared model owner transfer into the queue
engine and remain there until confirmed direct DESTROY.

CREATE returns an admitted process-local queue ID, including zero, and the
adapter maps the exact complete 8192-byte KFD process doorbell slice. It checks
the encoded returned offset, installs MADV_DONTFORK before enabling the VMA,
and exposes neither an address, pointer, fd, handle, nor public MMIO store. The
internal submission foundation initializes every ring header to exact INVALID
type 1 and the reviewed AMD AQL control prefix before GPU mapping. It uses the
canonical `fe2o3-aql` single-producer model, the actual acquire/read counters,
and the additive V2 fixed-batch bound of one through 8192 packets. A maximum
batch requires a ring of at least 512 KiB. One batch performs one
acquire-release write-pointer fetch-add by the full count, copies all INVALID
packet bodies before any aligned release header, publishes headers in packet
order, and performs one release-fenced x86-SFENCE volatile `u64` doorbell
store of the last packet ID. Counter divergence/regression and every possible
side-effect failure poison the non-Clone owner; only full or insufficient
space before the actual reservation is retryable. The publication
path revalidates the live process-global runtime transition, event, all shadow
headers, payload, and currentness before publication. Public submission is
reachable only through the addressless fixed-dispatch custody path below.

The private completion slice owns one separate 512 KiB host-coherent GTT arena
containing exactly 8192 distinct aligned `AmdBusyCompletionSignalV1` objects.
The large fixed-cardinality packet and retention arrays are heap-owned. All
signals are constructed as exact pending user signals before GPU mapping. A
batch of one through 8192 packets receives one unique slot per packet; the
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

### One-shot queue liveness probe

`Gfx942BarrierProbePollBoundV1::new` validates a nonzero bounded poll count
without consuming device authority. The public consuming
`CheckedGfx942XnackMinusDevice::run_compute_aql_barrier_probe` operation then
creates an exact fresh queue with no dispatch resources, leases one completion
signal, publishes one zero-dependency system-scoped BARRIER_AND packet, and
returns redacted success evidence only after exact completion validation,
release-reset of that signal, and confirmed queue destruction.

`run_compute_aql_executable_ring_barrier_probe` performs the same consuming
operation with a crate-private plain executable GTT ring whose CPU and GPU spans
both equal the logical ring size. The ordinary probe, reusable queue creation,
and all dispatch entry points retain the doubled special-AQL backing. The
alternate probe therefore changes only the ring flags and mapping span.
Every success, failure, and quarantined-custody observation records the selected
backing, and the backing plus exact logical/GPU span is bound into queue plan
and configuration identity.

`run_compute_aql_userptr_ring_barrier_probe` performs the same operation with
the exact `0xd6000004` one-times USERPTR diagnostic profile. The shared VMA is
created with FE's DONTFORK and NORESERVE safety policy, initialized before its
selected-GPU map, and freed with the USERPTR-specific BO-before-VMA order. This
is the smallest backing-only discriminator, not full ROCr allocation or
all-visible-GPU mapping parity. Any failure after entering inner USERPTR queue
creation is `TerminalCreation` because a VMA or KFD registration may remain
under process custody.

The probe binds only queue and signal generations; it does not invent code,
kernarg, or dispatch generations. Submission cancellation is permitted only
for a stage-classified full or insufficient-space result before any native
side effect. Execution failures return opaque quarantined queue custody that
must remain until process teardown. A `TerminalTeardown` failure recovers no
authority: native resource disposition is indeterminate, process termination
is required, and retry, reopen, or confirmed-cleanup claims are prohibited.
Any `CREATE_QUEUE` result not explicitly reported as failed with no effect, and
every fallible boundary after successful creation, is `TerminalCreation`: no
authority is recovered and process termination is required.
The operation arms a process-global KFD runtime-gate poison before beginning
destroy and clears it only after end-to-end confirmed success. An error or
panic retains the poison, including failures after the native mutex owner
would otherwise have been released, so another thread cannot reopen a queue in
the teardown window.
No safe result exposes a GPU address, signal address, descriptor, doorbell, or
MMIO authority.

### Addressless fixed dispatch binding

`SharedGttMemorySessionV1::create_compute_aql_queue_with_fixed_dispatch`
consumes the exact existing checked device/VM session, one through 32
authenticated `ValidatedKernelEnvelope` values, one through 8192 complete
packet descriptions, and one through 16 existing mapped device-local or
host-visible coherent data authorities.
Packet descriptions contain program indices, checked geometry, scalar kernarg
bytes, zero device-pointer fields, and bounded allocation subranges. They
contain no native address or caller-supplied effect. Pointer offsets,
alignments, and read/write effects come only from inspected kernel metadata.
Read and read/write arguments require sealed full-extent initialization;
write-only arguments may consume uninitialized exclusive storage. Coherent GTT
can obtain that sealed state only from an owned whole-extent copy before map;
the arbitrary scoped-write API does not mint it.

For kernels with hidden metadata, construction requires one exact trailing
256-byte COV6 implicit suffix and requires every caller byte in that suffix to
be zero. The retained owner privately initializes only metadata-declared block
counts, group sizes, partial-group remainders, zero global offsets, grid
dimensions, and dynamic LDS size before mapping the kernarg arena. Block counts
are `grid / workgroup`; remainders are `grid % workgroup`. Inactive dimensions
remain count one, group size one, and remainder zero. A kernel declared with
uniform workgroups rejects any nonzero remainder. Queue pointers and every
runtime-service or address field, including printf, hostcall, heap, default
queue, completion action, multigrid, private base, and shared base, are rejected
before native allocation.

Construction also rejects missing or duplicate global-buffer bindings, nonzero
pointer fields, range or alignment drift, intra-packet aliases, incomplete live
lease sets, and read access without initialization before queue publication. It
does not infer how many bytes a kernel actually accesses from a caller subrange.

Each authenticated object is materialized exactly into an owned executable GTT
allocation, hashed after materialization, CPU-sealed, and GPU-mapped. Each
selected kernel descriptor is resolved by checked subtraction from the loader's
image base and checked addition to its private mapped base. Kernargs occupy one
owned mapped arena with distinct aligned slices per packet. Device pointers are
inserted only inside a closure-scoped CPU initialization borrow; no numeric code,
kernarg, or device address is returned by safe public API.

The queue retains every code allocation, the kernarg arena, and every data lease
while it publishes the batch and observes its unique per-packet signals. One
nonzero dispatch generation advances from prepared to in-flight to completed to
recycled in lockstep with C4. Ordinary pre-publication ring occupancy can cancel
the inert binding. Any generation divergence, currentness loss, publication or
observation ambiguity, timeout, fault, partial recycle, or teardown ambiguity
poisons the session and requires process teardown. Explicit release occurs only
after every signal was recycled. A recycled-only detach releases code and
kernarg while keeping the same native queue, ring, completion arena, event,
runtime, and doorbell alive. Its exact detached-lease ledger must be consumed by
a later `bind_fixed_dispatch` or explicit release. The later batch may have a
different program count, packet count, geometry, scalar bytes, and dispatch-data
set. Live memory mutations run only while the authoritative memory model is
restored to the shared session; the queue engine reclaims it before any later
lifecycle operation. The later batch is still published by one reservation and
one final doorbell store.

Storage that entered fully initialized remains fully initialized across generic
completion and can be rebound without another upload. Exact pre-publication
content descriptors are not returned as current after device publication.
Storage admitted uninitialized under inspected write-only access remains
uninitialized until a separate exact full-coverage effect join exists.

While the same batch remains attached, the completed-and-recycled generation
can copy an owned byte range from host-visible coherent data. A request binds
the exact dispatch generation and data ordinal and must be contained in exactly
one global-buffer binding whose inspected actual access is write-only or
read-write. Device-local, read-only, unwritten, out-of-range, multiply
intersecting, stale-generation, pre-completion, and pre-recycle requests fail.
The CPU mapping never escapes, no GPU address is returned, and copying bytes
does not promote partial writes to full-allocation initialization. Reusing or
rebinding the queue advances the dispatch generation and stales old requests.

Return is all-or-terminal. Once queue destruction is confirmed, any later
event, runtime, doorbell, CWSR, queue-resource, code, kernarg, completion-arena,
or model-restoration failure yields no recoverable returned state. The consumed
session and its no-effect drops retain any possibly live native resources for
process teardown; there is no partial in-process cleanup or retry.

There is no initialization boolean or caller-supplied read premise. Implicit
fields outside the exact geometry/dynamic-LDS subset remain unsupported.
Per-segment GPU permission behavior for the uniformly mapped code allocation,
concrete effect/alias semantics, CPU/GPU coherence, firmware packet execution,
acquire-observed device-write visibility, and quiescence remain Contracted. The host
state machines and mock fault tests are not a concrete Verus or machine
refinement; the public custody path alone is not hardware execution evidence.

### C6 unbacked device-content copy foundation

The private C6 foundation binds one exact mapped GTT source publication to one
exact mapped C3 destination generation, device and VM, requested byte extent,
semantic role and ordinal, content SHA-256 and canonical content identity. A
copy intent additionally binds nonzero operation and publication identities,
the exact queue generation, and one dispatch generation. All substitutions and
size failures retain both inputs before any possible side effect.

After publication begins, packet-body, release-header, doorbell, completion
observation, and signal-recycle failures are terminal and retain both resources
for process teardown. Only an explicitly no-effect failure is retryable. The
initialized-content state is reachable only through an opaque authenticated
copy-completion token that binds the exact copy, queue, dispatch, completion
batch, signal slot and generation, and last packet. Production deliberately has
no constructor for that token, so neither public data, an internal descriptor,
nor a boolean can mint initialized memory.

This remains a prerequisite content-authentication state machine, not the
device-copy implementation described below. The queue can return its real
mapped C3 set only after exact C4 recycle and confirmed destruction, but that
return path is deliberately not connected to the content state machine. The R7
SDMA engine moves bytes and observes its own completion generation; it does not
authenticate source semantics or construct the opaque C6 initialized-content
token. A later composition must join exact content identity with exact copy
publication and completion. The independent public-VRAM CPU initialization path
also does not authenticate a device copy.

### R7 gfx942 SDMA and pooled buffers

An active compute session may add one generic classic KFD SDMA queue, an exact
two-queue directional profile, or a balanced striped set with any even count
from 2 through 16 targeted queues. Every queue has a 4096-byte ring and at most 63 in-flight
64-byte submissions. Directional and striped admission requires a fresh,
generation-consistent observation of exactly two ordinary SDMA engines and
eight queues per engine before and after targeted queue creation and
destruction. The striped profile alternates engine indices 0 and 1, rejects
duplicate native queue IDs, selects one queue round-robin per successfully
published batch, and does not advance its selection on rejection. KFD engine
index 1 carries directional H2D and index 0 carries directional D2H; these are
indices, not the public HSA engine bit masks. Each submission contains one
reviewed gfx942 linear-copy packet and one system-memory completion fence.
Move-only host-coherent and HBM buffers retain the exact queue owner and a
concrete pool generation, and remain in queue custody until the exact slot
generation is observed. Copy tickets bind that same non-reused queue occurrence
in addition to the native queue ID, ring slot, and slot generation. Cross-queue
recycle and submission are rejected before mutation. Nonblocking submit, poll,
deadline wait, and batch forms are public. A batch writes complete packet
images while the visible write pointer is unchanged, then performs one release
write-pointer publication and one final release doorbell. The split API uses one
operational-currentness envelope for submission and another for waiting; the
checked combined API uses one envelope from preparation through observed
completion.

Polling and queue-progress observations are nonblocking. Progress binds the
native queue occurrence, submitted/completed/pending counts, and ring byte
counters to one host `Instant`; that is host-monotonic observation time, not a
calibrated GPU timestamp. Deadline waits use bounded adaptive
spin/yield/sleep backoff. The admitted SDMA fence protocol does not create a
KFD completion event: the queue event is an exception signal, so completion
remains coherent-memory polling. Published packets cannot be cancelled or
retracted safely through this KFD interface. Cancellation validates custody
and returns a typed unsupported result; callers must poll or explicitly drain
the retained ticket batch.

The session also owns a best-fit pool keyed by buffer kind, physical bytes, and
alignment. Recycling is explicit, is possible only after buffer authority has
returned from completion, and advances the concrete generation before reuse.
Pool trim releases every free allocation before queue teardown. Outstanding or
pooled buffers block destruction. SDMA queue
destruction precedes compute queue destruction, followed by explicit unmap and
release of the copy ring, control page, and completion arena. A partial
directional create or destroy failure returns no retryable state: native and
mapped custody is terminally retained for process teardown.

An exact full coherent upload buffer can be moved into fixed-dispatch data only
after its complete physical extent matches a content descriptor. An exact full
H2D completion can similarly move its device destination without a second
allocation or copy, but only after the complete coherent source matches the
descriptor. The move-only bridge binds queue occurrence, storage identity,
extent, and pool generation; demotion checks those coordinates and advances
the pool generation before returning SDMA custody. Partial copies, pooled
logical/physical extent differences, foreign owners, digest substitution, and
detached-ledger substitution fail closed. This is a custody-preserving role
transition, not a proof of DMA execution or coherence.

The fixed AQL batch supports multiple independent dispatch packets on one
compute queue. This work does not create multiple compute queues inside one
checked-device owner: the device, VM, and shared-memory session are linear and
splitting that authority requires a separate native-owner transition. Striped
queues apply only to SDMA work.

The copy benchmark's `stripedN` profiles are an explicit concurrency
diagnostic over that queue set. Each direction is partitioned into
`min(N, depth)` balanced batches, every batch is published before the first
wait, and each successful batch advances to the next native queue. Submission
and wait retain their existing per-batch currentness envelopes. This is not an
all-or-nothing multi-queue publication transaction: a later shard failure after
an earlier publication is a terminal benchmark failure, and no production
atomic-batch claim follows from the measurement.

The separate production
`submit_gfx942_striped_sdma_copy_batch_v1` operation accepts at most 1,008
requests over exactly 2 through 16 admitted striped queues. It deterministically
balances no more than 63 requests per queue, prepares every shard before the
first visible publication, and commits its round-robin cursor only after every
shard publishes and closing currentness succeeds. A no-effect preflight failure
returns the intact requests for retry. Any terminal failure instead returns
move-only, observation-only custody that must be retained until process teardown:
all request count/ordinals before publication; confirmed published shards, at
most one indeterminate failing shard, and untouched request ordinals after
partial publication; or all confirmed shards after a closing failure. It exposes
no drain, ticket, buffer, or resubmission authority. Earlier publications cannot
be rolled back, so this is a bounded prepare-all/typed-partial-outcome boundary,
not an atomic device transaction. Safe fault-injection tests exercise the same
production preparation/publication algorithms. Closing-currentness injection
exercises the shared post-publication cursor/state transition, not the outer
live-session currentness/poison path. These tests do not claim live native fault
injection or firmware/coherence proof.

`GFX942_SDMA_COPY_MANIFEST_V1` pins the additive KFD SDMA schema and topology
capability sidecar, reviewed ROCr revision, direction policy and packet sources,
packet bytes, bounds, currentness, failure, and teardown contracts. Verus proves
only the separate abstract R7 lease-generation,
retention, quarantine, dependency, and device-coordinate theorems. It does not
refine this Rust code. Ioctl truth, doorbell MMIO, CPU/GPU coherence, firmware
consumption, completion, liveness, and performance remain contracted or
measured.

The hardware benchmarks print stable key/value records containing queue depth,
batch size, direction, concurrency, doorbells per batch, and p50/p95 latency.
Representative commands are:

```text
cargo run -p fe2o3-kfd --example kfd-sdma-copy-benchmark -- <gpu> <bytes> <depth> <warmups> <samples> directional
cargo run -p fe2o3-kfd --example kfd-sdma-copy-benchmark -- <gpu> <bytes> <depth> <warmups> <samples> striped8
cargo run -p fe2o3-kfd --example kfd-sdma-multi-device-benchmark -- <gpu0> <gpu1> <bytes> <depth-per-device> <warmups> <samples>
cargo run -p fe2o3-kfd --example kfd-sdma-xgmi-peer-benchmark -- <gpu0> <gpu1> <bytes> <depth> <warmups> <samples>
```

The ordinary and XGMI publication changes are packet-model and fake-MMIO
tested. Throughput, completion latency, copy-engine overlap, and cross-GPU
behavior remain hardware-unverified until these commands are run on the named
gfx942 topology and compared with equivalent HIP/HSA workloads.

### R9 native XGMI and machine-structure admission

The [R9 boundary](docs/r9-native-xgmi-machine-structure-v1.md) defines a
bounded low-level native XGMI path. Topology discovery retains exact
directional link records. Route admission binds one enabled type-11 XGMI
`io_links` edge, same-hive gfx942 endpoints, nonzero bandwidth, the exact
2+14-engine inventory, and one topology-recommended XGMI engine. PUBLIC HBM
owners map an exact canonical two-GPU array with cumulative-prefix recovery;
an errored full map is cleanup-only and an errored full unmap quarantines
without granting free authority. A BY_ENG_ID XGMI SDMA queue exposes
nonblocking bounded submission and exact completion custody in both
directions. A bounded batch performs all fallible packet construction and
retained-mapping allocation before native mutation, writes every packet while
the visible write pointer is unchanged, then performs one release write-pointer
publication and one final doorbell store under one topology-currentness
envelope. Nonblocking poll, progress observation, bounded adaptive wait, typed
cancellation rejection, and explicit batch drain mirror the ordinary SDMA
surface. Forward and reverse route queues use the topology-recommended XGMI
engine for their exact direction. The current safe API holds both device
sessions mutably for one route queue, so the bidirectional benchmark runs
forward then reverse and reports `concurrency=1`; simultaneous opposite-route
striping requires a future split peer-session authority.

The authenticated LLVM/MC analyzer separately classifies a closed set of
global/LDS integer RMW atomic instructions and collective building blocks. Its
move-only receipt binds exact payload, descriptor, entry, reachable machine
sites, encodings, widths, and memory classes, then matches those coordinates
to a loader-prepared dispatch. The reviewed collective structure roster covers
exact LDS read/write/permutation and workgroup-barrier spellings; all `_DPP`
spellings are rejected. The safe structure-required wrapper lives in the
integration-only `fe2o3-runtime-machine-adapter` crate. It consumes the applied
receipt, independent Worker V3 authority, and a checked device, delegates to
the sole authorized dispatch transition, and returns the retained structure
with the normal result. This is Checked machine-structure evidence.
It does not prove opcode semantics, ordering, scope, convergence, compiler
refinement, coherence, or hardware behavior and itself grants no load or
launch authority. KFD atomics and collectives remain code-object behavior
rather than standalone ioctls. Worker V3 remains responsible for exact
semantic and launch authority; concrete device, queue, publication, and
completion custody remains with the native owner.

Before event or queue creation, the composition takes a crate-global linear
owner and executes exact KFD RUNTIME_ENABLE mode 1 with zero debugger address,
capabilities, and TTMP-save. Success is required while the process has no user
queue. It then creates one first-internal-page auto-reset signal event, admits
only IDs 1 through 255, and replaces exactly the first three owned PROT_NONE
CWSR reservation pages at each `base + xcc * 0x1621000` stride with 24 private
anonymous control-stack pages. Each page transitions PROT_NONE,
MADV_DONTFORK, then read/write. A separate private DONTFORK page holds the
aligned zero event-reason word so a non-empty control-stack copy cannot
overwrite event state. The same exact 40-byte header is read back from the
executable-GTT BO and the first CPU shadow page for each XCC.
No mapping address, pointer, fd, event handle, or MMIO capability is public.
An armed unpublished custody guard zeroes and unmaps the separate payload page
on every failure after installation and until immediately before the native
`CREATE_QUEUE` call. The guard disarms at that exact boundary because an
ambiguous native result may have published the header payload pointer and then
requires process teardown.

Explicit cleanup first requires every completion batch recycled, then confirms
queue DESTROY and event DESTROY, immediately zeroes, protects, and unmaps the
separate event-payload page, then performs runtime disable, doorbell unmap, and
complete CWSR/queue-resource/completion-arena unmap and FREE. The process-global
guard remains held through resource return. Published-owner `Drop` performs
none of those native operations; only the armed, unpublished payload guard has
the explicit zero/protect/unmap behavior above. The isolated
`kfd-compute-aql-queue` example confirms this lifecycle live on the selected
MI300X while publishing zero packets and performing zero MMIO stores. It also
forks to confirm the doorbell, all 24 shadow VMAs, and the separate event-payload
VMA are absent in the child. It accepts one unique ID or
`--all`; the latter uses a
separate process and queue lifecycle for every topology GPU.

Unpublished install failure also explicitly zeroes and unmaps the payload page.
Because published payload cleanup occurs at the event-destroy boundary, a later
unrelated runtime, doorbell, BO, callback, or resource-release failure cannot
strand that standalone writable mapping. Once event destruction succeeds, any
payload zero, protection, or unmap failure aborts the process; the consuming
transition never returns while its only mapping owner may remain live.

A debugger-enabled target can consume `KfdTargetRuntimeDebugTokenV1` into the
same native queue session. The token's admitted control descriptor remains
private while the checked device retains its independent VM/queue descriptor;
both are process-bound and neither is exported. Runtime authority leaves the
token before event or queue lifecycle mutation, so no later failure can run the
token's no-queue disable path. `KfdTargetRuntimeDebugQueueV1::destroy` returns a
linear teardown owner that disables the runtime only after confirmed queue and
event destruction. The unsafe one-shot debug-target dispatch entry point reuses
the existing request preparation, queue, AQL submission, completion, and
resource teardown transaction. Its safety and process-termination obligations
are identical to the ordinary unsafe dispatch entry point; it is not a second
queue or launch implementation.

The serial live validation ptrace-attaches a child, acknowledges target runtime
enable, observes the real 4 KiB KFD queue through the bounded debugger snapshot,
observes its removal after queue/event destruction, acknowledges runtime
disable, and completes bounded detach/reap of that direct child leader. The
test also suspends the idle queue and captures a complete zero-byte opaque
checkpoint before explicitly resuming it. It does not establish descendant
containment or a non-empty hardware checkpoint. It does not qualify decoded
wave/register state, source debugging, target-memory access, kernel execution
under the debugger, timing, or performance.

The cooperative target-telemetry channel is a separate authority-free
observation aid for `fe2o3-debug live-kfd`. It uses a private Unix
`SOCK_SEQPACKET` pair with fixed 256-byte, versioned, checksummed records,
kernel sender credentials, a per-session nonce, strict sequence/lifecycle
validation, and a 4,096-record consumer bound. The target endpoint is send-only
and records contain logical digests, lengths, geometry, access classes, and
phases only. They contain no native address, pointer, PID, descriptor, handle,
or path and do not prove load, dispatch, allocation, execution, or faults.

`admit_inherited_kfd_target_debug_telemetry_v1` validates the complete inherited
environment ABI, duplicates the endpoint with `CLOEXEC`, binds it to the live
debugger process instance, and protects the original inherited descriptor
before returning. Partial or noncanonical input fails closed. Child-isolated
tests cover malformed environments, invalid and wrong-type descriptors, wrong
peers, the positive handoff, credential binding, hostile ancillary data, and
record lifecycle substitution.

V2 is a distinct fixed 384-byte contract for ROCgdb correlation and leaves V1
byte-exact. Its declaration is followed by at most one
`NativeDispatchPublished` record emitted inside the direct-KFD queue owner at
the real post-AQL-publication boundary. The record binds a process instance,
queue occurrence, dispatch generation, artifact, geometry, selected KFD GPU,
actual queue ID, and actual packet ID. Those native observations remain private
correlation inputs. Failed or cancelled terminals are legal both before and
after publication; a completed terminal is legal only after publication and is
owned by the safe runtime after its final validation gates.

`kfd-compute-aql-queue-policy` links the default
production closure for the no-ROCm ELF audit.

The private bounded exception wait is one-shot and terminal. WAIT result and
volatile payload must agree, but timeout plus zero is only a racy observation,
not proof that no exception occurred. Timeout, an exception, disagreement,
unknown reason, reuse, or syscall uncertainty forbids later publication and
in-process cleanup; recovery is process teardown. Foreign KFD clients in the
same process remain outside the crate-global ownership claim.

This is queue-exception preparation, not actual fault-delivery evidence. The
direct-KFD stopped-state API can retain the KFD-header-bounded control-stack
and wave-state bytes as a private, zeroizing opaque checkpoint after exact
session-owned suspension. It double-reads every non-empty segment, rereads all
headers, binds the checkpoint correlation identity to local admitted session
state plus exact queue/device/header/content identities, and returns explicit
no-prefix truncation at a caller-selected bounded limit. The direct KFD
queue/device snapshots are reobserved before and after, but runtime and
suspension are only local session invariants. Segment pairs are sequential and
non-atomic; complete means every announced extent was captured, not one
coherent stopped instant or authenticated hardware provenance.
Neither the Linux KFD UAPI nor installed public headers specify the inner
gfx942 record layout, so wave, lane, register, PC, and source values remain
typed unavailable. Ordinary hardware CWSR preemption and restore remain
Contracted. Non-empty live checkpoint qualification, live kernel dispatch,
hardware completion evidence, a production data-copy and implicit-kernarg
premise producer, and an injected-fault observation remain separate gates.

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

`KFD_SEMANTIC_OBSERVATION_MANIFEST_V1` binds the source-profile identities,
inputs, output bounds, redactions, availability claims, and authority
exclusions. Reports and capability queries are fixed-size and allocation-free.
Hostile tests use only crate-private detached-fact constructors; they perform
no KFD, DRM, HIP, HSA, or ROCm runtime discovery.

## R17 persistent device-allocation custody core

`Gfx942PersistentDeviceAllocationV1` is an additive, addressless lifecycle
core around exactly one existing mapped device-local allocation. It accepts
either the ordinary single-device mapped lease or an already complete,
canonical two-device peer mapping. The non-cloneable owner and every use lease
are thread-affine. They expose no GPU address, native allocation handle,
descriptor, pointer, or mapping identity. Internal `Rc` identity is used only
to bind move-only custody to one owner incarnation; there is no interior
locking or externally cloneable allocation authority.

The owner retains the existing private allocation generation, selected device,
VM, and mapped-state identity for exact substitution checks. Each concrete
owner also has a private, non-resettable incarnation identity carried by every
use lease and dependency frontier, so demotion and re-promotion of the same
native allocation cannot make old custody current again. A fixed 64-slot ledger
admits Compute, LocalSdma, and peer-mapped source/destination
classifications over checked nonempty allocation subranges. Operation selects
the access class; callers cannot provide a contradictory access value.
Overlapping reads can coexist and disjoint uses can coexist. Any active
overlap involving a writer is rejected. A later overlapping use involving a
writer must name the exact current host-confirmed successful frontier for the
same native binding. Reservations then move through `Reserved`, `Prepared`,
`Published`, `Completed`, and `Settled`; reserved and prepared uses can be
cancelled, while a timeout returns the exact published custody unchanged.
Settlement is ordered by reservation sequence so one frontier cannot skip an
earlier live use. Retained settled history is bounded and can be retired only
after every use is settled. Normal native-authority extraction requires no
active use. Caller-reported indeterminate publication or currentness loss
quarantines the owner and blocks normal extraction; `Drop` performs no KFD
operation.

Used directly, this core does not connect to the compute AQL or SDMA packet owners. Its
`Prepared`, `Published`, `Completed`, dependency-frontier, timeout, and
quarantine transitions are host-side custody states invoked by a future
adapter; they are not observations of a queue, packet, signal, device, driver,
firmware, or hardware. In particular, a complete two-device peer mapping is
not a `Gfx942XgmiRouteV1`: the peer-mapped operation names grant no route
direction, topology generation, home endpoint, selected SDMA engine, XGMI
publication, or completion authority. The currentness-loss entry point records
only a caller report and performs no currentness check. Queue-owned publication
callbacks and exact compute binding remain separate integration work. There is
no executable-Rust refinement proof for this ledger.

## R18 targeted persistent local-SDMA adapter

`promote_sdma_device_buffer_to_persistent_allocation_v1` moves one existing
queue-owned device buffer into `Gfx942QueuePersistentAllocationV1` without
changing `sdma_outstanding_buffers`. The wrapper remains bound to the exact
parent `QueueKeyV1`, native child queue ID, target engine, pool generation,
mapped allocation identity, and full physical extent. Admission is limited to
one page-multiple local allocation no larger than 256 MiB and one single
targeted queue. Engine 1 admits only H2D; engine 0 admits only D2H. Directional,
striped, generic untargeted, peer-mapped, XGMI, compute, and concurrent range
uses are outside this adapter.

`submit_persistent_sdma_copy_v1` consumes that wrapper and exactly one ordinary
host SDMA buffer. It reserves and prepares the derived R17 LocalSdma use, then
moves the exact device lease into the existing SDMA queue record. Prepared and
published custody bind the host storage identity, pool generation, logical and
physical extents, plus the exact planned lower ticket slot and generation.
Clean pre-publication rejection restores only those exact inputs and cancels the
prepared use.
Lower-layer `Retained` custody quarantines the use while it is still Prepared;
it never fabricates a Published transition. Only confirmed lower publication
advances the use to Published. Nonblocking
`poll_persistent_sdma_copy_v1` and bounded
`wait_persistent_sdma_copy_for_v1` retain the same move-only submission on
pending or timeout. Exact completion authenticates the queue record's device
identity, restores the original persistent owner, and advances through
Completed and Settled. Demotion is available only after quiescent,
non-quarantined restoration and advances the inherited pool generation once.
The consuming `retire_settled_frontier_v1` allocation transition reclaims the
bounded settled ledger after quiescence; it accepts only the exact latest
frontier and returns both move-only inputs unchanged on stale or substituted
frontiers. Retirement performs no native operation and does not change the
inherited outstanding-buffer debit.

Terminal currentness, publication, ticket, or completion uncertainty returns
opaque process-teardown custody. No post-publication failure path allocates a
replacement custody object or exposes the raw adapted device buffer to the
ordinary recycle/release API. Host tests cover Prepared quarantine, exact
detach/restore, pending and timeout retention, exact completion settlement,
terminal completion custody, more than 64 sequential publish/complete/retire
cycles, stale and substituted frontier rejection, demote/re-promote frontier
ABA rejection, same-queue host and ticket substitution rejection, and the
recoverable/retained/confirmed branch separation. These tests perform no KFD,
DRM, SDMA, HIP, HSA, firmware, or GPU work. The adapter is not hardware
execution evidence, a concurrency claim, a copy-performance result, or an
executable-Rust/formal refinement proof.

## R19 directional persistent local-SDMA adapter

The additive R19 surface promotes one existing queue-owned device buffer into
`Gfx942DirectionalQueuePersistentAllocationV1`, bound to the exact parent queue
occurrence and the ordered pair of distinct native child queues: engine 1 for
H2D and engine 0 for D2H. Unlike R18, pooled backing is admitted with
`0 < logical <= physical <= 256 MiB`; the physical extent must remain page
rounded and is the extent owned by R17, while every copy is bounded by the
current logical extent. Promotion, use, completion, frontier retirement, and
demotion preserve the one inherited `sdma_outstanding_buffers` debit.

`submit_directional_persistent_sdma_copy_v1` selects direction explicitly on
every use. After exact completion and frontier retirement, any next direction
is admitted, including repeated same-direction chunks and arbitrary H2D/D2H
sequences. Dependency chaining is not exposed: the exact completed frontier
must be retired before the next use, which then reserves without a dependency.
The lower single-copy prepare/publish path retains one request,
packet, and full ticket inline; it does not allocate batch `Vec` rosters or an
owned doorbell error string per packet. The lower maximum linear copy remains
`0x003f_ffe0` bytes, so larger logical transfers require sequential chunks.
Nonblocking `poll_directional_persistent_sdma_copy_v1` and bounded
`wait_directional_persistent_sdma_copy_for_v1` retain exact pending custody.

Clean rejection returns both owners and cancels Prepared. A retained lower
publication quarantines Prepared, confirmed publication alone advances to
Published, and exact child/ticket/range/storage restoration alone completes and
settles the use. Promotion and demotion failures also distinguish explicit
retryable custody from opaque process-teardown custody; no moved native owner is
represented by `None`. Terminal currentness or publication ambiguity poisons
the session, and outstanding persistent custody continues to block queue
destruction. No topology or sysfs discovery occurs per packet.

The frozen R19 manifest digest is
`c04f67240eecff85cffb092a228554c88a72cb89f1d49865c123db559cfae319`.
Evidence is native-neutral host custody and failure-injection testing only.
R19 remains single-flight and local: it does not claim concurrent range borrows,
striping, peer/XGMI copies, compute integration, hardware execution,
copy-performance parity, or executable-Rust/formal refinement.
