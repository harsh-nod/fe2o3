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
`cd5ef3941d3347c7f6fcbfc78ef0fa7f4f179d87` from 2026-08-05. The primary
source is `cuda-oxide-book/appendix/supported-features.md` at that commit. Its
94 feature rows are reproduced below in the same category order, including
partial, experimental, planned, and N/A rows. The supplemental audit also
accounts for capabilities demonstrated elsewhere in the repository.

The fe2o3 current-state column is based on commit
`37eee8f15b985190449ece7a93f4ab386aa3cb18`.
<!-- parity-status:baseline:end -->

Post-snapshot update: commit
`90b6fe31cbb1d89b82755f194ac7950c4eef4756` extends the bounded `gfx942`
Worker V2 path through a two-kernel compiler, artifact, proof-binding, host
selection, and HSA lifecycle spine without changing any row to Complete. One
external Cargo fixture declares two kernel roots and one shared helper. The
frontend assigns that helper one canonical identity, Kernel IR lowering checks
both internal calls against its exact signature, and AMDGPU lowering emits the
helper once. The sealed Cargo backend invokes the direct LLVM/LLD worker and
publishes one independently inspected HSACO containing exactly both entries.
The worker calls LLVM and LLD library APIs directly and does not use COMGR or
command-line linking.

The existing V1 artifact wire format now has a strict `gfx942` profile with two
canonically ordered entries over one digest-validated native payload. Each
entry has a separate proof binding over its kernel, ABI, effects, launch,
source, target, and shared executable identities. Host admission can select two
different compiler-generated marker types from that executable and rejects
name, binding, physical-layout, target, payload, effects, launch, and
executable substitution. The reviewed HSA adapter can resolve a fixed set of
distinct native symbols and retains them in a non-clone value that borrows the
loaded executable, so safe Rust cannot unload it while the set is live.

The evidence remains bounded. The MI300X Worker V2 test compiles, inspects, and
publishes the two-kernel `gfx942` code object but does not dispatch it. The
second host selection is deliberately inert, multi-symbol HSA tests establish
identity and lifetime rather than a generated typed ABI, and dispatch still
uses the exact vecadd kernarg profile. General manifest-derived packing, safe
dispatch of both entries, asynchronous composition, broad Rust signatures,
and machine-code refinement remain incomplete. The next bounded scope and exit
gate are defined by the
[general typed dispatch V1 contract](general-typed-dispatch-v1.md). The
generated dashboard and status blocks remain the older pinned evidence snapshot
until their separate evidence-admission lane updates them.

At `90b6fe3` fe2o3 also has a HIP runtime, explicit unsafe raw module and launch
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
`#[kernel(typed)]` still connects only one exact
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
| Normative | 0 | 45 | 37 | 12 | 94 |
| Supplemental | 0 | 10 | 5 | 0 | 15 |
<!-- parity-status:counts:end -->

An IR type, schema, parser, or isolated proof is classified as **Partial** only
when it implements a meaningful part of the row; it does not stand in for
end-to-end compiler/runtime behavior.

A post-snapshot read-only audit found qualifying bounded evidence to move rows
01, 26, 40, 51, and 87 from Missing to Partial and supplemental row S11 from
Missing to Partial. Row 47 remains Missing: the authenticated inline-assembly
lowering described for row 87 is not a public `amdgpu_asm!` macro with the
required operand and clobber surface. Once the authoritative status TSV is
updated by its owning lane, the expected projection is 0 Complete, 50 Partial,
32 Missing, and 12 N/A normative rows, plus 0 Complete, 11 Partial, and 4
Missing supplemental rows. This narrative does not move the generated status
or dashboard snapshot to `90b6fe3`; those changes require the full
evidence-generation gate.

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
  cross-kernel values. Safe generated value binding and launch integration are
  still limited to exact profiles; structs, closures, return values, and the
  full acceptance target are not complete.
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
  execution. Complete signed/unsigned operations, casts, overflow forms, and
  general source lowering remain absent.
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
  typed registration. Full crate/kernel binding IDs are derived independently
  by the Cargo wrapper, macro, and backend and qualify private host/accessor
  symbols; a real two-rlib same-name link test rejects silent archive
  coalescing. The backend also validates the normalized monomorphized signature
  and rejects a token-level `type f32 = f64` spoof. The association is still a
  trusted compiler contract, general signatures and cross-crate finalization
  are absent, and only the exact vecadd profile has a generated safe launch.
