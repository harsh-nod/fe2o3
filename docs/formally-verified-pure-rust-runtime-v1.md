# Formally Verified Pure-Rust Runtime V1

Status: accepted R0 architecture and trust-boundary contract for issue #137.

This document freezes the first production profile for replacing the HIP-backed
`fe2o3-core` and reviewed ROCr/HSA adapter with a Rust implementation over the
Linux KFD and AMDGPU DRM UAPIs. It is a child workstream of #134. Persistent
execution in #135 may use this runtime only after the applicable lifecycle,
queue, reset, and quiescence proof gates close.

This decision does not claim that the runtime exists yet. R0 fixes the boundary,
vocabulary, crate ownership, failure rules, oracle role, and staged gates so
later implementations cannot gain authority by weakening an assumption.

## Decision

The fe2o3 production runtime will be implemented in Rust and verified as a
refinement of a versioned abstract runtime state machine. Its user-space
production dependency closure will contain no HIP, ROCr/HSA runtime, COMGR,
`libdrm`, C or C++ runtime shim, build-time bindgen, or build script that
compiles or links native code.

The initial runtime talks directly to:

- `/dev/kfd` through checked-in, versioned Rust definitions of the Linux KFD
  ioctl UAPI;
- the selected `/dev/dri/renderD*` node through checked-in, versioned Rust
  definitions of the AMDGPU DRM ioctl UAPI;
- AMDHSA COV6 code objects as an authenticated data format; and
- an AQL queue as a hardware-facing packet and memory-ordering ABI.

AMDHSA and AQL remain part of the design. They are data and execution protocols,
not permission to link ROCr. The Linux kernel, scheduler firmware, and GPU remain
external contracts. Calling the implementation "pure Rust" means the complete
fe2o3 user-space runtime implementation and production build closure are Rust;
it does not mean that Rust replaces the kernel, firmware, or machine ABI.

The exact decision identity is:

```text
fe2o3.runtime.pure-rust.gfx942.v1
```

Any semantic change to this document, its proof model, or the dependency policy
requires a new identity. Existing evidence must not be silently reinterpreted.

## Current Paths and Their Limits

`fe2o3-core` currently depends on `fe2o3-hip-sys`. Its build selects
`libamdhip64`, and native probes compile against HIP headers. HIP owns context,
allocation, copy, module, stream, event, and launch behavior.

`fe2o3-hsa-runtime` is a narrower reviewed path, but it is not independent of
ROCm. Its build compiles `native/runtime.c`, includes HIP and HSA headers, and
links both `libhsa-runtime64` and `libamdhip64`. Its direct AQL submission is
valuable implementation evidence, but its resource creation, executable
loading, device correlation, and ABI boundary are still delegated to native
ROCr/HIP code.

Those paths remain useful during migration. They do not satisfy the new trust
boundary because:

1. Their large native implementations and transitive dependencies are outside
   the Verus model.
2. C handles and callback lifetimes do not encode fe2o3 allocation, VM, queue,
   dispatch, and completion identities in Rust types.
3. A successful HIP or HSA call does not bind the loaded code, kernarg bytes,
   packet, queue generation, and proof evidence into one claim.
4. Cleanup and timeout behavior is determined by opaque runtime state. fe2o3
   cannot prove exactly when ownership may be returned after ambiguity.
5. Deployment and generic CI inherit ROCm installation, header, loader, and DSO
   variation even when no GPU is needed.

The Rust path is better for fe2o3 because it makes the proof boundary small and
owned, not because HIP or ROCr are generally poor runtimes. HIP and ROCr retain
broader hardware support, mature recovery behavior, and years of compatibility
work. fe2o3 must earn removal of those paths through shadow execution and
explicit performance and reliability gates.

## Pure-Rust Production Boundary

### Permitted

- Rust crates with no `links` key. A Cargo custom build target is permitted only
  when its exact `name@version` has a reviewed pure-Rust configuration-script
  exception in the versioned policy and its Cargo source matches that review.
- Rust `unsafe` code in a sealed syscall adapter, with each operation linked to
  a reviewed precondition/postcondition contract.
- The host C ABI supplied by the platform libc for the bounded system interfaces
  required by the adapter. Dynamic loading and process launch are excluded; the
  initial ELF policy permits only the named baseline system DSOs.
