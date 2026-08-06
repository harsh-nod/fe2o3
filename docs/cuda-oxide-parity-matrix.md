# cuda-oxide Parity Matrix

Status: normative parity baseline for fe2o3 v2.

Pinned commits, row IDs, and current statuses are generated from
[`cuda-oxide-parity-status.tsv`](cuda-oxide-parity-status.tsv). Run
`scripts/parity-matrix.sh check` to validate this projection or
`scripts/parity-matrix.sh generate` after changing the source of truth.

## Baseline and Scope

<!-- parity-status:baseline:start -->
The fixed comparison point is the fetched cuda-oxide `origin/main` commit
`cd5ef3941d3347c7f6fcbfc78ef0fa7f4f179d87` from 2026-08-05. The primary
source is `cuda-oxide-book/appendix/supported-features.md` at that commit. Its
94 feature rows are reproduced below in the same category order, including
partial, experimental, planned, and N/A rows. The supplemental audit also
accounts for capabilities demonstrated elsewhere in the repository.

The fe2o3 current-state column is based on commit
`ea7ca6e4cce38ebe07083fb18d4bc5165e8eb048`.
<!-- parity-status:baseline:end -->

At that commit fe2o3 has a HIP runtime, explicit unsafe raw module and launch
paths, versioned kernel registration, reachable MIR collection, a canonical
target-neutral kernel IR and verifier, opt-in end-to-end verified-IR AMDGPU fill
and exact three-slice vecadd paths, a default narrow elementwise `f32`/`f64`
LLVM/HSACO emitter, fail-closed formal affine memory/race obligations for
modeled effects, artifact and proof schemas, generated `KernelMarkerV1` types,
exact loaded module/function authority, host argument-alias admission, bounded
HSACO inspection/finalization, event-backed asynchronous transfer lifetimes,
transactional artifact publication, and Verus vecadd and fill harnesses.
`#[kernel(typed)]` additionally connects one exact
`pub fn(&[f32], &[f32], DisjointSlice<f32>)` profile to a backend-generated,
canonical embedded artifact and safe load, prepare, synchronous launch, and
non-escapable scoped launch API. This narrow vertical is not a general compiler,
general typed module system, authenticated proof-carrying artifact, or
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
| Normative | 0 | 21 | 61 | 12 | 94 |
| Supplemental | 0 | 7 | 8 | 0 | 15 |
<!-- parity-status:counts:end -->

An IR type, schema, parser, or isolated proof is classified as **Partial** only
when it implements a meaningful part of the row; it does not stand in for
end-to-end compiler/runtime behavior.

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

- Rows 12, 17, 20, 24, and 25: the manifest ABI model and canonical
  target-neutral kernel IR represent part of the required type, control-flow,
  arithmetic, and cast semantics. Structured MIR lowering covers a tested
  vecadd-shaped subset. The opt-in `kernel-ir-v1` backend now takes the exact
  fill and three-slice vecadd kernels through translation, verification,
  legalization, G1 AMD lowering, transactional publication, and hardware
  execution. Compiler-generated host packing exists only for the exact typed
  vecadd profile; general Rust signatures and general AMDGPU lowering are
  absent.
- Rows 32, 33, and 35: `#[kernel]` emits strict V1 registration metadata tied to
  a direct function pointer and a deterministic, doc-hidden typed
  `KernelMarkerV1`; public kernels expose that marker publicly. Reachable helper
  collection, helper-call translation, and multiple kernels are exercised. For
  the exact vecadd signature, `#[kernel(typed)]` emits a public generated host
  module and a V2 typed registration. Full crate/kernel binding IDs are derived
  independently by the Cargo wrapper, macro, and backend and qualify private
  host/accessor symbols; a real two-rlib same-name link test rejects silent
  archive coalescing. The association is still a trusted compiler contract, and
  the narrow emitters are not a general compiler or multi-kernel bundle.
- Rows 36-38 and 41-43: one-source builds, AMDGPU LLVM/HSACO sidecars, diagnostic
  dumps, bounded HSACO inspection, project-local cleanup, and the opt-in exact
  fill and vecadd paths exist. The general pipeline, user-facing `inspect`
  command, and external-project orchestration do not.
