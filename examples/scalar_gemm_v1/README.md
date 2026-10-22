# scalar_gemm_v1

The original F32 SIMT source and `run-gfx942.sh` remain available. GPU
qualification for gfx942 is pending. The gfx950 adapter selects the device
CPU, Wave64/XNACK contract, source providers, and ROCm device-library ISA
selector together. It is a compile-only adapter; it does not grant artifact,
proof, launch, or hardware qualification authority. Existing host dependencies
and the host program's runtime limitations are retained.

Use nightly-2026-04-03, a prebuilt production extractor, and ROCm 7.2.4:

```bash
export FE2O3_RUSTC_EXTRACTOR=/path/to/fe2o3-rustc-extract
export ROCM_PATH=/opt/rocm-7.2.4
FE2O3_EXAMPLE_COMPILE_ONLY=1 bash examples/scalar_gemm_v1/run-gfx942.sh
FE2O3_EXAMPLE_COMPILE_ONLY=1 bash examples/scalar_gemm_v1/run-gfx950.sh simt
FE2O3_EXAMPLE_COMPILE_ONLY=1 bash examples/scalar_gemm_v1/run-gfx950.sh tile
```

The original gfx942 common runner needs the prebuilt extractor's backend
library directory in `LD_LIBRARY_PATH` when it is outside the root target.
The gfx950 adapter adds that directory itself. The reviewed 7.2.4 OCML/tool
closure is checked before compilation; `FE2O3_GFX950_OCML_MANIFEST` can select
another existing reviewed closure matching the installed ROCm. All gfx950
attempts retain LLVM, crate binding, HSACO, notes, disassembly and hashes.

`structured-tile` selects checked 1x64 F32 output memory tiles. These paths use
ordinary Rust arithmetic and capability-checked stores. They do not introduce
reduced-precision types or matrix instructions. `gfx950-target` selects the
gfx950 source providers where needed. The default features preserve gfx942.

The host `target` module checks gfx950 agent-name matching and launch extents.
These checks do not discover a device or launch a kernel. CPU oracle tests
cover nonquarter values, complete dynamic boundary cases, and input immutability;
GPU comparison and device allocation canaries remain pending physical runs.
The existing host program is not a GPU execution adapter.

Run host tests through the installed production CLI with nightly-2026-04-03
first on PATH and CARGO set to that toolchain's cargo:

```bash
cd examples/scalar_gemm_v1
cargo fe2o3 test --locked --offline --all-targets -j2 -- --test-threads=1
cargo fe2o3 test --locked --offline --all-targets -j2 --features gfx950-target,structured-tile -- --test-threads=1
```

Tile mode views flattened C as a row of checked 1x64 tiles. It keeps the
original per-element dynamic K recurrence, separate multiply and add,
nonempty-N guard, positive-zero K=0 behavior and rounded 256-thread launch.
The original FMA-distinguishing and no-dispatch tests remain in place.