- Checked-in UAPI constants and `#[repr(C)]` Rust structures generated or
  transcribed from a pinned Linux UAPI revision, provided independent layout
  tests bind every size, alignment, offset, ioctl number, and version.
- Parsing already-finalized, authenticated ELF64 AMDHSA COV6 bytes in Rust.
- Direct MMIO/doorbell and atomics only behind a reviewed AQL abstraction whose
  executable ordering matches its Verus model.
- HIP and ROCr in separately selected development tests and hardware oracle
  jobs that cannot contribute production dependencies or proof authority.

### Prohibited

- `libamdhip64`, `libhsa-runtime64`, `libamd_comgr`, `libdrm`,
  `libdrm_amdgpu`, HIPRTC, or a renamed/repackaged equivalent.
- C/C++/assembly shims compiled by a crate build script, including a static
  archive that avoids a `DT_NEEDED` record.
- Production `libdl`, `dlopen`/`dlsym`, process-launch, or equivalent escape
  hatches for loading a prohibited runtime out of process.
- Loader-controlled search/audit/filter indirection through `DT_RPATH`,
  `DT_RUNPATH`, `DT_AUDIT`, `DT_DEPAUDIT`, `DT_FILTER`, or `DT_AUXILIARY`.
- Build-time bindgen, `pkg-config`, CMake, `cc`, or shell discovery of ROCm.
- Calling COMGR to compile, relocate, or load code at runtime.
- Treating an arbitrary raw pointer, GPU virtual address, queue handle, signal,
  or descriptor as authenticated authority.
- Treating a hardware test, differential match, signature, hash, or successful
  ioctl as a Verus proof.

### Audit Enforcement

The checked-in policy is `scripts/runtime-pure-rust-policy.json`. The auditor is
`scripts/runtime_pure_rust_audit.py`. It uses only the Python standard library
and has no ROCm dependency.

Audit the current locked, target-filtered production closure, including normal
and build edges. The auditor invokes Cargo directly so CI cannot accidentally
reuse detached or stale metadata:

```bash
python3 scripts/runtime_pure_rust_audit.py metadata \
  --cargo \
  --root fe2o3-kfd \
  --root fe2o3-kfd-uapi \
  --root fe2o3-runtime-model
```

Do not use `cargo metadata --no-deps`: the auditor requires the complete resolve
graph and fails if it is missing. Dev-only edges are excluded so a separately
compiled oracle may use HIP/HSA. Normal and build edges are always included.
Every reachable `links` package and every unapproved custom build target is
rejected.

The V1 configuration-script exceptions are exactly `libc@0.2.189` and
`rustix@1.1.4` from the crates.io registry. Their reviewed build scripts select
Rust `cfg` behavior and do not invoke a native compiler, generate a native
object, or add a native library. Cargo metadata can retain rustix alternatives
that are mutually exclusive in the selected `linux_raw_sys` feature closure;
in particular, a conservative resolve graph can still reach libc. Allowing both
exact identities prevents this Cargo representation detail from being confused
with a native shim.

An allowlist entry is not proof authority. Each version/source change requires
review and a policy update. Release evidence binds the exact package source,
lockfile, metadata, policy digest, and resulting ELF. The deterministic audit
report names every allowlisted build script actually exercised by the selected
closure; an unused allowlist entry is not reported as if it ran.

Audit the final host executable or shared object:

```bash
python3 scripts/runtime_pure_rust_audit.py elf \
  --input /absolute/path/to/fe2o3-runtime-host
```

The ELF audit parses `PT_DYNAMIC`, its dynamic tags, `DT_NEEDED`, `SHT_DYNSYM`,
and their string tables without invoking `ldd`, `readelf`, or the dynamic
loader. It rejects unknown DSOs (including `libdl`), prohibited imports/exports,
standard dynamic-loader and process-launch imports, loader-controlled
search/audit/filter tags, and known hidden loader literals. A malformed,
truncated, unsupported, symlinked, changing, or incompletely inspectable input
is an audit error, not a pass. The input digest and counts are reported
deterministically.

