# row_softmax_general_v1

This existing F32 tutorial has ordinary Rust SIMT and checked 1x64 output
memory-tile paths. The local gfx950 adapter selects the device CPU, Wave64,
XNACK-off source providers and reviewed ROCm ISA950 libraries together.
The original gfx942 source/providers and shared extraction runner remain.
Neither target is GPU-qualified until the full numerical command actually
passes on physical hardware.

## Build and numerical qualification

Start in the repository root after installing the [root requirements](../../README.md#requirements),
including the pinned nightly-2026-04-03 components. Build the production
extractor once with the normal root compiler build, then set its path and
loader directories for both target runners:

```bash
CARGO_TARGET_DIR="$PWD/target" rustup run nightly-2026-04-03 cargo build \
    --locked --manifest-path "$PWD/Cargo.toml" \
    -p rustc-codegen-fe2o3 --bin fe2o3-rustc-extract
export FE2O3_RUSTC_EXTRACTOR="$PWD/target/debug/fe2o3-rustc-extract"
extractor_dir="$(dirname -- "$FE2O3_RUSTC_EXTRACTOR")"
sysroot="$(rustup run nightly-2026-04-03 rustc --print sysroot)"
export LD_LIBRARY_PATH="$extractor_dir:$extractor_dir/deps:$sysroot/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export ROCM_PATH=/opt/rocm-7.2.4
FE2O3_EXAMPLE_COMPILE_ONLY=1 bash examples/row_softmax_general_v1/run-gfx942.sh
FE2O3_EXAMPLE_COMPILE_ONLY=1 bash examples/row_softmax_general_v1/run-gfx950.sh simt
FE2O3_EXAMPLE_COMPILE_ONLY=1 bash examples/row_softmax_general_v1/run-gfx950.sh tile
```

To reuse an already built extractor, skip the build and set
`FE2O3_RUSTC_EXTRACTOR` to that executable before the loader setup above.
Keep its matching backend library alongside it or in its `deps` directory.
The gfx950 adapter checks the existing reviewed ROCm 7.2.4 OCML/tool closure.
For an installation of ROCm 7.2.1, select its matching reviewed closure from
the repository root before either gfx950 command:

```bash
export ROCM_PATH=/opt/rocm-7.2.1
export FE2O3_GFX950_OCML_MANIFEST="$PWD/examples/gfx950_low_precision/gfx950-ocml-rocm-7.2.1.manifest"
```

With the appropriate physical GPU lease and visibility, the full commands are:

```bash
FE2O3_EXAMPLE_COMPILE_ONLY=0 bash examples/row_softmax_general_v1/run-gfx950.sh simt
FE2O3_EXAMPLE_COMPILE_ONLY=0 bash examples/row_softmax_general_v1/run-gfx950.sh tile
FE2O3_EXAMPLE_COMPILE_ONLY=0 bash examples/row_softmax_general_v1/run-gfx942.sh
```

These commands compile the real kernel, inspect its target, Wave64, workgroup,
symbol and explicit ABI metadata, then run the real HIP kernel and compare
complete device allocations against independent CPU oracles. Compile-only
success grants no GPU numerical or proof qualification. gfx950 attempts retain
LLVM, source pins, crate binding, HSACO, notes, ISA and hashes.

The explicit `qualification-legacy-hip` example feature opts into the same
`fe2o3-core/qualification-unsafe-launch` numerical route used by the existing
general BF16/Flash examples. It uses typed buffers, checked interior views,
streams and owned `KernelParams`, with the documented unsafe module/launch
boundary for the runner's trusted external HSACO. It creates no Worker V3
publication, protected execution, proof/refinement receipt or artifact-currentness
authority. The original `fe2o3-host` dependency and default production boundary
remain. No verifier, proof, or receipt requirement is disabled or synthesized.

All input/output allocations have leading and trailing bit canaries. Every
input bit is checked after completion; every logical output and every guard
or padding element is checked. Non-finite logical results reject. Empty
outputs skip dispatch and are reported separately. Ambiguous stream completion
aborts the qualification process before allocation/module destruction.

`structured-tile` selects genuine checked F32 output memory tiles; it adds no
matrix instructions or reduced-precision types. `gfx950-target` selects the
gfx950 source providers. The host target/launch checks reject other processors,
XNACK-on observations, non-Wave64 devices, and invalid/overflowed grids.

Run host tests through the installed production CLI, with nightly-2026-04-03
first on PATH and CARGO set to that toolchain's cargo:

```bash
cd examples/row_softmax_general_v1
cargo fe2o3 test --locked --offline --all-targets -j2 -- --test-threads=1
cargo fe2o3 test --locked --offline --all-targets -j2 --features gfx950-target,qualification-legacy-hip -- --test-threads=1
cargo fe2o3 test --locked --offline --all-targets -j2 --features gfx950-target,structured-tile,qualification-legacy-hip -- --test-threads=1
```

The qualification fixture/validator tests are CPU-only and never initialize a
GPU. They establish neither execution nor protected qualification.

SIMT launches one Wave64 per row. Tile mode launches one Wave64 per 64-column
output tile, repeats the complete row max/sum reductions for that tile and
clips the final column tile. Dynamic widths 1 through 4096 and independent
input/output row strides remain supported. Tile mode's checked grid is
`rows * ceil(columns/64)` waves, rather than SIMT's `rows` waves.
The 48-byte explicit ABI is two `(pointer, usize length)` pairs followed by
four `u32` dimensions/strides. The physical numerical matrix includes 28 cases:
zero rows, multiple rows, nonquarter values, full widths, tails, strided padding
and large logits requiring max subtraction. Every row must normalize.