- Rows 35-38 and 41-43: one-source builds, AMDGPU LLVM/HSACO sidecars, diagnostic
  dumps, bounded HSACO inspection, a read-only `cargo fe2o3 inspect` command,
  complete external-project build/run orchestration, project-local cleanup, and
  the opt-in exact fill and vecadd paths exist. The sealed external-Cargo
  Worker V2 fixture also compiles two roots and one shared helper into one
  deterministically inspected and published `gfx942` payload; the canonical
  artifact profile indexes both entries over that payload and rejects duplicate
  or conflicting identities. Project build scripts and procedural macros
  remain trusted; pipeline inspection is not stage-complete, broad Rust
  semantics and cross-crate finalization are absent, and neither entry has a
  generated general dispatch path.
- Rows 27, 28, and 39: bounded device FFI macros and compiler validation bind
  import/export direction, exact symbols, physical scalar/pointer ABI,
  address spaces, effects, target, code-object version, and semantic identity.
  A standalone worker implements canonical Rust/C++ request/response codecs,
  LLVM bitcode linking, AMDGPU `TargetMachine` emission, and in-process LLD,
  with no COMGR or command-line linker dependency. In the post-snapshot
  `gfx942` Worker V2 slice, Cargo consumes an exact compiler-produced
  symbol-role manifest, requires byte-identical output from two worker
  executions, independently inspects the raw HSACO, and durably publishes it
  under the originating build attempt with a provenance receipt. The path now
  covers two kernel roots with one canonical shared helper and feeds a strict
  two-entry artifact profile with per-kernel proof bindings. Compiler origin
  authentication and compiler-to-machine-code refinement remain outside the
  claim. The MI300X ignored integration test establishes target-specific
  compile, inspection, and publication evidence, not two-kernel GPU execution
  or optimized-Release evidence.
- Rows 44 and 45: `cargo fe2o3 sanitize` and `debug` retain plan-only mode and
  can execute an exact descriptor-pinned native ROCgdb binary with bounded
  output, timeout, process cleanup, an environment allowlist, and diagnostic
  evidence. Precise-memory support is checked at execution and fails closed
  when unavailable. It is not a race, API, initialization, synchronization, or
  memory-safety proof; source-debug metadata and aggregate inspection remain
  unvalidated.
- Rows 48, 49, and 60: one-dimensional `DisjointSlice` and `ThreadIndex` APIs,
  target-neutral launch-axis verification, and observed target/capability facts
  exist. Kernel IR derives formal affine regions, bounds, runtime-alias, and
  inter-invocation race obligations for modeled effects and fails closed on
  unsupported effects. Compile-fail tests reject witness copying, transfer, and
  index-space mismatch. The exact generated vecadd adapter authenticates its
  fixed one-dimensional launch contract and maps three runtime allocations to
  it; complete 2D/3D branded construction, general launch extents, and general
  parameter/allocation mappings remain incomplete.
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
  absent.
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
  ABI, effects, launch, target, physical-layout, or executable identities. The
  second selection is inert and has no generated argument packer or dispatch
  method. General typed signatures, arbitrary Rust layouts, machine-code effect
  verification, and safe multi-kernel execution are incomplete, so both rows
  remain Partial.
- Row 80: the general `launch!` macro remains an explicit unsafe raw-ABI escape
  hatch with compile-fail coverage. The generated vecadd module instead exposes
  safe `prepare(...).launch(...)`; the example contains no raw parameter pack,
  artifact pathname, or unsafe user launch. The two-entry Worker V2 path does
  not yet generate equivalent methods or manifest-derived packing for either
  entry. This is one fixed safe profile, not a general generated launch macro,
  so the row remains Partial.
- Row 81 and supplemental row S03: the generated vecadd `launch_scoped` API
  retains typed resource borrows, loaded authority, alias admission, and packed
  parameters through event completion or stronger stream quiescence. Its
  higher-ranked callback cannot return the in-flight operation. Generalized
  returnable borrowed or owned generated async operations, cancellation, and
  composition are incomplete. The linear HSA kernel set prevents executable
  unload while resolved kernels are retained, but it does not add an async
  operation or typed dispatch for the second entry, so both rows remain
  Partial.
