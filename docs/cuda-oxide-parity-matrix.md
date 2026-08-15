# cuda-oxide Parity Matrix

Status: normative parity baseline for fe2o3 v2.

Pinned commits, row IDs, and current statuses are generated from
[`cuda-oxide-parity-status.tsv`](cuda-oxide-parity-status.tsv). Run
`scripts/parity-matrix.sh check` to validate this projection or
`scripts/parity-matrix.sh generate` after changing the source of truth.
The generated [evidence dashboard](generated/cuda-oxide-parity-dashboard.md)
and its machine-readable TSV are validated with
`scripts/parity-dashboard.sh check`.

## Baseline and Scope

<!-- parity-status:baseline:start -->
The fixed comparison point is the fetched cuda-oxide `origin/main` commit
`2db97134d9a3a79fe71c211e65a616dacdf03235` from 2026-08-07. The primary
source is `cuda-oxide-book/appendix/supported-features.md` at that commit. Its
94 feature rows are reproduced below in the same category order, including
partial, experimental, planned, and N/A rows. The supplemental audit also
accounts for capabilities demonstrated elsewhere in the repository.

The fe2o3 status floor and default claim snapshot are based on commit
`2fee8b63b77df73b92f4de79caaabc5b623ab867`.
Qualifying per-row evidence may name a landed descendant of that commit; this
projection does not claim that every change at current HEAD has qualifying parity
evidence.
<!-- parity-status:baseline:end -->

The source of truth now pins the cuda-oxide commit and date above. Its supported
feature appendix is byte-identical to the prior snapshot, so the 94-row scope is
unchanged. Post-snapshot fe2o3 updates extend the bounded `gfx942` Worker V2 and
general typed foundations without changing any row to Complete. The archived
remote compile/publication evidence includes an external Cargo fixture with two
kernel roots and one shared helper; the frontend and Kernel IR path retain one
exact helper identity, and the sealed Cargo backend invokes the direct LLVM/LLD
worker to publish one inspected HSACO containing both entries. The worker uses
LLVM and LLD library APIs directly and does not use COMGR or command-line
linking.

The existing V1 artifact wire format now has a strict `gfx942` profile with two
canonically ordered entries over one digest-validated native payload. Each
entry has a separate proof binding over its kernel, ABI, effects, launch,
source, target, and shared executable identities. Host admission can select two
different compiler-generated marker types from that executable and rejects
name, binding, physical-layout, target, payload, effects, launch, and
executable substitution. The reviewed HSA adapter can resolve a fixed set of
distinct native symbols and retains them in a non-clone value that borrows the
loaded executable, so safe Rust cannot unload it while the set is live.

The post-snapshot source/unit work adds expectation-only V3 registration for
bounded scalars, shared slices, and `DisjointSlice`; rustc-semantic type/layout
reconstruction; checked `DeviceBuffer` views and borrow-checked mutable splits;
lifetime-branded host packing; and backend-issued semantic witnesses. It also
lands exact single-source typed Rust `alpha` and `zeta` profiles. Their
logical/export roles and source argument names are authenticated in the ABI
identity: alpha is
`scale/input/output`, zeta is `a/b/bias/output`, and their explicit/complete
COV6 kernarg sizes are `40/296` and `56/312`. The macro generates exact
signature-specific `Arguments` and host preparation/dispatch adapters from the
same named ABI model. Other General-V3 signatures retain inert `Arguments`.

The rustc backend recognizes only the exact alpha/zeta MIR shapes and lowers
their trusted thread index, guarded `DisjointSlice::get_mut`, slice loads,
floating multiply/add, bounds control flow, and 256-thread launch contract into
verified Kernel IR. Role, argument name, type, mutability, branch provenance,
float policy, and target substitutions fail closed. Worker V2 emits and links
the private witness accessors and uses LLVM and LLD library APIs directly,
without COMGR or a command-line linker. COV6 canonicalization restores the
complete 256-byte implicit-argument contract after optimization and reconciles
the authenticated descriptor's complete size with AMDHSA metadata's explicit
prefix; unrelated size/profile mismatches remain rejected.

