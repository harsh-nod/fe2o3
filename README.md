# fe2o3

`fe2o3` is an experimental single-source Rust GPU stack for AMD GPUs.

The current architecture keeps the working AMD runtime while incrementally
replacing the elementwise MIR recognizer with a target-neutral compiler
pipeline and adding source-level Verus contracts. The general form remains
incomplete, while bounded `gfx942` vertical slices exercise the compiler,
artifact, runtime, and proof boundaries described below. See the
[living v2 architecture](docs/architecture-v2.md),
[production compiler convergence design](docs/production-pipeline-convergence-v1.md),
[gfx942 production LDS reduction](docs/gfx942-production-lds-reduction-v1.md),
[workspace ownership policy](docs/workspace-layers-and-ownership.md),
[Pliron Wave 0 architecture](docs/pliron-wave0-architecture.md),
[cuda-oxide parity matrix](docs/cuda-oxide-parity-matrix.md),
[evidence-backed parity dashboard](docs/generated/cuda-oxide-parity-dashboard.md),
[verification model](docs/verification-model.md),
[GPU safety contract v1](docs/gpu-safety-contract-v1.md), and
[implementation roadmap](docs/implementation-roadmap-v2.md). The
[testing guide](docs/testing.md) defines the generic, Verus, ROCm compile, and
hardware execution lanes.

Feature-free production has one compiler transaction and no pipeline selector.
`cargo fe2o3 build` and `cargo fe2o3 run` first build the selected crate graph
for the fixed AMDGPU target through fe2o3, commit the exact device artifact
generation, and then build or run the same selection for the pinned host target
with ordinary rustc. Callers do not pass `--target`. Remaining V1/V2/V3 names
identify frozen records and protocols, not implementations. The target
architecture has exactly one executable compiler, publication, load, and launch
route. Temporary qualification implementations are deletion backlog: their
useful evidence must move to differential fixtures or offline tools, after
which their features and code are removed rather than retained as alternate
in-tree pipelines. Default `cargo-fe2o3` tests now compile the same production
route as the feature-free binary. The backend is feature-invariant in normal
builds: the remaining qualification feature can compile only inert unit-test
fixtures, and neither `cfg(test)` nor an environment variable can select an
alternate rustc route.

The feature-independent extraction driver now enters the same production
typestate transaction under a real `amdgcn-amd-amdhsa` rustc session. Its
analysis-only custody has passed collection, dynamic ranked-bounds, reference
binding, and loop-carried BF16/F32 MFMA GEMM lanes on gfx942 through semantic
MIR, ranked PLIRON, verified Kernel IR, composed memory checks, and
deterministic LLVM extraction. It cannot publish, finalize HSACO, load, or
launch, so this checkpoint changes no parity status.

The production workgroup slice now goes farther through that same transaction.
The ordinary attributed `lds_publish_read_reduce_i32_v1` Rust kernel retains
its exact `64x1x1` launch and 256-byte static-LDS contract through semantic MIR,
ranked PLIRON, verified Kernel IR, composed memory admission, upstream-LLVM
AMDGPU lowering, a compiler-bound handoff, the measured target-machine and
in-process LLD worker, and COV6 descriptor inspection. Deterministic replay
produces identical HSACO bytes with an exact 256-byte static-LDS contract. This
route selects no workload profile and uses neither COMGR nor a shell linker. It
does not grant load or launch authority; current executable evidence must enter
through the Worker V3 application, production verifier, and KFD runtime.

## CUDA-Oxide status

Against the pinned cuda-oxide baseline, the evidence ledger currently records
`0 Complete / 82 Partial / 0 Missing / 12 N/A` normative rows and
`0 Complete / 15 Partial / 0 Missing` supplemental rows. Zero Missing means
every in-scope row now has at least one bounded, tested implementation slice;
it does not mean that any row satisfies its full acceptance contract or that
fe2o3 has reached cuda-oxide parity.

The newest `gfx942` slices cover trusted memory operations, bounded closures
and control flow, cross-crate device roots, typed groups and collectives,
managed barriers and standard atomics, static proof-carrying tiles, launch
policies, FP8/MX and MFMA/LDS contracts, composite O0 debug metadata, and a
closed diagnostic/assembly surface. Device-library and tile interop are narrow:
the former demonstrates one directly linked OCML operation, and the latter one
BF16 XOR4 tile/stream contract. The bounded
[gfx942 wave/LDS V2 slice](docs/gfx942-wave-lds-v2.md) additionally carries one
masked `u32` wave64 reduction and one 256-thread static-LDS reduction through
exact compiler/LLVM checks, Verus proofs, and direct LLVM/LLD MI300X execution.
V2 admits only the full canonical `gfx942:xnack-` target, persists that binding
in Kernel IR, and checks it against the Worker V2 envelope. The older V2
compiler-created fixture remains separate, but the production `i32` WG64 LDS
reduction now joins genuine Rust source to reproducible inspected HSACO through
the one compiler transaction. Worker V3/KFD execution remains open. The
dashboard records the exact commits, tests, target lanes, evidence strengths,
and limitations for each Partial row.

