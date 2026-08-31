# fe2o3 Implementation Roadmap v2

Status: living execution roadmap and chronological milestone record.

Commit identities in individual sections identify those bounded historical
checkpoints, not the current repository head. Statements such as "next
blocker" inside a checkpoint are local to that checkpoint unless repeated in
the current parity matrix. Current implementation strength and remaining work
are tracked by [architecture-v2.md](architecture-v2.md), the
[cuda-oxide parity matrix](cuda-oxide-parity-matrix.md), and generated parity
evidence.

The bounded MoE V2 checkpoint remains
`10e5f90ece1937aaee77492e8e4e4742863d013b`; it identifies that scoped
fail-closed host boundary, not the current integration checkpoint.

This roadmap turns [architecture-v2.md](architecture-v2.md),
[production-pipeline-convergence-v1.md](production-pipeline-convergence-v1.md),
[verification-model.md](verification-model.md), the
[GPU safety contract v1](gpu-safety-contract-v1.md), the
[general typed dispatch V1 contract](general-typed-dispatch-v1.md), and the
[cuda-oxide parity matrix](cuda-oxide-parity-matrix.md) into independently
owned work with staged integration gates. Gates are evidence-based; calendar
dates depend on staffing and hardware availability.

## Current production LDS checkpoint

The 2026-08-27 checkpoint completes the first genuine Rust workgroup kernel
through the one production transaction. The WG64 `i32` LDS reduction reaches
semantic MIR, ranked PLIRON, verified Kernel IR, composed memory admission,
upstream-LLVM AMDGPU lowering, compiler-bound handoff, measured target-machine
and in-process LLD execution, and reproducible inspected COV6 HSACO. The
existing scoped atomic kernel is requalified through the same code-object
harness. Neither path has current load or launch authority. See
[gfx942 production LDS reduction V1](gfx942-production-lds-reduction-v1.md).

The direct pure-Rust KFD packet mechanics and invocation-specific runtime
authority gate are now implemented and measured with the exact LDS artifact.
The measurement uses a manually asserted unsafe diagnostic authority, not the
production verifier. A second MI300X lane authenticates the exact scalar-GEMM
Worker V3 artifact with an explicitly synthetic test verifier, joins it to
macro-generated host-memory arguments and one checked KFD device, executes the
move-only invocation, and validates numerical output, completion writeback, and
canaries. The production compiler and host now also preserve the exact signed
aggregate MIR-to-live-PLIRON execution: a V4 association inside the frozen V3
capsule binds all five compiler stages, and Worker V3 independently reimports
the receipt, cross-checks middle-end V5, and retains it beside signed compiler
currentness through the HSA lifecycle. This proves custody and signature
consistency, not LLVM/final-machine refinement or runtime authority. The
2026-08-29 continuation closes exact LLVM-to-HSACO stage custody: the Worker
records linked and optimized modules, the generated object, ordered native
inputs, canonical path-independent in-process LLD arguments, and final HSACO;
Rust independently reconstructs the evidence identity, request-derived order,
linker policy, and final payload relation, then requires every measured stage
to agree across strict Worker V3 replay. A real gfx942 run produced and
independently inspected one scalar-GEMM COV6 artifact through upstream LLVM
22.1.8. This remains
measured derivation evidence, not a semantic-refinement or runtime-authority
claim.
Compiler-execution authority now has signed request and receipt
records plus separate crash-safe issuer and Worker journals. The Worker journal
verifies and durably reacquires the exact request and receipt before the issuer
can acknowledge publication, and recovery accepts only the three legal
cross-journal positions. A fixed-width canonical packet protocol and bounded
allocation-free `SOCK_SEQPACKET` loop now make that composition the sole public
issuer transition path over an admitted connection. This closes local service
transport, durable receipt publication, and root-owned distinct-UID launcher
implementation. Exact account/socket provisioning, privileged end-to-end
qualification, independent monotonic deployment, and the production Worker
verifier remain open. The next major gates are:

1. implement the reviewed concrete production Worker V3 verifier behind the
   sealed host boundary, make generated
   applications invoke the canonical inherited pure-KFD transition, and prove
   one public build/publish/run command on MI300X without external HSACO
   injection;
2. general race, alias, address-space, bounds, and barrier-convergence proof
   obligations with Verus-consumable evidence;
3. source-authentic tiled LDS GEMM through the same transaction;
4. fused softmax/attention and MoE primitives built from qualified operations;
5. reproducible caching, stable diagnostics/API, CI hardware qualification,
   tutorials, and performance baselines for widespread use.

## Issue #134/#135 Infrastructure and Scalar Checkpoint