- Supplemental rows S01 and S02: the V1 container, bundle index, direct-link
  evidence, descriptor finalization, transactional publication, and durable
  recovery records form a canonical bounded artifact path. The `gfx942`
  profile carries two independently identified entries and two
  non-substitutable proof bindings over one digest-validated native payload.
  Descriptor-pinned snapshots retain finalized IR and HSACO in one generation
  across pathname replacement. This is not general compiler production,
  all-target loading, or machine-code refinement evidence.
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
  hardware execution are incomplete. This evidence does not satisfy row 47,
  which remains Missing because no baseline-equivalent public
  `amdgpu_asm!` macro exists.

## Normative 94-row Matrix

### Compiler: Memory Model

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 01 | HMM / Unified Memory Management | Full | AMD-equivalent | Missing | Fine-grained host/device shared allocations with capability checks; reference captures retain host lifetime and fail when the platform cannot provide coherent access | G3, G6 |
| 02 | Unified Struct ABI without `#[repr(C)]` | Full | Exact | Partial | Host and device use rustc-reported `repr(Rust)` layout, including padding and reordered fields | G2, G3 |
| 03 | Dynamic Layout Matching | Full | Exact | Partial | Layout importer records field offset order, size, alignment, variants, and explicit padding; ABI tests compare host and device views | G2 |
| 04 | Pointer Distance (`offset_from`) | Full | Exact | Missing | Signed/unsigned element and byte distances use pointee layout, provenance checks, and reject zero-sized pointees where Rust requires it | G2 |
| 05 | Volatile Load/Store | Full | Exact | Missing | Volatile survives import, optimization, LLVM export, and AMD instruction selection; mem2reg never promotes it | G2 |
| 06 | Bulk Copy (`copy_nonoverlapping`) | Full | Exact | Missing | Element counts scale by rustc layout, address spaces are preserved, overlap is an unsafe precondition, and LLVM/AMDGPU output is tested | G2 |

### Compiler: Type System

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 07 | Generics and Monomorphization | Full | Exact | Partial | Generic and const-generic kernels/helpers are collected at final use sites with deterministic symbols and cross-crate tests | G1, G2 |
| 08 | Enums (`Option`, `Result`, custom) | Full | Exact | Partial | Direct and niche layouts, discriminants, payloads, matches, and supported enum constants follow rustc layout | G2 |
| 09 | Struct Construction and Field Access | Full | Exact | Partial | Literals, projections, by-value parameters/returns, nested structs, and padding pass layout-differential tests | G2 |
| 10 | Array Types (`[T; N]`) | Full | Exact | Partial | Construction, constants, nested arrays, runtime/constant indexing, mutation, and padded element stride work | G2 |
| 11 | `CuSimd<T, N>` SIMD Type | Full | Exact | Missing | Neutral `GpuSimd<T, N>` offers equivalent lane construction/access and lowers legally on AMD targets | G2, G4 |
| 12 | ABI Scalarization | Full | Exact | Partial | Slices, references, closures, structs, and scalar fields are packed from the manifest and reconstructed exactly; no handwritten safe packing | G2, G3 |

### Compiler: Closures

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 13 | Move Closures (`FnOnce`) | Full | Exact | Missing | Captured values are monomorphized, layout-correct, passed by value, and callable in generic kernels | G2, G3 |
| 14 | Reference Closures (`Fn`/`FnMut`) | Full | Exact | Missing | Reference captures require an eligible shared-memory allocation, preserve borrow lifetime through completion, and fail closed otherwise | G2, G3 |
| 15 | Host-to-Device Closures | Full | Exact | Missing | Host-created captures and call shims compile through the device graph with typed launch packing | G2, G3 |
| 16 | Device-Internal Closures | Full | Exact | Missing | Device-created closures, captures, and calls lower without host ABI assumptions | G2 |

### Compiler: Control Flow

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 17 | Match Expressions (integer switch) | Full | Exact | Partial | Integer switches preserve Rust semantics and lower to legal AMDGPU control flow | G1, G2 |
| 18 | Match on Enums | Full | Exact | Missing | Variant tests, payload projections, and niche layouts work in nested control flow | G2 |
| 19 | For Loops (range, iterator, enumerate) | Full | Exact | Missing | MIR-desugared ranges, slice iteration, enumerate, nesting, and early exits compile and execute | G2 |
| 20 | While Loops / If-Else | Full | Exact | Partial | Arbitrary reducible baseline control flow works; support is no longer restricted to recognized elementwise shapes | G1, G2 |
| 21 | Break and Continue | Full | Exact | Missing | Loop exits and continue edges preserve values and pass nested-loop tests | G2 |
| 22 | Loop Unroll Annotations | Partial | Exact | Missing | Match the pinned baseline's supported full/partial unroll semantics and limits, with diagnostics for unsupported loop shapes | G2 |
| 23 | Monomorphization-Dead Branches | Partial | Exact | Missing | Collection, panic checks, and address-space analysis ignore only branches proved dead by the defined constant-folding policy | G2 |

