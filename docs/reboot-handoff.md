# Reboot Handoff

This file captures the fe2o3 state around bringing up the AMD GPU driver stack.

## Current Commit

- Branch: `main`
- Remote: `https://github.com/powderluv/fe2o3.git`
- Last reboot-handoff commit: `0f7a056 Add reboot handoff notes`

## Implemented

- `cargo-fe2o3 build` builds and loads `librustc_codegen_fe2o3.so`.
- The backend delegates host codegen through `rustc_codegen_llvm`.
- Kernel roots named `fe2o3_kernel_*` are collected from rustc MIR.
- Direct device MIR calls are walked from kernel roots.
- Reachable `std` functions are rejected.
- Intrinsic placeholder bodies are skipped.
- The current vector-add MIR shape emits AMDGPU LLVM IR.
- Generated LLVM IR is compiled through ROCm clang and linked with `ld.lld` into
  `target/fe2o3/vecadd.hsaco`.
- The `vecadd` example loads HSACO from `FE2O3_HSACO_DIR`, which `cargo-fe2o3`
  sets to `target/fe2o3`.

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

The end-to-end command passed:

```bash
ROCM_ROOT=/home/nod/github/TheRock/.venv-rocm-latest/lib/python3.12/site-packages/_rocm_sdk_devel
PATH=/home/nod/github/TheRock/.venv-rocm-latest/bin:$PATH \
  ROCM_PATH=$ROCM_ROOT \
  HIP_PATH=$ROCM_ROOT \
  LD_LIBRARY_PATH=$ROCM_ROOT/lib:${LD_LIBRARY_PATH:-} \
  cargo run -p cargo-fe2o3 -- run -p fe2o3-vecadd
```

Result:

```text
vecadd passed for 1024 elements
```

The generated `vecadd` IR is gated by a MIR vector-add shape recognizer, derives
pointer plus length kernel parameters from the Rust kernel ABI, and preserves
source argument names such as `a_ptr`, `b_ptr`, and `c_ptr` when MIR debug info
provides them.

## After Reboot

Confirm ROCm can see the GPU:

```bash
rocminfo
hipconfig --full
```

Then rerun the build smoke:

```bash
cd /home/nod/github/claude-rocm-workspace/fe2o3
rm -rf target/fe2o3
cargo clean -p fe2o3-vecadd
cargo run -p cargo-fe2o3 -- build -p fe2o3-vecadd
/opt/rocm/lib/llvm/bin/llvm-readobj --notes target/fe2o3/vecadd.hsaco
```

If `rocminfo` succeeds, run the end-to-end vecadd path:

```bash
cargo run -p cargo-fe2o3 -- run -p fe2o3-vecadd
```

Expected result:

```text
vecadd passed for 1024 elements
```

If autodetection is not available, set `FE2O3_TARGET` to the architecture
reported by ROCm, for example `gfx1201`, `gfx90a`, or `gfx942`.

## Next Implementation Step

Replace the temporary vector-add MIR recognizer/emitter with real lowering:

1. Add the first Pliron dependency and local dialect/import scaffolding.
2. Import collected MIR into a minimal intermediate form.
3. Lower the vecadd operations from MIR: args, basic blocks, integer arithmetic,
   pointer arithmetic, loads/stores, branch, and return.
4. Lower `thread::index_1d` to AMDGPU workitem/workgroup intrinsics.
5. Keep the current HSACO smoke test as the regression target.