- Rows 48, 49, and 60: one-dimensional `DisjointSlice` and `ThreadIndex` APIs,
  target-neutral launch-axis verification, and observed target/capability facts
  exist. Kernel IR derives formal affine regions, bounds, runtime-alias, and
  inter-invocation race obligations for modeled effects and fails closed on
  unsupported effects. The exact generated vecadd adapter authenticates its
  fixed launch contract and maps three runtime allocations to it; general
  launch extents and parameter/allocation mappings remain unauthenticated.
- Rows 78 and 79: for the exact public
  `fn(&[f32], &[f32], DisjointSlice<f32>)` profile, `#[kernel(typed)]`
  generates the public `<kernel>_gpu` module with `Kernel` and `Prepared`
  aliases. The backend embeds one
  canonical container holding the native payload, target, exact physical ABI,
  read/read/write effects, and one-dimensional launch contract. `Kernel::load`
  authenticates those embedded bytes against the observed context and exact
  profile. `prepare` checks context, equal nonzero lengths, u32 index geometry,
  resource limits, and alias admission while retaining the loaded authority and
  typed buffers. General typed signatures, compiler-derived Rust type/layout
  identities, and multi-kernel modules are not complete, so both rows remain
  Partial.
- Row 80: the general `launch!` macro remains an explicit unsafe raw-ABI escape
  hatch with compile-fail coverage. The generated vecadd module instead exposes
  safe `prepare(...).launch(...)`; the example contains no raw parameter pack,
  artifact pathname, or unsafe user launch. This is one fixed profile, not a
  general generated launch macro, so the row remains Partial.
- Row 81 and supplemental row S03: the generated vecadd `launch_scoped` API
  retains typed resource borrows, loaded authority, alias admission, and packed
  parameters through event completion or stronger stream quiescence. Its
  higher-ranked callback cannot return the in-flight operation. Generalized
  returnable borrowed or owned generated async operations, cancellation, and
  composition are incomplete, so both rows remain Partial.
- Supplemental rows S01-S05, S14, and S15: the corresponding bounded models,
  parsers, lifetime types, target query, exact proof-evidence matching, and
  focused UI tests exist. The typed vecadd backend now emits and embeds a
  canonical container, but its source identity is finalized LLVM IR and its
  Rust type/layout identities are deterministic opaque declarations rather
  than compiler-derived evidence. Transaction-held, descriptor-pinned snapshots
  keep finalized IR and HSACO in one generation even after republishing or
  pathname replacement. Verus proof binding/refinement is not authenticated
  into that artifact, general generated safe-launch integration is incomplete,
  and host-object embedding is limited to `x86_64-unknown-linux-gnu`.

## Normative 94-row Matrix

### Compiler: Memory Model

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 01 | HMM / Unified Memory Management | Full | AMD-equivalent | Missing | Fine-grained host/device shared allocations with capability checks; reference captures retain host lifetime and fail when the platform cannot provide coherent access | G3, G6 |
| 02 | Unified Struct ABI without `#[repr(C)]` | Full | Exact | Missing | Host and device use rustc-reported `repr(Rust)` layout, including padding and reordered fields | G2, G3 |
| 03 | Dynamic Layout Matching | Full | Exact | Missing | Layout importer records field offset order, size, alignment, variants, and explicit padding; ABI tests compare host and device views | G2 |
| 04 | Pointer Distance (`offset_from`) | Full | Exact | Missing | Signed/unsigned element and byte distances use pointee layout, provenance checks, and reject zero-sized pointees where Rust requires it | G2 |
| 05 | Volatile Load/Store | Full | Exact | Missing | Volatile survives import, optimization, LLVM export, and AMD instruction selection; mem2reg never promotes it | G2 |
| 06 | Bulk Copy (`copy_nonoverlapping`) | Full | Exact | Missing | Element counts scale by rustc layout, address spaces are preserved, overlap is an unsafe precondition, and LLVM/AMDGPU output is tested | G2 |