### Compiler: Arithmetic and Casting

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 24 | 64-bit Arithmetic | Full | Exact | Partial | Signed/unsigned arithmetic, comparison, shifts, bitwise operations, overflow forms, and descriptor packing pass CPU/GPU differential tests | G1, G2 |
| 25 | Type Casting (all kinds) | Full | Exact | Partial | Integer/float widths, bitcasts, pointer casts, coercions, pointer/integer conversions, and provenance policy are explicit and tested | G2 |
| 26 | Packed bf16x2 FMA | Full | AMD-equivalent | Missing | Target-gated packed BF16 FMA uses an AMD intrinsic or a documented equivalent sequence with matching lane and rounding semantics | G4 |

### Compiler: Interop

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 27 | Bi-directional LTOIR Support | Full | AMD-equivalent | Partial | Rust calls AMDGPU bitcode/device objects and external device code calls exported Rust functions through a versioned direct LLVM/LLD link contract | G6 |
| 28 | Device FFI (`extern "C"`) | Full | AMD-equivalent | Partial | Typed declarations preserve AMDGPU ABI, convergence/effect attributes, layouts, symbols, and diagnostics | G6 |
| 29 | MathDx FFI (cuFFTDx / cuBLASDx) | Full | AMD-equivalent | Missing | Demonstrate equivalent in-kernel FFT and matrix-library integration where ROCm supplies device-callable artifacts; unsupported targets report the gap | G6 |
| 30 | Tile interop | Experimental | AMD-equivalent | Missing | AMD tile/SIMT kernels share allocations and HIP streams between kernels; intra-kernel interop remains experimental unless a stable AMD contract exists | G6 |
| 31 | Cross-Crate Kernels | Full | Exact | Missing | Library kernels and helpers finalize concrete monomorphizations in the application bundle | G1, G2, G3 |

### Compiler: Functions

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 32 | `#[kernel]` Attribute | Full | Exact | Partial | Multiple generic/non-generic entries generate stable metadata, AMD kernel calling convention, typed markers, and clear diagnostics | G0, G2, G3 |
| 33 | `#[device]` Helper Functions | Full | Exact | Partial | Reachable helpers, recursion policy, inlining attributes, calls, returns, and cross-crate definitions lower generally | G1, G2 |
| 34 | Standalone `#[device]` Functions | Full | Exact | Missing | Export device functions without a kernel root for external AMD device linking | G6 |
| 35 | Multi-Kernel Modules | Full | Exact | Partial | Multiple entries share one deterministic artifact bundle/module and deduplicate helpers; separate per-kernel HSACO is not final parity | G1, G3 |