This audit is a necessary negative control, not a semantic proof. It cannot
show that arbitrary statically linked machine code is benign or rule out a
custom loader/executor implemented directly with syscalls. Release admission
must bind the policy, Cargo invocation and exact metadata, source tree, compiler
invocation, final ELF digest, and audit result. Repository review prohibits
obfuscating or recreating the rejected boundary.

## Initial `gfx942` Profile

V1 supports one deliberately bounded target:

| Dimension | V1 decision |
|---|---|
| host OS | Linux x86-64 |
| GPU ISA | exactly `gfx942` |
| feature mode | exactly `xnack-` |
| wave size | Wave64 |
| code object | AMDHSA COV6, ELF64 little-endian |
| topology | one physical GPU and one process VM |
| queue | one single-producer AQL compute queue |
| dispatch | synchronous, one in-flight dispatch for the first vertical slice |
| completion | dedicated generation-bound completion signal |
| memory | pinned host-visible memory first; VRAM and SDMA are later gates |
| kernels | compiler-produced, authenticated descriptors only |
| reset | fail closed and invalidate the complete runtime generation |

V1 does not claim `xnack+`, `gfx950`, consumer RDNA, multi-GPU peer mappings,
SR-IOV partitions, multi-process sharing, queue preemption recovery, dynamic
parallelism, graph capture, asynchronous callbacks, or persistent residency.
An observation outside the exact profile returns `Unsupported`; it may not be
rounded to a nearby target or downgraded to an unchecked launch.

The first live test dispatches one bounded, generated vecadd-like kernel. GEMM,
FlashAttention, MoE, megakernels, and persistent workers reuse the same runtime
model only after their resource, synchronization, scratch, async, and progress
extensions have separate proof and hardware gates.

## Runtime Identities and State

Raw integers are observations, not authority. The model assigns non-forgeable,
generation-bound identities to:

```text
KfdAbiId
PhysicalDeviceId
RuntimeGeneration
VmId
AllocationId
MappingId
CodeLoadPlanId
LoadedCodeId
QueuePlanId
QueueInstanceId
QueueGeneration
DispatchId
CompletionId
```

Every state transition consumes a value for the old state and produces a value
for the new state. Safe Rust does not expose constructors from raw handles.
Conceptually:

```rust,ignore
let device: Device<Observed> = Runtime::discover(selector)?;
let device: Device<Bound<Gfx942XnackMinus>> = device.admit(profile)?;
let allocation: Allocation<Allocated> = device.allocate(layout)?;
let mapping: Mapping<Mapped, GpuReadWrite> = allocation.map(&device.vm())?;
let code: Code<Loaded, Authenticated> = loader.load(bundle, &device)?;
let queue: Queue<Ready, SingleProducer> = device.create_queue(plan)?;
let dispatch: Dispatch<Prepared> = code.prepare(args, &mapping, geometry)?;
let dispatch: Dispatch<Published> = queue.publish(dispatch)?;
let dispatch: Dispatch<Quiescent> = dispatch.wait()?;
```

Executable types prevent local misuse. Verus proves that the implementation of
each transition preserves the ghost identity, ownership, mapping, queue-slot,
publication, and lifetime invariants.

## Crate Responsibilities

These are logical ownership boundaries. Adding them to the workspace is a later
milestone and requires the existing dependency-layer policy to be updated in a
separate reviewed change.

| Logical crate | Owns | Must not own |
|---|---|---|
| `fe2o3-runtime-model` | executable pure state machine, ghost views, invariants, transition specs, property vocabulary | ioctls, file descriptors, atomics, ELF parsing |
| `fe2o3-kfd-uapi` | pinned KFD/AMDGPU UAPI constants, `repr(C)` wire structs, checked ioctl encoding, layout witnesses | device policy, resource lifecycle, C headers at build time |
| `fe2o3-kfd` | retained device/render FDs, identity correlation, VM/allocation/map/queue ioctl adapter, reset generation | code-object semantics, AQL packet publication, kernel ABI |
| `fe2o3-amdhsa-loader` | bounded COV6 parsing, segment/relocation plan, descriptor admission, authenticated load correspondence | compiling code, COMGR calls, raw dispatch |
| `fe2o3-aql` | packet and ring representation, reservation, release publication, doorbell order, signals, completion generations | KFD discovery, arbitrary pointer construction, kernel semantics |
| `fe2o3-runtime` | safe typestate API, artifact/evidence binding, argument admission, lifecycle composition, public errors | unreviewed syscalls, caller-packed kernargs, legacy handle escape |
| `fe2o3-host` | generated kernel-specific preparation and ergonomic ownership transfer | raw GPU VAs, packet headers, proof-status promotion |

