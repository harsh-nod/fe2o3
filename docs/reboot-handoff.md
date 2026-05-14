# Reboot Handoff

This file captures the fe2o3 state around bringing up the AMD GPU driver stack.

## Current Commit

- Local path: `/home/nod/github/fe2o3`
- Branch: `main`
- Remote: `https://github.com/powderluv/fe2o3.git`
- Latest pushed checkpoint before this access-sketch update:
  `721d671 Cross-check elementwise shapes with MIR records`

## Implemented

- `cargo-fe2o3 build` builds and loads `librustc_codegen_fe2o3.so`.
- The backend delegates host codegen through `rustc_codegen_llvm`.
- Kernel roots named `fe2o3_kernel_*` are collected from rustc MIR.
- Direct device MIR calls are walked from kernel roots.
- Reachable `std` functions are rejected.
- Intrinsic placeholder bodies are skipped.
- `FE2O3_DUMP_MIR=1` imports the collected device MIR into a first
  Pliron-facing scaffold and prints function, block, statement, and terminator
  shape.
- `dialect-mir` defines local `mir.*` operation/type names for the MIR import
  scaffold and future Pliron lowering.
- The MIR scaffold builds a flat typed `mir.*` operation-record stream for the
  future Pliron builder, including return, argument, local type labels,
  statement destinations, statement operands, assignment operation labels, and
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
- The current `f32`/`f64` elementwise MIR expression shapes emit AMDGPU LLVM IR.
- Generated LLVM IR is compiled through ROCm clang and linked with `ld.lld` into
  `target/fe2o3/*.hsaco`.
- Generated HSACO metadata is validated with `llvm-readobj --notes` when that
  ROCm tool is available.
- Supported read-only slice index helpers now include:
  - `ThreadIndex::offset(<usize constant>)`
  - `ThreadIndex::offset_signed(<isize constant>)`
  - `ThreadIndex::stride(<usize constant>)`
  - `ThreadIndex::stride_offset(<usize constant>, <isize constant>)`
- Raw `usize` arithmetic derived from `idx.get()` is recognized for constant
  add, subtract, and multiply patterns, including debug MIR `*WithOverflow`
  tuple values.
- Raw `usize` arithmetic can combine two tracked affine index expressions.
- Raw `usize` arithmetic can form constant-minus-index expressions with
  negative strides.
- Raw `usize` arithmetic can form index-minus-index expressions that collapse
  to constant indexes.
- The `vecadd`, `add-inplace`, `copy`, `downsample`, `fill`, `gather-odd`,
  `scale`, `shift`, `previous`, `stencil`, `raw-add-index`,
  `raw-const-minus`, `raw-parenthesized-sub`, `raw-disjoint-inplace-shift`,
  `raw-disjoint-shift`, `raw-gather`, `raw-neighbors`, `raw-output-shift`,
  `saxpy`, `axpy-inplace`, `negate`, `normalize`, `pipeline`, and
  `vecadd-f64` examples load HSACO from
  `FE2O3_HSACO_DIR`, which `cargo-fe2o3` sets to `target/fe2o3`.
- `downsample` verifies strided reads (`idx * 2`), `gather-odd` verifies affine
  reads (`idx * 2 + 1`), and `stencil` verifies multiple derived reads from the
  same input slice.
- `raw-gather` verifies the same affine read using raw Rust `usize` arithmetic
  instead of the `ThreadIndex::stride_offset` helper.
- `raw-add-index` verifies affine reads formed by adding two raw Rust `usize`
  index expressions.
- `raw-const-minus` verifies constant-minus-index reads with a negative stride.
- `raw-parenthesized-sub` verifies index subtraction that collapses to a
  constant read index.
- `raw-disjoint-shift` verifies raw Rust `usize` arithmetic on a
  `DisjointSlice<f32>` output store through `get_mut_at`.
- `raw-disjoint-inplace-shift` verifies raw Rust `usize` arithmetic on a
  `DisjointSlice<f32>` output read-before-write store through `get_mut_at`.
- `raw-neighbors` verifies raw Rust `usize` add/sub neighbor reads independent
  of the helper API.
- `raw-output-shift` verifies raw Rust `usize` arithmetic on an indexed
  `&mut [f32]` output store.

## Verified Before Reboot

