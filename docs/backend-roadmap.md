# fe2o3 Backend Roadmap

## Implemented In This Scaffold

- Project naming and reserved symbol namespace use `fe2o3`.
- A HIP runtime wrapper can allocate buffers, copy data, load HSACO modules, look
  up kernels, and launch them with packed parameter arrays.
- `#[kernel]` marks device functions by renaming them to the reserved
  `fe2o3_kernel_*` namespace for later rustc collection.
- `cargo-fe2o3 doctor` validates ROCm/HIP toolchain discovery.
- `rustc-codegen-fe2o3` contains the first real backend utility:
  `.ll -> .o -> .hsaco` using ROCm clang and `ld.lld`.

## Next Compiler Milestones

1. Add a rustc codegen backend entry point that delegates host codegen to the
   standard LLVM backend and collects `fe2o3_kernel_*` instances for device code.
2. Port cuda-oxide's MIR collection shape, but keep the AMD path target-specific:
   `MIR -> Pliron dialect-mir -> AMDGPU LLVM dialect/export -> LLVM IR`.
3. Replace device stubs in `fe2o3-device` with lowering rules:
   - `thread::thread_idx_*` -> `llvm.amdgcn.workitem.id.*`
   - `thread::block_idx_*` -> `llvm.amdgcn.workgroup.id.*`
   - `sync::syncthreads` -> `llvm.amdgcn.s.barrier`
   - `block_dim_*` and grid dimensions -> dispatch packet reads
4. Define the device kernel ABI explicitly:
   - Rust slices lower to pointer plus `usize` length.
   - `DisjointSlice<T>` lowers to mutable pointer plus `usize` length.
   - Plain scalars pass by value.
5. Teach `cargo-fe2o3 build/run` to load `librustc_codegen_fe2o3.so`, pass the
   required MIR flags, emit HSACO next to the host binary, and run the result.

## Runtime ABI Assumption

The launch macro currently packs slice-like values as two HIP kernel arguments:
device pointer then `usize` length. The compiler backend should generate matching
kernel entry signatures.
