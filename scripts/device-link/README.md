# G7 source-evidence runner

The runner uses Rust `nightly-2026-04-03` through `rustup` and requires both
nested `Cargo.lock` files with `--locked --offline`. Override the executable or
toolchain explicitly with `FE2O3_RUSTUP` and `FE2O3_RUST_TOOLCHAIN`; an absent
toolchain is a hard error.

Run `scripts/device-link/run.sh` with one evidence class:

- `cpu-source-model` compares an independent extent-aware CPU oracle with the
  boundary source model. It is not GPU evidence.
- `source-check` checks Rust syntax, macro expansion, source contracts, and the
  canonical external evidence manifest. It does not compile AMDGPU code.
- `llvm-verify` runs `llvm-as` and `opt -passes=verify` when both are available.
  Set `FE2O3_LLVM_AS` and `FE2O3_OPT` to explicit executables. Missing tools
  produce exit 77 and an `UNAVAILABLE` result; verification failures are hard
  failures.
- `hardware` exits 77 because no production loading or execution exists.
- `all` requires CPU and source checks, attempts LLVM verification, and
  preserves an explicit unavailable status from LLVM or hardware evidence.

No mode invokes COMGR or a command-line linker. Every result retains these
limitations: no compiler-derived closure, no production loader, and no
hardware execution. Source evidence explicitly grants neither load nor launch
authority.