At implementation commit `c4fcb4d980cf979c0527dfa135a7b9f4fe72a811`, the tiled-GEMM
Slice 1 path has a separate exact vertical slice. Source/IR groundwork from
[#85](https://github.com/harsh-nod/fe2o3/issues/85),
[#86](https://github.com/harsh-nod/fe2o3/issues/86), and
[#93](https://github.com/harsh-nod/fe2o3/issues/93) enters the sealed
[#96](https://github.com/harsh-nod/fe2o3/issues/96) profile.
[#97](https://github.com/harsh-nod/fe2o3/issues/97) performs direct upstream
LLVM target-machine emission and in-process LLD finalization with no COMGR or
command-line tools; [#99](https://github.com/harsh-nod/fe2o3/issues/99)
generates the borrowed A/B/C host adapter; and
[#100](https://github.com/harsh-nod/fe2o3/issues/100) joins both sides in
private, non-`Clone`, one-shot `Joined -> Loaded -> Completed -> Unloaded`
states. Integration [#94](https://github.com/harsh-nod/fe2o3/issues/94) and
children #96/#97/#99/#100 are closed. The path fixes artifact target
`gfx942:xnack-`, requires a compatible observed target, and rechecks grid
`[1,1,1]`, workgroup `[64,1,1]`, 1,024 static LDS bytes, zero private/dynamic
bytes, and a 48-byte explicit plus 256-byte hidden COV6 kernarg.

One exact MI300X `gfx942` run passed all 256 output bits against the CPU
reference, immutable A/B checks, prefix/suffix guard canaries, and terminal
unload. Its measured worker was
`fe2o3-worker-v1-sha256-6c3dfd5f784b3babe140006aba57a214a897b171860928440184fa201b6f96db`
with upstream LLVM 22.1.8 build
`upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1`; the marker
bound finalizer
`078e9b523164b679ff7af3b4e819ad041713c53c6841399ac7cea95090f09774`
and unload
`df2f77ee798444a9e1fe5e27f219bdf720386eb8603a9a74fccc0df8efb3921c`.
The older six-case observational LDS run remains separate evidence.

At `daf0b459`, the ignored real-Cargo MI300X Worker V2 integration builds both
source kernels, validates both backend witnesses, independently inspects and
canonically finalizes one two-entry `gfx942:xnack-` COV6 artifact, and exports
bytes with SHA-256
`3a916cdabca05ac74d340889aab2067221d6d1252a7cde13e61c1786252565c4`.
The feature-gated hardware lane loaded that digest-pinned artifact once,
resolved distinct raw alpha/zeta symbols, ran both kernels for lengths `1`,
`255`, `256`, `257`, and `1023`, checked independent CPU oracles and
prefix/suffix canaries, and unloaded the executable once. The hardware harness
uses the reviewed unsafe raw HSA boundary. At `dc9738e`, a second ignored run
passes the same digest and length matrix through generated checked slice
capabilities, typed alpha/zeta preparation, the reviewed executable lifecycle,
and safe `dispatch`. That test uses test-only semantic witnesses and an
explicitly fake prerequisite authenticator, so it establishes runtime
composition and hardware behavior but does not authenticate prerequisites.

The evidence remains bounded. Durable Worker V2 publication, finalized-bundle
host admission, currentness leasing, an authenticated load state machine, the
generated alpha/zeta safe dispatch SPI, and the reviewed
`fe2o3-hsa-runtime` adapter exist. Required-envelope mode consumes a measured
upstream canonical envelope-input capsule, binds and durably stages it, and can
reconstruct the exact canonical envelope from durable input and HSACO claims
after restart. Cargo does not synthesize or authenticate the capsule's compiler,
proof, or effect claims. Recovered host admission independently reacquires a
fresh lease and revalidates finalized bytes and descriptor lineage, but returns
an inert descriptor with no bytes, authentication, load, launch, or prerequisite
authority. Production application handoff and an implementation of
`WorkerV2PrerequisiteAuthenticatorV1` remain absent.

The new compiler-transaction capsule is inert caller-measured evidence. The
pre-envelope proof capsule binds persistent ancestry but supplies neither
durable single-use enforcement nor compiler refinement. The bounded `gfx942`
machine-effect model analyzes caller-supplied straight-line mechanics rather
than extracting LLVM IR or HSACO behavior. Rust borrowing enforces
`split_at_mut` exclusivity, without a mechanical Verus split proof. The
alpha/zeta generated-safe hardware result covers only MI300X `gfx942:xnack-`
and uses an explicitly fake prerequisite authenticator. The exact Slice 1
lifecycle above does not use that fake path, but it also does not authenticate
compiler origin, consume production Verus certificates
([#91](https://github.com/harsh-nod/fe2o3/issues/91)), or prove MIR/KIR/LLVM/ISA
refinement ([#106](https://github.com/harsh-nod/fe2o3/issues/106) and
[#107](https://github.com/harsh-nod/fe2o3/issues/107)). General
illegal-memory/race proofs, shapes, and protected Slice 3/4 remain open. These
gaps keep every Complete count at zero and prevent any cuda-oxide parity claim.

The [monomorphization-dead V1 foundation](monomorphization-dead-v1.md) defines
one fixed-width, fail-closed folding policy and compiler-private MIR
observation. It gates reachable-function collection, panic traversal, and MIR
import only for policy-proven direct constant branches. That bounded compiler
slice qualifies row 23 as Partial. It still does not bind the observation
through semantic IR to machine address-space analysis, and it has no archived
configured-compiler or `gfx942` execution evidence.

The next bounded scope and exit gate are defined by the
[general typed dispatch V1 contract](general-typed-dispatch-v1.md). The status
TSV is authoritative; the deterministic matrix and dashboard generators project
that source together with archived evidence declarations.

At `dc9738e` fe2o3 also has a HIP runtime, explicit unsafe raw module and launch
paths, versioned kernel registration, reachable MIR collection, bounded rustc
frontend and general layout records, a canonical target-neutral kernel IR and
verifier, concrete generic-helper collection, semantic constants,
reducible-CFG analysis, block-argument lowering, opt-in end-to-end
verified-IR AMDGPU fill and exact three-slice vecadd paths, experimental LDS,
scoped integer atomics, wave operations, fence, and workgroup-barrier lowering,
a default narrow elementwise `f32`/`f64` LLVM/HSACO emitter, fail-closed formal
affine memory/race obligations for modeled effects, artifact and proof schemas,
generated `KernelMarkerV1` types, exact loaded module/function authority, host
argument-alias admission, bounded HSACO inspection/finalization, event-backed
asynchronous transfer lifetimes, transactional artifact publication, bounded
Verus driver records, and paired Verus harnesses.
`#[kernel(typed)]` still connects only one exact executable
`pub fn(&[f32], &[f32], DisjointSlice<f32>)` profile to a backend-generated,
canonical embedded artifact and safe load, prepare, synchronous launch, and
non-escapable scoped launch API. That narrow launch path authenticates canonical
rustc-derived argument layout evidence and validates the finalized physical
AMDHSA kernarg shape. It is not a general compiler, general typed module
system, machine-code effect proof, authenticated proof-carrying artifact, or
proof-requiring build.

This matrix compares capabilities and observable semantics, not identical
vendor syntax. It is not a claim that either project is production-ready.

## Classification

Every baseline row has exactly one portability class:

- **Exact**: fe2o3 must provide the same target-neutral Rust behavior and safety
  contract. Names may use the neutral fe2o3 vocabulary.
- **AMD-equivalent**: the CUDA mechanism is vendor-specific, so fe2o3 must
  provide the closest semantically defensible AMD/ROCm capability. Differences
  in scope, width, ordering, or resource behavior must be explicit.
- **N/A**: there is no AMD counterpart required for parity, or the pinned
  cuda-oxide baseline itself intentionally excludes the feature. N/A rows need
  an explicit diagnostic or documented omission, not a silent approximation.

Current fe2o3 status is separate:

- **Partial**: some behavior exists, but it does not meet the row's acceptance
  contract.
- **Missing**: no qualifying implementation exists.
- **N/A**: intentionally outside the AMD parity contract.

A row may be marked **Complete** only after its gate and row definition of done
pass.

<!-- parity-status:counts:start -->
| Scope | Complete | Partial | Missing | N/A | Total |
|:--|--:|--:|--:|--:|--:|
| Normative | 0 | 82 | 0 | 12 | 94 |
| Supplemental | 0 | 15 | 0 | 0 | 15 |
<!-- parity-status:counts:end -->

An IR type, schema, parser, or isolated proof is classified as **Partial** only
when it implements a meaningful part of the row; it does not stand in for
end-to-end compiler/runtime behavior.

The authoritative status TSV and evidence dashboard record 0 Complete, 82
Partial, 0 Missing, and 12 N/A normative rows, plus 0 Complete, 15 Partial, 0
Missing, and 0 N/A supplemental rows. The 36 promotions added by the bounded
`gfx942` milestone are Partial only: each implements a meaningful slice and
names its exact landed commit, commands, lanes, strengths, and limitations.
Zero Missing does not satisfy any row's full acceptance contract and is not a
cuda-oxide parity claim.

## Gates

| Gate | Scope |
|:--|:--|
| G0 | Baseline, honest unsafe boundaries, CI, and versioned contracts |
| G1 | General frontend, typed `mir.*`, `gpu.*`, SSA, and AMDGPU compiler spine |
| G2 | Rust language, layout, constants, control flow, calls, and closures |
| G3 | Manifest-derived ABI, artifact bundles, typed sync/async runtime |
| G4 | Core AMD GPU model: LDS, atomics, waves, collectives, math, and debug |
| G5 | Verus V1: bounds, provenance, injective writes, and prepared launches |
| G6 | Device linking, FFI, advanced AMD capabilities, and multi-device runtime |
| G7 | Verus V2-V4: barriers, atomics, subgroups, and asynchronous operations |
| G8 | Differential fuzzing, sanitizers, debuggers, performance, and parity release |

The detailed dependencies and exit criteria are in
[implementation-roadmap-v2.md](implementation-roadmap-v2.md).

## Evidence Behind Current Partial Status

- Row 01 and supplemental row S06: the HIP ABI records managed-memory,
  concurrent-access, VMM, and peer capabilities against exact context and
  physical-device identities. Linear managed allocations support bounded
  advice, prefetch, location queries, and reclamation; VMM typestates cover
  reserve, map, per-device access, query, and reverse-order cleanup. Opt-in HIP
  tests exercise managed allocation lifecycle and two-`gfx942` VMM access.
  Safe host-reference capture, coherent CPU/GPU access semantics, in-flight
  launch retention, and broad topology evidence remain incomplete.
- Row 07: reachable collection now distinguishes concrete generic and
  const-generic helper instances, deduplicates them deterministically,
  terminates recursive graphs, and diagnoses unavailable cross-crate MIR with
  a complete call chain. Generic registered kernel roots and final
  application-bundle integration remain unsupported.
- Rows 02, 03, and 08-10: a rustc-private bounded extractor records fully
  monomorphized scalar, pointer, array, tuple, struct, direct-enum, and
  niche-enum layouts, including ABI representation, field offsets, variants,
  valid ranges, alignment, stride, and padding. A target-neutral semantic type
  model independently validates and canonicalizes the same shape families.
  Neither path is connected to host packing, artifact identity, constants, or
  general device codegen, so these rows remain Partial.
- Row 12: canonical ABI records represent scalar and approved slice components,
  rustc-derived type/layout identities, source argument indexes, physical
  offsets, widths, alignments, address spaces, and effects. The bounded
  `GeneratedArgumentPackingPlanV1` rejects omission, duplication, reordering,
  wrong kind/width/access/address space, pointer-width mismatch, and
  cross-kernel values. General V3 binding now accepts canonical scalar and slice
  identities, and generated slice capabilities consume checked subregions while
  retaining parent-allocation identity and exact allocation-relative intervals.
  Exact alpha/zeta `Arguments` authenticate source role and field names, retain
  those capabilities and borrows, and feed macro-generated packing and
  preparation adapters. Other V3 signatures remain inert. Production artifact
  publication/currentness/admission infrastructure exists, but no production
  `WorkerV2PrerequisiteAuthenticatorV1` promotes authenticated
  compiler/proof/effect evidence into these adapters. General structs, closures,
  return values, and the full acceptance target are not complete.
- Rows 17 and 20: authenticated control-flow records, canonical successor and
  block-argument models, reducible-CFG validation, and bounded MIR branch/loop
  fixtures exist. The compiler rejects malformed predecessor, block-argument,
  and unsupported control-flow shapes rather than approximating them. General
  integer `match`, arbitrary reducible while/if graphs, nested loop exits, and
  broad source-to-hardware execution remain incomplete.
- Rows 24 and 25: the manifest ABI model and canonical target-neutral Kernel IR
  represent a bounded arithmetic and cast subset. The opt-in `kernel-ir-v1`
  backend takes exact fill and three-slice vecadd through translation,
  verification, AMD lowering, transactional publication, and hardware
  execution. The Worker V2 profile additionally lowers exact alpha multiply and
  zeta add/bias expressions from MIR through Kernel IR and executes them on
  `gfx942:xnack-`. Complete signed/unsigned operations, casts, overflow forms,
  architecture breadth, and general source lowering remain absent.
- Rows 26 and 40 and supplemental row S11: authenticated device identities and
  Kernel IR float contracts cover f16/BF16 conversion, packed BF16x2 fused
  multiply-add, strict divide, and selected math calls. Exact `gfx942` golden
  lowering uses constrained operations and pinned OCML imports, with malformed
  contract, capability, symbol, target, and type rejection plus code-object
  compilation tests. The public source API, full math/type matrix, edge-case
  GPU execution, other AMD targets, packed storage/arithmetic breadth, and
  cuda-oxide-equivalent rounding coverage are incomplete.
- Rows 32, 33, and 35: `#[kernel]` emits strict V1 registration metadata tied to
  a direct function pointer and a deterministic, doc-hidden typed
  `KernelMarkerV1`; public kernels expose that marker publicly. Reachable helper
  collection now assigns one canonical source identity to a helper reached by
  two roots, and Kernel IR lowering validates both calls against the collected
  helper's exact signature. The real-source Worker V2 fixture emits one helper
  definition and two entries into one `gfx942` HSACO. For the exact vecadd
  signature, `#[kernel(typed)]` emits a public generated host module and a V2
  typed registration. Other bounded scalar/slice signatures emit
  expectation-only V3 registrations that rustc checks against semantic
  primitive and trusted `DisjointSlice<T, Index1D>` identities. Full
  crate/kernel binding IDs are derived independently by the Cargo wrapper,
  macro, and backend and qualify private host/accessor symbols; a real two-rlib
  same-name link test rejects silent archive coalescing. The backend rejects
  token aliases and local trusted-type lookalikes. V3 requires a backend-issued
  semantic witness; Worker V2 emits deterministic private host-object accessors,
  and the alpha/zeta native-worker integration links and validates both. For the
  exact authenticated alpha/zeta roles, the macro generates named-ABI
  `Arguments` plus safe preparation and synchronous dispatch adapters. Other V3
  `Arguments` remain inert. Durable publication, finalized-bundle admission,
  currentness leasing, authenticated loading, and the reviewed HSA adapter
  exist, but the production prerequisite-authenticator implementation does not.
  Cross-crate finalization is absent. Both raw and generated-safe MI300X paths
  pass, but the latter uses explicit test authority.
- Rows 35-38 and 41-43: one-source builds, AMDGPU LLVM/HSACO sidecars, diagnostic
  dumps, bounded HSACO inspection, a read-only `cargo fe2o3 inspect` command,
  complete external-project build/run orchestration, project-local cleanup, and
  the opt-in exact fill and vecadd paths exist. The sealed external-Cargo
  Worker V2 fixture also compiles two roots and one shared helper into one
  deterministically inspected and published `gfx942` payload; the canonical
  artifact profile indexes both entries over that payload and rejects duplicate
  or conflicting identities. At `daf0b459`, that fixture is the exact typed
  alpha/zeta source pair and exports one independently inspected, canonically
  finalized COV6 artifact with SHA-256
  `3a916cdabca05ac74d340889aab2067221d6d1252a7cde13e61c1786252565c4`.
  Required-envelope mode accepts only a measured upstream canonical
  envelope-input capsule. It durably binds that input to the attempt, publishes
  the exact canonical Worker V2 load envelope, and reconstructs the same
  envelope after restart from durable input and HSACO claims. Cargo neither
  synthesizes nor authenticates the supplied direct-link, proof, compiler, or
  effect evidence. Recovered host admission reacquires and revalidates the exact
  durable publication but returns an inert descriptor with no bytes,
  authentication, load, or launch authority. Project build scripts and
  procedural macros remain trusted; pipeline inspection is not stage-complete,
  broad Rust semantics and cross-crate finalization are absent. Application
  handoff and a production `WorkerV2PrerequisiteAuthenticatorV1` are absent. The
  generated-safe MI300X harness composes the runtime pieces only with a fake
  prerequisite authenticator.
- Rows 27, 28, and 39: bounded device FFI macros and compiler validation bind
  import/export direction, exact symbols, physical scalar/pointer ABI,
  address spaces, effects, target, code-object version, and semantic identity.
  A standalone worker implements canonical Rust/C++ request/response codecs,
  LLVM bitcode linking, AMDGPU `TargetMachine` emission, and in-process LLD,
  with no COMGR or command-line linker dependency. In the post-snapshot
  `gfx942` Worker V2 slice, Cargo consumes an exact compiler-produced
  symbol-role manifest, requires byte-identical output from two worker
  executions, and independently inspects the raw HSACO. Descriptor-bearing COV6
  is canonically finalized before durable publication under the originating
  build attempt; raw COV5 remains a compatibility path. Exact restart recovery
  covers both publication kinds and legacy migration. The path now
  covers two kernel roots with one canonical shared helper and feeds a strict
  two-entry artifact profile with per-kernel proof bindings. Compiler origin
  authentication and compiler-to-machine-code refinement remain outside the
  claim. The direct LLVM Worker V2 uses LLVM linking and in-process LLD APIs,
  never COMGR. The MI300X ignored integration test now establishes
  target-specific source compile, exact MIR-to-Kernel-IR lowering, direct link,
  COV6 inspection/finalization, publication, and exported-artifact identity.
  The paired raw HSA test establishes two-kernel GPU behavior for five boundary
  lengths, but not optimized production-worker, general-target, or
  compiler-refinement evidence.
- Cross-cutting compiler, proof, and effect evidence remains authority-free. A
  canonical compiler-transaction capsule binds caller-measured source,
  dependencies, invocation, tools, Worker V2 traffic, target, raw/final HSACO,
  and artifact identities without authenticating the compiler. A bounded
  pre-envelope proof capsule binds exact policy, execution/result records,
  payload, and persistent-ledger ancestry, without durable single-use or
  compiler-refinement authority. The `gfx942` machine-effect capsule validates
  caller-supplied straight-line call/effect mechanics; it does not establish
  correspondence to LLVM IR or HSACO.
- Rows 44 and 45: `cargo fe2o3 sanitize` and `debug` retain plan mode and can
  execute an exact descriptor-pinned native ROCgdb binary with bounded
  output, timeout, process cleanup, an environment allowlist, and diagnostic
  evidence. Precise-memory support is checked at execution and fails closed
  when unavailable. It is not a race, API, initialization, synchronization, or
  memory-safety proof. Row 45 additionally has a bounded alpha-only
  `gfx942:xnack-` O0/COV6 pilot that validates source locations, one scalar
  argument, physical slice components, and one local through native ROCgdb.
  General source-debug metadata, aggregate inspection, optimized debugging,
  and production-v2 evidence remain unvalidated.
- Rows 48, 49, and 60: one-dimensional `DisjointSlice` and `ThreadIndex` APIs,
  target-neutral launch-axis verification, and observed target/capability facts
  exist. Checked shared/exclusive `DeviceBuffer` views preserve exact allocation
  identity and selected-region provenance, reject invalid ranges and arithmetic
  overflow, and enforce exclusive parent borrowing. Kernel IR derives formal
  affine regions, bounds, runtime-alias, and
  inter-invocation race obligations for modeled effects and fails closed on
  unsupported effects. `DeviceBuffer::split_at_mut` creates simultaneously live
  exclusive views with exact disjoint allocation-relative regions, and nested
  splits retain those identities. Rust borrowing enforces exclusivity;
  compile-fail tests reject overlap and lifetime escape. The exact generated
  vecadd adapter authenticates its fixed one-dimensional launch contract and
  maps three runtime allocations to it; complete 2D/3D branded construction,
  general launch extents, and general parameter/allocation mappings remain
  incomplete. A mechanical Verus proof of the mutable split implementation is
  still absent.
- Row 51: `PreparedLaunch<K>`, loaded prepared launch, artifact-prepared launch,
  and cooperative admission types bind bounded geometry and resources to exact
  kernel, context, module/function, and capability observations. Construction
  and compile-fail tests reject rank, brand, context, and loaded-kernel
  substitution. The reusable public path is still centered on exact profiles
  and does not yet derive arbitrary resource and geometry proofs from every
  manifest entry.
- Rows 53-55, 64-67, 70, and 71: canonical Kernel IR models scoped
  integer atomics, static and dynamic workgroup memory, scoped fences,
  convergence-bearing workgroup barriers, and explicit wave32/wave64 lane,
  ballot, vote, and bounded shuffle operations. AMD lowering for those bounded
  operations has compiled into `gfx1151`, `gfx942`, and, where recorded by the
  claim ledger, `gfx950` code objects. Branded dynamic-LDS views enforce
  bounded disjoint typestates at the source API. Ordinary Rust frontend
  integration, dynamic-LDS launch-byte admission, float and standard-library
  atomics, broad wave types/tiles/collectives, and GPU semantic execution are
  still absent.
- Rows 57, 58, and 61: Kernel IR distinguishes static LDS from the one dynamic
  LDS base, derives required capabilities, validates extent/alignment/address
  space, and represents workgroup barriers with convergence, scope, ordering,
  and memory-space claims. AMD lowering emits address-space-3 storage and
  convergent `llvm.amdgcn.s.barrier` plus required fences, and rejects
  conditional or cyclic barrier placement under the bounded proof. Ordinary
  Rust source integration, dynamic launch-byte admission, LDS initialization
  transfer, general barrier convergence proof, and GPU semantic execution are
  absent in the general paths. The bounded
  [gfx942 wave/LDS V2](gfx942-wave-lds-v2.md) exception adds one compiler-created
  1,024-byte static-LDS capability, exact 256-thread barrier schedule, Verus
  ownership/participation proof, and numerical MI300X run from verified Kernel
  IR. V2 also binds the full canonical `gfx942:xnack-` identity through Kernel
  IR and Worker V2. It does not join genuine Rust source to the executed HSACO.
  The separate [LDS-tiled GEMM slices](tiled-gemm-lds-slices.md) add one exact
  WG64 `16x16x16` BF16/FP32 path. The collector authenticates the exact
  attributed Slice 1 root, reviewed MIR sequence, ABI, geometry, and
  compiler-derived LDS resources before selecting canonical verified IR. A
  sealed profile then carries those identities through a metadata-strict direct
  LLVM/LLD finalizer, generated borrowed host adapter, and one-shot protected
  load/dispatch/wait/unload lifecycle. The exact MI300X run checked every one of
  256 result bits plus A/B immutability and allocation canaries. An older
  six-case run remains separate numerical evidence. The identity-bound Verus
  model adds 96 checked source/IR obligations, but no production Verus result is
  consumed and no compiler or machine-code refinement is proved. K32 loop IR,
  bounded multi-phase Verus evidence, padded-stride Slice 3 IR/lowering, and
  tail/alpha/beta Slice 4 proof/IR/lowering remain independent; Slice 3/4 lack
  protected execution. These additions therefore do not change the Partial
  classifications.
- Rows 65, 72, and 73: the gfx942 wave/LDS V2 slice lowers a logically masked
  `u32` wave64 sum through one ballot and six XOR shuffles and lowers the same
  activity contract to an exact 256-thread static-LDS reduction with 18
  barriers. LLVM shape, gfx942:xnack- assembly and metadata, host-oracle MI300X
  execution, and an exact Verus model are present. Broader operations and
  types, scans in this proof/hardware lane, wave32 and target breadth, partial
  physical EXEC masks, authenticated source-to-HSACO finalization, and compiler
  refinement remain absent, so all three rows remain Partial.
- Row 74: observed capabilities retain exact live
  contexts. Cooperative admission retains the exact loaded function and stream
  and conservatively accepts one workgroup until per-function occupancy is
  available. It does not prove grid-wide synchronization semantics or general
  occupancy-safe cooperative execution.
- Rows 78 and 79: for the exact public
  `fn(&[f32], &[f32], DisjointSlice<f32>)` profile, `#[kernel(typed)]`
  generates the public `<kernel>_gpu` module with `Kernel` and `Prepared`
  aliases. The backend embeds one
  canonical container holding the native payload, target, exact physical ABI,
  read/read/write effects, canonical rustc-derived type/layout identities, and
  one-dimensional launch contract. The identities encode the exact source
  shapes, rustc ABI class, pointer width, size, alignment, and ordered physical
  components. `Kernel::load` independently reconstructs those host layouts and
  recomputes the kernel identity over the full marker binding and contract.
  Before embedding, the backend binds ELF entries to AMDHSA descriptors and
  requires the exact six pointer/length kernargs. `prepare` checks context,
  equal nonzero lengths, u32 index geometry, resource limits, and alias
  admission while retaining the loaded authority and typed buffers. Separately,
  the `gfx942` two-entry profile binds both entries to one payload and host
  admission can select either compiler-generated marker without exchanging
  ABI, effects, launch, target, physical-layout, or executable identities.
  General V3 now has semantic scalar/slice
  descriptors, backend-emitted witnesses, checked views, packing foundations,
  and signature-specific generated `Arguments`. At `d509ca5`, their slice
  capabilities preserve exact checked subregions through packing and alias
  admission. Exact named alpha/zeta roles now receive macro-generated packing,
  preparation, and synchronous dispatch adapters, while other signatures remain
  inert. Durable publication, finalized-bundle admission, currentness leasing,
  authenticated loading, required-envelope persistence/recovery, recovered host
  admission, and the reviewed runtime adapter exist. The envelope and recovered
  descriptor remain authority-free, and only test/fake
  `WorkerV2PrerequisiteAuthenticatorV1` implementations can promote evidence
  into the generated adapters. The generated-safe MI300X run uses fake
  prerequisite authority. Arbitrary Rust layouts, authenticated machine-code
  effect verification, architecture breadth, application handoff, and
  production authenticated dispatch are incomplete, so both rows remain
  Partial.
- Row 80: the general `launch!` macro remains an explicit unsafe raw-ABI escape
  hatch with compile-fail coverage. The generated vecadd module instead exposes
  safe `prepare(...).launch(...)`; the example contains no raw parameter pack,
  artifact pathname, or unsafe user launch. The two-entry Worker V2 path does
  generate manifest-checked preparation and dispatch for the exact alpha/zeta
  roles, and the production publication/admission/load state machines exist.
  The missing production prerequisite authenticator prevents authenticated
  compiler/proof/effect evidence from reaching that safe SPI. The generated-safe
  MI300X alpha/zeta harness passes through the safe dispatch SPI only by using a
  fake prerequisite authenticator. These fixed profiles are not a general
  generated launch macro or production authority path, so the row remains
  Partial.
- Row 81 and supplemental row S03: the generated vecadd `launch_scoped` API
  retains typed resource borrows, loaded authority, alias admission, and packed
  parameters through event completion or stronger stream quiescence. Its
  higher-ranked callback cannot return the in-flight operation. Generalized
  returnable borrowed or owned generated async operations, cancellation, and
  composition are incomplete. The linear HSA kernel set prevents executable
  unload while resolved kernels are retained, and exact alpha/zeta have
  synchronous generated adapters. They do not have generated asynchronous
  operations, so both rows remain Partial.
- Supplemental rows S01 and S02: the V1 container, bundle index, direct-link
  evidence, descriptor finalization, transactional publication, and durable
  raw/finalized crash-recovery records form a canonical bounded artifact path.
  The `gfx942`
  profile carries two independently identified entries and two
  non-substitutable proof bindings over one digest-validated native payload.
  Descriptor-pinned snapshots retain finalized IR and HSACO in one generation
  across pathname replacement. Exact alpha/zeta COV6 publication additionally
  reconciles the authenticated complete implicit-kernarg contract with AMDHSA
  metadata and produced the digest-pinned MI300X hardware artifact. Required
  mode now persists a measured upstream envelope-input capsule and reconstructs
  the exact canonical envelope across restart; recovered host admission
  revalidates the durable publication. Neither path authenticates compiler,
  proof, or effect evidence or grants load/launch authority, and no production
  prerequisite authenticator promotes that evidence into generated dispatch.
  This is not general compiler production, all-target loading, or machine-code
  refinement evidence.
- Supplemental rows S04 and S05: bounded `DeviceCopy`, pinned-memory, event,
  and transfer-lifetime models exist, but general manifest-gated device type
  interpretation and broad asynchronous hardware ordering remain incomplete.
- Supplemental row S07 specifically has a bounded semantic-constant and data
  relocation model that rejects function, vtable, thread-local, and unknown
  relocations. Rustc promotions, emitted device globals/statics, and GPU use
  are not integrated.
- Supplemental row S10: the deterministic model generator/reducer remains,
  and a separate bounded harness now compiles and executes fill, vecadd, and
  affine kernels against an independent HIP/CPU oracle over boundary lengths,
  deterministic data, NaN/infinity policy, and physical canaries. This is a
  narrow conformance corpus, not general MIR fuzzing or safety proof.
- Supplemental row S14: target detection and explicit override produce
  canonical AMD target identities with observed-device correlation, capability
  checks, and fail-closed XNACK/SRAMECC handling in bounded host/HSA paths. Not
  every payload producer and cache identity is covered across the full target
  matrix.
- Supplemental row S15: compile-fail suites cover kernel/index brands,
  non-transferable witnesses, private generated fields, unsafe pointer
  binding, loaded-executable borrows, non-clone typed selections and HSA kernel
  sets, and unload-before-release errors. The suite does not yet cover every
  general typed signature, async cancellation path, barrier lifecycle, or
  remaining unsafe transition.
- Row 87: authenticated rustc inline-assembly records bind statement,
  function, contract, and frontend-unit identities before a closed `gfx942`
  instruction table emits static LLVM templates. V1 covers a small set of
  register-only 32-bit move and ALU instructions and rejects unknown mnemonics,
  memory, atomics, barriers, control flow, convergence, special-state effects,
  and operand mismatches. Source MIR integration, broad operands/effects, and
  hardware execution are incomplete. Row 47 separately has a public, closed
  `amdgpu_asm!` contract for six typed `u32` VGPR operations wired through
  trusted MIR, Kernel IR, and gfx942 lowering. It remains Partial because it
  does not expose arbitrary assembly, general operands, clobbers, memory,
  control flow, or hardware execution.

## Normative 94-row Matrix

### Compiler: Memory Model

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 01 | HMM / Unified Memory Management | Full | AMD-equivalent | Partial | Fine-grained host/device shared allocations with capability checks; reference captures retain host lifetime and fail when the platform cannot provide coherent access | G3, G6 |
| 02 | Unified Struct ABI without `#[repr(C)]` | Full | Exact | Partial | Host and device use rustc-reported `repr(Rust)` layout, including padding and reordered fields | G2, G3 |
| 03 | Dynamic Layout Matching | Full | Exact | Partial | Layout importer records field offset order, size, alignment, variants, and explicit padding; ABI tests compare host and device views | G2 |
| 04 | Pointer Distance (`offset_from`) | Full | Exact | Partial | Signed/unsigned element and byte distances use pointee layout, provenance checks, and reject zero-sized pointees where Rust requires it | G2 |
| 05 | Volatile Load/Store | Full | Exact | Partial | Volatile survives import, optimization, LLVM export, and AMD instruction selection; mem2reg never promotes it | G2 |
| 06 | Bulk Copy (`copy_nonoverlapping`) | Full | Exact | Partial | Element counts scale by rustc layout, address spaces are preserved, overlap is an unsafe precondition, and LLVM/AMDGPU output is tested | G2 |

### Compiler: Type System

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 07 | Generics and Monomorphization | Full | Exact | Partial | Generic and const-generic kernels/helpers are collected at final use sites with deterministic symbols and cross-crate tests | G1, G2 |
| 08 | Enums (`Option`, `Result`, custom) | Full | Exact | Partial | Direct and niche layouts, discriminants, payloads, matches, and supported enum constants follow rustc layout | G2 |
| 09 | Struct Construction and Field Access | Full | Exact | Partial | Literals, projections, by-value parameters/returns, nested structs, and padding pass layout-differential tests | G2 |
| 10 | Array Types (`[T; N]`) | Full | Exact | Partial | Construction, constants, nested arrays, runtime/constant indexing, mutation, and padded element stride work | G2 |
| 11 | `CuSimd<T, N>` SIMD Type | Full | Exact | Partial | Neutral `GpuSimd<T, N>` offers equivalent lane construction/access and lowers legally on AMD targets | G2, G4 |
| 12 | ABI Scalarization | Full | Exact | Partial | Slices, references, closures, structs, and scalar fields are packed from the manifest and reconstructed exactly; no handwritten safe packing | G2, G3 |

### Compiler: Closures

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 13 | Move Closures (`FnOnce`) | Full | Exact | Partial | Captured values are monomorphized, layout-correct, passed by value, and callable in generic kernels | G2, G3 |
| 14 | Reference Closures (`Fn`/`FnMut`) | Full | Exact | Partial | Reference captures require an eligible shared-memory allocation, preserve borrow lifetime through completion, and fail closed otherwise | G2, G3 |
| 15 | Host-to-Device Closures | Full | Exact | Partial | Host-created captures and call shims compile through the device graph with typed launch packing | G2, G3 |
| 16 | Device-Internal Closures | Full | Exact | Partial | Device-created closures, captures, and calls lower without host ABI assumptions | G2 |

### Compiler: Control Flow

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 17 | Match Expressions (integer switch) | Full | Exact | Partial | Integer switches preserve Rust semantics and lower to legal AMDGPU control flow | G1, G2 |
| 18 | Match on Enums | Full | Exact | Partial | Variant tests, payload projections, and niche layouts work in nested control flow | G2 |
| 19 | For Loops (range, iterator, enumerate) | Full | Exact | Partial | MIR-desugared ranges, slice iteration, enumerate, nesting, and early exits compile and execute | G2 |
| 20 | While Loops / If-Else | Full | Exact | Partial | Arbitrary reducible baseline control flow works; support is no longer restricted to recognized elementwise shapes | G1, G2 |
| 21 | Break and Continue | Full | Exact | Partial | Loop exits and continue edges preserve values and pass nested-loop tests | G2 |
| 22 | Loop Unroll Annotations | Partial | Exact | Partial | Match the pinned baseline's supported full/partial unroll semantics and limits, with diagnostics for unsupported loop shapes | G2 |
| 23 | Monomorphization-Dead Branches | Partial | Exact | Partial | Collection, panic checks, and address-space analysis ignore only branches proved dead by the defined constant-folding policy | G2 |

### Compiler: Arithmetic and Casting

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 24 | 64-bit Arithmetic | Full | Exact | Partial | Signed/unsigned arithmetic, comparison, shifts, bitwise operations, overflow forms, and descriptor packing pass CPU/GPU differential tests | G1, G2 |
| 25 | Type Casting (all kinds) | Full | Exact | Partial | Integer/float widths, bitcasts, pointer casts, coercions, pointer/integer conversions, and provenance policy are explicit and tested | G2 |
| 26 | Packed bf16x2 FMA | Full | AMD-equivalent | Partial | Target-gated packed BF16 FMA uses an AMD intrinsic or a documented equivalent sequence with matching lane and rounding semantics | G4 |

### Compiler: Interop

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 27 | Bi-directional LTOIR Support | Full | AMD-equivalent | Partial | Rust calls AMDGPU bitcode/device objects and external device code calls exported Rust functions through a versioned direct LLVM/LLD link contract | G6 |
| 28 | Device FFI (`extern "C"`) | Full | AMD-equivalent | Partial | Typed declarations preserve AMDGPU ABI, convergence/effect attributes, layouts, symbols, and diagnostics | G6 |
| 29 | MathDx FFI (cuFFTDx / cuBLASDx) | Full | AMD-equivalent | Partial | Demonstrate equivalent in-kernel FFT and matrix-library integration where ROCm supplies device-callable artifacts; unsupported targets report the gap | G6 |
| 30 | Tile interop | Experimental | AMD-equivalent | Partial | AMD tile/SIMT kernels share allocations and HIP streams between kernels; intra-kernel interop remains experimental unless a stable AMD contract exists | G6 |
| 31 | Cross-Crate Kernels | Full | Exact | Partial | Library kernels and helpers finalize concrete monomorphizations in the application bundle | G1, G2, G3 |

### Compiler: Functions

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 32 | `#[kernel]` Attribute | Full | Exact | Partial | Multiple generic/non-generic entries generate stable metadata, AMD kernel calling convention, typed markers, and clear diagnostics | G0, G2, G3 |
| 33 | `#[device]` Helper Functions | Full | Exact | Partial | Reachable helpers, recursion policy, inlining attributes, calls, returns, and cross-crate definitions lower generally | G1, G2 |
| 34 | Standalone `#[device]` Functions | Full | Exact | Partial | Export device functions without a kernel root for external AMD device linking | G6 |
| 35 | Multi-Kernel Modules | Full | Exact | Partial | Multiple entries share one deterministic artifact bundle/module and deduplicate helpers; separate per-kernel HSACO is not final parity | G1, G3 |

### Compiler: Compilation Pipeline

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 36 | Unified Single-Source Compilation | Full | Exact | Partial | One Cargo command drives Verus when requested, normal host rustc, and device extraction from one executable source | G1, G3, G5 |
| 37 | PTX Output | Full | AMD-equivalent | Partial | General pipeline emits target-correct HSACO for the declared AMD target set; elementwise recognition is not the default path | G1 |
| 38 | NVVM IR Output | Full | AMD-equivalent | Partial | Emit inspectable, validated AMDGPU LLVM IR/bitcode with target and code-object policy recorded | G1, G6 |
| 39 | LTOIR Linking | Full | AMD-equivalent | Partial | Link AMDGPU bitcode/relocatable device artifacts with deterministic provenance and option records | G6 |
| 40 | Float Math Intrinsics (libdevice) | Full | AMD-equivalent | Partial | Rust float methods map to OCML/OCKL or LLVM intrinsics with target, precision, denormal, and contraction policy tests | G4 |
| 41 | Pipeline Inspection | Full | Exact | Partial | `cargo fe2o3 pipeline` shows imported MIR, post-SSA IR, `gpu.*`, lowered LLVM IR, and artifact metadata | G1 |
| 42 | PTX Inspect | Full | AMD-equivalent | Partial | `cargo fe2o3 inspect` prints AMDGPU LLVM, disassembly/metadata, or selected bundle payload without executing | G1, G3 |
| 43 | Local Clean | Full | Exact | Partial | `cargo fe2o3 clean` safely removes only guarded `target/fe2o3` output; pinned cuda-oxide removes the full project target directory | G0 |
| 44 | Compute Sanitizer Wrapper | Full | AMD-equivalent | Partial | `cargo fe2o3 sanitize` invokes supported ROCm GPU sanitizers/checkers and clearly reports unavailable tools or checks | G8 |
| 45 | cuda-gdb Source Debugging | Full | AMD-equivalent | Partial | Debug build and `cargo fe2o3 debug` launch ROCgdb with kernel source locations | G8 |
| 46 | cuda-gdb Local / Argument Inspection | Partial | AMD-equivalent | Partial | A bounded local alpha/O0 pilot inspects one scalar, physical slice pointer/length components, and one local; qualifying production-v2 evidence and baseline-scope reference, struct, tuple, and array inspection remain absent | G8 |
| 47 | `ptx_asm!` Macro | Partial | AMD-equivalent | Partial | `amdgpu_asm!` supports typed operands, outputs, clobbers, side-effect/convergence options, and baseline-equivalent limits where LLVM permits | G6 |

### Runtime Library: Safety

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 48 | `DisjointSlice<T, IndexSpace>` | Full | Exact | Partial | Index-space and allocation-aware writable view accepts only matching branded witnesses; safe writes are bounded and disjoint | G0, G3, G5 |
| 49 | `ThreadIndex<'kernel, IndexSpace>` | Full | Exact | Partial | Opaque, launch-branded, non-transferable, non-`Copy` witness with checked 1D/2D/3D constructors | G0, G3, G5 |
| 50 | Proof-carrying static views | Full | Exact | Partial | One checked tile/view grants statically bounded constant accesses without repeated checks, with compile-fail coverage | G5 |
| 51 | `PreparedLaunch<K>` | Full | Exact | Partial | Reusable geometry/resource proof is branded to kernel, artifact, context, dimensions, and capability set | G0, G3, G5 |
| 52 | `ManagedBarrier` Typestate | Full | Exact | Partial | Lifecycle misuse is a compile error; Verus separately proves participant and epoch obligations | G4, G7 |

### Runtime Library: Atomics

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 53 | Device-Scope Atomics | Full | Exact | Partial | Integer and supported float RMW operations implement all Rust orderings at device/agent scope | G4, G7 |
| 54 | Block-Scope Atomics | Full | Exact | Partial | Workgroup-scope atomics use the correct AMD synchronization scope and reject unsupported operations/types | G4, G7 |
| 55 | System-Scope Atomics | Full | Exact | Partial | System-scope atomics operate only on eligible coherent allocations and preserve CPU/GPU ordering | G4, G6, G7 |
| 56 | `core::sync::atomic` Support | Full | Exact | Partial | Standard Rust atomics lower with documented default scope and complete ordering tests | G4 |

### Runtime Library: Shared Memory

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 57 | Static Shared Memory | Full | Exact | Partial | Const-sized aligned workgroup arrays lower to LDS with per-kernel accounting and initialization tracking | G4, G7 |
| 58 | Dynamic Shared Memory | Full | Exact | Partial | Launch-sized aligned LDS views are bounded by `PreparedLaunch` resource metadata | G3, G4, G7 |
| 59 | Distributed Shared Memory (DSMEM) | Full | N/A | N/A | No cross-workgroup LDS address mapping is promised; targets without a proven semantic equivalent reject the capability | G6 |

### Runtime Library: Thread and Synchronization

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 60 | Thread/Block/Grid Intrinsics | Full | Exact | Partial | Complete 3D workitem/workgroup IDs and dimensions plus branded linear/tiled indexes, with rank validation and runtime 2D row width bound to the indexed slice | G1, G4, G5 |
| 61 | Block Synchronization | Full | Exact | Partial | Workgroup barrier lowers correctly and carries convergence, scope, and memory semantics through IR | G4, G7 |
| 62 | Async Barriers (mbarrier) | Full | AMD-equivalent | Partial | Target-gated AMD split/named barrier abstraction exposes only semantics supported by the selected architecture | G6, G7 |
| 63 | Cluster Synchronization | Full | N/A | N/A | No CUDA thread-block-cluster promise; reject cluster-only kernels unless a future AMD target adds a modeled equivalent | G6 |
| 64 | Fence Operations | Full | AMD-equivalent | Partial | Provide scoped AMD fences and supported wait/sleep operations; CUDA proxy-only semantics are omitted or rejected | G4, G6 |

### Runtime Library: Warp and Cooperative Groups

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 65 | Warp Shuffle Operations | Full | AMD-equivalent | Partial | Wave shuffle/permutation operations support declared types and explicit wave32/wave64 width/active-lane contracts | G4, G7 |
| 66 | Warp Vote Operations | Full | AMD-equivalent | Partial | Wave all/any/ballot return width-correct masks and define inactive-lane behavior | G4, G7 |
| 67 | Lane/Warp ID | Full | AMD-equivalent | Partial | `lane_id` and wave ID use AMD semantics; no fixed width of 32 is assumed by portable code | G4 |
| 68 | Typed Group Handles | Full | AMD-equivalent | Partial | Provide `Grid`, `Workgroup`, `SubgroupTile<N>`, and active-lane groups; unsupported CUDA `Cluster` behavior is unavailable | G4, G6 |
| 69 | Group Universal API | Full | Exact | Partial | Every supported group has typed `size`, `thread_rank`, and legal synchronization behavior | G4 |
| 70 | Warp Tile Partitioning | Full | AMD-equivalent | Partial | Wave tiles are valid only for supported divisors and wave widths, with active-lane and convergence contracts | G4, G7 |
| 71 | Warp Collectives | Full | AMD-equivalent | Partial | Ballot, vote, shuffle, match, and active-mask operations cover baseline types with wave-width-correct semantics | G4, G7 |
| 72 | Warp Reductions / Scans | Full | AMD-equivalent | Partial | Wave reductions/scans cover the pinned operation/type matrix across supported widths | G4, G7 |
| 73 | Block Reductions / Scans | Full | Exact | Partial | Workgroup collectives use LDS and barriers, support the baseline operation/type matrix, and work across wave widths | G4, G7 |
| 74 | Cooperative Kernel Launch | Full | AMD-equivalent | Partial | HIP cooperative launch and grid synchronization are capability-checked, occupancy-safe, and encoded in the launch contract | G6, G7 |

Rows 68 and 69 have a fail-closed source contract documented in
[Typed groups foundation V1](typed-groups-foundation-v1.md). It establishes
private non-`Send`/non-`Sync`/non-`Clone` arithmetic snapshot types, checked
ranks, wave64 const-width restrictions, and an unsafe workgroup barrier with
global plus workgroup visibility. The snapshots do not authenticate a launch,
target, epoch, or EXEC state, and no movable token grants convergence authority.
The compiler-visible contracts qualify both rows as Partial. General source
lowering, authenticated execution identity, Verus, artifact, and hardware
evidence remain absent.

### Runtime Library: Debug

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 75 | `gpu_printf!` Macro | Full | AMD-equivalent | Partial | Formatted device output lowers through a supported ROCm device ABI with format/type checking | G4 |
| 76 | `gpu_assert!` Macro | Full | Exact | Partial | Failed assertions trap and, where supported, report message and source metadata without unwind | G4 |
| 77 | Debug Intrinsics | Full | AMD-equivalent | Partial | Clock, trap, breakpoint/debug trap, and supported profiling markers have target-gated AMD mappings | G4, G8 |

### Runtime Library: Kernel Launch

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 78 | `#[cuda_module]` Typed Launch | Full | Exact | Partial | A neutral module macro embeds bundles and generates typed sync/async methods from manifest entries | G3 |
| 79 | `#[launch_contract]` / `PreparedLaunch<K>` | Full | Exact | Partial | Contracts check rank, exact/bounded block shape, resources, capabilities, context, and kernel identity | G0, G3, G5 |
| 80 | `cuda_launch!` Macro | Full | Exact | Partial | `launch!` is explicitly unsafe for runtime-loaded raw functions and exposes complete obligations | G0, G3 |
| 81 | `cuda_launch_async!` Macro | Full | Exact | Partial | Raw lazy launch is unsafe; typed operations retain borrowed/owned resources through completion and cancellation | G3 |
| 82 | `#[launch_bounds]` | Full | AMD-equivalent | Partial | Emit and validate AMD flat workgroup-size/occupancy metadata with architecture-specific limits | G4 |
| 83 | `#[cluster_launch]` | Full | N/A | N/A | CUDA cluster dimensions are not accepted as portable AMD launch metadata | G6 |

### Runtime Library: TMA

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 84 | TMA Bulk Tensor Copy (1D-5D) | Full | N/A | N/A | No claim of CUDA TMA descriptor parity; separate AMD async global-to-LDS capabilities may be added under their own semantics | G6 |
| 85 | TMA Multicast | Full | N/A | N/A | No CUDA TMA multicast contract is exposed on AMD targets | G6 |
| 86 | TMA Commit/Wait Groups | Full | N/A | N/A | CUDA TMA group semantics are not emulated; AMD async operations use their native completion model | G6 |

### Baseline Planned and Intentionally Unsupported

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 87 | Rust `asm!` macro | Planned | AMD-equivalent | Partial | Lower MIR inline assembly for AMDGPU when rustc/LLVM operand semantics can be preserved; separate from `amdgpu_asm!` | G6 |
| 88 | FP8 / MX Data Types | Planned | AMD-equivalent | Partial | Add target-gated AMD FP8 and supported microscaling formats with explicit layout, conversion, and matrix-operation tests | G6 |
| 89 | Dynamic Dispatch (`dyn Trait`) | N/A | N/A | N/A | Not a parity deliverable; use monomorphized static dispatch | G8 |
| 90 | Heap Allocation (`Box`, `Vec`) | N/A | N/A | N/A | No default device allocator; raw target extensions require a separate proposal | G8 |
| 91 | `String` / `format_args!` | N/A | N/A | N/A | No device string allocation; use bounded formatting/device print facilities | G8 |
| 92 | Panic / Unwinding | N/A | N/A | N/A | Device panic paths trap; unwind edges are rejected or erased only under documented abort semantics | G2, G8 |
| 93 | Standard Library (`std`/`alloc`) | N/A | N/A | N/A | Device graph supports `core` plus approved device crates; diagnostics include the reachable call chain | G1, G8 |
| 94 | Texture Memory | N/A | N/A | N/A | Not required by the pinned baseline; a future AMD image/texture proposal is an extension, not parity | G8 |

## Supplemental Repository Audit

The pinned appendix is normative, but the checkout demonstrates additional
cross-cutting capabilities in crates, examples, and tests. These are required
for a credible parity release even though they are not separate appendix rows.

| ID | Demonstrated cuda-oxide capability | Class | fe2o3 now | Acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|
| S01 | Versioned target-neutral artifact container | Exact | Partial | Embedded, content-addressed bundle with entries, payloads, options, ABI, capabilities, and proof identity | G3 |
| S02 | Artifact finalization and provenance | AMD-equivalent | Partial | Deterministic AMDGPU link/finalize records include inputs, target, options, and toolchain identity | G3, G6 |
| S03 | Typed async `DeviceOperation` model | Exact | Partial | Lazy borrowed/owned operations, stream policy, events, cancellation-safe reclamation, and composition | G3 |
| S04 | `DeviceCopy` derive and manifest-gated device types | Exact | Partial | Host byte-transfer types have compile-pass/fail bit-validity and padding checks; safe device interpretation additionally requires manifest-derived type/ABI identity, provenance, and capabilities | G3 |
| S05 | Pinned host buffers and events | Exact | Partial | RAII pinned memory, explicit unsafe async-copy lifetimes, timing, and ordering events | G3 |
| S06 | VMM, peer access, and multi-device memory | AMD-equivalent | Partial | HIP/HSA-supported virtual/peer memory has context, lifetime, topology, and capability checks | G6 |
| S07 | Device constants, statics, and relocations | Exact | Partial | Layout-aware constants/globals preserve supported pointer relocations and reject unsupported provenance | G2, G6 |
| S08 | Kernel families and compile-time policies | Exact | Partial | Tuned monomorphized variants share a typed logical interface and carry policy identity in the bundle | G2, G3 |
| S09 | Source debug metadata | Exact | Partial | A bounded local alpha/O0 pilot preserves function, argument, and local metadata; qualifying production-v2 evidence, aggregate layouts, broader kernels, and supported optimized modes remain absent | G2, G8 |
| S10 | Differential MIR/codegen fuzzer | Exact | Partial | Generated accepted programs compare CPU reference behavior and AMD execution; reducer preserves failures | G8 |
| S11 | Half/BF16 types and conversions | Exact | Partial | Scalar and packed formats, conversions, arithmetic, constants, ABI, and edge cases are tested | G2, G4 |
| S12 | Tensor/matrix instructions | AMD-equivalent | Partial | Capability-gated MFMA/WMMA abstractions cover supported shapes/types with ISA and numerical tests | G6 |
| S13 | LDS swizzles and matrix load/store helpers | AMD-equivalent | Partial | AMD-native layouts expose bank/alignment contracts and compose with proof-aware views | G6, G7 |
| S14 | Target auto-detection and override | AMD-equivalent | Partial | Detect AMD architecture and features, accept explicit override, and record the resolved target in every payload | G0, G3 |
| S15 | Compile-fail safety suite | Exact | Partial | UI tests cover launch brands, rank, index spaces, witness transfer, async lifetime, barrier lifecycle, and unsafe transitions | G0, G3, G5 |

The current Verus lane has three positive harnesses and fifteen
expected-rejection mutation fixtures. The production vecadd and Verus expand
the same control/index/guarded-memory/write body, but use different arithmetic
adapters. The Verus adapter is a total model operation; it does not refine
production IEEE `f32` addition, compiler lowering, HSACO, or GPU execution.

## Row Definition of Done

A row can become **Complete** only when all applicable requirements pass:

1. The public semantics and unsupported cases are documented.
2. Target-neutral unit tests pass without a GPU.
3. Import, IR verification, lowering, and artifact tests cover the feature.
4. Compile-pass and compile-fail tests cover its safe/unsafe boundary.
5. CPU/GPU differential tests pass on required AMD target families.
6. Generated ABI and metadata are inspected when the row affects launch or
   layout.
7. Sanitizer and debugger jobs cover memory/synchronization behavior when
   applicable.
8. Verification obligations and assurance level are defined; a compiler test
   is never mislabeled as a Verus proof.
9. The parity matrix links to the owning tests in the future generated status
   report.
10. N/A rows have a deterministic diagnostic or documented absence.

Partial cuda-oxide baseline rows need only match the behavior and limitations
at the pinned commit; broader support is an extension. AMD-equivalent rows must
include an explicit semantic-difference note in their user documentation.

## Parity Release Rule

The project may announce parity with this baseline only when:

- all Exact and AMD-equivalent normative rows whose baseline status is Full are
  Complete;
- Partial rows meet at least the pinned partial scope;
- Experimental rows meet the demonstrated pinned scope and remain labeled
  experimental where appropriate;
- Planned rows are not required until cuda-oxide implements them, but fe2o3's
  chosen status is explicit;
- every N/A row has been reviewed against the selected AMD target set;
- supplemental rows S01-S15 are Complete or carry a public, approved exception;
- G8 release evidence is archived with compiler, ROCm, driver, and hardware
  identities.

Verus is an additional fe2o3 objective, not a cuda-oxide parity shortcut.
Verification progress is reported independently by property tier and cannot be
used to waive compiler or runtime parity rows.
