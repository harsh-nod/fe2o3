# fe2o3 Backend Roadmap

For the full milestone plan, see [implementation-plan.md](implementation-plan.md).

## Implemented In This Scaffold

- Project naming and reserved symbol namespace use `fe2o3`.
- `dialect-mir` defines the local `mir.*` operation/type naming seam that the
  MIR import scaffold can later back with Pliron operations.
- A HIP runtime wrapper can allocate buffers, copy data, load HSACO modules, look
  up kernels, and launch them with packed parameter arrays.
- `#[kernel]` marks device functions by renaming them to the reserved
  `fe2o3_kernel_*` namespace for later rustc collection.
- `cargo-fe2o3 doctor` validates ROCm/HIP toolchain discovery.
- `cargo-fe2o3 build` builds and loads `librustc_codegen_fe2o3.so`.
- `rustc-codegen-fe2o3` wraps `rustc_codegen_llvm` for host codegen and detects
  kernel candidates in rustc codegen units.
- The backend collects device-reachable MIR functions from `fe2o3_kernel_*`
  roots, skips intrinsic placeholder bodies, rejects actual `std` reachability,
  and dumps a deterministic collection summary.
- `FE2O3_DUMP_MIR=1` imports the collected device MIR into a small
  Pliron-facing scaffold and prints function, block, statement, and terminator
  shape without changing the current HSACO emission path. The scaffold also
  builds a flat typed `mir.*` operation-record stream for the future Pliron
  builder, including typed locals, statement destination and operand labels, and
  terminator call callee, destination, and operand labels, plus the first
  operation-specific lowering records such as `mir.load`, `mir.store`,
  `mir.gep`, `mir.slice_len`, arithmetic ops, comparisons, and casts. The dump
  also builds a first record-driven lowering-plan summary from the flat record
  stream. The AMDGPU emission path consumes that plan to cross-check kernel
  argument types, required store/return ops, thread-index calls, record load
  coverage, and selected index/arithmetic shape markers before emitting through
  the existing MIR recognizer. Load/store record place labels are parsed into a
  small access sketch so read-only slice loads and direct `&mut [T]` output
  stores can be checked by MIR local.
- `rustc-codegen-fe2o3` contains the first real backend utilities:
  - ABI validation for supported kernel arguments from monomorphized MIR locals.
  - A narrow MIR recognizer and AMDGPU LLVM IR emitter for `f32`/`f64` elementwise
    expression kernels using read-only slice operands, scalar operands, one
    mutable output slice, in-place reads from that output slice, float literal
    constants, unary negation, and leaf-only copy stores.
  - `.ll -> .o -> .hsaco` using ROCm clang and `ld.lld`.
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

1. Replace the temporary elementwise MIR recognizer/emitter with the first
   Pliron import/lowering path:
   `MIR -> Pliron dialect-mir -> AMDGPU LLVM dialect/export -> LLVM IR`.
2. Replace device stubs in `fe2o3-device` with lowering rules:
   - `thread::thread_idx_*` -> `llvm.amdgcn.workitem.id.*`
   - `thread::block_idx_*` -> `llvm.amdgcn.workgroup.id.*`
   - `sync::syncthreads` -> `llvm.amdgcn.s.barrier`
   - `block_dim_*` and grid dimensions -> dispatch packet reads
3. Define the device kernel ABI explicitly:
   - Rust slices lower to pointer plus `usize` length.
   - `DisjointSlice<T>` lowers to mutable pointer plus `usize` length.
   - Plain scalars pass by value.
4. Generalize artifact placement beyond sidecar files in
   `target/fe2o3`.
5. Add a repeatable hardware test target for the generated host binary plus
   HSACO path.

## Runtime ABI Assumption

The launch macro currently packs slice-like values as two HIP kernel arguments:
device pointer then `usize` length. The compiler backend should generate matching
kernel entry signatures.
