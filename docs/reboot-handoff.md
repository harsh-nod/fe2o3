# Reboot Handoff

This file captures the current fe2o3 state before rebooting the machine to bring
up the AMD GPU driver stack.

## Current Commit

- Branch: `main`
- Remote: `https://github.com/powderluv/fe2o3.git`
- Last implementation commit: `cae3b7c Emit AMDGPU LLVM IR and HSACO for vecadd`

## Implemented

- `cargo-fe2o3 build` builds and loads `librustc_codegen_fe2o3.so`.
- The backend delegates host codegen through `rustc_codegen_llvm`.
- Kernel roots named `fe2o3_kernel_*` are collected from rustc MIR.
- Direct device MIR calls are walked from kernel roots.
- Reachable `std` functions are rejected.
- Intrinsic placeholder bodies are skipped.
- The current `vecadd` kernel shape emits AMDGPU LLVM IR.
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
- global-buffer pointer args for `a_ptr`, `b_ptr`, and `c_ptr`

## Blocker Before Reboot

Local GPU execution was not tested because:

```text
ROCk module is NOT loaded, possibly no GPU devices
```

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
FE2O3_TARGET=gfx1100 cargo run -p cargo-fe2o3 -- run -p fe2o3-vecadd
```

Expected result:

```text
vecadd ok
```

If the GPU target differs from `gfx1100`, set `FE2O3_TARGET` to the architecture
reported by ROCm, for example `gfx90a` or `gfx942`.

## Next Implementation Step

Replace the temporary `vecadd`-specific LLVM IR emitter with real lowering:

1. Add the first Pliron dependency and local dialect/import scaffolding.
2. Import collected MIR into a minimal intermediate form.
3. Lower the vecadd operations from MIR: args, basic blocks, integer arithmetic,
   pointer arithmetic, loads/stores, branch, and return.
4. Lower `thread::index_1d` to AMDGPU workitem/workgroup intrinsics.
5. Keep the current HSACO smoke test as the regression target.
