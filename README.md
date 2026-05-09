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

The HIP runtime layer and public API are compile-checkable. The rustc MIR
collector/lowerer is not wired yet; that is the next backend milestone.

Run diagnostics:

```bash
cargo run -p cargo-fe2o3 -- doctor
```

Check the workspace:

```bash
cargo check --workspace
```
