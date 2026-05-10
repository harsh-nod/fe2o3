# fe2o3

`fe2o3` is an experimental single-source Rust GPU stack for AMD GPUs.

The intended end state is:

```text
Rust host + #[kernel] device code
        |
        v
rustc frontend and MIR
        |
        +--> native host binary
        |
        +--> fe2o3 device backend -> AMDGPU LLVM IR -> HSACO
                                                |
                                                v
                                     HIP module load/launch
```

This initial implementation establishes the crate layout and runtime/compiler
interfaces needed to reach that target:

- `fe2o3-device`: no-std device API and `#[kernel]` re-export.
- `fe2o3-macros`: kernel marker macro and reserved symbol naming.
- `fe2o3-core`: HIP-backed host runtime, buffers, streams, modules, launches.
- `fe2o3-host`: user-facing launch macro.
- `rustc-codegen-fe2o3`: backend support code and HSACO toolchain hooks.
- `cargo-fe2o3`: cargo subcommand and environment diagnostics.
- `dialect-amdgcn`: AMDGPU intrinsic naming seam for the future Pliron lowering.

## Current Status

The HIP runtime layer and public API are working for the current `vecadd`
example on AMD hardware. `cargo-fe2o3 build` builds and loads
`librustc_codegen_fe2o3.so`, delegates host codegen through
`rustc_codegen_llvm`, detects `#[kernel]` functions in rustc codegen units, and
dumps the currently collected device-reachable MIR functions.

The backend also has the first AMDGPU artifact path: for the `vecadd` example it
validates the Rust kernel ABI from monomorphized MIR argument locals, emits a
minimal AMDGPU LLVM IR kernel, and compiles it through ROCm clang plus `ld.lld`
into `target/fe2o3/vecadd.hsaco`. This is intentionally scoped to the current
`vecadd` kernel shape; general MIR/Pliron lowering is still the next compiler
milestone.

On a `gfx1201` AMD Radeon AI PRO R9700 with TheRock ROCm
`7.13.0a20260509`, `cargo-fe2o3 run -p fe2o3-vecadd` generates the HSACO, loads
it through HIP, launches the kernel, and validates the result.

See [docs/implementation-plan.md](docs/implementation-plan.md) for the full
compiler/runtime plan.

Run diagnostics:

```bash
cargo run -p cargo-fe2o3 -- doctor
```

If `FE2O3_TARGET` is not set, `cargo-fe2o3` tries to infer the target from
`rocminfo` and falls back to `gfx1100`.

Check the workspace:

```bash
cargo check --workspace
```

Smoke-test the current backend entry point:

```bash
cargo run -p cargo-fe2o3 -- build -p fe2o3-vecadd
```

On a machine with a working AMD GPU and ROCm driver stack:

```bash
cargo run -p cargo-fe2o3 -- run -p fe2o3-vecadd
```