### Compiler: Compilation Pipeline

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 36 | Unified Single-Source Compilation | Full | Exact | Partial | One Cargo command drives Verus when requested, normal host rustc, and device extraction from one executable source | G1, G3, G5 |
| 37 | PTX Output | Full | AMD-equivalent | Partial | General pipeline emits target-correct HSACO for the declared AMD target set; elementwise recognition is not the default path | G1 |
| 38 | NVVM IR Output | Full | AMD-equivalent | Partial | Emit inspectable, validated AMDGPU LLVM IR/bitcode with target and code-object policy recorded | G1, G6 |
| 39 | LTOIR Linking | Full | AMD-equivalent | Partial | Link AMDGPU bitcode/relocatable device artifacts with deterministic provenance and option records | G6 |
| 40 | Float Math Intrinsics (libdevice) | Full | AMD-equivalent | Missing | Rust float methods map to OCML/OCKL or LLVM intrinsics with target, precision, denormal, and contraction policy tests | G4 |
| 41 | Pipeline Inspection | Full | Exact | Partial | `cargo fe2o3 pipeline` shows imported MIR, post-SSA IR, `gpu.*`, lowered LLVM IR, and artifact metadata | G1 |
| 42 | PTX Inspect | Full | AMD-equivalent | Partial | `cargo fe2o3 inspect` prints AMDGPU LLVM, disassembly/metadata, or selected bundle payload without executing | G1, G3 |
| 43 | Local Clean | Full | Exact | Partial | `cargo fe2o3 clean` safely removes only guarded `target/fe2o3` output; pinned cuda-oxide removes the full project target directory | G0 |
| 44 | Compute Sanitizer Wrapper | Full | AMD-equivalent | Partial | `cargo fe2o3 sanitize` invokes supported ROCm GPU sanitizers/checkers and clearly reports unavailable tools or checks | G8 |
| 45 | cuda-gdb Source Debugging | Full | AMD-equivalent | Partial | Debug build and `cargo fe2o3 debug` launch ROCgdb with kernel source locations | G8 |
| 46 | cuda-gdb Local / Argument Inspection | Partial | AMD-equivalent | Missing | Match the pinned baseline's scalar, pointer/reference, struct, tuple, and array inspection scope in ROCgdb, with known gaps listed | G8 |
| 47 | `ptx_asm!` Macro | Partial | AMD-equivalent | Missing | `amdgpu_asm!` supports typed operands, outputs, clobbers, side-effect/convergence options, and baseline-equivalent limits where LLVM permits | G6 |

### Runtime Library: Safety

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 48 | `DisjointSlice<T, IndexSpace>` | Full | Exact | Partial | Index-space and allocation-aware writable view accepts only matching branded witnesses; safe writes are bounded and disjoint | G0, G3, G5 |
| 49 | `ThreadIndex<'kernel, IndexSpace>` | Full | Exact | Partial | Opaque, launch-branded, non-transferable, non-`Copy` witness with checked 1D/2D/3D constructors | G0, G3, G5 |
| 50 | Proof-carrying static views | Full | Exact | Missing | One checked tile/view grants statically bounded constant accesses without repeated checks, with compile-fail coverage | G5 |
| 51 | `PreparedLaunch<K>` | Full | Exact | Missing | Reusable geometry/resource proof is branded to kernel, artifact, context, dimensions, and capability set | G0, G3, G5 |
| 52 | `ManagedBarrier` Typestate | Full | Exact | Missing | Lifecycle misuse is a compile error; Verus separately proves participant and epoch obligations | G4, G7 |

### Runtime Library: Atomics

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 53 | Device-Scope Atomics | Full | Exact | Partial | Integer and supported float RMW operations implement all Rust orderings at device/agent scope | G4, G7 |
| 54 | Block-Scope Atomics | Full | Exact | Partial | Workgroup-scope atomics use the correct AMD synchronization scope and reject unsupported operations/types | G4, G7 |
| 55 | System-Scope Atomics | Full | Exact | Partial | System-scope atomics operate only on eligible coherent allocations and preserve CPU/GPU ordering | G4, G6, G7 |
| 56 | `core::sync::atomic` Support | Full | Exact | Missing | Standard Rust atomics lower with documented default scope and complete ordering tests | G4 |

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
| 62 | Async Barriers (mbarrier) | Full | AMD-equivalent | Missing | Target-gated AMD split/named barrier abstraction exposes only semantics supported by the selected architecture | G6, G7 |
| 63 | Cluster Synchronization | Full | N/A | N/A | No CUDA thread-block-cluster promise; reject cluster-only kernels unless a future AMD target adds a modeled equivalent | G6 |
| 64 | Fence Operations | Full | AMD-equivalent | Partial | Provide scoped AMD fences and supported wait/sleep operations; CUDA proxy-only semantics are omitted or rejected | G4, G6 |