### Compiler: Type System

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 07 | Generics and Monomorphization | Full | Exact | Missing | Generic and const-generic kernels/helpers are collected at final use sites with deterministic symbols and cross-crate tests | G1, G2 |
| 08 | Enums (`Option`, `Result`, custom) | Full | Exact | Missing | Direct and niche layouts, discriminants, payloads, matches, and supported enum constants follow rustc layout | G2 |
| 09 | Struct Construction and Field Access | Full | Exact | Missing | Literals, projections, by-value parameters/returns, nested structs, and padding pass layout-differential tests | G2 |
| 10 | Array Types (`[T; N]`) | Full | Exact | Missing | Construction, constants, nested arrays, runtime/constant indexing, mutation, and padded element stride work | G2 |
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
| 27 | Bi-directional LTOIR Support | Full | AMD-equivalent | Missing | Rust calls AMDGPU bitcode/device objects and external device code calls exported Rust functions through a versioned COMGR/lld link contract | G6 |
| 28 | Device FFI (`extern "C"`) | Full | AMD-equivalent | Missing | Typed declarations preserve AMDGPU ABI, convergence/effect attributes, layouts, symbols, and diagnostics | G6 |
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
| 39 | LTOIR Linking | Full | AMD-equivalent | Missing | Link AMDGPU bitcode/relocatable device artifacts with deterministic provenance and option records | G6 |
| 40 | Float Math Intrinsics (libdevice) | Full | AMD-equivalent | Missing | Rust float methods map to OCML/OCKL or LLVM intrinsics with target, precision, denormal, and contraction policy tests | G4 |
| 41 | Pipeline Inspection | Full | Exact | Partial | `cargo fe2o3 pipeline` shows imported MIR, post-SSA IR, `gpu.*`, lowered LLVM IR, and artifact metadata | G1 |
| 42 | PTX Inspect | Full | AMD-equivalent | Partial | `cargo fe2o3 inspect` prints AMDGPU LLVM, disassembly/metadata, or selected bundle payload without executing | G1, G3 |
| 43 | Local Clean | Full | Exact | Partial | `cargo fe2o3 clean` safely removes only `target/fe2o3`; pinned cuda-oxide removes the full project target directory, and complete external-project build orchestration remains pending | G0 |
| 44 | Compute Sanitizer Wrapper | Full | AMD-equivalent | Missing | `cargo fe2o3 sanitize` invokes supported ROCm GPU sanitizers/checkers and clearly reports unavailable tools or checks | G8 |
| 45 | cuda-gdb Source Debugging | Full | AMD-equivalent | Missing | Debug build and `cargo fe2o3 debug` launch ROCgdb with kernel source locations | G8 |
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
| 53 | Device-Scope Atomics | Full | Exact | Missing | Integer and supported float RMW operations implement all Rust orderings at device/agent scope | G4, G7 |
| 54 | Block-Scope Atomics | Full | Exact | Missing | Workgroup-scope atomics use the correct AMD synchronization scope and reject unsupported operations/types | G4, G7 |
| 55 | System-Scope Atomics | Full | Exact | Missing | System-scope atomics operate only on eligible coherent allocations and preserve CPU/GPU ordering | G4, G6, G7 |
| 56 | `core::sync::atomic` Support | Full | Exact | Missing | Standard Rust atomics lower with documented default scope and complete ordering tests | G4 |

### Runtime Library: Shared Memory

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 57 | Static Shared Memory | Full | Exact | Missing | Const-sized aligned workgroup arrays lower to LDS with per-kernel accounting and initialization tracking | G4, G7 |
| 58 | Dynamic Shared Memory | Full | Exact | Missing | Launch-sized aligned LDS views are bounded by `PreparedLaunch` resource metadata | G3, G4, G7 |
| 59 | Distributed Shared Memory (DSMEM) | Full | N/A | N/A | No cross-workgroup LDS address mapping is promised; targets without a proven semantic equivalent reject the capability | G6 |

### Runtime Library: Thread and Synchronization

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 60 | Thread/Block/Grid Intrinsics | Full | Exact | Partial | Complete 3D workitem/workgroup IDs and dimensions plus branded linear/tiled indexes, with rank validation and runtime 2D row width bound to the indexed slice | G1, G4, G5 |
| 61 | Block Synchronization | Full | Exact | Missing | Workgroup barrier lowers correctly and carries convergence, scope, and memory semantics through IR | G4, G7 |
| 62 | Async Barriers (mbarrier) | Full | AMD-equivalent | Missing | Target-gated AMD split/named barrier abstraction exposes only semantics supported by the selected architecture | G6, G7 |
| 63 | Cluster Synchronization | Full | N/A | N/A | No CUDA thread-block-cluster promise; reject cluster-only kernels unless a future AMD target adds a modeled equivalent | G6 |
| 64 | Fence Operations | Full | AMD-equivalent | Missing | Provide scoped AMD fences and supported wait/sleep operations; CUDA proxy-only semantics are omitted or rejected | G4, G6 |