Dependency direction is from orchestration toward the pure model and sealed
mechanisms. `fe2o3-runtime-model` must remain independently Verus-checkable.
UAPI and AQL wire structs do not grant authority merely because their layout is
correct. The public runtime is the only safe composition boundary.

The existing `fe2o3-hsaco` and artifact crates may supply bounded read-only
parsing and identity primitives if their production closures pass this policy.
Reusing code does not permit the new runtime to depend on `fe2o3-core` or
`fe2o3-hsa-runtime`, since both lead back to HIP/HSA.

## Proof, Check, Contract, and Measurement

Every claim is property-specific. The implementation and evidence format must
use these statuses without aliases:

| Status | Meaning | Examples |
|---|---|---|
| `Proved` | Verus accepted the named theorem for the exact model and executable Rust unit, with only its recorded allowlist | allocator transition preserves unique ownership; two reservations cannot own one slot |
| `Checked` | a deterministic parser, validator, or dynamic admission check accepted exact bytes/observations | UAPI layout fixture, ELF structure, target match, descriptor bounds |
| `ProvedUnderContract` | a Verus theorem is valid if every named external contract behaves as specified | observed terminal signal implies quiescence under the AQL completion contract |
| `Contracted` | behavior is supplied by a versioned external component and is not proved by fe2o3 | KFD ioctl semantics, firmware packet consumption, MFMA hardware behavior |
| `Measured` | a test or benchmark observed behavior for exact recorded inputs | HIP differential match, queue latency, MI300X reset test |
| `Unsupported` | the profile has no admissible semantics for the request | `xnack+`, multi-GPU, unknown relocation |

`Verified` is an aggregate presentation allowed only when every property
required by the selected profile is `Proved` or `ProvedUnderContract` and all
identity, check, and contract bindings match. A `Measured` result cannot satisfy
a proof requirement. `Checked` cannot be promoted to `Proved`. Unknown or
missing status fails closed.

### What Verus Proves

The V1 proof plan includes:

- UAPI request construction cannot overflow and every retained input meets the
  adapter precondition before the syscall;
- device, VM, allocation, mapping, loaded-code, queue, and completion identities
  cannot be mixed across runtime generations;
- GPU virtual ranges are aligned, in bounds, live, permission-compatible, and
  non-overlapping where exclusivity is required;
- partial allocation/map/load/queue failures release each acquired resource at
  most once and only in a valid reverse dependency order;
- the bounded loader plan maps exactly the authenticated bytes plus permitted,
  recorded relocations and selects the exact admitted descriptor;
- the selected kernel descriptor and kernarg layout match the compiler evidence;
- queue reservation and wraparound preserve unique slot ownership;
- all packet fields and retained argument bytes are initialized before the AQL
  header release-store that linearizes publication;
- no safe operation mutates or releases a reachable resource between publication
  and admitted quiescence;
- a returned completion is bound to the dispatch, signal generation, queue
  generation, runtime generation, and terminal observation;
- reset, timeout, and ambiguous post-publication failures never return live
  ownership as though execution had completed; and
- composition binds compiler artifact identity through loaded code, kernarg,
  packet, dispatch, and completion evidence.

The top-level shape is conditional:

```text
verified runtime implementation
+ admitted exact compiler artifact and launch contract
+ KFD/AMDGPU UAPI contract
+ AQL/firmware execution and completion contract
+ GPU ISA and memory-model contract
=> a reported successful completion corresponds to exactly one admitted
   dispatch of the authenticated kernel, with all referenced resources valid
   through quiescence
```

### What Remains Contracted

fe2o3 does not prove that:

- Linux implements the pinned ioctl UAPI correctly;
- the kernel cannot be compromised and returns truthful topology/mapping state;
- scheduler firmware fetches and executes AQL according to its specification;
- GPU atomics, cache coherence, barriers, MFMA instructions, or fault reporting
  match the machine model;