These passed on the pre-reboot system:

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
rm -rf target/fe2o3
cargo clean -p fe2o3-vecadd
cargo run -p cargo-fe2o3 -- build -p fe2o3-vecadd
/opt/rocm/lib/llvm/bin/llvm-readobj --notes target/fe2o3/vecadd.hsaco
```

`llvm-readobj` reported:

- format: `elf64-amdgpu`
- target: `amdgcn-amd-amdhsa--gfx1100`
- kernel name: `vecadd`
- global-buffer pointer args for slice pointers

## Blocker Before Reboot

Local GPU execution was not tested because:

```text
ROCk module is NOT loaded, possibly no GPU devices
```

The system had `amdgpu` blacklisted and `vfio-pci` configured for
`1002:7551,1002:ab40`. An explicit `sudo -n modprobe amdgpu` brought up
`/dev/kfd` for the current session.

## Verified After Driver Load

TheRock ROCm was installed in:

```text
/home/nod/github/TheRock/.venv-rocm-latest
```

The installed ROCm Python packages are:

- `rocm==7.13.0a20260509`
- `rocm-sdk-core==7.13.0a20260509`
- `rocm-sdk-devel==7.13.0a20260509`
- `rocm-sdk-libraries-gfx120X-all==7.13.0a20260509`

After `amdgpu` was loaded, `rocm-sdk test` passed all 26 tests.

`rocminfo` reported:

- GPU name: `gfx1201`
- Marketing name: `AMD Radeon AI PRO R9700`
- ISA: `amdgcn-amd-amdhsa--gfx1201`

The latest full smoke command that passed was:

```bash
cd /home/nod/github/fe2o3
rm -rf target/fe2o3
ROCM_ROOT=/home/nod/github/TheRock/.venv-rocm-latest/lib/python3.12/site-packages/_rocm_sdk_devel
env -u FE2O3_TARGET \
  PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- smoke
```

The end-to-end commands passed:

```bash
ROCM_ROOT=/home/nod/github/TheRock/.venv-rocm-latest/lib/python3.12/site-packages/_rocm_sdk_devel
PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-vecadd

PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-add-inplace

PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-copy

PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-downsample

PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-fill

PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-gather-odd

PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-scale

PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-shift

PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-previous

PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-stencil

PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-gather

PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-add-index

PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-const-minus

PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-parenthesized-sub

PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-disjoint-shift

PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-disjoint-inplace-shift

PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-neighbors

PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-output-shift

PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-saxpy

PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-axpy-inplace

PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-negate

PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-normalize

PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-pipeline

PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-vecadd-f64
```

Results:

```text
vecadd passed for 1024 elements
add_inplace passed for 1024 elements
copy passed for 1024 elements
downsample passed for 1024 elements
fill passed for 1024 elements
gather_odd passed for 1024 elements
scale passed for 1024 elements
shift passed for 1024 elements
previous passed for 1024 elements
stencil passed for 1024 elements
raw_add_index passed for 1024 elements
raw_const_minus passed for 1024 elements
raw_parenthesized_sub passed for 1024 elements
raw_disjoint_inplace_shift passed for 1024 elements
raw_disjoint_shift passed for 1024 elements
raw_gather passed for 1024 elements
raw_neighbors passed for 1024 elements
raw_output_shift passed for 1024 elements
saxpy passed for 1024 elements
axpy_inplace passed for 1024 elements
negate passed for 1024 elements
normalize passed for 1024 elements
pipeline passed for 1024 elements
vecadd_f64 passed for 1024 elements
```

The generated elementwise IR is gated by a MIR shape recognizer, derives pointer
plus length kernel parameters and scalar by-value parameters from the Rust kernel
ABI, and preserves source argument names such as `a_ptr`, `b_ptr`, `c_ptr`, and
`alpha` when MIR debug info provides them.

## After Reboot

Confirm ROCm can see the GPU:

```bash
rocminfo
hipconfig --full
```

If ROCm reports that ROCk is not loaded or no GPU devices are visible, the
system may still have `amdgpu` blacklisted for the other driver work. Try:

```bash
sudo -n modprobe amdgpu
```

Then rerun the build smoke:

```bash
cd /home/nod/github/fe2o3
rm -rf target/fe2o3
cargo run -p cargo-fe2o3 -- build -p fe2o3-vecadd
env -u FE2O3_TARGET cargo run -p cargo-fe2o3 -- build -p fe2o3-add-inplace
env -u FE2O3_TARGET cargo run -p cargo-fe2o3 -- build -p fe2o3-copy
env -u FE2O3_TARGET cargo run -p cargo-fe2o3 -- build -p fe2o3-downsample
env -u FE2O3_TARGET cargo run -p cargo-fe2o3 -- build -p fe2o3-fill
env -u FE2O3_TARGET cargo run -p cargo-fe2o3 -- build -p fe2o3-gather-odd
env -u FE2O3_TARGET cargo run -p cargo-fe2o3 -- build -p fe2o3-scale
env -u FE2O3_TARGET cargo run -p cargo-fe2o3 -- build -p fe2o3-shift
env -u FE2O3_TARGET cargo run -p cargo-fe2o3 -- build -p fe2o3-previous
env -u FE2O3_TARGET cargo run -p cargo-fe2o3 -- build -p fe2o3-stencil
env -u FE2O3_TARGET cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-add-index
env -u FE2O3_TARGET cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-const-minus
env -u FE2O3_TARGET cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-parenthesized-sub
env -u FE2O3_TARGET cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-disjoint-inplace-shift
env -u FE2O3_TARGET cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-disjoint-shift
env -u FE2O3_TARGET cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-gather
env -u FE2O3_TARGET cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-neighbors
env -u FE2O3_TARGET cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-output-shift
env -u FE2O3_TARGET cargo run -p cargo-fe2o3 -- build -p fe2o3-saxpy
env -u FE2O3_TARGET cargo run -p cargo-fe2o3 -- build -p fe2o3-axpy-inplace
env -u FE2O3_TARGET cargo run -p cargo-fe2o3 -- build -p fe2o3-negate
env -u FE2O3_TARGET cargo run -p cargo-fe2o3 -- build -p fe2o3-normalize
env -u FE2O3_TARGET cargo run -p cargo-fe2o3 -- build -p fe2o3-pipeline
env -u FE2O3_TARGET cargo run -p cargo-fe2o3 -- build -p fe2o3-vecadd-f64
/opt/rocm/lib/llvm/bin/llvm-readobj --notes target/fe2o3/vecadd.hsaco
```

If `rocminfo` succeeds, run the end-to-end paths:

```bash
cargo run -p cargo-fe2o3 -- smoke
```

Or run them individually:

```bash
cargo run -p cargo-fe2o3 -- run -p fe2o3-vecadd
cargo run -p cargo-fe2o3 -- run -p fe2o3-add-inplace
cargo run -p cargo-fe2o3 -- run -p fe2o3-copy
cargo run -p cargo-fe2o3 -- run -p fe2o3-downsample
cargo run -p cargo-fe2o3 -- run -p fe2o3-fill
cargo run -p cargo-fe2o3 -- run -p fe2o3-gather-odd
cargo run -p cargo-fe2o3 -- run -p fe2o3-scale
cargo run -p cargo-fe2o3 -- run -p fe2o3-shift
cargo run -p cargo-fe2o3 -- run -p fe2o3-previous
cargo run -p cargo-fe2o3 -- run -p fe2o3-stencil
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-add-index
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-const-minus
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-parenthesized-sub
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-disjoint-inplace-shift
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-disjoint-shift
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-gather
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-neighbors
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-output-shift
cargo run -p cargo-fe2o3 -- run -p fe2o3-saxpy
cargo run -p cargo-fe2o3 -- run -p fe2o3-axpy-inplace
cargo run -p cargo-fe2o3 -- run -p fe2o3-negate
cargo run -p cargo-fe2o3 -- run -p fe2o3-normalize
cargo run -p cargo-fe2o3 -- run -p fe2o3-pipeline
cargo run -p cargo-fe2o3 -- run -p fe2o3-vecadd-f64
```

Expected result:

```text
vecadd passed for 1024 elements
add_inplace passed for 1024 elements
copy passed for 1024 elements
downsample passed for 1024 elements
fill passed for 1024 elements
gather_odd passed for 1024 elements
scale passed for 1024 elements
shift passed for 1024 elements
previous passed for 1024 elements
stencil passed for 1024 elements
raw_add_index passed for 1024 elements
raw_const_minus passed for 1024 elements
raw_parenthesized_sub passed for 1024 elements
raw_disjoint_inplace_shift passed for 1024 elements
raw_disjoint_shift passed for 1024 elements
raw_gather passed for 1024 elements
raw_neighbors passed for 1024 elements
raw_output_shift passed for 1024 elements
saxpy passed for 1024 elements
axpy_inplace passed for 1024 elements
negate passed for 1024 elements
normalize passed for 1024 elements
pipeline passed for 1024 elements
vecadd_f64 passed for 1024 elements
```

If autodetection is not available, set `FE2O3_TARGET` to the architecture
reported by ROCm, for example `gfx1201`, `gfx90a`, or `gfx942`.

## Next Implementation Step

Short-term backend surface step:

1. Move the current elementwise shape analysis from raw rustc MIR onto the
   record-driven lowering plan one piece at a time. The plan now carries typed
   locals, statement operation labels, call destinations/operands, and parsed
   load/store access sketches. The next slice should derive affine index
   expressions from those records instead of re-reading raw rustc MIR.

Then replace the temporary elementwise MIR recognizer/emitter with real
lowering:

1. Lower the current elementwise operations from MIR: args, basic blocks,
   integer arithmetic, pointer arithmetic, loads/stores, branch, and return.
2. Lower `thread::index_1d` to AMDGPU workitem/workgroup intrinsics.
3. Keep the current HSACO smoke test as the regression target.