### Runtime Library: Warp and Cooperative Groups

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 65 | Warp Shuffle Operations | Full | AMD-equivalent | Partial | Wave shuffle/permutation operations support declared types and explicit wave32/wave64 width/active-lane contracts | G4, G7 |
| 66 | Warp Vote Operations | Full | AMD-equivalent | Partial | Wave all/any/ballot return width-correct masks and define inactive-lane behavior | G4, G7 |
| 67 | Lane/Warp ID | Full | AMD-equivalent | Partial | `lane_id` and wave ID use AMD semantics; no fixed width of 32 is assumed by portable code | G4 |
| 68 | Typed Group Handles | Full | AMD-equivalent | Missing | Provide `Grid`, `Workgroup`, `SubgroupTile<N>`, and active-lane groups; unsupported CUDA `Cluster` behavior is unavailable | G4, G6 |
| 69 | Group Universal API | Full | Exact | Missing | Every supported group has typed `size`, `thread_rank`, and legal synchronization behavior | G4 |
| 70 | Warp Tile Partitioning | Full | AMD-equivalent | Partial | Wave tiles are valid only for supported divisors and wave widths, with active-lane and convergence contracts | G4, G7 |
| 71 | Warp Collectives | Full | AMD-equivalent | Partial | Ballot, vote, shuffle, match, and active-mask operations cover baseline types with wave-width-correct semantics | G4, G7 |
| 72 | Warp Reductions / Scans | Full | AMD-equivalent | Missing | Wave reductions/scans cover the pinned operation/type matrix across supported widths | G4, G7 |
| 73 | Block Reductions / Scans | Full | Exact | Missing | Workgroup collectives use LDS and barriers, support the baseline operation/type matrix, and work across wave widths | G4, G7 |
| 74 | Cooperative Kernel Launch | Full | AMD-equivalent | Partial | HIP cooperative launch and grid synchronization are capability-checked, occupancy-safe, and encoded in the launch contract | G6, G7 |

### Runtime Library: Debug

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 75 | `gpu_printf!` Macro | Full | AMD-equivalent | Missing | Formatted device output lowers through a supported ROCm device ABI with format/type checking | G4 |
| 76 | `gpu_assert!` Macro | Full | Exact | Missing | Failed assertions trap and, where supported, report message and source metadata without unwind | G4 |
| 77 | Debug Intrinsics | Full | AMD-equivalent | Missing | Clock, trap, breakpoint/debug trap, and supported profiling markers have target-gated AMD mappings | G4, G8 |

### Runtime Library: Kernel Launch

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 78 | `#[cuda_module]` Typed Launch | Full | Exact | Partial | A neutral module macro embeds bundles and generates typed sync/async methods from manifest entries | G3 |
| 79 | `#[launch_contract]` / `PreparedLaunch<K>` | Full | Exact | Partial | Contracts check rank, exact/bounded block shape, resources, capabilities, context, and kernel identity | G0, G3, G5 |
| 80 | `cuda_launch!` Macro | Full | Exact | Partial | `launch!` is explicitly unsafe for runtime-loaded raw functions and exposes complete obligations | G0, G3 |
| 81 | `cuda_launch_async!` Macro | Full | Exact | Partial | Raw lazy launch is unsafe; typed operations retain borrowed/owned resources through completion and cancellation | G3 |
| 82 | `#[launch_bounds]` | Full | AMD-equivalent | Missing | Emit and validate AMD flat workgroup-size/occupancy metadata with architecture-specific limits | G4 |
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
| 87 | Rust `asm!` macro | Planned | AMD-equivalent | Missing | Lower MIR inline assembly for AMDGPU when rustc/LLVM operand semantics can be preserved; separate from `amdgpu_asm!` | G6 |
| 88 | FP8 / MX Data Types | Planned | AMD-equivalent | Missing | Add target-gated AMD FP8 and supported microscaling formats with explicit layout, conversion, and matrix-operation tests | G6 |
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
| S08 | Kernel families and compile-time policies | Exact | Missing | Tuned monomorphized variants share a typed logical interface and carry policy identity in the bundle | G2, G3 |
| S09 | Source debug metadata | Exact | Missing | Spans, functions, arguments, locals, and aggregate layouts survive supported optimization/debug modes | G2, G8 |
| S10 | Differential MIR/codegen fuzzer | Exact | Partial | Generated accepted programs compare CPU reference behavior and AMD execution; reducer preserves failures | G8 |
| S11 | Half/BF16 types and conversions | Exact | Missing | Scalar and packed formats, conversions, arithmetic, constants, ABI, and edge cases are tested | G2, G4 |
| S12 | Tensor/matrix instructions | AMD-equivalent | Missing | Capability-gated MFMA/WMMA abstractions cover supported shapes/types with ISA and numerical tests | G6 |
| S13 | LDS swizzles and matrix load/store helpers | AMD-equivalent | Missing | AMD-native layouts expose bank/alignment contracts and compose with proof-aware views | G6, G7 |
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