The 2026-08-18 ownership refactor makes issues
[#134](https://github.com/harsh-nod/fe2o3/issues/134) and
[#135](https://github.com/harsh-nod/fe2o3/issues/135) infrastructure-enabled,
but both issues remain open.

- Canonical, Pliron-independent ownership now exists for the MIR model,
  compiler API, solver-neutral proof contracts, target-neutral host-operation
  contracts, and executable-free persistent-service model.
- `cargo-fe2o3` and `rustc-codegen-fe2o3` now own the sole managed production composition. The retired standalone compiler driver has been removed.
- `fe2o3-pliron` provides the pinned D0 context, private context identity,
  registration, bounded pass-plan shell, and owner-held textual bridge. The
  bridge recursively verifies operations and meters complete owner/session tree
  accounting, but arbitrary registered parsers remain trusted. Seven
  target-neutral Pliron dialect shells cover `kernel.*`, `schedule.*`,
  `tile.*`, `gpu.*`, `proof.*`, `dispatch.*`, and `autotune.*`.
  `dialect-mir` adds a bounded `mir.*` shell only with feature `pliron` while
  preserving its default compatibility facade over `fe2o3-mir-model`.
- `fe2o3-lower-mir-kernel` retains narrow, bounded, terminally fail-closed
  MIR-to-kernel conformance with context-bound results. Detached KIR-envelope
  and kernel-to-GPU services were removed; rustc owns the production semantic
  MIR through canonical KIR transaction.
- Existing AMDGPU lowering is owned by `fe2o3-amdgcn-model` and re-exported by
  the historical `dialect-amdgcn` facade. A production `gpu.*` to `amdgcn.*`
  Pliron route has not landed; the implemented scalar dialect slice is not
  that general route.
- The graph pins dialect-only `pliron-llvm` with `default-features = false`.
  The closed gfx942 General GEMM profile now has live graph-derived V2 export,
  deterministic LLVM assembly, exact LLVM/LLD build-policy admission, and
  retained graph-to-post-link owners for both schedules. General GEMM MIR
  analysis also enforces one aggregate 512-call and 32-terminal budget before
  the positive boundary. Finalization freshly
  derives distinct graph, Worker V2, and finalized-machine axes from the
  retained concrete owners. Production remains fail-closed until the #174
  authenticated MIR-to-KIR receipt and the rustc-owned final authority join
  consume this #173 structural and late-machine route.
- `fe2o3-service-host` consumes the service and host models through
  authority-free, borrow-retaining typestates. It has no HSA/HIP handles and
  performs no allocation, publication, load, launch, execution, wait,
  persistence, proof, or storage release.

This checkpoint changes ownership, representation, and one exact scalar
execution slice, not parity status. The scalar slice performs measured MI300X
GPU execution, but it does not establish a general production compiler,
persistent service execution, proof promotion, or performance qualification.
The production-directed finalizer remains the isolated pinned upstream LLVM
22.1.8 target-machine and in-process LLD path, with no COMGR or shell GPU
linker. It remains the sole machine authority.

## Selective Pliron LLVM Target State

The integration permits `pliron-llvm` only in the Pliron LLVM
dialect/lowering layer. Every use must set `default-features = false`; the
optional `llvm-sys` converter is excluded from the producer and from the
production worker. The dialect layer may own transient `llvm.*` construction,
transformation, and verification. It may not own LLVM code generation, object
emission, LLD linking, canonical identity, or evidence, and its printer output
is not the finalizer contract.

fe2o3 owns canonical Handoff V2, the live-graph extractor, deterministic bounded
LLVM-assembly serialization, and all stable stage receipts and evidence around
finalization. For the closed General GEMM profile, the live graph carries the
complete structural LLVM policy and a separately hashed bounded envelope
carries only non-graph inputs. Fresh owner-borrowing export binds graph,
assembly, and worker request admission before the graph owner is released. That
admission checks an exact LLVM/LLD build policy but does not authenticate worker
measurement, and it remains inert.

The historical measured scalar-add slice established one bounded backend-fixture result, not a general compiler route. Its dedicated Worker V2 join and one-shot consumer are now retired; current Rust kernels use the shared ranked-PLIRON, KIR, target-backend, and Worker V3 pipeline.

The successful MI300X run records
`evidence=69238ad704470649b9811b41cf0194bb392be8116a1b0618adb1dcbe7e1bbd4f`
with ROCr 1.18 runtime image
`7010eba894569c044749b71b63ff782080c4a91e19ff24d6dc93e857045ab37e`.
The compile-time checkout policy and self-consistent marker are not an external
signature or CI attestation. This exact fixture route makes no CUDA-Oxide
parity, general memory-safety, or race-freedom claim.
See [pliron-llvm-gfx942-coverage.md](pliron-llvm-gfx942-coverage.md) for the
pinned operation, attribute, metadata, exporter, and finalizer gap audit.

## Program Rules

1. Implement vertical slices. A feature includes frontend, IR, lowering,
   diagnostics, tests, and manifest changes where applicable.
2. Keep the existing elementwise emitter runnable until G1 passes all current
   examples through the new path.
3. Put shared schemas and interfaces behind versioned tests before parallel
   agents build against them.
4. Do not mark a parity row complete from source presence or compilation alone.
5. Keep raw APIs explicitly unsafe; safe APIs must derive ABI and launch facts
   from the artifact manifest.
6. Treat Verus proof results and compiler correctness as separate evidence.
7. Merge small changes with one primary owner. Avoid long-lived branches that
   each edit shared registry, dialect, or manifest files.
8. Unsupported semantics fail at compile time with a source span and call
   chain. No approximate lowering is accepted.

## Parallel Workstreams

These workstreams can be staffed by separate agents once their input contracts
are frozen.

| Lane | Owns | Depends on | First deliverable |
|:--|:--|:--|:--|
| A: Frontend | rustc driver, kernel metadata, mono-item/call-graph collection, layout extraction | Kernel metadata schema | One non-generic kernel and helper serialized as typed frontend fixtures |
| B: IR | `fe2o3-mir-model`, target-neutral dialects, verifiers, mem2reg, canonicalization | Versioned model and IR schemas | Round-trip and verifier tests for control flow, memory, and GPU ops |
| C: AMD backend | `fe2o3-amdgcn-model`, future `gpu.*` legalization, LLVM export, OCML/OCKL, HSACO finalization | `gpu.*` contracts, target capability schema | General vecadd from IR fixture to validated HSACO |
| D: Runtime/API | `fe2o3-host-api`, artifacts, ABI, generated modules, prepared launches, buffers, streams, events, async operations | Manifest v1 | Safe typed vecadd launch with retained lifetimes |
| E: Verification | contracts, abstract model, Verus harness, proof policy, proof manifest | Launch/index/memory schema | Verified map kernel with exact proof binding |
| F: Quality | parity status generator, compile tests, differential runner/fuzzer, hardware CI, sanitizer/debug jobs | Stable command/test interfaces | Reproducible baseline dashboard and current-example regression suite |
| G: Advanced AMD | atomics, LDS, waves, matrix/async operations, device linking | G1 compiler and capability interfaces | Target-gated LDS reduction and wave collective suites |

An agent owns a lane's implementation files during a milestone. Shared schema
files have a designated integrator. Other agents propose schema changes as
small patches with fixture updates, then rebase after the integrator merges
them.

## Shared Interfaces to Freeze Early

Parallel work is effective only after these contracts have golden fixtures:

1. Kernel metadata emitted by macros.
2. Frontend function/type/layout serialization.
3. `mir.*` and `gpu.*` textual forms and verifier rules.
4. Target capability registry and versioning policy.
5. Artifact manifest and bundle wire format.
6. Kernel ABI field and layout model.
7. Launch contract and `PreparedLaunch<K>` identity.
8. Proof record, assurance level, and executable semantic identity.
9. Diagnostic codes used by compile-fail tests.
10. Compiler request/result and stage-receipt contracts.
11. Host operation and persistent-service transition contracts.

Freeze means backward-compatible evolution through explicit version changes,
not permanent immutability.

## Row-softmax LLVM 22 release checkpoint

The fixed width-64 row-softmax compiler lane now has a two-commit release
protocol. Implementation Commit A contains the source-to-LLVM profile, direct
upstream LLVM target emission, in-process LLD linking, C++ and Rust post-link
inspection, hostile parser/metadata tests, and a fail-closed gate, but
deliberately no release manifest. Only a subsequent manifest-only Commit B may
pin Commit A and its tree plus the complete host-specific LLVM, device-library,
Cargo/rustc, source, sysroot, runtime-DSO, Worker, probe, and HSACO identities.
Every compliant B requires the caller to supply an independently reviewed
manifest digest; the checkout supplies no default.

The measured LLVM 22.1.8 contract is exact: one `gfx942:xnack-` COV6 kernel,
workgroup `[64, 1, 1]`, wave64, zero group/private segments, a 288-byte kernarg,
four explicit slice fields, nineteen hidden arguments, exact register/spill
counts, and exact optional-field presence or absence. C++ reconciles `SHT_NOTE`
and `PT_NOTE` metadata views, rejects conflicting descriptors, and consumes one
complete MessagePack object. Rust independently inspects the same emitted
profile. Release acceptance requires two fresh replays with identical outputs.

This closes a reproducible compiler/code-object checkpoint only. It is not
origin authentication, source-to-machine or Verus refinement, a memory-safety
or race-freedom proof, protected runtime authority, or a GPU result. The
separately accepted W0 boundary below does not upgrade this checkpoint or the
parity status.

## Rejected W0-B static-wrapper diagnostic

Candidate `2e5ad53bcb20f2a46e91128a42e838d918d61581` (tree
`892f014381cd3e34f81cb05df3b9bbda4a412478`) is rejected. It is not integrated,
public, or accepted. Its MI300X run passed structural and hostile
static-wrapper probes, reached `stage=binding-wrapper`, authenticated Cargo and
pinned rustc, loaded the backend, and collected the kernel. It then failed
closed before the release main phase with:

```text
backend has no cargo-fe2o3 executable identity for broker authentication
```

The Worker executed zero times. The run reached no artifact admission, GPU
loading or dispatch, or `/dev/kfd` or `/dev/dri` access, and opened COMGR zero
times. The direct GPU link path remains pinned upstream LLVM 22 plus in-process
`lld::lldMain`; it does not use COMGR or a shell GPU linker.

Code and security review found that the candidate invokes a dynamically linked
host `rust-lld` while authenticating only the executable. Its dynamic loader and
system DSOs, CRTs, archives and objects, search roots, and forwarded Cargo target
artifacts remain outside the authenticated closure. Clearing the environment is
not a substitute for authenticating and revalidating those inputs. The retained
run is diagnostic only: it is not signed, protected, or archived evidence; it
does not support parity, GPU, memory-safety, race-freedom, or source-to-machine
refinement claims.

## Accepted W0/P0 bounded host-link boundary

W0/P0 provides a descriptor-sealed static host-link boundary using
`fe2o3-host-lld` built from pinned upstream LLVM/LLD archives.
`HostLinkClosureV1` resolves and seals the admitted link inputs, binds the link
plan, launches the exact approved executable with `execveat`, and transfers the
result through a receiver-owned sealed output. Landlock enforces the filesystem
boundary, and seccomp denies network and descriptor-transfer operations.

Two fresh guarded MI300X builds produced the same 85,597,472-byte static tool
with SHA-256
`7c1a7429e93896393eb743ed54ead78ec6d492e3ed887183e67737b3872d7bf9`.
The registered `fe2o3-host-lld-secure-protocol-v2` CTest passed in a separate
execution, and a separate real `HostLinkClosureV1` slice linked through that
static tool successfully.

The build records are measured/no-authority evidence. W0 grants no protected
publication, broker or durable artifact handoff, runtime, load, launch, or GPU
authority or evidence. It is not a memory-safety or race-freedom proof and does
not establish source-to-machine or Verus-to-machine refinement. It closes a
prerequisite without promoting any cuda-oxide parity row. W1/P0 Broker V4 is the
next production blocker.

## Implemented Checkpoint: `90b6fe3`

The `90b6fe3` checkpoint establishes a bounded `gfx942` multi-kernel spine:

- one external Cargo fixture declares two kernel roots and one reachable shared
  helper;
- MIR import assigns the helper one canonical source identity, and Kernel IR
  lowering validates calls against the collected helper's exact signature;
- AMDGPU lowering is deterministic for two kernels, one shared helper, and
  shared OCML declarations;
- Worker V2 compiles the real Rust fixture into one independently inspected and
  durably published HSACO through the sealed Cargo backend path;
- the V1 artifact wire format carries exactly two canonically ordered kernel
  entries that reference one digest-validated native `gfx942` payload;
- each kernel has an independently keyed proof binding over its own ABI,
  effects, launch contract, source identity, and the shared executable;
- host admission can select two distinct compiler-generated kernel markers from
  the same authenticated executable without allowing marker, target, layout,
  payload, or executable substitution; and
- the reviewed HSA adapter can resolve and linearly retain a fixed set of
  distinct symbols while borrowing the loaded executable.

This is compilation, artifact, proof-binding, selection, and lifecycle
evidence. It is not general typed dispatch. The generated safe launch surface
and reviewed HSA argument initializer still implement only the exact vecadd
profile, and the second host selection is deliberately inert. No parity row is
promoted by this checkpoint.

## Implemented Alpha/Zeta gfx942 Checkpoint: `dc9738e`

The post-snapshot implementation through
`dc9738e367c392f7716eacb8459ca73fa32abbbb` completes the bounded alpha/zeta
compiler-to-HSACO path and exercises the exact artifact on MI300X:

- `#[kernel(typed)]` preserves the exact vecadd V2 expansion and emits an
  expectation-only V3 registration for bounded scalar, shared-slice, and
  `DisjointSlice` signatures outside that compatibility profile;
- rustc validates the V3 registration against semantic primitive types and
  genuine trusted `DisjointSlice<T, Index1D>` identities, then derives variable
  physical layouts and COV6 descriptors. The alpha and zeta fixtures are
  `40/296` and `56/312` explicit/complete kernarg bytes;
- rustc uses the authenticated logical and export names from kernel collection,
  together with the exact semantic signature, to select alpha/zeta source roles.
  Macro and compiler independently agree on names, layout, effects, binding,
  and host contract identity. Renames, logical/export disagreement, reordered
  arguments, and lookalike types retain positional fields and cannot acquire
  the exact generated adapter;
- the exact source forms lower through typed Kernel IR with trusted thread-index
  provenance, strict `gfx942` floating-point policy, and a dominance-checked
  `DisjointSlice::get_mut` `Some` edge. Escaped payloads, false or merged bounds
  edges, unsupported targets/pipelines, and untrusted call lookalikes fail
  closed;
- host argument binding validates canonical scalar and slice identities,
  retains allocation borrows in lifetime-branded packed values, checks the
  generated marker binding against the admitted Worker V3 descriptor before
  verification, and checks the complete generated layout before dispatch;
- checked shared and exclusive `DeviceBuffer` views preserve parent allocation
  and selected-region provenance, reject invalid ranges, and enforce exclusive
  parent borrowing;
- the macro emits signature-specific `Arguments`. General lookalikes remain
  inert; the exact authenticated alpha/zeta roles additionally receive an
  unsafe generated host-SPI implementation, safe `prepare` method, and linear
  prepared value whose synchronous `dispatch` retains arguments, alias and
  in-flight admission, selected kernel, loaded executable, and physical kernarg;
- generated slice capabilities accept checked immutable/exclusive subregions,
  preserve their parent allocation and allocation-relative byte interval, and
  carry that exact region into packing and alias admission. UI tests retain the
  allocation borrow and reject writable use of an immutable view;
- the witness wire contract, reserved symbols, parser, and rustc backend
  host-object emitter remain qualification-only. The genuine Worker V2 fixture
  emits, links, and validates deterministic private witness accessors for both
  alpha and zeta; production Worker V3 host compilation uses ordinary rustc and
  emits no witness accessor dependency;
- Worker V2 canonically finalizes descriptor-bearing COV6 before publication,
  preserves descriptor-free COV5 compatibility, and recovers exact raw and
  finalized publications across process crashes with legacy-marker migration;
- COV6 finalization reconciles explicit metadata sizes with complete descriptor
  sizes, and the direct LLVM worker canonicalizes every optimized COV5/6 kernel
  to the complete 256-byte implicit block after inference passes. Native
  descriptors, metadata, and host admission therefore agree; and
- native worker tests preserve two COV6 entries, both `.kd` symbols, and one
  shared helper. `.fe2o3.kd.v1` authentication, finalized-bundle admission,
  currentness leases, and the authenticated load state machine are implemented
  downstream;
- a fresh native Worker V2 build on `mi300x` passes all three CTests. This is
  native LLVM/LLD and COV6 boundary evidence, not GPU execution or an archived
  parity result;
- Cargo can deterministically assemble an exact alpha/zeta
  `ArtifactContainerV1` candidate from finalized COV6, descriptor, attempt,
  plan, and receipt evidence. It retains lineage absent from the V1 wire and
  deliberately grants no current-publication, load, or launch authority. At
  this checkpoint the Cargo adapter remained inert, had no
  container/serialization accessor, and was compiled only for tests; and
- the genuine Worker V2 integration publishes exactly alpha and zeta in one
  inspected COV6 HSACO and exports the bytes through a create-new evidence
  boundary. The resulting `gfx942` payload has SHA-256
  `3a916cdabca05ac74d340889aab2067221d6d1252a7cde13e61c1786252565c4`.
  The opt-in raw MI300X harness loads one executable and runs both kernels for
  lengths `1`, `255`, `256`, `257`, and `1023`; independent CPU oracles and
  all prefix/suffix canaries pass; and
- a now-retired second MI300X harness passed the same digest and matrix through
  generated checked slice capabilities, typed alpha/zeta selection and
  preparation, the reviewed load/resolve/dispatch/unload lifecycle, and safe
  `dispatch`. Its semantic witnesses and prerequisite authenticator are
  explicit test fixtures, so this is runtime-composition evidence only.

This historical checkpoint supplied exact-digest source, compiler, direct
LLVM/LLD, COV6, raw hardware, and generated-safe runtime-composition evidence.
Both host harnesses and their workload-specific adapters have since been
deleted. Follow-on work provides canonical durable
lease reacquisition, sealed finalizer intent, a bounded Worker V2 envelope,
Cargo publication and reconstruction of that inert envelope, the production V3
descriptor handoff, and one-history persistent multi-kernel proof admission.
The recovered Worker V2 host route is deleted. The handoff is not protected production
authority, the records are not production-bound to compiler origin and the
exact payload, and no production `WorkerV3VerifierV1` exists.
Therefore this is not production proof-authenticated safe
dispatch, no parity row is promoted solely by this checkpoint,
repository-wide CUDA-Oxide parity is not claimed, and Complete remains `0`.

## Retired MoE host alternatives

The host-side MoE V1/V2 bridges, generated adapters, denial boundary, exact
top-2 lifecycle, and workload-specific HSA launcher were never part of the
production application route. They duplicated pieces of argument admission,
resource observation, lifecycle ownership, and dispatch around Worker V3 and
have been removed.

The fixed attributed Rust kernels and the standalone 19-obligation Verus model
with seven negative mutations remain useful example and proof evidence. They
do not grant runtime authority.

The remaining MoE milestone is one Worker V3 vertical slice: collect and lower
the attributed kernels, publish one authenticated multi-kernel descriptor,
bind routing and expert arguments through the generic generated plan, validate
aliases and physical resources, dispatch through the common HSA lifecycle, and
join output evidence to the retained semantic oracle and proofs. No
workload-specific host or HSA route may be added to complete that milestone.
## Ordered Critical Milestones

These milestones are sequential authority gates. Work inside one milestone can
be parallelized, but a later gate must not manufacture evidence that assumes an
earlier authority transition.

1. **Accepted W0/P0: bounded authenticated host-link closure.** The dedicated
   static `fe2o3-host-lld`, descriptor-sealed `HostLinkClosureV1`, exact
   `execveat` launch, receiver-owned sealed output, and Landlock/seccomp boundary
   have passed the bounded evidence described above. Keep the GPU code-object
   path separate: pinned upstream LLVM 22.1.8 and in-process `lld::lldMain`,
   with no COMGR or shell GPU linker. W0 remains measured/no-authority and
   grants no publication, runtime, load, launch, or GPU authority.
2. **Next blocker W1/P0: Broker V4 executable identity and handoff.** Derive the
   `cargo-fe2o3` broker identity from the accepted command and host-link closure,
   bind the release request and completed host-link transcript, and consume the
   admitted output through one-shot authority transitions. Reject replacement,
   path aliasing, stale identity, replay, wrong invocation, and validation/exec
   races. Protected durable publication remains a dependent gate. Crossing W1
   alone grants no runtime, load, launch, or GPU authority.
3. **Implemented foundation: durable publication-lease reacquisition
   (`5ec6f6f`).** A canonical inert published claim and an API revalidate its
   receipt, complete plan, exact files,
   current generation, path identity, and lock before returning a fresh
   non-clone lease. Reject stale generations, mutation, replacement, and lock
   contention.
4. **Implemented foundation: sealed finalizer intent and raw/final snapshots
   (`15ac976`).** Publication-plan derivation is sealed behind
   `fe2o3-hsaco-finalize`; Cargo's duplicate domain hashes are removed, and
   exact raw and finalized snapshots survive crash recovery and migration.
5. **Implemented foundation: canonical Worker V2 load envelope (`a949518`,
   `7b01057`).** The bounded shared wire type retains the artifact container,
   bundle/proof index, direct-link evidence, descriptor
   lineage, raw HSACO, finalized payload identity, and published claim. A lease
   is process-local authority and must never be serialized.
6. **Implemented inert W2/P0 foundation: Cargo envelope publication.** The
   adapter is compiled outside tests and durably publishes and reconstructs the
   canonical envelope from sealed lineage before completing the build attempt.
   This was implemented ahead of W1 authority and deliberately grants none.
7. **Implemented production foundation: Worker V3 application handoff.** The
   host reacquires a fresh lease and rechecks the envelope, lineage, semantic and
   physical ABI, currentness, and marker facts. Cargo transfers only pinned
   read-only descriptors. The separate recovered Worker V2 host admission and
   launch bridge are deleted. The pure-KFD runtime now has one safe,
   invocation-specific authority transition, compiler-generated host-memory
   invocation adapter, and measured unsafe-diagnostic replay. The generated
   adapter and authenticated Worker V3 executable now join into one private,
   move-only KFD invocation, and an exact scalar-GEMM replay passes on MI300X
   only under the explicit synthetic-verifier test feature. Default builds
   seal verifier implementation and keep decision construction private. The
   canonical inherited-application API now
   derives the kernel from its generated type and consumes the Cargo handoff,
   verifier, generated arguments, current publication, and checked device into
   that invocation. A production verifier, generated-application adoption, and
   replay without external HSACO injection remain open. The default decision
   already requires and owns the exact V4 compiler proof inputs: five decoded
   stage preimages plus an independently imported signed aggregate
   MIR-to-live-PLIRON receipt. Protected compiler-currentness evidence is
   retained alongside it through load and dispatch. The 2026-08-31 singleton
   continuation adds a second move-only owner that strictly decodes target
   binding, AMDHSA data layout, and semantic-to-LLVM association, rederives the
   semantic layout identity, checks every receipt coordinate, replays exact
   KIR-to-LLVM lowering, and cross-binds final LLVM to both compiler handoff and
   independently reconstructed finalizer state. Promotion also binds COV6 and
   exact workgroup dimensions to the admitted descriptor. Multi-root target
   lineage, semantic preservation, LLVM-to-machine refinement, and
   dynamic-launch refinement remain separate open joins.
8. **Implemented bounded foundation: authenticated physical-machine bundle.**
   The sole supported `gfx942` LLVM Object/MC analysis path runs from a sealed
   worker image under an immutable measured runtime closure. One canonical
   response binds closed effect sites to a complete byte-exact instruction/CFG
   trace using payload file offsets. The inert trace analysis also derives
   bounded dominators, post-dominators, exact reaching definitions, and
   canonical natural loops with exit edges, while rejecting blocks that cannot
   reach an exit. The bounded EXEC-control layer additionally binds exact
   zero/nonzero EXEC branches to unique two-half reaching definitions, taken and
   fallthrough blocks, an immediate post-dominator candidate, scalar mask
   operands, and structurally matching saved-mask OR sites. These facts do not
   assign opcode semantics, establish hardware reconvergence, prove a mask empty,
   or establish termination. Production admission
   now has exact deterministic target-KIR-to-LLVM replay from the neutral KIR
   receipt, including byte equality with the retained LLVM. Production
   admission now also retains exact linked-module, optimized-module,
   generated-object, ordered native-input, path-independent LLD-invocation, and
   final-HSACO identities, independently recomputes the evidence identity and
   request/output-derived relations, and requires bootstrap/replay equality for
   every measured stage. Singleton Worker V3 admission now independently
   decodes target, data-layout, and semantic association records, checks every
   capsule receipt coordinate, owns exact KIR-to-LLVM replay, binds final LLVM
   to handoff and finalizer state, and rejects valid-but-foreign target-side
   splices. Multi-root target-binding custody remains open. It must still
   establish formal KIR-to-LLVM semantic
   preservation and LLVM-to-machine semantic
   refinement, then bind the machine receipt into Worker V3 runtime authority.
   Hardware success remains an independent evidence class.
9. **Implemented bounded foundation: alpha/zeta source proofs and proof
   records.** Mechanical source-model proofs, negative mutations, freshness,
   and executable-evidence records exist. They do not give Rust source an
   operational semantics, establish compiler/machine refinement, or bind
   production proof authority to the final payload.
10. **Production verification authentication.** Implement the crate-owned
   concrete `WorkerV3VerifierV1` behind the existing sealed boundary only from
   reviewed immutable compiler,
   Verus/solver, proof-to-executable, Rust-layout, and machine-effect records.
   Every digest, identity, mutation, and stale-replay edge must fail closed.
11. **Implemented API foundation: split mutable views.** Safe two-way and
   guarded three-way splits yield simultaneous non-overlapping mutable views
   while retaining parent identity and exact allocation-relative intervals.
   Unit and compile-fail coverage exists; mechanical Verus correspondence and
   general same-allocation MI300X execution remain open.
12. **Feature and architecture breadth.** Generalize beyond exact alpha/zeta only
   after the preceding authority/evidence gates: additional signatures and Rust
   semantics, core AMD operations, async/runtime behavior, then `gfx1151` and
   `gfx950` compile and hardware lanes. Every capability needs target gating,
   negative tests, differential oracles, and evidence scoped to its exact
   architecture.

### Parallel delivery shape

The claim, finalizer-intent, envelope-schema, persistent proof-set, Cargo
publication, cooperative handoff, and recovered-host foundations landed
independently and are now composed as an inert path. Protected application
handoff, production prerequisite authentication, and proof/effect admission are
the remaining authority integration gates and can progress in parallel against
the frozen wire and claim APIs.

The production authenticator must not be implemented from today's descriptive
digests. Its independent evidence lanes are:

- an implemented non-`Clone`, persistently fresh multi-kernel proof-set
  foundation (`2241cd7`, hardened by `f6efb26`) that requires one exact local
  ledger history and honest separation of safe-dispatch properties from full
  IEEE-754 functional correctness, but does not provide rollback resistance;
- a canonical proof-input/dependency capsule and reviewed Verus/solver recorder
  that derives properties from actual sealed execution;
- an authenticated compiler transaction binding source closure, rustc/backend
  invocation, semantic witnesses, Kernel IR, Worker response, and final HSACO;
- a finalized workload-neutral `gfx942` LLVM Object/MC analyzer whose
  authenticated bundle joins exact static effects and instruction/CFG facts;
- an evidence join plus rollback anchor that grants no authority until every
  identity, currentness, proof, compiler, ABI, and machine-effect edge agrees.

These lanes can execute concurrently after their shared identity records are
frozen. The first sound production profile is a measured trusted-toolchain
derivation for exact `gfx942:xnack-` COV6 alpha/zeta, not a claim that Verus
proves compiler-to-machine-code refinement. The sole production unsafe
authenticator implementation is the final integration PR and accepts no
caller-supplied evidence digest.

## G0: Baseline and Safety Boundary

### Objectives

- Turn the parity document into machine-checkable data or a checked generated
  view without losing the pinned 94-row baseline.
- Record current examples, expected artifacts, compiler toolchain, and hardware
  targets as regression fixtures.
- Make low-level launch APIs and arbitrary argument packing explicitly unsafe.
- Define kernel metadata, capability, launch contract, ABI, artifact, and proof
  schema version 1.
- Establish CPU-only CI and named CDNA/RDNA hardware queues.

### Parallel assignments

- Lane A replaces kernel name substring discovery with generated metadata while
  retaining compatibility tests.
- Lane D introduces unsafe raw launch methods and a minimal kernel brand plus
  `PreparedLaunch<K>` skeleton.
- Lane F ports current examples into a manifest-driven smoke list and adds
  compile-fail infrastructure.
- Lane E builds an erased-contract compatibility spike that compiles the same
  tiny kernel under Verus and ordinary rustc without asserting a proof-to-code
  theorem.

### Exit gate

G0 passes when:

- every current fe2o3 example still builds through the old emitter;
- raw launch and raw pointer packing require an explicit unsafe call site;
- a safe prepared vecadd launch rejects wrong rank, wrong kernel brand, wrong
  context, and insufficient resource declarations before HIP launch;
- metadata and manifest v1 have round-trip, malformed-input, and unknown-field
  tests;
- CI verifies that changes outside approved generated directories do not
  rewrite golden schemas;
- the pinned cuda-oxide and fe2o3 commits are shown by the parity status tool.

## G1: General Compiler Spine

### Objectives

- Add the explicit device extraction driver while keeping a compatibility
  adapter in `rustc-codegen-fe2o3`.
- Import Stable MIR through `rustc_public` into typed `mir.*` operations.
- Add IR structural/type verification, memory-form translation, mem2reg, and
  basic canonicalization.
- Lower baseline control flow, calls, arithmetic, comparisons, loads/stores,
  slices, pointer offsets, and thread coordinates into `gpu.*` and AMDGPU LLVM.
- Emit validated HSACO without elementwise shape recognition.

### Required vertical slices

1. Scalar return helper.
2. Branching fill kernel.
3. Vecadd with three slice ABI values.
4. Stencil with multiple indexed loads.
5. Pipeline with two kernels and shared helpers.
6. Cross-block loop fixture that requires correct SSA promotion.

### Parallel assignments

- Lane A owns extraction and serialized frontend fixtures.
- Lane B imports fixtures and owns `mir.*`/`gpu.*` verifiers and passes.
- Lane C lowers checked fixtures to AMDGPU and finalizes HSACO.
- Lane F compares old/new outputs for every current example and runs negative IR
  fixtures against verifiers.

The lanes integrate through serialized fixtures before linking crates together.
This permits backend progress without a live rustc frontend and frontend
progress without AMD hardware.

### Exit gate

G1 passes when:

- every current example builds and executes through the new compiler path on
  one supported AMD machine;
- CPU-only tests cover every accepted `mir.*` and `gpu.*` operation;
- malformed dominance, types, address spaces, barriers, and capabilities fail
  verification;
- `cargo fe2o3 pipeline` shows each IR stage and `cargo fe2o3 inspect` shows the
  selected payload and metadata;
- the old and new paths agree on kernel ABI for current examples;
- the elementwise recognizer is disabled by default but retained temporarily as
  a differential oracle.

## G2: Rust Semantic Coverage

### Objectives

Implement the target-neutral language rows before advanced GPU features:

- rustc layout fidelity for structs, tuples, arrays, enums, variants, and ZSTs;
- constants, statics, promotions, and supported pointer relocations;
- generics, const generics, closures, function items, drop glue, and cross-crate
  calls;
- complete baseline control flow, iterators, matches, loops, break/continue, and
  supported unrolling;
- integer/float operations, checked operations, casts, pointer distance,
  volatile access, and bulk copy;
- device panic-as-trap and unsupported-call diagnostics.

### Test corpus

Port tests by semantic category from the pinned cuda-oxide checkout. Do not copy
CUDA-specific expected IR. Each test has one or more of:

- a frontend/IR golden fixture;
- a compile-pass or compile-fail assertion;
- a CPU reference and AMD execution comparison;
- layout bytes independently produced by host rustc;
- an LLVM/HSACO metadata assertion.

Priority order is control flow and calls, aggregates/layout, constants, then
closures and uncommon pointer cases. This order unblocks runtime ABI work while
the long tail continues.

### Exit gate

G2 passes when:

- Exact compiler rows 02-25 and 31-35 meet their pinned acceptance targets;
- a generic closure map, enum match, nested loop, padded struct array, const
  relocation, and cross-crate kernel execute correctly;
- all supported host/device layouts compare equal by size, alignment, field
  offsets, discriminant, and parameter bytes;
- unsupported `std`, allocation, unwind, dynamic dispatch, and relocation cases
  produce stable diagnostics with reachable call chains;
- no G2 test depends on the old recognizer.

## G3: Artifact, ABI, and Runtime Contract

### Objectives

- Make a versioned target-neutral artifact bundle the only source of entry,
  payload, ABI, launch, capability, and proof metadata.
- Generate typed module loaders and launch methods from kernel declarations and
  validate them against the finalized manifest.
- Implement structurally host-valid `DeviceCopy`, manifest-derived type/ABI,
  provenance, address-space, and capability gates, layout-safe buffers, pinned
  memory, events, and context ownership.
- Add lazy typed async operations with borrowed and owned forms.
- Retain resources through completion, cancellation, callback failure, and
  stream error paths.
- Add deterministic cache keys and local clean behavior.

### G3.1: General typed multi-kernel dispatch

This remains the next critical vertical slice. It replaces the exact vecadd-only
packing and dispatch bridge with one path generated from each admitted kernel
entry. Its normative scope and authority transitions are frozen in the
[general typed dispatch V1 contract](general-typed-dispatch-v1.md): by-value
scalars, shared slices, and exclusive `DisjointSlice` arguments already
represented by the bounded ABI model. Aggregates, return values, asynchronous
launch, and language coverage not yet accepted by G2 are not silently added
here.

Parallel ownership is split at frozen records:

| Slice | Owns | Produces |
|:--|:--|:--|
| G3.1-A: compiler ABI | rustc layout extraction, physical parameter expansion, effect/alias declarations | Canonical per-entry ABI descriptor fixtures |
| G3.1-B: artifact/module | multi-entry bundle validation, descriptor-to-payload binding, generated module declarations | One module descriptor with two independently typed kernel entries |
| G3.1-C: host packing | generated HSA and address-free KFD argument capabilities, checked offset/alignment writes, descriptor-derived fixups, retained borrows and KFD completion writeback | Kernel-specific packed arguments whose HSA/KFD storage routes cannot be exchanged |
| G3.1-D: HSA dispatch | multi-symbol resolution, reviewed COV6 hidden arguments, queue submission, completion, unload ordering | Generic synchronous dispatch for an admitted kernel descriptor |
| G3.1-E: adversarial tests | UI tests, mutation tests, CPU oracles, MI300X execution evidence | Reproducible positive and fail-closed evidence |

Progress at `dc9738e` recorded bounded raw and generated-safe Worker V2 test
vertical slices but did not close the production authority gate. Those host
harnesses are now deleted. G3.1-A has V3
registration, rustc-semantic
reconstruction, authenticated alpha/zeta role and ABI naming, guarded typed
lowering, exact descriptors, and linked backend witnesses. G3.1-C has checked
buffer views, allocation-relative capabilities, generic Worker V3 `Arguments`,
safe preparation, retained packing/alias lifetimes, and linear synchronous
dispatch. Versioned compiler transaction records supply canonical COV6 publication and restart recovery to
G3.1-B; explicit/complete kernarg reconciliation and post-optimization implicit
ABI canonicalization now permit the genuine two-entry build to publish.
G3.1-E records the exact SHA-256 and passing raw and generated-safe MI300X runs
of alpha and zeta at lengths `1`, `255`, `256`, `257`, and `1023`, including
independent oracles and canaries. The safe run uses an explicitly fake
prerequisite authenticator and test-only witnesses.

Durable publication, Worker V3 verification admission, currentness lease
revalidation, the generic authenticated load/dispatch state machine, the
reviewed runtime adapter, durable lease reacquisition, Cargo
publication of the canonical load envelope, V3-only production descriptor
handoff, and safe split mutable views already exist. The recovered Worker V2
host route is deleted. Default builds now seal `WorkerV3VerifierV1`, keep its
decision constructor private, and omit the synthetic implementation surface.
Still missing is the crate-owned concrete verifier that can independently
authenticate the carried evidence before invocation-specific pure-KFD launch
custody is created. The protected compiler-execution service now provides the
policy and exact Worker-ledger part through one terminal transaction: it
reacquires the canonical record, byte-compares the complete carried receipt,
and returns a pinned-key signature over authority-free canonical verification
evidence and a fresh challenge. Cargo now provisions the application endpoint
through the fixed supervisor before ACK, and a one-use host auditor consumes
the endpoint and checks the exact signed response without granting authority.
The concrete host verifier must still join protected key custody, the external
monotonic anchor, and owned compiler, KIR, Verus, machine, and launch receipts.
The runtime authority gate and compiler-generated
host-memory KFD preparation now bind the exact current HSACO, geometry, effects,
checked device, and authenticated decision into one private move-only
invocation. The existing hardware proof uses a synthetic verifier and the
canonical inherited KFD transition is not yet exercised by an ordinary
generated application process. Recovered admission and verifier requests are
now device-independent. The temporary HSA route receives and retains HIP
observation only at HSA authorization, while the KFD route consumes a checked
device only when it creates joined invocation authority.
Bounded machine-effect and Verus proof records are not production-bound to
compiler origin and the exact artifact. These are the ordered critical
milestones above; feature and architecture breadth follows them.

The compiler ABI descriptor is the integration boundary. Runtime code may
compare an untrusted manifest with a compiler-generated descriptor, but it may
not synthesize a safe Rust argument interface from manifest bytes alone. Each
packed argument value is branded by kernel, executable, context, and descriptor
identity and retains all referenced resources until quiescence.

G3.1 passes only when all of the following are true:

1. One ordinary Cargo project declares two kernels with different nontrivial
   signatures and one shared Rust helper; the sealed backend emits one `gfx942`
   HSACO containing exactly both entries and one helper definition.
2. The backend emits canonical ordered physical fields, offsets, sizes,
   alignments, address spaces, mutability/effects, launch contract, target, and
   code-object identity for each entry. Repeated clean builds are byte-identical.
3. One V1 bundle references the shared payload from both entries, and
   independent HSACO inspection matches each entry to exactly one descriptor.
4. Generated host declarations expose distinct safe argument and prepared
   launch types for both kernels. No kernel name, signature, offset, or byte
   count is special-cased in `fe2o3-host` or `fe2o3-hsa-runtime`.
5. Safe packing writes every explicit kernarg field from its manifest-derived
   descriptor, preserves resource borrows and alias classes, initializes
   padding deterministically, and rejects arithmetic overflow.
6. One loaded HSA executable resolves both symbols. Each typed selection can be
   prepared and synchronously dispatched through the same generic path, and
   the executable cannot unload while either selection, packed arguments,
   launch authorization, or submitted dispatch remains live.
7. An MI300X runs both kernels from that one executable and compares all output
   bytes with independent CPU oracles for empty/rejected, single-element,
   boundary, and multi-workgroup lengths. The evidence records `gfx942`, ROCm,
   LLVM worker, rustc, and commit identities.
8. Negative tests reject swapped argument order or type, changed physical
   layout, wrong symbol or kernel marker, target/context/executable
   substitution, stale payload, changed effects or launch contract, duplicate
   HSA symbols or kernel objects, cross-kernel proof substitution, alias
   violations, and unload-before-quiescence.
9. CPU-only unit tests, compile-fail tests, package tests, strict Clippy, the
   ignored Worker V2 integration test, and the MI300X execution test all pass
   from the same commit with commands recorded in [testing.md](testing.md).
10. The exact vecadd public API either uses the new descriptor-driven path or
    remains explicitly marked as a compatibility profile. Async operations,
    cross-crate finalization, and broader G2 aggregate support remain separate
    gates and are not implied by G3.1.

Current-head convergence has completed the macro and host-API portion of item
10. Ordinary exact vecadd now emits the same Worker V3 expectation and
`Arguments` surface as every other supported typed signature. Its former V2
registration, embedded artifact contract, and generated `Kernel`/`Prepared`
types exist only behind explicit qualification features. Production execution
still fails closed until the Worker V3 application verifier and runtime
authorization join are complete.

Runtime groundwork now also includes bounded model-only registries for
multiple in-flight queue operations and host-visible/device-local transfers.
They retain exact queue, dispatch, completion, executable, and mapping
identities through completion or quarantine; reject allocation aliases and
cross-queue conflicts; and release storage only after explicit visibility
consumption. A gfx942 private-segment admission model binds post-link metadata
and launch geometry to queue-owned scratch storage. Its capacity values remain
explicit policy inputs rather than authenticated hardware facts, and native
allocation, copy, submission, polling, and currentness refinement remain open.

The debugger/profiler inspection lane can now decode a bounded canonical
source-to-ISA observation collection through `cargo fe2o3 inspect`, including
typed missing and unavailable outcomes. This is deterministic evidence
inspection only; live debugger control, profiler collection, and semantic
source/ISA authority remain separate qualification gates.

### Safety tests

Compile-fail tests must cover:

- constructing or mutating a kernel brand;
- wrong launch rank, block shape, context, or kernel identity;
- passing the wrong argument type/order;
- mutable aliasing between arguments;
- freeing, moving, or mutating a borrowed buffer while work may execute;
- dropping a submitted future before completion;
- using a stale proof or artifact record;
- treating arbitrary bytes as a valid bundle.

Fault-injection tests cover allocation, launch, event, callback, and
synchronization failures. The failure policy prefers a bounded leak over
freeing storage still reachable by the GPU.

### Exit gate

G3 passes when:

- all safe launches use manifest-derived typed arguments and prepared geometry;
- raw sync and async launches require `unsafe` and list complete obligations;
- multi-kernel and cross-crate artifacts are embedded and found without
  filename sidecar conventions;
- async borrowed and owned pipelines pass Miri-compatible host logic tests and
  AMD execution tests;
- cancellation/failure tests demonstrate that in-flight buffers are not freed;
- artifact parsing is fuzzed and version/unknown-capability rejection is
  covered;
- Exact runtime rows 48-51 and 78-81 plus S01-S05 meet their targets, excluding
  Verus-specific proof claims reserved for G5.

## G4: Core AMD GPU Model

### Objectives

- Complete 3D workitem/workgroup/grid operations.
- Implement static/dynamic LDS, workgroup barriers, scopes, and fences.
- Implement integer and supported floating atomics for workgroup, device, and
  system scopes.
- Implement wave32/wave64 lane, vote, shuffle, match, reduction, and scan
  operations.
- Implement workgroup reductions/scans independent of wave width.
- Add OCML/OCKL math, half/BF16 types, debug print/assert/trap, and launch
  bounds.
- Run divergence, effect, address-space, and barrier validation before AMD
  lowering.

### Target policy

Portable subgroup tests run for every supported wave width. A target-specific
test names its required architecture/capabilities and is skipped only with a
machine-readable reason. A successful fallback must satisfy the same semantics;
otherwise compilation fails.

### Exit gate

G4 passes when:

- map, 2D stencil, tiled transpose, workgroup reduction, wave reduction, atomic
  histogram, and math suites pass on the required target matrix;
- LDS allocation/alignment and launch metadata are inspected in the code
  object;
- atomics pass ordering/scope litmus tests and system atomics reject ineligible
  allocations;
- no portable test assumes a wave width of 32;
- rows 40, 53-58, 60-73, 75-77, and 82 meet their non-Verus acceptance targets.

## G5: Verus V1 and Safe Data Parallelism

G5 runs in parallel with G2-G4 after G0 schemas stabilize. It does not wait for
advanced GPU operations.

### Objectives

- Formalize launch domains, allocation provenance, index spaces, views, and
  per-thread effects in Verus.
- Implement branded `ThreadIndex`, `DisjointSlice`, and proof-carrying static
  views.
- Prove bounds, address overflow freedom, initialization, injective writes,
  race freedom, and functional postconditions for independent-thread kernels.
- Emit proof manifests and bind them to executable semantic identity and launch
  contracts.
- Add `cargo fe2o3 verify` and `build --require-proof`.

### Required verified kernels

- fill and copy;
- vecadd/map/zip;
- affine gather;
- out-of-place stencil with halo guards;
- injective transpose or permutation;
- generic pure helper composition.

Each kernel needs a negative mutation that Verus rejects, such as an omitted
bounds guard, zero-stride output, aliasing contract violation, or incorrect
postcondition.

### Exit gate

G5 passes when:

- the required kernels have complete proof records under the approved axiom
  policy;
- stale source, dependency, feature, contract, model, and tool-version records
  are rejected;
- `--require-proof` never silently downgrades to `Checked`;
- runtime output clearly states the source-level trust assumption;
- compile-fail tests prevent index witness transfer, copying, scope escape, and
  index-space mismatch;
- rows 48-51 and 79 meet their Verus acceptance targets.

## G6: Interop and Advanced AMD Capabilities

### Objectives

- Add AMDGPU bitcode/relocatable device linking through a pinned worker that
  calls LLVM and LLD library APIs directly, plus bidirectional device FFI.
- Keep the LLVM worker out of rustc's process, use pinned upstream LLVM 22.1.8
  for parse/link/optimize/codegen/native link, and do not use COMGR. Selective
  `pliron-llvm` remains a dialect-only producer of fe2o3 canonical handoff data
  and evidence, never a machine-code authority.
- Support standalone device exports and external libraries through reviewed ABI
  and effect contracts.
- Add cooperative grid launch where HIP and hardware support it.
- Add target-gated split barriers and asynchronous global-to-LDS operations
  using AMD semantics.
- Add MFMA/WMMA, FP8, supported microscaling types, LDS swizzles, and matrix
  load/store helpers.
- Add VMM, peer access, coherent shared-memory capabilities, and multi-device
  runtime tests.
- Implement AMD inline assembly boundaries and source-level debug metadata.

CUDA cluster DSMEM, cluster launch, and TMA remain N/A unless a future AMD
target provides a semantic equivalent. Native AMD extensions get separate
matrix entries rather than being mislabeled TMA.

### Exit gate

G6 passes when:

- Rust calls one external AMDGPU device function and external device code calls
  one exported Rust function;
- a cooperative kernel validates launch capability and executes a grid-wide
  synchronization test on supported hardware;
- representative MFMA/WMMA and async-copy pipelines pass numerical, ISA, and
  resource tests;
- unsupported target combinations fail during capability legalization;
- AMD-equivalent rows 01, 26-30, 39, 47, 62, 64, 74, 87-88 and supplemental
  S02, S06, S12-S13 meet their declared scope;
- N/A rows 59, 63, 83-86 still reject rather than approximate CUDA semantics.

## G7: Verus V2-V4

### Objectives

- Add workgroup synchronization epochs and LDS initialization transfer.
- Prove barrier convergence and compatible dynamic barrier order.
- Add atomic invariants, scope reasoning, and linearization points.
- Add subgroup active-lane and wave-width-parametric proofs.
- Verify async-copy/barrier protocols and host operation dependencies where a
  reviewed primitive model exists.
- Distinguish source proofs from trusted contracts for external libraries,
  inline assembly, and matrix instructions.

### Required verified kernels

- tiled transpose using LDS;
- tree reduction with workgroup barriers;
- one scoped atomic counter or histogram;
- one wave collective parametric over supported width;
- one asynchronous copy pipeline, only if its primitive model is approved.

### Exit gate

G7 passes when:

- each required kernel proves safety and its stated functional invariant;
- divergent or misordered barrier mutations are rejected;
- wrong atomic scope/order mutations are rejected or require explicit unsafe
  assumptions;
- proof manifests list every trusted intrinsic/library contract;
- rows 52-58, 61-74 and advanced supplemental rows report honest per-property
  verification status.

## G8: Hardening and Parity Release

### Objectives

- Run differential fuzzing across MIR import, optimization, lowering, and AMD
  execution.
- Integrate available GPU memory, initialization, race, and synchronization
  checking tools through `cargo fe2o3 sanitize`.
- Generalize the bounded alpha/O0 ROCgdb pilot to supported kernels,
  optimization modes, and aggregate local/argument inspection, then admit it
  through protected production-v2 evidence.
- Validate release behavior on supported RDNA and CDNA families.
- Compare representative kernels with equivalent HIP C++ and relevant ROCm
  libraries for correctness, generated code, and performance.
- Generate parity and verification dashboards only from archived test evidence.

### Release evidence

Archive:

- fe2o3 and pinned baseline commits;
- rustc, Verus, solver, LLVM, ROCm, driver, firmware, and hardware identities;
- matrix row status with links to tests and logs;
- sanitizer/debugger results and known tool gaps;
- correctness and performance results with commands and datasets;
- approved N/A reviews and parity exceptions;
- trusted axiom, FFI, inline assembly, and external library lists.

### Exit gate

G8 passes when the parity release rule in
[cuda-oxide-parity-matrix.md](cuda-oxide-parity-matrix.md) is satisfied, the
current recognizer has no remaining default users, and removal of migration
code does not reduce archived coverage.

## Dependency Graph

```text
G0 contracts and safety
 |\
 | +-----------------------> G5 Verus V1 --------+
 v                                               |
G1 compiler spine --> G2 Rust semantics --> G3 runtime/artifacts
        |                    |                    |
        +--------------------+------> G4 core AMD+
                                             |   |
                                             v   v
                                            G6  G7
                                             \   /
                                              G8
```

G2 and G3 overlap after layout and ABI schemas stabilize. G4 begins from IR
fixtures before all G2 language features are complete. G5 begins from contract
fixtures after G0 and integrates concrete compiler identities as G1-G3 land.

## Integration Cadence

For each milestone:

1. The integrator publishes schema fixtures and affected matrix IDs.
2. Lane agents implement against fixtures in disjoint ownership areas.
3. Each lane lands unit tests before cross-lane wiring.
4. A vertical integration patch wires frontend to IR to backend/runtime.
5. Lane F adds hardware and negative tests and updates machine-readable status.
6. The gate owner records evidence; documentation status changes only from that
   evidence.

Changes to shared schemas require a version bump or a backward-compatible
reader, all fixture updates, and approval from every consuming lane owner.

## Pull Request Contract

Every implementation pull request states:

- owned parity row IDs and gate;
- layer boundaries changed;
- new or changed unsafe/trusted obligations;
- test evidence by CPU-only, compile-fail, hardware, sanitizer, and proof
  category;
- manifest or IR version impact;
- target capability and N/A behavior;
- migration-path impact.

A pull request should normally complete one narrow vertical behavior. Bulk
mechanical generated intrinsic updates are isolated from handwritten semantic
changes.

## Completion Criteria

The architecture program is complete when:

- all parity release requirements pass;
- fe2o3 host compilation no longer depends on a custom codegen backend;
- the general IR pipeline owns all supported kernels;
- ABI and launch APIs derive from versioned bundles;
- assurance levels are mechanically bound and honestly reported;
- V1 verified data-parallel kernels are stable, with V2-V4 status explicit;
- every remaining unsafe and trusted boundary is documented and tested;
- AMD-specific capabilities are named and gated instead of presented as CUDA
  features or universal GPU behavior.
