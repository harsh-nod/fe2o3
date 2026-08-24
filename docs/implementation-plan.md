# fe2o3 Implementation Plan

Status: living plan with historical MVP milestones.

The architecture source of truth is [architecture-v2.md](architecture-v2.md).
Milestone status in this file describes bounded implemented profiles; it does
not imply general Rust coverage, Verus-to-machine refinement, or cuda-oxide
parity.

## Goal

The original MVP goal was a native Rust backend for AMD GPUs that could compile
and run a single combined host plus device Rust file:

```rust
#[kernel]
pub fn vecadd(a: &[f32], b: &[f32], mut c: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    if let Some(out) = c.get_mut(idx) {
        *out = a[idx.get()] + b[idx.get()];
    }
}

fn main() {
    // allocate buffers, load generated HSACO, launch vecadd, verify output
}
```

That success criterion has been reached for bounded profiles. The current goal
is to replace profile-specific compiler, artifact, and launch paths with the
general typed architecture while retaining exact source, ABI, proof, target,
toolchain, machine-code, and runtime evidence boundaries.

## Architecture

The target architecture follows the proven cuda-oxide shape, with AMD-specific
compiler and runtime pieces:

```text
Rust source
  |
  v
rustc frontend: parse, typecheck, MIR, monomorphization
  |
  +-- host path --> normal rustc LLVM backend --> native executable
  |
  +-- device path
        |
        v
     collect #[kernel] roots and reachable no_std functions
        |
        v
     canonical MIR model -> Pliron mir.* -> target-neutral dialect ladder
        |
        v
     gpu.* -> AMDGPU lowering -> dialect-only Pliron llvm.*
        |
        v
     fe2o3 canonical finalizer handoff and evidence
        |
        v
     pinned upstream LLVM 22.1.8 target-machine APIs -> relocatable ELF
        |
        v
     in-process LLD library APIs -> HSACO
        |
        v
     embedded path or sidecar artifact loaded by fe2o3-core
```

The production-directed device finalizer runs in the isolated LLVM worker and
uses pinned upstream LLVM 22.1.8 for parsing, linking, optimization, target-
machine code generation, and native in-process LLD linking. It is the sole
machine authority. It does not use COMGR and does not shell out to `clang`,
`llc`, or `ld.lld`. Early elementwise prototypes used ROCm command-line clang
and `ld.lld`; references to that path below are historical compatibility notes,
not the target architecture.