The 2026-08-19 [#134](https://github.com/harsh-nod/fe2o3/issues/134)
checkpoint now includes additional fail-closed ownership boundaries. The rustc
frontend retains one non-cloneable, same-session typed MIR/CFG graph with exact
item, instance, source, MIR, ABI, import, and Pliron-graph identities; only the
return-only subset is admitted, and it grants no compiler authority. General
GEMM collection also enforces one aggregate 512-call and 32-trusted-terminal
budget. The closed
gfx942 General GEMM structural route retains its live Pliron LLVM graph,
compiler machine, Worker V2 execution owner, finalized bytes, and post-link
inspection for both schedules. Its late graph, worker, and finalizer axes are
freshly derived from those retained owners. Build-policy admission is not
worker-measurement authentication, and the axes grant no authority. Production
extraction now accepts one safe dynamic General GEMM and emits its deterministic
gfx942 LLVM through the general semantic pipeline. Protected General GEMM
publication remains disabled until #106 is consumed by the owner-carrying #174
receipt and the rustc-owned final authority join consumes that receipt together
with the #173 late-machine binding.
`pliron-llvm` has default features disabled, and no COMGR,
`llvm-sys`, or subprocess compiler/linker has artifact authority on this route.

The closure landed in `fd6520d88` (exact Worker machine effects), `70f9c5ad7`
(structural ELF, descriptor, and decoded-machine inspection), `e016833d3`
(measured-HSACO gate), `c9e8ca702` (move-only Worker execution evidence),
`62efd243e` (repository policy, finalizer join, and sealed one-shot HSA
consumer), and `228c88ed9` (descriptor-versus-runtime kernarg alignment). The
code target is exactly `gfx942:xnack-`; the qualifying MI300X reported
`gfx942:sramecc+:xnack-`. The repository pins Worker executable SHA-256
`12c06e0da5d812c1db6f33450f99a8d70087c585eec552f7f8616077704361fd`,
HSACO SHA-256
`011671a80384051232fb684c90afadd9b5e9d81c13d216238f15af55dd3880b1`,
and ROCr HSA 1.18 image SHA-256
`7010eba894569c044749b71b63ff782080c4a91e19ff24d6dc93e857045ab37e`.
The COV6 descriptor requires 280 kernarg bytes aligned to 8; the observed HSA
kernel requires the same 280 bytes in runtime storage aligned to 16.

The successful run consumed the finalized bytes through the sole typed,
move-only runtime transition, produced bit-exact `3.75f32`, preserved the input
and all allocation canaries, and reached terminal unload. Its
`FE2O3_REPOSITORY_SCALAR_ADD_V1_MI300X_OK` marker is a canonically serialized,
self-consistent record of the bounded policy, artifact, runtime image, device,
dispatch, result, canary, and unload observations. It is not a signature or CI
attestation; process-local runtime, agent, executable, dispatch, and kernarg
identities may differ between runs. Likewise, the compile-time checkout policy
is repository/build provenance, not an externally signed or separately
authenticated approval.
This checkpoint changes no CUDA-Oxide parity row or count and proves neither
general memory safety nor race freedom; it also does not establish general
GEMM, attention, or MoE support.

The Wave64 and workgroup-synchronization slices now start from ordinary
`#[kernel(typed)]` Rust sources rather than explanatory pseudocode. They include
deterministic CPU oracles, hostile source tests, and bounded Verus models for a
masked Wave64 reduction/scan and an LDS/barrier/scoped-atomic profile. The typed
device ABI preserves mutable global address-space pointers and exposes a linear,
compiler-only exact-LDS capability. The Wave64 compiler profile now authenticates
the exact attributed source, FnAbi, trusted definitions, complete reachable MIR,
mask semantics, ordered collectives, and output ownership before selecting a
closed semantic Kernel IR sidecar. The two workgroup profiles likewise
authenticate their exact source, ABI, trusted provider terminals, and complete
reachable MIR closures before selecting closed semantic profiles. Separate
configured finalizer tests use the pinned upstream LLVM target-machine and
in-process LLD worker, and separately scoped protected `gfx942` hardware lanes
exist. Those lanes remain ignored behind exact measured prerequisites and do
not establish source-to-machine refinement, production artifact or launch
authority, or generalized memory/race safety. Their evidence is tracked in
[#117](https://github.com/harsh-nod/fe2o3/issues/117) and
[#118](https://github.com/harsh-nod/fe2o3/issues/118).

The fixed 64-element row-softmax slice uses one shared numerical oracle and an
inert deterministic certificate that binds its exact Rust source, reviewed MIR
profile, Kernel IR and LLVM identities, numerical policy, and Verus/Z3 closure.
The exact compiler/finalizer lane still checks the direct upstream LLVM/LLD
worker exchange, OCML closure, artifact, descriptor, ABI, geometry, and resource
identities. Its workload-specific host token, typed HSA lifecycle, hardware
launcher, and Cargo `legacy-hsa-runtime` switch are deleted. Row-softmax can
return to hardware only through the generic Worker V3 application path.

The row profile also has a host-specific compiler/code-object release gate. By
protocol, implementation Commit A contains the gate but deliberately contains
no release manifest. Only a subsequent manifest-only Commit B directly above A
may select an independently reviewed SHA-256, pinned upstream LLVM 22.1.8
source and package closure, in-process LLD, the exact OCML/device-library
closure, Cargo/rustc and their offline source/sysroot closures, runtime DSOs,
Worker and layout-probe ELFs, and the retained HSACO. Both C++ and Rust require
the measured metadata exactly, including four explicit and nineteen hidden
arguments, presence or absence of
optional fields, register/spill values, resources, symbols, and target. Release
evidence can be claimed only when a compliant B and two runs from distinct fresh
build and Cargo directories reproduce the same caller-supplied manifest digest
and byte-identical outputs. That combination establishes only operator-selected
reviewed integrity, not origin authentication or GPU evidence.

W0/P0 is now accepted as a bounded host-link prerequisite. Its dedicated,
genuinely static `fe2o3-host-lld` is built from pinned upstream LLVM/LLD
archives. `HostLinkClosureV1` supplies descriptor-sealed inputs, launches the
exact approved executable with `execveat`, and returns a receiver-owned sealed
output. Landlock enforces the filesystem boundary, while seccomp denies network
and descriptor-transfer operations. Two fresh MI300X builds produced the same
85,597,472-byte tool with SHA-256
`7c1a7429e93896393eb743ed54ead78ec6d492e3ed887183e67737b3872d7bf9`.
The registered secure-protocol CTest and a real `HostLinkClosureV1` link slice
also passed in separate executions.

This build evidence is measured/no-authority. W0 provides no protected
publication, broker or durable artifact handoff, runtime, load, launch, or GPU
evidence. It proves neither memory safety nor race freedom and provides no
source-to-machine or Verus-to-machine refinement. W1/P0 Broker V4 is the next
production blocker. The parity counts remain `0/82/0/12` normative,
`0/15/0` supplemental, and `0/97/0/12` combined. The direct GPU link path
remains separate and pinned to upstream LLVM 22 with in-process `lld::lldMain`,
without COMGR or a shell GPU linker. The replacement Worker V3 row-softmax
slice remains tracked under [#120](https://github.com/harsh-nod/fe2o3/issues/120).
The subsequent fixed FlashAttention and top-2 MoE vertical slices are tracked by
[#122](https://github.com/harsh-nod/fe2o3/issues/122) through
[#125](https://github.com/harsh-nod/fe2o3/issues/125).

The exact [MoE expert-compute source slice](examples/moe_expert_v1/README.md)
extends the fixed T8/E4/K2/C4 router with two ordinary attributed Rust kernels:
one host-selected `16x16x16` BF16/F32 expert GEMM and one deterministic top-2
weighted combine. Its executable CPU schedule and independent oracle still
provide source/CPU evidence, while the original pinned Verus model verifies 15
logical expert-memory obligations and rejects six named mutations.

The [bounded MoE V1 checkpoint](docs/bounded-moe-v1.md) adds three separate,
narrower joins. The router's rustc admission now emits a private same-session
structural diagnostic over rustc-loaded source, the complete checked `FnAbi`
identity and bounded projection, full imported-MIR diagnostics, and a canonical
31-entry KIR/profile table. A separate inert `E4/C4/routes16/width16/tile256`
compact-plan model verifies 19 Verus obligations, rejects seven mutations, and
is differentially checked across all 625 valid count vectors. Neither is a
MIR-to-KIR refinement proof or an authority-bearing proof receipt.

The former MoE V1/V2 host bridges, generated adapters, exact top-2 lifecycle,
and workload-specific HSA launcher were non-production qualification
alternatives. They have been removed so MoE execution cannot bypass the sole
Worker V3 application, descriptor, argument, HSA, and unload lifecycle. The
ordinary Rust kernels, rustc/KIR diagnostics, compact-plan verifier and Verus
evidence, and independent source/oracle tests remain. MoE hardware execution
through Worker V3 is still pending and no parity promotion is claimed.
The public [tiled GEMM V1 work](examples/tiled_gemm_v1/README.md) now combines
the checked host contract with bounded production-directed LDS slices. An
ordinary `#[kernel(typed, ...)]` Rust function contains the fixed `16x16x16`
BF16/F32 algorithm. The compiler collector authenticates the exact attributed
root, reviewed portable MIR operations and ABI, derives two distinct aligned
512-byte LDS allocations, and consumes a single-use receipt to select the
verified canonical Slice 1 Kernel IR. This is bounded source correspondence,
not a general lowering or compiler-refinement proof. A separate identity-bound
Verus source model discharges 96 obligations for exact lengths, LDS
initialization, publish ordering, and unique output ownership; four hostile
length, barrier, ownership, and portable-MIR identity mutations are rejected.

The exact Slice 1 profile now continues through a sealed registry and direct
upstream LLVM finalizer, without COMGR or shelling out to `llc` or `ld.lld`.
The worker uses the LLVM target-machine and LLD library APIs, and the finalizer
admits only the canonical COV6 descriptor and metadata. A generated borrowed
host adapter then joins the exact finalized artifact to typed A/B/C regions.
The public one-shot lifecycle has private, non-`Clone` states
`Joined -> Loaded -> Completed -> Unloaded`; it exposes no finalized bytes,
native handles, raw kernarg, or generic launch bypass.

That path fixes the artifact target at `gfx942:xnack-`, accepts only a
compatible observed target, and rechecks grid `[1,1,1]`, workgroup `[64,1,1]`,
1,024 static LDS bytes, zero private and dynamic bytes, and a COV6 kernarg of 48
explicit plus 256 hidden bytes. An exact MI300X `gfx942` run passed all 256
output bits against the CPU reference, preserved A and B, and preserved every
prefix/suffix guard canary. It used upstream LLVM 22.1.8 build
`upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1` and worker
`fe2o3-worker-v1-sha256-6c3dfd5f784b3babe140006aba57a214a897b171860928440184fa201b6f96db`;
the success receipt reported finalizer
`078e9b523164b679ff7af3b4e819ad041713c53c6841399ac7cea95090f09774`
and unload
`df2f77ee798444a9e1fe5e27f219bdf720386eb8603a9a74fccc0df8efb3921c`.

The K32 follow-on has two-phase IR, inspected upstream-LLVM machine shape, and
a bounded Verus model for one through four K phases. Slice 3 adds an exact
`M=64,N=48,K=16` padded-stride IR and lowering with workgroup-X/Y machine
inspection. Slice 4 adds an exact `M=17,N=19,K=18` tail-safe, two-phase IR with
predicated accesses, unconditional barriers, and `alpha=2,beta=-1`, alongside
its bounded Verus edge model and inspected upstream-LLVM COV6 lowering.

This makes the exact bounded Slice 1 functional and measured, but it is not
production proof authority. The HSACO is identity-joined through the closed
profile and canonical re-lowering; compiler-origin authentication, production
Verus certificate consumption, and MIR/KIR/LLVM/ISA refinement are absent.
General illegal-memory and race proofs, general shapes, and protected Slice 3/4
execution also remain open. The older six-case LDS observation and
[direct-global observation](docs/tiled-gemm-v1-mi300x-observation.md) remain
separate evidence. No CUDA-Oxide parity row or count changes from this slice.

The source/IR groundwork landed under
[#85](https://github.com/harsh-nod/fe2o3/issues/85),
[#86](https://github.com/harsh-nod/fe2o3/issues/86), and
[#93](https://github.com/harsh-nod/fe2o3/issues/93). The shared integration
epic [#94](https://github.com/harsh-nod/fe2o3/issues/94) and its exact-profile,
finalizer, host-adapter, and lifecycle children
[#96](https://github.com/harsh-nod/fe2o3/issues/96),
[#97](https://github.com/harsh-nod/fe2o3/issues/97),
[#99](https://github.com/harsh-nod/fe2o3/issues/99), and
[#100](https://github.com/harsh-nod/fe2o3/issues/100) are closed. Production
certificate consumption [#91](https://github.com/harsh-nod/fe2o3/issues/91),
refinement [#106](https://github.com/harsh-nod/fe2o3/issues/106) and
[#107](https://github.com/harsh-nod/fe2o3/issues/107), and the other Slice 2-4
issues remain open.

The intended end state is:

```text
Rust host + #[kernel] device code
        |
        v
rustc frontend and MIR
        |
        +--> native host binary
        |
        +--> fe2o3 device backend -> AMDGPU LLVM IR -> HSACO
                                                |
                                                v
                                  typed HSA / HIP load/launch
```

## Architecture

The 2026-08-18 ownership refactor splits representation, compiler composition,
target lowering, and host execution into explicit ownership boundaries:

- Canonical contracts and models: `fe2o3-mir-model` owns the
  Pliron-independent MIR schema and transformations; `fe2o3-compiler-api`
  owns target-neutral compiler request/result contracts;
  `fe2o3-proof-contracts` owns solver-neutral property records;
  `fe2o3-host-api` owns inert compile/admit/load/dispatch/wait records; and
  `fe2o3-service-model` owns executable-free persistent-service semantics.
  These records validate representation and consistency. They do not prove a
  claim, compile a kernel, execute a service, or grant artifact/runtime
  authority.
- Compiler composition: `fe2o3-compiler-driver` executes one production
  request through one configured backend and has no selector or fallback slot.
  Inspection is an observation of that transaction rather than another
  compiler implementation. The production rustc backend likewise has no
  selector. Temporary non-publishing migration oracles compile only into the
  feature-enabled library test binary; no backend dylib can select them. The
  shard policy inventories selector-bearing integration targets as retired
  migration debt and prevents them from entering production CI.
- General kernel checks: `fe2o3-kernel-analysis` owns the fixed pre-lowering
  Kernel IR sequence for structure, control flow, bounds obligations, race
  freedom, barrier convergence, and workgroup-memory initialization/reuse.
  The production MIR-to-Kernel-IR boundary runs the sequence for every kernel
  and rejects concrete failures before transformation. `Incomplete` and
  `Clean` reports remain non-authoritative; see the
  [V1 pipeline contract](docs/general-kernel-check-pipeline-v1.md).
- Pliron framework: `fe2o3-pliron` is a bounded D0 context, registration,
  context-identity, pass-planning, and owner-held textual bridge over Pliron
  v0.17.0 at reviewed fork commit
  `5bdf861bf03e7f20242b25717fb653336d02e487`, a strict descendant of the
  upstream v0.17.0 commit `2610651306ea3ba670f68d5d8b1e1159bcd521ed`.
  The bridge recursively verifies imported operations and enforces bounded
  owner/session accounting, but arbitrary registered `Parsable` implementations
  remain trusted parser code and the bridge grants no compiler authority. Seven
  target-neutral representation shells exist for `kernel.*`, `schedule.*`, `tile.*`,
  `gpu.*`, `proof.*`, `dispatch.*`, and `autotune.*`. `dialect-mir` remains a
  compatibility facade over `fe2o3-mir-model` and additionally exposes a
  bounded `mir.*` Pliron shell only with its non-default `pliron` feature.
  These crates construct and verify in-memory representations; they do not
  form a production MIR-to-HSACO pipeline.
- Bridge and transformation shells: `fe2o3-kir-pliron-bridge` retains exact
  canonical KIR V1-V5 bytes in an opaque context-bound envelope and rejects any
  inconsistent, substituted, or foreign-context Pliron projection.
  `fe2o3-lower-mir-kernel` implements a narrow, terminally fail-closed
  `mir.*`-to-`kernel.*` service, and `fe2o3-lower-kernel-gpu`
  implements a bounded target-neutral `kernel.*`-to-`gpu.*` detached service.
  Their results are context-bound and stale handles fail with typed errors.
  The rustc integration now retains an owner-controlled typed MIR graph for a
  return-only subset; every other observed MIR operation rejects terminally.
  These services do not implement Pliron's in-tree `Pass` contract and are not
  a general Rust frontend, AMD lowering, artifact producer, or production route.
- Target model and facades: `fe2o3-amd-target` owns canonical AMD target
  contracts. The existing strict AMDGPU lowering implementation moved to
  `fe2o3-amdgcn-model`; `dialect-amdgcn` now preserves the historical crate API
  by re-exporting that model and is not yet an `amdgcn.*` Pliron dialect.
- Host and service boundaries: `fe2o3-core`, `fe2o3-host`,
  `fe2o3-hsa-runtime`, and `fe2o3-hip-sys` own the existing executable runtime.
  In contrast, `fe2o3-service-host` is an authority-free, `no_std` typestate
  adapter over `fe2o3-service-model` and `fe2o3-host-api`; it retains storage
  borrows and checks lifecycle descriptions but allocates, loads, launches,
  waits for, and executes nothing.
- Pure-Rust runtime foundation: `fe2o3-kfd-uapi`, `fe2o3-kfd`, and
  `fe2o3-runtime-model` provide reviewed KFD 1.18 encodings, fail-closed device
  observation, and Verus-backed lifecycle modeling. They do not yet replace
  the existing HIP/HSA execution path or establish persistent GPU execution.
- Artifact, build, proof, and evidence boundaries remain in
  `fe2o3-artifacts`, `fe2o3-kernel-descriptor`, `fe2o3-hsaco`,
  `fe2o3-hsaco-finalize`, `fe2o3-artifact-transaction`,
  `fe2o3-runtime-protocol`, `fe2o3-build-authority`,
  `fe2o3-host-link-closure`, `fe2o3-broker-authority-service`,
  `fe2o3-external-anchor-protocol`, `fe2o3-process-identity`,
  `fe2o3-protected-publisher`, `fe2o3-verifier`, and
  `fe2o3-differential`. `fe2o3-worker-v2-bundle` remains only for
  qualification-oracle and compatibility tests; it is absent from the normal
  feature-free dependency graphs of `cargo-fe2o3` and `fe2o3-host`.

The machine-checked layer policy forbids dependencies that invert these
ownership directions. The production-directed GPU finalizer continues to use
an isolated worker built against pinned upstream LLVM target-machine APIs and
in-process LLD library APIs. COMGR is not part of the architecture, and shell
`clang`/`ld.lld` use belongs only to the historical compatibility path.

[#134](https://github.com/harsh-nod/fe2o3/issues/134) and
[#135](https://github.com/harsh-nod/fe2o3/issues/135) are both still open. The
landed crates make parallel implementation possible and enforce representation
boundaries; they do not mean that the Pliron production compiler or persistent
GPU service exists. No parity row or count is promoted by this refactor.

Safe buffer element types and their limits are documented in the
[device memory safety contract](docs/device-memory-safety.md). `DeviceCopy`
establishes structural host-side byte validity only. Safe device interpretation
also requires manifest-derived type and ABI identity, provenance/address-space,
and capability evidence.
Safe ownership of resources used by asynchronous copies is documented in
[device operations](docs/device-operations.md).

## Current Status

### Working end to end

- `cargo-fe2o3 build` builds and loads the custom backend, delegates host
  codegen to `rustc_codegen_llvm`, discovers strict versioned registrations
  emitted by `#[kernel]`, collects device-reachable MIR, and emits HSACO
  sidecars. Registration identifies compiler semantics; it is not package or
  artifact authentication.
- For a public kernel, `#[kernel]` emits a public, doc-hidden marker with the
  deterministic symbol `__fe2o3_kernel_marker_<function>` and an unsafe
  `KernelMarkerV1` implementation tied to the exact Rust function type and V1
  registration. The marker does not authenticate an executable or establish
  its full packed ABI and semantics; generated binding remains an unsafe
  compiler/runtime boundary.
- `#[kernel(typed)]` emits a generic Worker V3 marker and typed argument plan.
  The generated surface retains slice lifetimes, mutability, aliasing, and
  canonical rustc-derived type/layout identities, but exposes no load or launch
  method and no embedded artifact bytes.
- The backend binds each kernel occurrence, target, descriptor, finalized
  payload, proof records, and generated argument contract into the canonical
  Worker V3 envelope. Production applications may dispatch only after the V3
  verifier authenticates that complete graph; examples without that verifier
  fail closed before runtime dispatch.
- Production compilation is the only unselected compiler route and never
  falls back to a workload-specific implementation. Historical emitters and
  exact workload paths remain only as migration evidence until equivalent
  production coverage permits their deletion.
- The feature-enabled library test binary exercises exact `fill`, `vecadd`,
  and other retained oracle fixtures through imported MIR, canonical
  target-neutral kernel IR, verification, exact-shape legalization, and G1
  AMDGPU lowering without exposing an executable compiler selector. The
  oracle, ABI, witness dataflow, bounds control flow, and accepted shapes are
  fail closed: invalid values and unsupported kernels remove stale generation
  artifacts and never fall back or acquire production publication authority.
- `fe2o3-core` provides HIP-backed contexts, streams, device buffers, pinned
  host buffers, events, synchronous transfers, and event-backed borrowed and
  owned asynchronous transfers. It exports no raw module, function, parameter
  pack, launch configuration, or launch function in any downstream build.
- Production applications load and dispatch only through the authenticated
  Worker V3 transaction, compiler-generated typed arguments, and reviewed HSA
  adapter. The former host `launch!` macro and the selectable raw-HIP core
  feature are deleted; raw HIP module/launch mechanics remain private unit-test
  implementation details.
- `DeviceCopy` and its derive macro restrict safe byte transfers to supported
  layouts and have compile-pass/compile-fail coverage.

Historical `gfx1151` and `gfx942` runs generated, inspected, and executed the
then-runnable qualification paths. The current hardware lane runs focused
runtime checks and fill/vecadd compiler qualification tests. Example binaries
do not provide production Worker V3 execution until their verifier integration
is complete, and the recorded runs grant no current production authority.

### Implemented foundations

- The structured MIR importer lowers the vecadd-shaped subset, including
  scalar control flow, helper calls, and slice memory operations, into the
  target-neutral `fe2o3-kernel-ir`. Its verifier checks types, SSA uses,
  control-flow edges, memory accesses, launch axes, capabilities, barriers, and
  atomics. The IR has a bounded canonical V1 wire format. The G1 lowering now
  owned by `fe2o3-amdgcn-model` and re-exported through the historical
  `dialect-amdgcn` facade lowers the verified 1D fill and vecadd subset to
  deterministic AMDGPU LLVM and is connected to the opt-in `kernel-ir-v1` fill
  and vecadd paths above; it is not yet general or the default. For its
  modeled effects, Kernel IR derives formal allocation identities, affine byte
  regions, bounds requirements, runtime-alias requirements, and
  inter-invocation race obligations. Unsupported index widths, arithmetic,
  calls, or memory effects make the analysis incomplete rather than silently
  granting authority. Even a complete result is conditional on an explicit
  launch extent; the extent and mappings from formal parameters to runtime
  allocations remain unauthenticated and grant no proof or launch authority.
- A bounded rustc-front record models canonical collected function signatures,
  source locations, and CFG edges, and the rustc backend can construct those
  records for monomorphized functions. Reducible-CFG analysis and
  block-argument-to-LLVM-phi lowering are implemented and tested, including
  loop-shaped Kernel IR. The production device pipeline still does not consume
  these records generally, and most Rust MIR operations remain absent.
- The G2 type foundation records validated semantic scalars, pointers,
  references, slices, tuples, arrays, structs, direct and niche enums, padding,
  and rustc ABI facts. A bounded rustc-private extractor obtains those facts
  for fully monomorphized types. Separate bounded foundations now model
  semantic constants/data relocations and manifest-driven scalar/slice packing,
  but neither is a general rustc-to-artifact integration. Dedicated fixtures
  make the current generic, const-generic, aggregate, integer-match, loop, and
  cross-crate collection/lowering frontiers explicit; generic registered kernel
  roots remain unsupported.
- Versioned artifact manifests, ABI layouts, launch contracts, bounded
  containers, payload digests, native-kernel selection, and proof records have
  canonical encoders, decoders, and adversarial tests. The bounded
  `Gfx942TwoKernelBundleV1` profile admits exactly two canonically ordered
  kernels backed by one digest-validated native payload and requires a separate
  proof binding for each kernel. Duplicate proof keys, shared-payload
  substitution, stale ABI/effect/launch identities, and cross-kernel proof
  swaps fail closed. These proof records remain descriptive evidence and grant
  neither load nor launch authority.
- G3 adds a canonical multi-kernel bundle index, validated compiler-generated
  argument-packing plans, and explicit asynchronous operation lifecycle
  records. These are bounded data and typestate foundations; no general
  manifest-to-host-code generator or composable cancellation API consumes all
  of them yet.
- Canonical AMD target IDs, HIP-observed device properties, HSACO metadata and
  descriptor inspection, kernel-descriptor binding, and bounded post-link
  finalization are implemented as separate validation layers.
- The G4 model includes capability tables for supported AMD targets, branded
  3D invocation and wave-lane witnesses, canonical Kernel IR for static and
  dynamic LDS, scoped atomics, fences, and convergence-bearing workgroup
  barriers. The experimental AMD lowering emits LDS, scoped integer atomics,
  fences, workgroup barriers, and explicit wave32/wave64 lane, ballot, vote,
  and bounded shuffle operations. The exact gfx942 wave/LDS V2 path adds an
  authenticated Rust-facing wave64 active-mask reduction and non-forgeable
  1,024-byte static-LDS reduction capability with fail-closed canonical
  `gfx942:xnack-` Kernel IR and Worker target binding. Its independently constructed
  Kernel IR has passed numerical MI300X execution, but the genuine Rust fixture
  reaches only verified Kernel IR. Dynamic-LDS launch-byte plumbing, broad
  atomics and collectives, general source-to-HSACO finalization, and compiler
  refinement remain fail-closed gaps.
- `fe2o3-host` exposes one Worker V3 execution graph. It consumes a recovered
  pinned descriptor, authenticates compiler and verification evidence, grants
  one exact HSA load authorization, resolves the selected kernel, validates
  generated arguments and geometry against the admitted descriptor, packs the
  complete COV6 kernarg, admits aliases, and dispatches through the reviewed HSA
  adapter. The old HIP module/function loader, raw `KernelParams`, `launch!`,
  cooperative-launch bridge, embedded-artifact contract, and profile-specific
  vecadd host route are deleted. `PreparedLaunch<K>` and argument admission are
  inert validation foundations; neither can load or dispatch an executable.
- `DeviceBuffer::view`, `view_mut`, and `split_at_mut` produce checked,
  borrow-typed contiguous regions while retaining the parent allocation
  identity, context, base address, full extent, and selected region. Splitting
  creates two simultaneously live exclusive views with exact disjoint
  allocation-relative byte regions; nested splits preserve those identities.
  Range, size, address, zero-sized-type, overflow, and null-allocation failures
  are explicit. Rust borrowing enforces exclusivity, but there is not yet a
  mechanical Verus proof of the split implementation. These views are a host
  provenance foundation, not launch authority.
- The bounded general typed V3 foundation accepts by-value `i8`/`u8` through
  `i64`/`u64`, `f32`/`f64`, shared slices, and genuine trusted
  `DisjointSlice<T, Index1D>` arguments. The macro emits an expectation-only V3
  registration while rustc independently reconstructs semantic types, layouts,
  effects, and physical ABI. The ordinary host build has no custom-backend
  object or private semantic-witness symbol dependency. Exact single-source
  typed Rust kernels named `alpha` and `zeta` form the first General-V3 vertical
  slice. Their source roles and argument names are authenticated as part of the
  ABI identity rather than inferred positionally: alpha binds
  `scale/input/output`, and zeta binds `a/b/bias/output`. The corresponding
  descriptors have explicit/complete COV6 kernarg sizes `40/296` and `56/312`.
  Exact role, name, signature, mutability, or layout substitutions fail closed.

  The macro generates signature-specific `Arguments` for the workload-neutral
  Worker V3 host contract. The generic adapter retains allocation borrows,
  reconstructs the named ABI, validates the selected Worker V3 descriptor,
  packs the explicit prefix, admits aliases and geometry, allocates the complete
  aligned COV6 kernarg, initializes the implicit region, and exposes synchronous
  preparation/dispatch. Production verifier implementations are still needed
  to promote compiler, Verus/proof, and effect evidence into that path.

  The rustc path recognizes only the exact alpha/zeta MIR shapes and lowers
  their trusted thread index, `Option`-guarded `DisjointSlice::get_mut`, slice
  loads, multiply/add operations, bounds control flow, and 256-thread launch
  contract through canonical Kernel IR. Unsupported targets, float policy,
  names, signatures, branches, or payload provenance fail closed. This is an
  exact lowering profile, not general Rust GPU lowering.
- The recovered Worker V2 host descriptor, launch-metadata bridge, synchronous
  HSA handoff, and Scalar GEMM Worker V2 hardware harness are deleted. The
  reviewed HSA adapter and physical observations remain shared mechanics for
  the production Worker V3 lifecycle. Qualification-only compiler publication
  records remain as isolated evidence inputs, but there is no Worker V2 or raw
  HIP host execution authority in any feature configuration.
- Compiler artifact publication is transactional and generation-owned. Typed
  generation results contain bounded immutable IR and HSACO snapshots captured
  through exact staged file descriptors and validated after publication while
  the transaction lock is still held. Later publication or pathname
  replacement cannot mix generations. Build-attempt and canonical rustc
  invocation descriptors are versioned and bounded. Worker V2 raw/final
  publication intent is derived by `fe2o3-hsaco-finalize` from sealed lineage;
  Cargo no longer duplicates its domain hashes. Completed publications produce
  a canonical inert `DurablePublishedHsacoClaimV1`, from which the transaction
  can reacquire a fresh non-clone currentness lease after revalidating the
  attempt, plan, receipt, generation, directory and file identities, digest,
  and publication lock. A bounded canonical compiler-transaction capsule binds
  source, dependencies, features, invocation, caller-measured compiler/backend
  identities, semantic and Kernel IR identities, Worker V2 request/response,
  target, raw and finalized HSACO, and artifact identity. The capsule is inert
  caller-measured evidence: it does not authenticate the compiler or establish
  source-to-machine-code refinement. None of these values grants load or launch
  authority.
  The protected required-envelope Cargo route now carries its compiler handoff,
  publication intent, receipt, completion, and restart state through the
  closure-bound V2 schema with no V1 fallback. Its cleanup escrow and exact
  successor lease make predecessor retirement restartable across every durable
  boundary, including a crash after a newer `Ready` marker is published. The
  ordinary compatibility route remains schema-separated on V1. This is durable
  provenance and crash recovery, not the still-missing production proof
  authenticator or host/HSA dispatch-authority bridge.
  Strict Worker V3 finalization now has a separate bounded restart route: a
  move-only compact transcript retains the slot and transaction axes, durable
  storage owns each unique outer/provider/finalized component, and fresh
  persistence and process recovery share one validator that reconstructs both
  worker exchanges, rederives the complete semantic binding, re-inspects raw
  HSACO, and requires byte-identical canonical re-finalization. The recovered
  owner crosses one audited, move-only publication-authority boundary; safe
  transaction callers cannot construct that authority from hashes. The
  transaction revalidates the complete durable intent under its publication
  lock, commits version-separated pending/final V3 receipts, returns a bounded
  canonical V3 claim and currentness lease, and reconstructs the same move-only
  production result after either backend-claimed or completed process restart.
  That result can now transfer into a move-only `WorkerV3LoadEnvelopeV1`. A
  claim-bound audited bridge publishes the exact canonical envelope, claim,
  and terminal custody receipt; restart can discover and revalidate that
  custody from the attempt registry before reconstructing the same inert live
  envelope. Exact terminal custody authorizes retirement of the duplicate
  current-generation replay intent, while registry-rooted scavenging removes
  only superseded custody. The envelope still grants no descriptor
  authentication, semantic admission, HSA readiness, load, or launch
  authority. Production now transfers only the canonical V3 envelope and
  artifact-directory descriptors to an identity-pinned sealed application. A
  fresh occurrence binds those descriptors and the ACK channel; Cargo checks
  the challenge-bound ACK and retains the current-publication lease through
  application exit. Cargo has no Worker V2 application transfer branch in
  either production or qualification builds; stale V2 envelope names are
  recognized only so they can be rejected before application spawn.
  Feature-free `fe2o3-host` builds export only the Worker V3 application,
  admission, verification, HSA load, and generated dispatch route. Worker V2
  application recovery, bundle admission, prerequisite authentication, HSA
  lifecycle, launch metadata, and workload-specific host adapters have been
  deleted rather than retained behind a qualification feature. General
  `#[kernel(typed)]` expansion, including the exact `f32` vecadd signature,
  emits only the generic Worker V3 adapter. The old
  `qualification_worker_v2` macro option, embedded vecadd artifact contract,
  generated `Kernel`/`Prepared` API, and example feature have been deleted.
  Production Worker V3 verification authority remains open; authorized Worker
  V3 HSA loading and generated dispatch are the only host execution route.
- Linux-only rustc and codegen-backend primitives use descriptor-backed procfs
  paths. The external Cargo path copies the backend into a rehashed, immutable
  sealed memfd and installs it after a compile-shaped managed wrapper
  invocation. The caller-selected compiler executable is not authenticated as
  rustc. This protects the measured bytes from pathname substitution; it is not
  a sandbox for hostile build scripts or procedural macros, which remain
  trusted inputs.
- `examples/regression-manifest-v2.txt` is the authoritative package/source-artifact
  inventory for ordinary checks and explicit artifact qualification. The route is
  data, never inferred from a package name; only `fe2o3-fill` currently selects the
  bounded `kernel-ir-v1` oracle. The manifest grants no production or GPU-execution
  authority.
- The Verus vecadd, fill, active-wave, LDS, and exact gfx942 wave/LDS
  harnesses prove bounded source-model properties under documented
  assumptions. The exact control,
  index, guarded memory access, and write body of the production `f32` vecadd
  kernel is mechanically shared with Verus through explicit thread and
  arithmetic adapters. Positive harnesses and paired expected-rejection
  mutations run in the required proof lane; the three real-body mutations
  additionally require one exact primary diagnostic and failed source clause.
  Verus uses a total model arithmetic adapter and does not prove that
  production IEEE `f32` addition,
  compiler output, HSACO, or GPU execution refines that model. Proof-record
  matching rejects incomplete or mismatched identities, but the records remain
  synthetic evidence rather than authenticated compiler-refinement evidence.
  `PersistentlyFreshMultiKernelProofAdmissionV1` additionally consumes
  non-clone per-kernel bindings from one exact local ledger history and requires
  unique kernels and generations, one finalized executable, and identical
  measured verifier policy and tool identities. It is local persistent
  consistency evidence, not rollback resistance, compiler refinement,
  prerequisite authentication, or load/launch authority.
  A separate bounded canonical pre-envelope proof capsule binds proof policy,
  execution and result records, target and payload identity, and the complete
  persistent-ledger ancestry used for freshness. It checks the finalized digest
  against the persistent executable binding and has bounded process-local
  duplicate detection. It does not provide durable single-use enforcement,
  compiler refinement, prerequisite authentication, or runtime authority.
- The G5 contracts now describe bounded independent-thread reads and writes,
  allocation provenance, bounds, injective writes, and deterministic proof
  obligations. Paired copy, gather, and affine elementwise bodies have positive
  and negative Verus harnesses. `fe2o3-verifier` canonicalizes bounded tool,
  policy, invocation, and result records, has a bounded shell-free process
  executor, and can convert validated results into descriptive proof records.
  Bounded canonical `gfx942` machine-effect evidence can compute call closure
  and straight-line effects from caller-supplied mechanics, requiring explicit
  complete bounds and rejecting indirect calls, unsupported control flow,
  malformed identities, and resource-limit violations. A separate worker path
  uses LLVM Object and MC APIs to extract a closed, exact alpha/zeta physical
  profile from supplied finalized HSACO bytes. Neither path is authenticated
  into each production payload or proves compiler, source, or Verus refinement.
  The verifier still has no reviewed production Verus adapter, authenticated
  proof-to-executable join, or runtime authority.
- G6/G7 includes canonical multi-input AMDGPU link plans and a standalone
  direct LLVM/LLD worker with bounded Rust/C++ protocols. Device FFI macros and
  compiler validation bind import/export symbols, physical ABI, address spaces,
  effects, target, and code-object version. Cooperative and peer capabilities
  retain exact contexts, streams, and cleanup ownership. The opt-in
  `kernel-ir-worker-v2` flow now carries a real Cargo crate containing two Rust
  kernel roots and one shared internal helper through rustc collection,
  canonical helper-call resolution, verified kernel IR, an attempt-scoped
  textual LLVM handoff, exact compiler-produced symbol-role manifests, Cargo
  wrapper consumption, and byte-identical GenericLink/V2 execution in a
  measured direct LLVM/LLD worker for `gfx942`. Internal calls resolve by their
  canonical Rust source identity to one collected helper definition and its
  exact predeclared signature and export symbol; ambiguous, uncollected, or
  signature-incompatible callees fail closed. The worker links both kernels and
  the helper into one HSACO using LLVM and LLD library APIs directly, without
  COMGR or command-line linking. Cargo independently checks the exact two-kernel
  symbol set and the returned raw HSACO. Descriptor-free COV5 remains a raw
  compatibility publication; descriptor-bearing COV6 is canonically finalized
  downstream and the exact finalized bytes are durably published with an
  attempt-bound provenance receipt. The durable transaction has adversarial,
  legacy-marker migration, and raw/finalized process crash-recovery coverage.
  Recovery revalidates the exact journal, plan, admission, route, receipt, and
  completed attempt before clearing durable state. Fault exits are available
  only under the non-default `worker-v2-fault-injection-test-only` feature.

  At the worker boundary, COV6 is protocol version 6, LLVM module flag 600, and
  AMDHSA ELF ABI version 4. Native tests preserve two metadata entries, both
  `.kd` symbols, and one shared helper in a single deterministic output. The
  worker does not authenticate `.fe2o3.kd.v1` or construct an
  `ArtifactContainerV1`; those remain downstream responsibilities.

  For this exact profile, the worker canonicalizes every COV5/COV6 kernel to
  the complete 256-byte implicit-argument contract after optimization. The
  finalizer accepts the AMDHSA metadata's explicit-prefix size only for the
  authenticated General-V3 `gfx942:xnack-` COV6 producer and reconciles it with
  the descriptor's complete size. All other size or profile mismatches remain
  rejected.

  At commit `daf0b459ced07a25376670c83b1474eaebcd1a68`, the ignored native
  integration test builds the exact alpha/zeta Rust fixture, lowers both MIR
  bodies through Kernel IR, validates both backend witnesses, links with the
  direct LLVM Worker V2, independently inspects and canonically finalizes one
  two-entry COV6 artifact, and exports exact bytes with SHA-256
  `3a916cdabca05ac74d340889aab2067221d6d1252a7cde13e61c1786252565c4`.
  A feature-gated MI300X HSA run then loaded that digest-pinned artifact once on
  `gfx942:xnack-`, resolved distinct raw `alpha` and `zeta` symbols, ran both
  kernels for lengths `1`, `255`, `256`, `257`, and `1023`, checked independent
  CPU oracles and prefix/suffix canaries, and unloaded the executable once.
  The hardware harness deliberately uses the reviewed unsafe raw HSA boundary;
  it is evidence for the generated artifact's code, ABI, and behavior, not an
  execution of the production generated adapter or a general safety proof.

  At commit `dc9738e367c392f7716eacb8459ca73fa32abbbb`, a now-retired ignored
  MI300X test passed the same digest and boundary-length matrix through the
  generated alpha/zeta argument capabilities, selected-kernel preparation,
  reviewed load/resolve/dispatch/unload lifecycle, and safe `dispatch` SPI.
  It uses an explicitly fake prerequisite authenticator and test-only semantic
  witnesses, so it validates runtime composition and hardware behavior but is
  not production authentication, Verus evidence, or a machine-code safety
  proof. That Worker V2 host harness and its workload-specific adapters are no
  longer part of the source tree; the observation remains historical evidence.

  Compiler identity and origin are not authenticated, no Verus result or
  compiler/machine-code refinement proof is authenticated and bound to this
  executable, and the publication receipt grants no HSA load or launch
  authority. On the MI300X `gfx942:xnack-` lane, the ignored real-Cargo
  alpha/zeta Worker V2 publication test and the digest-pinned HSA hardware
  tests passed alongside the earlier direct-source and external-bitcode-provider
  publication tests. The retired runs do not constitute current production
  execution coverage. A production Worker V3 verifier must authenticate and
  join compiler, proof, and effect evidence before the sole safe SPI can run.
- G8 adds deterministic model generation/reduction and a bounded conformance
  harness that executes fill, vecadd, and affine kernels against an independent
  HIP/CPU oracle. `cargo fe2o3 inspect` performs bounded read-only decoding.
  `sanitize` and `debug` retain plan mode and can execute descriptor-pinned
  native ROCgdb under bounded supervision. ROCgdb precise-memory diagnostics
  are not a race, API, initialization, synchronization, or safety proof.
  The historical S09 source-debug pilot built one exact General V3 `alpha` profile
  at O0 into an alpha-only COV6 HSACO for `gfx942:xnack-`. It binds inert
  semantic and build identity records to the physical `alpha`/`alpha.kd` pair,
  verifies linked DWARF, executes a dedicated controller over lengths 1, 255,
  256, 257, and 1023 with CPU-oracle and canary checks, and uses native ROCgdb
  to inspect scalar and aggregate arguments, a reference value, physical slice
  fields, and local `i`; tuple and array locals also carry located DWARF. The
  archived lane produced only `local-capability-v2` evidence: it does not
  authenticate the compiler or runner, install production trust, materialize
  tuple/array runtime values at the fixed stop, cover optimized or general
  debugging, or provide a safety proof. Rows 45 and 46 and supplemental row S09
  are therefore `Partial`. See the
  [S09 pilot contract](docs/s09-source-debug-pilot-v1.md).

### Not yet integrated

- General MIR to kernel IR to AMDGPU lowering is not complete. `kernel-ir-v1`
  accepts the exact fill and vecadd shapes, and `kernel-ir-worker-v2` additionally
  accepts only the exact alpha/zeta General-V3 shapes on `gfx942:xnack-`; the
  elementwise recognizer remains the default emitter.
- The production semantic-MIR route now has one mandatory target-neutral
  ranked-PLIRON verifier sequence before Kernel IR lowering: memory bounds,
  global race freedom, barrier convergence, workgroup-memory initialization
  and publication, and declared semantic refinement. The passes are generic
  dialect checks rather than workload recognizers and share a bounded sparse
  index/dataflow analysis. Rust projection is complete for static ranked
  accesses and the checked `ThreadIndex`/`DisjointSlice` dynamic-access
  contract. Other dynamic pointer provenance, semantic CFG projection for
  barriers/workgroup memory, and source-declared equivalence still fail closed;
  the PLIRON passes and textual lit coverage do not by themselves establish
  source-to-machine correspondence.
- General V3 lexical registration, rustc-semantic
  scalar/shared-slice/`DisjointSlice` reconstruction, variable COV6 descriptor
  generation, safe value binding, checked buffer regions, lifetime-retaining
  packing primitives, backend witness emission, and signature-specific
  `Arguments` are implemented as source/unit foundations. At `d509ca5`, their
  generated slice capabilities can consume checked shared/exclusive subregions,
  retain allocation-relative region identity, and feed the existing alias and
  packing foundations. The macro emits the generic Worker V3 argument and
  preparation contract for every accepted signature; exact alpha/zeta host
  adapters have been deleted.
  Aggregates, return values, and arbitrary rustc layouts also remain outside V3.
  In required-envelope mode, the Cargo production path consumes a measured
  upstream canonical envelope-input capsule rather than synthesizing direct-link
  or proof evidence. It binds that input to the build attempt, durably stages
  it, publishes the exact canonical Worker V3 load-readiness envelope, and
  reconstructs the same envelope from durable input and HSACO claims across
  restart. The envelope retains the artifact container, bundle index,
  direct-link evidence,
  descriptor lineage, per-kernel proof records, raw HSACO, finalized payload,
  and canonical reacquirable publication claim. Cargo validates transport,
  canonical encoding, identities, and restart closure; it does not authenticate
  the supplied compiler, proof, or machine-effect claims. A bounded cooperative
  application handoff now transfers pinned envelope and artifact-directory
  descriptors to an identity-pinned sealed application while Cargo retains a
  fresh current-publication lease. The V3 occurrence binds the envelope,
  artifact directory, and ACK descriptors; the host revalidates them before
  returning an inert descriptor. This is the sole protected production
  descriptor handoff, but it grants no prerequisite, load, or launch authority.

  No production Worker V3 verifier yet promotes compiler, Verus/proof, Rust
  ABI, and machine-effect evidence into safe dispatch. Retired Worker V2 test
  authority is not an alternate route and cannot be selected in any build.
- Checked mutable views now support simultaneously live disjoint subviews via
  `split_at_mut`, with exclusivity enforced by Rust borrowing. The mechanical
  Verus proof of that split and its allocation-relative region theorem remains
  open.
- The generated contract identity authenticates compiler declarations and the
  exact payload bytes. A dedicated worker can extract a bounded physical
  machine-effect profile from supplied exact `gfx942` alpha/zeta HSACO bytes,
  and caller-supplied records can be canonicalized and checked. The production
  authority chain does not yet authenticate that extraction for each finalized
  payload, and neither mechanism proves correspondence to every executable
  memory access. The fixed lowering, Kernel IR checks, host alias admission,
  and tests provide separate defenses. The generic ranked-PLIRON bounds and
  race analyses now reject unsupported or conflicting compiler IR before
  lowering, but end-to-end source-to-machine safety still requires complete
  frontend projection plus authenticated compiler-refinement evidence.
  Trusted rustc diagnostic-item classification also remains part of the
  compiler TCB.
- Generated Worker V3 arguments retain the resources required for dispatch,
  but generalized asynchronous application APIs, cancellation, and composition
  remain incomplete.
- Worker V3 marker-to-artifact association remains part of the trusted
  compiler/linker contract and does not itself prove executable semantics.
- `cargo fe2o3 verify` and `build --require-proof` are roadmap commands. The
  current required Verus CI lane is invoked separately and does not prove the
  ordinary Rust function, compiler, ROCm, driver, or machine-code refinement.
  Verus proof identity/refinement is not authenticated into the generated
  vecadd artifact or required by its safe loader and launch API. The exact
  alpha/zeta source models have mechanical Verus proofs and bounded proof-record
  schemas, but no reviewed Rust-semantics refinement or authenticated
  source-to-Kernel-IR-to-machine-code refinement.
- The fail-closed rustc wrapper classifies and preserves approved bootstrap
  invocations, and the external Cargo path now composes compile-shaped managed
  invocations with the descriptor-pinned rustc executable and sealed backend
  snapshot. The selected executable is still not authenticated as rustc;
  rustc-descendant descriptor lifetime, dynamic loading, transitive shared
  libraries, and non-Linux execution remain unresolved.
- General Rust language support, frontend-to-layout integration, broad atomic
  and collective coverage, production direct-link integration, general
  device FFI, occupancy-complete cooperative launch, multi-device memory
  semantics, full sanitizer/debugger coverage, broad differential fuzzing, and
  authenticated Verus refinement remain parity work. The alpha/zeta hardware
  result covers only MI300X `gfx942:xnack-`; architecture-family breadth is
  absent. LDS, atomics, waves, collectives, fences, and barriers have bounded
  source/compiler paths. The exact gfx942 wave/LDS V2 Kernel IR also has one
  numerical MI300X result, but it is not joined to the genuine Rust artifact.
  These facilities are not yet broadly available from ordinary Rust kernels or
  validated across the full operation, type, target, and hardware matrix.

The evidence-gated comparison with cuda-oxide is tracked in the
[parity matrix](docs/cuda-oxide-parity-matrix.md) and the generated
[evidence dashboard](docs/generated/cuda-oxide-parity-dashboard.md). The
dashboard pins a status floor and records qualifying per-row evidence at that
commit or a landed descendant; it is not a claim that every change at the
current repository HEAD has qualifying parity evidence. fe2o3 is not yet at
parity.

See [docs/implementation-plan.md](docs/implementation-plan.md) for the original
compiler/runtime plan and
[docs/implementation-roadmap-v2.md](docs/implementation-roadmap-v2.md) for the
current staged roadmap.

## Commands

Run diagnostics:

```bash
cargo run -p cargo-fe2o3 -- doctor
```

Inspect a bounded fe2o3 artifact without loading it, or print a normalized
ROCgdb execution plan:

```bash
cargo run -p cargo-fe2o3 -- inspect target/fe2o3/kernel.hsaco
cargo run -p cargo-fe2o3 -- sanitize -- ./target/debug/application
cargo run -p cargo-fe2o3 -- debug -- ./target/debug/application
```

Execution is explicit and bounded. Debug execution requires an explicit batch
or interactive mode:

```bash
cargo run -p cargo-fe2o3 -- sanitize --execute -- ./target/debug/application
cargo run -p cargo-fe2o3 -- debug --execute --batch -- ./target/debug/application
```

Sanitize fails when requested precise-memory coverage is unavailable. Race and
API coverage are reported as unsupported rather than inferred from a clean run.

Preview or remove only fe2o3-generated artifacts under `target/fe2o3`:

```bash
cargo run -p cargo-fe2o3 -- clean --dry-run
cargo run -p cargo-fe2o3 -- clean
```

The clean command discovers the enclosing Cargo project or workspace and
preserves the rest of its target directory. Planning opens and retains the
canonical project-root capability. Each successful no-follow component open is
authoritative: substitution completed before that open selects the current
ordinary directory, while substitution after it cannot redirect later access.
Metadata is used only after an open failure to produce a fail-closed diagnostic.

Destructive cleanup is supported on Unix, where the opened `target/fe2o3`
directory is passed to capability-relative opened-directory removal. With the
pinned capability implementation, Windows removal is pathname-based, so fe2o3
fails closed there; `--dry-run` remains available. Unix opened-directory removal
is not atomic against every concurrent rename and can fail after partially
removing the opened directory's contents.

This is intentionally narrower than pinned cuda-oxide's clean command, which
removes the project's full target directory. External-project build and run
orchestration are now supported, but local-clean parity remains partial because
fe2o3 deliberately removes only its guarded `target/fe2o3` output.

If `FE2O3_TARGET` is not set, `cargo-fe2o3` tries to infer the target from
`rocminfo` and falls back to `gfx1100`.

Each external build uses a generation identity that binds the selected target,
backend, Worker V2 configuration, and effective Cargo configuration. A changed
or failed generation receives fresh Cargo fingerprint state; successful
incremental builds republish the exact generated snapshot.

Validate the authoritative example manifest and list a lane:

```bash
cargo run --locked -p cargo-fe2o3 -- examples check
cargo run --quiet --locked -p cargo-fe2o3 -- examples list artifact-qualification
cargo run --quiet --locked -p cargo-fe2o3 -- examples list artifact-kernel-ir-v1
cargo run --quiet --locked -p cargo-fe2o3 -- examples list cpu-test-raw
cargo run --quiet --locked -p cargo-fe2o3 -- examples list cpu-test-wrapper-managed
cargo fe2o3 test --locked --all-targets -p fe2o3-vecadd
```

The two CPU-test queries form a sorted, disjoint, exhaustive partition of
manifest packages selected for Rust checks but not artifact qualification. The
partition is computed from the exact structural wrapper-managed projection, so
generic CI runs namespace-free typed-kernel tests through the mandatory
`cargo fe2o3 test --all-targets` host path and leaves ordinary packages on raw
Cargo without package-name rules. CI revalidates both complete lists and the
full structural projection after tests.

This binding-only path executes trusted workspace source, Cargo configuration,
build scripts, procedural macros, linkers, and test bodies. It rejects a caller
`--target`, `--config`, Cargo-side `-Z`, `--doc`, and `--no-run`, and rejects
ambient compiler, rustdoc, protected fe2o3, and test-runner selection. Configured
compiler, protected fe2o3, loader, and test-runner selection is rejected;
configured rustdoc is overridden with the disabled selection, and ambient loader
variables are scrubbed. The fixed runner retains and hashes Cargo's original test
executable. While Cargo's path remains stable, this preserves ordinary
`current_exe` and `$ORIGIN` behavior and prevents directory-entry substitution
between pin and execution; the runner checks the retained object again afterward.
That is not same-inode immutability, origin authentication, a sandbox, or an
atomic Cargo-configuration snapshot. The pre/post protected-configuration scans
are diagnostic checks against persistent changes, not a TOCTOU proof.

The command produces ordinary Cargo host-test files but grants no fe2o3 backend,
HSACO, publication, or immutable-artifact authority. It neither requests nor
establishes GPU evidence and performs no performance prediction. Trusted test
code is not confined from files, the network, or device nodes.

Run the repository validation lanes:

```bash
scripts/ci-local.sh generic
scripts/ci-local.sh generic-core
scripts/ci-local.sh shard-policy
scripts/ci-local.sh rustc-codegen-shard 04-memory-math-gemm
scripts/ci-local.sh workspace-test
VERUS=/absolute/path/to/verus scripts/ci-local.sh verus
FE2O3_TARGET=gfx942 scripts/ci-local.sh rocm-compile
FE2O3_ALLOW_GPU_SMOKE=1 FE2O3_TARGET=gfx942 scripts/ci-local.sh hardware-smoke
```

`generic` remains the complete serial generic gate. Hosted CI runs
`generic-core` once and executes every selector-free production integration
target in the checked-in shard manifest. `shard-policy` derives the complete
test-target set from locked Cargo metadata, requires an exact active-or-retired
partition, and rejects overlap, missing, duplicate, renamed, unknown,
malformed, empty, or newly unassigned targets. Retired targets are migration
inventory, not executed evidence; their oracle logic runs only in library tests.
Each hosted core or shard job uses separate Cargo and log directories; the
stable `Generic validation` check succeeds only after the core and all shards
succeed.

The historical S09 hardware entry point is retired because its debug HSACO
generator used the removed Worker V2 selector and HSA runtime. Its offline
DWARF/transcript checkers remain tested; hardware evidence must be regenerated
through a production Worker V3 compiler path and KFD runtime before the lane
can be enabled again.

`workspace-test` is the comprehensive local test gate. It runs all workspace
test targets except `rustc-codegen-fe2o3`, then tests that package in a separate
Cargo process. Do not replace it with one `cargo test --workspace --all-targets`
invocation; the codegen backend's `rlib` and unversioned `dylib` outputs can
collide across build variants. This lane can link ROCm libraries. The ROCm and
hardware lanes require a matching AMD GPU and ROCm installation.
The release-evidence collector requires a complete archive-relative row-link
map and records Git, rustc, LLVM, ROCm, driver, target, and stable lane
identities without changing matrix status:

```bash
scripts/parity-evidence.sh collect \
  --rows rows.tsv --hardware-lane mi300x-gfx942-release > evidence.tsv
scripts/parity-evidence.sh validate evidence.tsv
scripts/tests/parity-evidence.sh
scripts/parity-dashboard.sh check
scripts/tests/parity-dashboard.sh
```

`scripts/parity-dashboard.sh update` rewrites only the deterministic generated
Markdown and TSV dashboard. The check rejects stale paths, missing claims,
unsupported status upgrades, target/evidence mismatches, and generated drift.

To build or run one package directly:

```bash
cargo run --locked -p cargo-fe2o3 -- build -p fe2o3-vecadd
cargo run --locked -p cargo-fe2o3 -- run -p fe2o3-vecadd
env CARGO_PROFILE_DEV_DEBUG=1 cargo test --locked \
  -p rustc-codegen-fe2o3 --features qualification-oracles-test-only --lib
```

The first two commands enter the sole production route. The current vecadd
application fails closed before dispatch because its Worker V3 application
verifier has not yet been wired to the generated `Arguments`; it never falls
back to the embedded V2 artifact. The third command is the isolated offline
oracle fixture suite. `FE2O3_CODEGEN_PIPELINE` is no longer
accepted, and a production backend rejects `FE2O3_QUALIFICATION_ORACLE_V1`.

The retired manifest smoke command is not a production or direct-KFD path and
is no longer exposed. Hardware execution must enter an explicit runtime-owned
gate; artifact qualification never implies load or dispatch authority.