- physical memory, PCIe, HBM, or the GPU is fault-free;
- a timeout means work stopped; or
- the host Rust compiler and CPU execute the verified Rust semantics faithfully.

These contracts are versioned, named in evidence, minimized, and tested with
ABI fixtures, memory-ordering litmus tests, fault injection, hardware replay,
and HIP/HSA differential oracles. Contract tests can find implementation drift;
they do not turn the contracted component into proved code.

## Unsafe and Trusted Computing Base

The intended user-space trusted base contains:

1. the sealed Rust syscall/volatile/atomic adapter whose behavior is assumed at
   the Verus boundary;
2. the admitted specifications for the Linux UAPI, AQL, AMDHSA COV6, GPU ISA,
   and memory model;
3. each exact allowlisted pure-Rust configuration build script that influences
   the production compilation;
4. the Rust compiler, Verus translator/checker, SMT solver, artifact binder, and
   evidence verifier identified by exact versions; and
5. cryptographic hash and signature primitives used for identity and origin.

The external execution base contains the host kernel, KFD/DRM implementation,
firmware, CPU, GPU, and memory hardware. HIP, ROCr, COMGR, `libdrm`, C shims, and
their dependency closures are intentionally absent from both production lists.

Every `unsafe` block in the runtime must map to one reviewed contract ID. Broad
`unsafe impl Send/Sync`, untyped handle copying, and caller-provided proof facts
are prohibited. The repository must maintain an automatically checked unsafe
inventory before R6 closes.

## Failure and Recovery Policy

Failure behavior is part of the proof, not an error-handling afterthought.

### Before external mutation

Validation and planning errors have no device-visible effect. They return a
typed error and preserve all caller ownership.

### After an ioctl with known failure semantics

If the pinned contract guarantees that a failed ioctl performed no mutation,
the old state remains valid. If it guarantees a complete mutation despite a
reported condition, the returned witness represents the new state. The adapter
may not guess between those cases.

### Partial acquisition

Every acquired resource is recorded immediately in a linear cleanup plan.
Rollback runs in reverse dependency order. A cleanup action is attempted at
most once. Cleanup failure poisons the affected VM or runtime generation and
retains enough state for process-exit cleanup; it never fabricates success.

### Before AQL publication

A reserved but unpublished slot may be cancelled only while the model proves
the GPU cannot observe a valid header. Kernarg and resource borrows remain with
the prepared dispatch.

### At and after AQL publication

The release-store of the valid packet header is the linearization point. Once
it may have occurred, rollback is impossible. Doorbell failure, interruption,
timeout, signal ambiguity, or host panic leaves the dispatch in an in-flight or
quarantined state. Referenced memory cannot be unmapped, freed, or returned for
mutable reuse until an admitted quiescence or whole-generation teardown.

### Reset and device loss

A reset observation invalidates the runtime generation, all queues, mappings,
signals, loaded code, and outstanding completion tokens associated with it.
Stale tokens return `GenerationInvalidated`; they never admit completion. V1
does not attempt transparent replay.

### `Drop` and panic

`Drop` is not an authority path because it cannot report failure. Dropping a
prepared pre-publication value may perform proved local cleanup. Dropping an
in-flight value transfers it to runtime-owned quarantine. A panic cannot unwind
through an FFI/syscall boundary or release in-flight resources early. Process
exit remains the final cleanup boundary for irrecoverably poisoned state.

### Resource exhaustion and malicious inputs

All file sizes, segment counts, relocation counts, queue sizes, allocation
sizes, and arithmetic have explicit maxima and checked operations. Unknown
ioctl versions, relocations, metadata keys, target features, and packet forms
return `Unsupported` or `Malformed`; they do not enter a compatibility mode.

## HIP/HSA Oracle Role

HIP and ROCr are retained only for four bounded purposes:

1. compare device identity, topology, allocation, copy, and mapping observations;
2. compare kernel outputs and canaries for exact compiler artifacts;
3. compare completion, reset, and failure behavior where both paths expose a
   meaningful observation; and
4. provide a performance baseline during staged adoption.

