# fe2o3 Backend Roadmap

For the full milestone plan, see [implementation-plan.md](implementation-plan.md).

## Implemented In This Scaffold

This inventory includes the historical elementwise MVP. The
production-directed architecture now uses an isolated worker backed by one
pinned upstream LLVM build: LLVM target-machine APIs emit relocatable objects
and in-process LLD library APIs link HSACO. That path uses neither COMGR nor
shell invocations of `clang`, `llc`, or `ld.lld`.

- Project naming and reserved symbol namespace use `fe2o3`.
- `dialect-mir` defines the local `mir.*` operation/type naming seam that the
  MIR import scaffold can later back with Pliron operations.
- A HIP runtime wrapper can allocate buffers, copy data, load HSACO modules, look
  up kernels, and launch them with packed parameter arrays.
- `#[kernel]` emits strict V1 registration metadata with a direct function
  pointer. The collector rejects malformed, duplicate, inconsistent, or
  unregistered prefix-only candidates transactionally.
- `cargo-fe2o3 doctor` validates ROCm/HIP toolchain discovery.
- `cargo-fe2o3 build` builds and loads `librustc_codegen_fe2o3.so`.
- `rustc-codegen-fe2o3` wraps `rustc_codegen_llvm` for host codegen and detects
  kernel candidates in rustc codegen units.
- The backend collects device-reachable MIR functions from validated
  registrations, skips intrinsic placeholder bodies, rejects actual `std`
  reachability, and dumps a deterministic collection summary.
- `FE2O3_DUMP_MIR=1` imports the collected device MIR into a small
  Pliron-facing scaffold and prints function, block, statement, and terminator
  shape without changing the current HSACO emission path. The scaffold also
  builds a flat typed `mir.*` operation-record stream for the future Pliron
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
- `rustc-codegen-fe2o3` contains the first real backend utilities:
  - ABI validation for supported kernel arguments from monomorphized MIR locals.
  - A narrow MIR recognizer and AMDGPU LLVM IR emitter for `f32`/`f64` elementwise
    expression kernels using read-only slice operands, scalar operands, one
    mutable output slice, in-place reads from that output slice, float literal
    constants, unary negation, and leaf-only copy stores.
  - the historical `legacy-v1` `.ll -> .o -> .hsaco` sidecar path using ROCm
    command-line clang and `ld.lld`; this is compatibility history, not the
    production-directed finalizer.
- The production-directed direct LLVM/LLD worker parses and links modules,
  optimizes, emits relocatable ELF through pinned upstream LLVM target-machine
  APIs, and links HSACO through in-process LLD library APIs. It does not use
  COMGR or a command-line compiler or linker.
- `FE2O3_CODEGEN_PIPELINE=kernel-ir-v1` selects the first integrated G1 path:
  imported device MIR is translated to canonical kernel IR, verified, strictly
  legalized for the exact 1D `fill` shape, lowered by `dialect-amdgcn`, and
  published through the existing transactional LLVM/object/HSACO path. Invalid
  selectors and unsupported selected inputs fail without legacy fallback and
  remove stale artifacts. The default remains `legacy-v1` while coverage is
  extended.
- `dialect-amdgcn` lowers that verified fill subset to deterministic AMDGPU
  LLVM. Its code-object regression checks target/features, ELF and metadata
  versions, exact kernel symbol and descriptor, ABI, address space, and fixed
  workgroup metadata. Unsupported IR fails with located diagnostics.
- `cargo-fe2o3 build/run` writes `.ll` and `.hsaco` artifacts under
  `target/fe2o3`; `fe2o3-copy` covers a leaf-only store,
  `fe2o3-downsample` covers a constant-stride input load,
  `fe2o3-fill` covers a literal-root store,
  `fe2o3-gather-odd` covers a stride-plus-offset input load,
  `fe2o3-shift` covers a constant-offset input load,
  `fe2o3-previous` covers a negative constant-offset input load,
  `fe2o3-stencil` covers multiple derived loads from one input slice,
  `fe2o3-raw-add-index` covers affine reads formed by adding two raw index
  expressions,
  `fe2o3-raw-const-minus` covers constant-minus-index reads with a negative
  stride,
  `fe2o3-raw-parenthesized-sub` covers index subtraction that collapses to a
  constant read index,
  `fe2o3-raw-disjoint-inplace-shift` covers raw `usize` arithmetic for a
  `DisjointSlice<f32>` output read-before-write store,
  `fe2o3-raw-disjoint-shift` covers raw `usize` arithmetic for a
  `DisjointSlice<f32>` output store,
  `fe2o3-raw-gather` covers raw affine `usize` index arithmetic,
  `fe2o3-raw-neighbors` covers raw `usize` add/sub neighbor reads,
  `fe2o3-raw-output-shift` covers raw `usize` arithmetic for an indexed
  `&mut [f32]` output store,
  `fe2o3-saxpy` covers a multi-op expression tree, and
  `fe2o3-axpy-inplace` covers indexed `&mut [f32]` output with read-before-write.