The 2026-08-18 ownership refactor implements the canonical model/API boundaries,
the pinned Pliron D0 context, identity, registration, and pass-plan shell,
seven target-neutral dialect shells, a feature-gated `mir.*` Pliron shell, an
opaque context-bound exact-byte KIR envelope, bounded detached MIR-to-kernel
and kernel-to-GPU lowering services, compiler routing
contracts, and inert host/service contracts. It does not yet connect the
general device path in the diagram.
The working compiler remains the existing `rustc-codegen-fe2o3` composition.
Unset selection enters the general `production-v1` transaction; the legacy
recognizer and bounded Kernel IR routes are explicit qualification tools only.
The closed scalar slice now uses dialect-only `pliron-llvm` with
`default-features = false`. Live graph-derived extraction (`62e66209e`),
deterministic bounded LLVM assembly (`3a3b43e90`), the inert attempt-scoped
request bridge (`cb571012f`), hardened Worker (`fd6520d88`), exact inspector
(`70f9c5ad7`), measured-HSACO gate (`e016833d3`), move-only custody
(`c9e8ca702`), sealed join (`62efd243e`), and runtime-alignment correction
(`228c88ed9`) are implemented. The bridge remains non-authoritative. The
dedicated join crate consumes one authorized execution and requires ROCr's
runtime kernarg alignment 16 even though the COV6 descriptor records alignment
8 for its 280-byte 24+256 layout.
Issues [#134](https://github.com/harsh-nod/fe2o3/issues/134) and
[#135](https://github.com/harsh-nod/fe2o3/issues/135) remain open.

The general initial runtime uses HIP's module API:

- `hipModuleLoad` or `hipModuleLoadData`
- `hipModuleGetFunction`
- `hipModuleLaunchKernel`
- `hipMalloc`, `hipMemcpyAsync`, streams, synchronization

The bounded scalar closure additionally uses a dedicated sealed HSA/ROCr
consumer. It reuses reviewed low-level adapters but does not make that
profile-specific join a general replacement for the HIP runtime.

## IR Strategy

Use the pinned Pliron workspace for the target compiler pipeline.

Pliron is the closest fit for the cuda-oxide-derived design because the source
pipeline is already Rust-native: MIR is imported into a custom IR, then lowered
to an LLVM-like dialect before textual LLVM IR export. For fe2o3, the key work is
making that final target AMDGPU instead of NVPTX.

Melior remains a good later option if we want to move more of the stack into
standard MLIR dialects:

- `gpu`, `amdgpu`, `rocdl`, `llvm`
- MLIR pass pipelines
- MLIR verifier coverage

Do not start with Melior unless the bounded Pliron route blocks on required IR
semantics. The selected path is typed `pliron-llvm` dialect construction,
fe2o3 canonical V2 extraction, and fe2o3 deterministic LLVM assembly, not the
upstream `pliron-llvm` LLVM-C exporter.

The current D0 closure is Pliron v0.17.0 commit
`2610651306ea3ba670f68d5d8b1e1159bcd521ed`. `fe2o3-pliron` provides a real
context, private identity anchor, explicit bounded registration, and bounded
pass-plan validation. It deliberately withholds generic pass execution because
upstream pointers are contextless. The current graph admits `pliron-llvm` only
with `default-features = false`. The bounded scalar crate constructs and
verifies real dialect operations, but no landed Pliron route completes a
production kernel or replaces the direct upstream LLVM and in-process LLD
finalizer.

The target path is a selective `pliron-llvm` integration for the `llvm.*`
dialect and its lowering only. Every dependency uses
`default-features = false`; the optional `llvm-sys` converter is excluded from
the producer and the isolated worker. fe2o3, not Pliron objects or printer
output, owns canonical V2, stable identities, receipts, evidence, and the
bounded deterministic LLVM-assembly serializer. The isolated measured
upstream LLVM 22.1.8 target machine and in-process LLD remain the sole machine
authority.

For the implemented scalar slice, the embedded backend fixture is structurally
parsed before its operations, operands, results, types, and CFG become the live
graph. An exact validated V1 sidecar still supplies the AMD calling convention,
target attributes, module metadata, and evidence because upstream v0.17.0 lacks
those dialect properties. The attempt-scoped bridge preserves that combined
identity chain but grants no worker, object, link, publication, load, or launch
authority. The backend fixture is not Rust user source.

## Crate Responsibilities

- `cargo-fe2o3`: user command, backend discovery, build/run orchestration.
- `fe2o3-artifact-transaction`: rustc-independent artifact ownership and publication protocol.
- `rustc-codegen-fe2o3`: rustc codegen backend and HSACO toolchain helpers.
- `fe2o3-compiler-api`: target-neutral compile request/result contracts.
- `fe2o3-compiler-driver`: fail-closed single-route API dispatch; not yet the
  production selector.
- `fe2o3-pliron-scalar-add-v1`: exact backend-fixture lineage, checkout policy,
  scalar finalizer join, and sealed one-shot HSA execution; not a general
  backend, source frontend, approval service, or runtime policy.
- `fe2o3-legacy-compiler`: dormant adapter contract for the existing compiler
  owner; it contains no codegen implementation.
- `fe2o3-macros`: `#[kernel]` and future device extern annotations.
- `reserved-fe2o3-symbols`: shared reserved symbol namespace.
- `fe2o3-device`: no-std device API and intrinsic stubs.
- `fe2o3-core`: HIP-backed context, stream, memory, module, and launch runtime.
- `fe2o3-host`: launch macro and host ergonomics.
- `fe2o3-mir-model`: canonical Pliron-independent MIR semantics and
  transformations.
- `dialect-mir`: compatibility facade over that model and a bounded
  feature-gated Pliron `mir.*` shell.
- `fe2o3-pliron`: pinned context, private identity, registration, verifier, and
  non-executing pass-plan shell.
- `dialect-kernel`, `dialect-schedule`, `dialect-tile`, `dialect-gpu`,
  `dialect-proof`, `dialect-dispatch`, `dialect-autotune`: target-neutral,
  representation-only Pliron shells.
- `fe2o3-kir-pliron-bridge`: opaque context-bound exact canonical KIR V1-V5
  byte envelope with a redundant checked Pliron projection.
- `fe2o3-lower-mir-kernel`, `fe2o3-lower-kernel-gpu`: bounded target-neutral
  detached lowering services; neither is an in-tree Pliron pass or production
  pipeline selector.
- `fe2o3-amdgcn-model`: existing AMDGPU intrinsic and strict lowering model.
- `dialect-amdgcn`: historical compatibility re-export; not yet an AMD Pliron
  dialect.
- `fe2o3-proof-contracts`: solver-neutral proof statement/evidence contracts.
- `fe2o3-host-api`: inert target-neutral host-operation contracts.
- `fe2o3-service-model`: executable-free persistent-service semantics.
- `fe2o3-service-host`: authority-free service lifecycle typestates; no service
  execution.

## Device ABI

The first ABI must be simple and explicit:

- Scalars pass by value.
- `&[T]` lowers to `(ptr addrspace(1), usize)`.
- `&mut [T]` and `DisjointSlice<T>` lower to `(ptr addrspace(1), usize)`.
- Kernel entry functions use `amdgpu_kernel`.
- Host launch packing must exactly match backend-generated kernel signatures.
- Device code is `no_std`; calls into `std` are compile errors.

The launch macro already packs slice-like values as pointer plus length. The
backend must generate matching entry signatures.

## AMDGPU Lowering MVP

The first lowering target is ordinary global-memory kernels using 1D indexing.

Required device stubs:

- `thread::thread_idx_x()` -> `llvm.amdgcn.workitem.id.x`
- `thread::block_idx_x()` -> `llvm.amdgcn.workgroup.id.x`
- `thread::block_dim_x()` -> dispatch packet workgroup size
- `sync::syncthreads()` -> `llvm.amdgcn.s.barrier`

Required LLVM IR properties:

- target triple: `amdgcn-amd-amdhsa`
- CPU: `FE2O3_TARGET` such as `gfx1100`, `gfx90a`, or `gfx942`
- kernel calling convention: `amdgpu_kernel`
- global buffer pointers in global address space where practical
- relocatable ELF emitted by pinned upstream LLVM target-machine APIs
- HSACO linked through in-process LLD library APIs from the same pinned build
- no COMGR or command-line compiler/linker in the production-directed path

## Milestones

### M0: Repo And Runtime Scaffold

Status: implemented.

- Workspace exists with core crates.
- HIP runtime compiles and links.
- `cargo-fe2o3 doctor` validates ROCm toolchain discovery.
- Vecadd example type-checks against the intended public API.

### M1: Backend Entry Point

Status: implemented for host delegation and kernel-count diagnostics.

- Build `librustc_codegen_fe2o3.so`.
- Load the backend through `-Z codegen-backend`.
- Delegate host codegen to rustc's normal LLVM backend.
- Add diagnostics proving the backend sees the crate and codegen units.

Acceptance:

- `cargo fe2o3 build -p fe2o3-vecadd` compiles host code through the custom
  backend without changing user source.

### M2: Kernel Discovery And Collection

Status: implemented for direct MIR calls, std rejection, intrinsic-stub skipping,
and deterministic collection dumps.

- Detect `fe2o3_kernel_*` functions generated by `#[kernel]`.
- Walk the MIR call graph from each kernel root.
- Reject reachable `std` functions.
- Collect local crate, `core`, and `fe2o3-device` functions needed by kernels.

Acceptance:

- A diagnostic dump lists `vecadd`, `thread::index_1d`, and all reachable device
  functions for the vecadd example.

### M3: Minimal AMDGPU LLVM IR

Status: MVP implemented for `f32`/`f64` elementwise expression kernel shapes.

- The backend validates supported kernel arguments from monomorphized MIR locals.
- The backend recognizes MIR body patterns for `output[index] = expr`, where
  expression leaves are read-only slice elements, scalar float arguments, or
  float literals, and expression nodes are `+`, `-`, `*`, `/`, or unary
  negation. Operands must match the output element type. Leaf-only stores such
  as `out[i] = x[i]` are also supported.
- Read-only slice inputs can use `ThreadIndex::offset(<constant>)` and
  `ThreadIndex::offset_signed(<constant>)` for simple shifted loads.
- Read-only slice inputs can use `ThreadIndex::stride(<constant>)` for simple
  strided loads.
- Read-only slice inputs can use
  `ThreadIndex::stride_offset(<constant>, <constant>)` for simple affine loads.
- Raw `usize` index arithmetic derived from `idx.get()` is recognized for
  constant add, subtract, and multiply patterns, including debug MIR
  `*WithOverflow` tuple results.
- Raw `usize` index arithmetic can combine two tracked affine index expressions
  for source and output indexes.
- `FE2O3_DUMP_MIR=1` imports the collected device MIR into a first
  Pliron-facing scaffold with local `mir.*` dialect names and dumps
  function/block/statement/terminator shape for lowering work. The scaffold
  also builds a flat typed `mir.*` operation-record stream for the future Pliron
  builder, including typed locals, statement destination and operand labels, and
  terminator call callee, destination, and operand labels, plus the first
  operation-specific lowering records such as `mir.assign`, `mir.load`,
  `mir.store`, `mir.gep`, `mir.slice_len`, arithmetic ops, comparisons, and
  casts. Evaluated integer constants are appended to constant operand labels
  when rustc can resolve them. The dump also builds a first record-driven
  lowering-plan summary from the flat record stream. The AMDGPU emission path
  consumes that plan to cross-check kernel argument types, required store/return
  ops, thread-index calls, record load coverage, and selected index/arithmetic
  shape markers before emitting through the existing MIR recognizer. Load/store
  record place labels are parsed into a small access sketch, helper/raw index
  records are parsed into a linear index sketch, and slice reads/writes are
  combined into a slice-access sketch keyed by ABI arg, MIR local, and affine
  index. The sketch tracks direct slice accesses plus
  `DisjointSlice::get_mut`/`get_mut_at` element references through option
  projection into the final deref load/store. The AMDGPU validator now checks
  read-only slice loads, direct `&mut [T]` output stores, and disjoint output
  read-before-write stores from that record-derived slice sketch. A record
  expression sketch also binds slice-load leaves, disjoint output element
  leaves, scalar args, float literals, unary/binary expression ops, and store
  roots so the validator can cross-check expression requirements. When that
  sketch can reconstruct the full expression root, the AMDGPU path now uses the
  record-derived `ElementwiseExpr` for LLVM IR emission; raw rustc MIR remains
  the temporary fallback for shape discovery the record plan does not yet own.
- The backend emits an AMDGPU LLVM IR `amdgpu_kernel` after validating the ABI
  and body pattern.
- The emitted IR uses `llvm.amdgcn.workitem.id.x` and
  `llvm.amdgcn.workgroup.id.x`.
- It matches the current host launch ABI: slice pointer plus `usize` length for
  slice arguments and by-value passing for scalar arguments.
- It bounds-checks the generated thread, shifted, strided, and affine indexes
  against every slice length before memory access.
- It supports `DisjointSlice<f32>` output and indexed `&mut [f32]` output,
  including in-place read-before-write updates.
- It assumes the current `LaunchConfig::for_num_elems` block size of 256.
- `vecadd` covers slice-plus-slice addition; `copy` covers a leaf-only store;
  `downsample` covers a constant-stride input load; `fill` covers a literal-root
  store; `gather-odd` covers a stride-plus-offset input load; `scale` covers
  scalar-times-slice multiplication; `shift` covers a positive constant-offset
  input load; `previous` covers a negative constant-offset input load; `stencil`
  covers multiple derived loads from one input slice; `raw-add-index` covers
  affine reads formed by adding two raw index expressions;
  `raw-const-minus` covers constant-minus-index reads with a negative stride;
  `raw-parenthesized-sub` covers index subtraction that collapses to a constant
  read index;
  `raw-disjoint-inplace-shift` covers raw `usize` arithmetic for a
  `DisjointSlice<f32>` output read-before-write store; `raw-disjoint-shift`
  covers raw `usize` arithmetic for a `DisjointSlice<f32>` output store;
  `raw-gather` covers raw affine `usize` index arithmetic; `raw-neighbors`
  covers raw `usize` add/sub neighbor reads; `raw-output-shift` covers raw
  `usize` arithmetic for an indexed mutable-slice output store; `saxpy` covers a
  two-op expression with four kernel arguments; `axpy-inplace` covers indexed
  mutable-slice in-place updates; `add-inplace` covers
  `DisjointSlice::get_mut` read-before-write; `negate` covers unary negation;
  `normalize` covers literal constants plus subtraction and division;
  `vecadd-f64` covers double-precision addition; `pipeline` covers two kernels
  emitted from one crate.

The selective Pliron scalar checkpoint is narrower and independent of that
legacy elementwise coverage. It structurally parses a backend fixture into a
real dialect load/add/store/return graph, extracts its live operands, results,
types, and CFG into canonical V2, and emits exact bounded LLVM assembly without
the `pliron-llvm` converter. Its validated V1 sidecar carries the AMD calling
convention, target attributes, module metadata, and evidence that the upstream
dialect cannot represent. The sealed route then finalizes and executes the
exact scalar once on MI300X. This is not a Rust user-source pipeline.

Remaining generalization:

- Adapt collected MIR from `fe2o3-mir-model` into the feature-gated Pliron
  `mir.*` shell.
- Run basic verification and mem2reg.
- Lower arithmetic, branches, loads/stores, pointer math, calls, and returns.
- Lower 1D thread-index intrinsics from device API calls instead of a fixed IR
  template.
- Export AMDGPU LLVM IR for kernels beyond the legacy elementwise profiles and
  the closed selective scalar slice.
- Generalize live-graph-to-V2 extraction and deterministic bounded assembly
  while keeping `pliron-llvm` at `default-features = false`; do not use its
  optional converter in either producer or worker.
- Generalize the exact #159/#161 scalar closure beyond its embedded fixture,
  fixed checkout policy, singleton geometry, one device lane, and one ROCr
  image. The current MI300X marker is
  `evidence=69238ad704470649b9811b41cf0194bb392be8116a1b0618adb1dcbe7e1bbd4f`
  with ROCr 1.18 image
  `7010eba894569c044749b71b63ff782080c4a91e19ff24d6dc93e857045ab37e`.
- Preserve more source-level debug metadata beyond kernel argument names.

The embedded checkout policy and success marker are self-consistent repository
evidence, not an external signature or CI attestation. This closure does not
establish CUDA-Oxide parity, general illegal-memory-access prevention, memory
safety, or race freedom.

Acceptance:

- `cargo fe2o3 build -p fe2o3-vecadd` and
  `cargo fe2o3 build -p fe2o3-scale` emit syntactically valid LLVM IR targeting
  `amdgcn-amd-amdhsa`; `cargo fe2o3 build -p fe2o3-saxpy` emits multiple
  floating-point operations in one kernel.

### M4: HSACO Generation

Status: historical command-line MVP implemented for elementwise sidecar
artifacts; superseded by the production-directed direct LLVM/LLD worker.

- The historical `legacy-v1` sidecar path compiled generated LLVM IR with ROCm
  command-line clang and linked with command-line `ld.lld -shared`.
- The current production-directed path parses and links modules, optimizes,
  emits the relocatable object with pinned upstream LLVM 22.1.8 target-machine
  APIs, and links HSACO through in-process LLD library APIs in the isolated
  worker. This worker remains the sole machine authority even after the
  dialect-only `pliron-llvm` integration.
- The current path uses neither COMGR nor shell compiler/linker invocations.
- The artifact is written under `FE2O3_HSACO_DIR`, which `cargo-fe2o3` sets to
  `target/fe2o3`.
- Direct backend invocations that compile kernels must set `FE2O3_HSACO_DIR`.
  The directory is a managed generated-artifact namespace; canonical `.ll`,
  `.o`, and `.hsaco` entries may be replaced or removed during reconciliation.
- Collection, MIR import, optional verification, lowering, compilation, and
  publication execute under one cooperating-writer transaction. A rejected
  codegen preflight invalidates the producer's prior artifacts and unowned
  generated artifacts before returning the compiler error.
- Compiler-side reconciliation begins only after rustc enters `codegen_crate`.
  The sealed manifest remains responsible for ensuring that files left by an
  earlier build are not launch authority when compilation fails before codegen.
- `llvm-readobj --notes` validates generated AMDGPU format, target metadata, and
  kernel name metadata when available.

Remaining generalization:

- Decide whether release artifacts should live next to the host executable,
  remain sidecars under `target/fe2o3`, or be embedded.
- Support monomorphized kernel names.

Acceptance:

- `cargo fe2o3 build -p fe2o3-vecadd` produces `vecadd.hsaco`, and
  `cargo fe2o3 build -p fe2o3-add-inplace` produces `add_inplace.hsaco`, and
  `cargo fe2o3 build -p fe2o3-copy` produces `copy.hsaco`, and
  `cargo fe2o3 build -p fe2o3-downsample` produces `downsample.hsaco`, and
  `cargo fe2o3 build -p fe2o3-fill` produces `fill.hsaco`, and
  `cargo fe2o3 build -p fe2o3-gather-odd` produces `gather_odd.hsaco`, and
  `cargo fe2o3 build -p fe2o3-scale` produces `scale.hsaco`, and
  `cargo fe2o3 build -p fe2o3-shift` produces `shift.hsaco`, and
  `cargo fe2o3 build -p fe2o3-previous` produces `previous.hsaco`, and
  `cargo fe2o3 build -p fe2o3-stencil` produces `stencil.hsaco`, and
  `cargo fe2o3 build -p fe2o3-raw-add-index` produces `raw_add_index.hsaco`, and
  `cargo fe2o3 build -p fe2o3-raw-const-minus` produces
  `raw_const_minus.hsaco`, and
  `cargo fe2o3 build -p fe2o3-raw-parenthesized-sub` produces
  `raw_parenthesized_sub.hsaco`, and
  `cargo fe2o3 build -p fe2o3-raw-disjoint-inplace-shift` produces
  `raw_disjoint_inplace_shift.hsaco`, and
  `cargo fe2o3 build -p fe2o3-raw-disjoint-shift` produces
  `raw_disjoint_shift.hsaco`, and
  `cargo fe2o3 build -p fe2o3-raw-gather` produces `raw_gather.hsaco`, and
  `cargo fe2o3 build -p fe2o3-raw-neighbors` produces `raw_neighbors.hsaco`, and
  `cargo fe2o3 build -p fe2o3-raw-output-shift` produces
  `raw_output_shift.hsaco`, and
  `cargo fe2o3 build -p fe2o3-saxpy` produces `saxpy.hsaco`, and
  `cargo fe2o3 build -p fe2o3-axpy-inplace` produces `axpy_inplace.hsaco`, and
  `cargo fe2o3 build -p fe2o3-negate` produces `negate.hsaco`, and
  `cargo fe2o3 build -p fe2o3-normalize` produces `normalize.hsaco`, and
  `cargo fe2o3 build -p fe2o3-vecadd-f64` produces `vecadd_f64.hsaco`.
- `cargo fe2o3 build -p fe2o3-pipeline` produces `scale_stage.hsaco` and
  `bias_stage.hsaco`.

### M5: First End-To-End Launch

Status: MVP implemented for the current elementwise examples.

- `cargo-fe2o3 run` sets `FE2O3_HSACO_DIR`.
- `cargo-fe2o3 build/run -p <package>` cleans explicit package artifacts first
  so the backend reruns and refreshes sidecar HSACO files.
- `cargo-fe2o3 smoke` runs the supported backend examples in sequence.
- If `FE2O3_TARGET` is not set, `cargo-fe2o3` tries to infer the target from
  `rocminfo`.
- The `vecadd`, `add-inplace`, `copy`, `downsample`, `fill`, `gather-odd`,
  `scale`, `shift`, `previous`, `stencil`, `raw-add-index`,
  `raw-const-minus`, `raw-parenthesized-sub`, `raw-disjoint-inplace-shift`,
  `raw-disjoint-shift`, `raw-gather`, `raw-neighbors`, `raw-output-shift`,
  `saxpy`, `axpy-inplace`, `negate`, `normalize`, `pipeline`, and
  `vecadd-f64` examples load their HSACO files from that directory.
- The examples use `fe2o3-core` to load modules, launch through HIP with the
  backend ABI, copy output back, and validate results.
- The path has run successfully on `gfx1201` with TheRock ROCm
  `7.13.0a20260509`.

Remaining work:

- Tighten runtime errors for missing HSACO, driver initialization failure, and
  kernel metadata mismatches.
- Add automated hardware coverage for at least one AMD GPU target.

Acceptance:

- `cargo fe2o3 run -p fe2o3-vecadd`, `cargo fe2o3 run -p fe2o3-add-inplace`,
  `cargo fe2o3 run -p fe2o3-copy`, `cargo fe2o3 run -p fe2o3-downsample`,
  `cargo fe2o3 run -p fe2o3-fill`, `cargo fe2o3 run -p fe2o3-gather-odd`,
  `cargo fe2o3 run -p fe2o3-scale`, `cargo fe2o3 run -p fe2o3-shift`,
  `cargo fe2o3 run -p fe2o3-previous`, `cargo fe2o3 run -p fe2o3-stencil`,
  `cargo fe2o3 run -p fe2o3-raw-add-index`,
  `cargo fe2o3 run -p fe2o3-raw-const-minus`,
  `cargo fe2o3 run -p fe2o3-raw-parenthesized-sub`,
  `cargo fe2o3 run -p fe2o3-raw-disjoint-inplace-shift`,
  `cargo fe2o3 run -p fe2o3-raw-disjoint-shift`,
  `cargo fe2o3 run -p fe2o3-raw-gather`,
  `cargo fe2o3 run -p fe2o3-raw-neighbors`,
  `cargo fe2o3 run -p fe2o3-raw-output-shift`,
  `cargo fe2o3 run -p fe2o3-saxpy`,
  `cargo fe2o3 run -p fe2o3-axpy-inplace`, `cargo fe2o3 run -p fe2o3-negate`,
  `cargo fe2o3 run -p fe2o3-normalize`, `cargo fe2o3 run -p fe2o3-pipeline`,
  and `cargo fe2o3 run -p fe2o3-vecadd-f64` print success on an AMD GPU.
- `cargo fe2o3 smoke` runs the same set successfully on an AMD GPU.

### M6: Usability And Coverage

- Generic kernel instantiation.
- Better symbol naming for monomorphized kernels.
- Atomics mapped to AMDGPU-compatible LLVM atomics and sync scopes.
- 2D/3D grid helpers.
- Shared/LDS memory.
- OCML/OCKL linkage for math operations.
- Clear diagnostics for unsupported MIR, unsupported Rust features, and ABI
  mismatches.

Acceptance:

- Add examples for atomics, 2D indexing, and simple reductions.

## Build Command Shape

Target command:

```bash
FE2O3_TARGET=gfx1100 cargo fe2o3 run -p fe2o3-vecadd
```

Compiler flags owned by `cargo-fe2o3`:

- `-Z codegen-backend=/path/to/librustc_codegen_fe2o3.so`
- `-Z mir-enable-passes=-JumpThreading`
- device artifact output directory
- target GPU architecture through `FE2O3_TARGET`

Jump threading must be disabled for device code because duplicating barrier
calls can break GPU synchronization semantics.

## Risks

- Rustc codegen backend APIs are unstable and will require a pinned nightly.
- The AMDGPU kernel ABI must match HIP module launch expectations exactly.
- Block/grid dimension reads from the dispatch packet are target-specific and
  need careful validation.
- Address-space mistakes can produce LLVM IR that compiles but misbehaves.
- Rust panics, formatting, allocation, and `std` calls need explicit device-side
  rejection or lowering policy.
- Wavefront semantics are not CUDA warp semantics; do not expose CUDA-shaped
  APIs without AMD-specific naming and behavior.

## Historical First-Lowering Task

The following was the immediate bootstrap task. Exact fill and vecadd structured
paths and several later bounded profiles are now implemented; current work is
tracked in the [implementation roadmap](implementation-roadmap-v2.md).

1. Move the current elementwise shape analysis from raw rustc MIR onto the
   record-driven lowering plan one piece at a time. The plan now carries typed
   locals, statement operation labels, call destinations/operands, parsed
   load/store access sketches, a linear index sketch, and a derived slice-access
   sketch for direct and disjoint read/write slice sources, plus a first
   expression sketch for leaves, literals, unary/binary ops, and store roots.
   The AMDGPU path now uses the record-derived expression root for emission when
   it is complete enough. The next slice should move `ElementwiseShape`
   output/source discovery itself off raw rustc MIR and onto the record-derived
   access/expression sketches.
2. Lower enough operations for the current elementwise kernel shapes: args,
   basic blocks, integer arithmetic, pointer arithmetic, loads/stores, branch,
   and return.
3. Lower `thread::index_1d` through AMDGPU workitem/workgroup intrinsics.
4. Preserve the current generated HSACO smoke test as the regression target.