Oracle code must be a dev-only dependency or a separately built harness. The
production Cargo closure audit excludes dev edges and then proves that no
normal/build edge reaches the oracle. The final production ELF is audited
independently.

An oracle result records exact GPU identity, firmware/kernel/ROCm versions,
artifact digest, launch, inputs, outputs, and command result. Disagreement
blocks promotion and is investigated; agreement is `Measured`, never proof.
The oracle may not supply kernel descriptors, proof records, expected digests,
or runtime state to the path under test. CPU specifications remain the primary
functional oracle where feasible.

The R1 identity comparison is implemented by the isolated, bounded lane in
`docs/runtime-identity-oracle-v1.md`. It executes the pure-Rust evidence producer
before starting `rocminfo`, re-audits the production closure and ELF, compares
exactly eight sorted MI300X identities, and emits only `Measured`,
non-authoritative evidence. That observation satisfies the R1 differential-test
deliverable; it does not discharge the generation proof or any external
contract. Its detached record binds the exact clean Git commit, runner, policy,
auditor, lockfile, bounded audit reports, comparator, and measured executables.
Contracted currentness and the VRAM-loss counter remain explicitly
pure-Rust-only rather than being laundered into the HSA comparison.

## Staged Adoption and Exit Criteria

### R0: boundary, model, and audit

Deliver this decision, the versioned dependency policy, deterministic offline
auditor, adversarial fixtures, abstract transition vocabulary, and a TCB/contract
inventory template.

Exit requires:

- generic CI runs all metadata/ELF audit unit tests without ROCm, audits the
  actual locked production closures rooted at `fe2o3-kfd`, `fe2o3-drm-uapi`,
  `fe2o3-kfd-uapi`, and `fe2o3-runtime-model`, builds `fe2o3-kfd`'s
  `kfd-version`, `kfd-topology`, and `kfd-device-identity` examples in a
  dedicated target directory, and audits all three linked ELFs;
- malformed metadata/ELF, missing resolve data, unknown DSO, prohibited DSO,
  `libdl`, prohibited symbol, dynamic-loader/process-launch symbol,
  loader-control dynamic tag, hidden loader literal, `links`, unapproved build
  script, unapproved allowlist source, and HIP/HSA production dependency
  fixtures all fail;
- a pure Rust closure, the exact reviewed rustix/libc configuration-script
  exceptions, and a baseline-system-DSO ELF fixture pass deterministically, with
  every exercised build-script exception named in the report;
- the current HIP and HSA paths are explicitly classified as oracle/legacy, not
  production-compliant; and
- architecture review accepts every contract and non-goal in this document.

R0 does not authorize a GPU launch.

### R1: UAPI and identity

Implement pinned KFD/DRM Rust wire definitions, retained device/render FDs,
version negotiation, node correlation, exact `gfx942:xnack-` admission, and the
first Verus identity transitions.

Exit requires exhaustive layout/ioctl-number fixtures against the pinned Linux
headers in a separate generation test, checked-in golden values for generic CI,
hostile version/topology tests, MI300X comparison with the HIP/HSA oracle, and
proof that device/VM/runtime generations cannot be mixed. No queue is created.

### R2: VM, allocation, and mapping

Implement allocation, GPU VA reservation, mapping, permissions, unmapping, and
linear cleanup plans for bounded host-visible memory.

Exit requires positive and fault-injected tests at every ioctl boundary; proofs
of range, alignment, overflow, ownership, mapping permission, and at-most-once
cleanup; reset poisoning tests; and canary/copy comparison on `mi300x`. Unknown
partial-mutation behavior blocks the milestone.

### R3: authenticated COV6 loader

Implement the bounded Rust loader, segment plan, permitted relocations, code/data
permissions, descriptor selection, and compiler-evidence binding.

Exit requires mutation tests for every ELF field and relocation, exact loaded
byte correspondence proofs, W^X enforcement, wrong-target and wrong-descriptor
rejection, deterministic artifact/load evidence, and no COMGR use in the Cargo
closure or host ELF.

### R4: AQL queue and synchronous dispatch

Implement one single-producer queue, slot reservation, packet initialization,
release publication, doorbell, dedicated completion signal, wait, and teardown.