- `fe2o3-add-inplace` covers `DisjointSlice::get_mut` output read-before-write.
- `fe2o3-negate` covers `fneg` emission from MIR unary negation.
- `fe2o3-normalize` covers `f32` literal constants, `fsub`, and `fdiv`.
- `fe2o3-vecadd-f64` covers double-precision elementwise emission.
- The `vecadd`, `add-inplace`, `copy`, `downsample`, `fill`, `gather-odd`,
  `scale`, `shift`, `previous`, `stencil`, `raw-add-index`,
  `raw-const-minus`, `raw-parenthesized-sub`, `raw-disjoint-inplace-shift`,
  `raw-disjoint-shift`, `raw-gather`, `raw-neighbors`, `raw-output-shift`,
  `saxpy`, `axpy-inplace`, `negate`, `normalize`, `pipeline`, and
  `vecadd-f64` examples load their HSACO files from `FE2O3_HSACO_DIR`, which is
  set by `cargo-fe2o3 build/run`.
- `cargo-fe2o3 build/run -p <package>` cleans explicit package artifacts before
  invoking Cargo so device sidecars are regenerated predictably.
- `cargo-fe2o3 smoke` runs the supported backend examples in sequence.
- Generated HSACO files are validated with `llvm-readobj --notes` when available
  to confirm AMDGPU format, target metadata, and kernel name metadata.
- `cargo-fe2o3` infers `FE2O3_TARGET` from `rocminfo` when the environment
  variable is not set.
- End-to-end `vecadd`, `add-inplace`, `copy`, `downsample`, `fill`,
  `gather-odd`, `scale`, `shift`, `previous`, `stencil`, `saxpy`,
  `raw-add-index`, `raw-const-minus`, `raw-parenthesized-sub`,
  `raw-disjoint-inplace-shift`, `raw-disjoint-shift`, `raw-gather`,
  `raw-neighbors`, `raw-output-shift`, `axpy-inplace`, `negate`, `normalize`,
  `pipeline`, and `vecadd-f64` have run successfully on `gfx1201` using TheRock
  ROCm `7.13.0a20260509`.

## Next Compiler Milestones

1. Extend the integrated `kernel-ir-v1` path from fill to vecadd and then every
   current example, preserving strict rejection and transactional cleanup,
   before making it the default and removing the temporary elementwise
   recognizer.
2. Move the remaining legacy `ElementwiseShape` output/source discovery off raw
   rustc MIR and onto
   the record-derived access/expression sketches.
3. Replace device stubs in `fe2o3-device` with lowering rules:
   - `thread::thread_idx_*` -> `llvm.amdgcn.workitem.id.*`
   - `thread::block_idx_*` -> `llvm.amdgcn.workgroup.id.*`
   - `sync::syncthreads` -> `llvm.amdgcn.s.barrier`
   - `block_dim_*` and grid dimensions -> dispatch packet reads
4. Define the device kernel ABI explicitly:
   - Rust slices lower to pointer plus `usize` length.
   - `DisjointSlice<T>` lowers to mutable pointer plus `usize` length.
   - Plain scalars pass by value.
5. Generalize artifact placement beyond sidecar files in
   `target/fe2o3`.
6. Add a repeatable hardware test target for the generated host binary plus
   HSACO path.

## Runtime ABI Assumption

The launch macro currently packs slice-like values as two HIP kernel arguments:
device pointer then `usize` length. The compiler backend should generate matching
kernel entry signatures.