### Runtime Library: Warp and Cooperative Groups

| ID | cuda-oxide feature | Baseline | Class | fe2o3 now | AMD/fe2o3 acceptance target | Gate |
|:--|:--|:--|:--|:--|:--|:--|
| 65 | Warp Shuffle Operations | Full | AMD-equivalent | Missing | Wave shuffle/permutation operations support declared types and explicit wave32/wave64 width/active-lane contracts | G4, G7 |
| 66 | Warp Vote Operations | Full | AMD-equivalent | Missing | Wave all/any/ballot return width-correct masks and define inactive-lane behavior | G4, G7 |
| 67 | Lane/Warp ID | Full | AMD-equivalent | Missing | `lane_id` and wave ID use AMD semantics; no fixed width of 32 is assumed by portable code | G4 |
| 68 | Typed Group Handles | Full | AMD-equivalent | Missing | Provide `Grid`, `Workgroup`, `SubgroupTile<N>`, and active-lane groups; unsupported CUDA `Cluster` behavior is unavailable | G4, G6 |
| 69 | Group Universal API | Full | Exact | Missing | Every supported group has typed `size`, `thread_rank`, and legal synchronization behavior | G4 |
| 70 | Warp Tile Partitioning | Full | AMD-equivalent | Missing | Wave tiles are valid only for supported divisors and wave widths, with active-lane and convergence contracts | G4, G7 |
| 71 | Warp Collectives | Full | AMD-equivalent | Missing | Ballot, vote, shuffle, match, and active-mask operations cover baseline types with wave-width-correct semantics | G4, G7 |
| 72 | Warp Reductions / Scans | Full | AMD-equivalent | Missing | Wave reductions/scans cover the pinned operation/type matrix across supported widths | G4, G7 |
| 73 | Block Reductions / Scans | Full | Exact | Missing | Workgroup collectives use LDS and barriers, support the baseline operation/type matrix, and work across wave widths | G4, G7 |
| 74 | Cooperative Kernel Launch | Full | AMD-equivalent | Missing | HIP cooperative launch and grid synchronization are capability-checked, occupancy-safe, and encoded in the launch contract | G6, G7 |

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
| S06 | VMM, peer access, and multi-device memory | AMD-equivalent | Missing | HIP/HSA-supported virtual/peer memory has context, lifetime, topology, and capability checks | G6 |
| S07 | Device constants, statics, and relocations | Exact | Missing | Layout-aware constants/globals preserve supported pointer relocations and reject unsupported provenance | G2, G6 |
| S08 | Kernel families and compile-time policies | Exact | Missing | Tuned monomorphized variants share a typed logical interface and carry policy identity in the bundle | G2, G3 |
| S09 | Source debug metadata | Exact | Missing | Spans, functions, arguments, locals, and aggregate layouts survive supported optimization/debug modes | G2, G8 |
| S10 | Differential MIR/codegen fuzzer | Exact | Missing | Generated accepted programs compare CPU reference behavior and AMD execution; reducer preserves failures | G8 |
| S11 | Half/BF16 types and conversions | Exact | Missing | Scalar and packed formats, conversions, arithmetic, constants, ABI, and edge cases are tested | G2, G4 |
| S12 | Tensor/matrix instructions | AMD-equivalent | Missing | Capability-gated MFMA/WMMA abstractions cover supported shapes/types with ISA and numerical tests | G6 |
| S13 | LDS swizzles and matrix load/store helpers | AMD-equivalent | Missing | AMD-native layouts expose bank/alignment contracts and compose with proof-aware views | G6, G7 |
| S14 | Target auto-detection and override | AMD-equivalent | Partial | Detect AMD architecture and features, accept explicit override, and record the resolved target in every payload | G0, G3 |
| S15 | Compile-fail safety suite | Exact | Partial | UI tests cover launch brands, rank, index spaces, witness transfer, async lifetime, barrier lifecycle, and unsafe transitions | G0, G3, G5 |

The current Verus lane has two positive harnesses and twelve
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