Exit requires proofs of slot uniqueness, wraparound, initialization-before-
publication, dispatch linearizability, lifetime retention, and completion
identity; at least 100,000 wraparound submissions in stress testing; ordering
litmus tests; every publication/fault boundary injected; and exact vecadd output
and canaries on `mi300x` against CPU and HSA oracles.

### R5: generated safe host integration

Bind generated Rust arguments, effects, artifact/proof identities, mappings,
geometry, packet, and completion into the public safe API.

Exit requires compile-fail tests for cross-device, cross-generation, alias,
premature-free, raw-address, and stale-completion misuse; no safe caller-packed
kernarg or raw launch; and one source-to-completion evidence chain for the exact
bounded kernel. Legacy raw APIs remain explicitly unsafe.

### R6: proof and evidence closure

Complete Verus refinement for every V1 transition, the unsafe/axiom inventory,
proof-manifest authentication, property-level status report, and top-level
conditional runtime theorem.

Exit requires zero unrecorded `assume`, `external_body`, unsafe block, or trusted
specification; hostile evidence substitution tests; reproducible verification;
independent review of all contracted boundaries; and fail-closed behavior when
any proof, model, target, artifact, device, runtime, or completion identity
differs.

Only R6 may use the aggregate `Verified` label for the bounded runtime profile.

### R7: asynchronous and performance-critical facilities

Add multiple in-flight dispatches, VRAM, SDMA copies, scratch, queue concurrency,
events/dependencies, and target extensions one at a time.

Each facility requires a model extension, proof of non-reuse and lifetime/order
invariants, reset/quiescence behavior, adversarial tests, hardware litmus tests,
and its own evidence property. Persistent kernels additionally require the #135
task-ring, drain, stop, progress, and residency contracts. No R7 feature inherits
R6 authority automatically.

### R8: shadow adoption and legacy removal

Run the Rust runtime and legacy oracle on representative GEMM, FlashAttention,
MoE, fused, and persistent workloads. Compare correctness, failures, queue
latency, launch overhead, throughput, and resource use under pinned conditions.

Exit requires no unresolved correctness divergence, reviewed recovery behavior,
the committed performance thresholds from #137, sustained generic and `mi300x`
CI, a rollback plan, and final Cargo/ELF audits for every shipped binary. Only
then may production features stop building HIP/HSA. Oracle jobs may remain
separate for regression detection.

## Evidence Record Requirements

Every admitted runtime execution record must bind at least:

```text
policy identity and digest
runtime model identity
Verus/checker/solver identities and proof manifest
source tree and exact production Cargo resolve graph
host compiler and final host ELF digest
Cargo-closure and ELF audit results
KFD/DRM UAPI contract identities
Linux kernel, firmware, physical device, target, and runtime generation
compiler artifact, COV6, load plan, loaded bytes, and descriptor identities
VM, allocation, mapping, queue, packet, kernarg, dispatch, and completion IDs
property-level Proved/Checked/Contracted/Measured/Unsupported statuses
```

Changing any field that contributes to a theorem or contract invalidates the
corresponding result. A report with a missing field cannot be repaired by a
human-readable note after execution.

## Consequences

The design gives fe2o3 a substantially smaller, inspectable user-space trusted
base; Rust ownership aligned with GPU lifetimes; explicit linearization and
quiescence rules; exact compiler/runtime artifact binding; deterministic generic
CI; and direct control of the dispatch path needed for low overhead, persistent
queues, and specialized kernels.

The cost is significant. fe2o3 takes responsibility for kernel UAPI drift,
device discovery, VM behavior, queue correctness, reset semantics, memory
ordering, target enablement, and years of compatibility behavior currently
handled by ROCr/HIP. Formal verification does not erase that work. The bounded
`gfx942` profile, fail-closed extensions, differential oracle, and staged removal
are mandatory controls against claiming breadth before it exists.

## R0 Validation

Run the ROCm-independent R0 gate with:

```bash
scripts/ci-local.sh runtime-policy
```

The same gate runs in `generic-core`. Its unit tests create adversarial ELF and
Cargo metadata fixtures in temporary directories. The gate additionally audits
the actual four-crate production closure and the freshly linked `kfd-version`,
`kfd-topology`, and `kfd-device-identity` ELFs. It does not inspect or require
the host ROCm installation.
