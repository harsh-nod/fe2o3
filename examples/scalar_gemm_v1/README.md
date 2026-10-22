# scalar_gemm_v1

This existing F32 tutorial has ordinary Rust SIMT and checked 1x64 output
memory-tile paths. The local gfx950 adapter selects the device CPU, Wave64,
XNACK-off source providers and reviewed ROCm ISA950 libraries together.
The original gfx942 source/providers and shared extraction runner remain.
Neither target is GPU-qualified until the full numerical command actually
passes on physical hardware.

## Build and numerical qualification

Use nightly-2026-04-03, a prebuilt production extractor, and ROCm 7.2.4.
The gfx950 adapter checks the existing reviewed 7.2.4 OCML/tool closure.
The original gfx942 common runner needs the extractor backend directory in
`LD_LIBRARY_PATH` when it is outside the root target.

```bash
export FE2O3_RUSTC_EXTRACTOR=/path/to/fe2o3-rustc-extract
export ROCM_PATH=/opt/rocm-7.2.4
FE2O3_EXAMPLE_COMPILE_ONLY=1 bash examples/scalar_gemm_v1/run-gfx942.sh
FE2O3_EXAMPLE_COMPILE_ONLY=1 bash examples/scalar_gemm_v1/run-gfx950.sh simt
FE2O3_EXAMPLE_COMPILE_ONLY=1 bash examples/scalar_gemm_v1/run-gfx950.sh tile
```

With the appropriate physical GPU lease and visibility, the full commands are:

```bash
FE2O3_EXAMPLE_COMPILE_ONLY=0 bash examples/scalar_gemm_v1/run-gfx950.sh simt
FE2O3_EXAMPLE_COMPILE_ONLY=0 bash examples/scalar_gemm_v1/run-gfx950.sh tile
FE2O3_EXAMPLE_COMPILE_ONLY=0 bash examples/scalar_gemm_v1/run-gfx942.sh
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
cd examples/scalar_gemm_v1
cargo fe2o3 test --locked --offline --all-targets -j2 -- --test-threads=1
cargo fe2o3 test --locked --offline --all-targets -j2 --features gfx950-target,qualification-legacy-hip -- --test-threads=1
cargo fe2o3 test --locked --offline --all-targets -j2 --features gfx950-target,structured-tile,qualification-legacy-hip -- --test-threads=1
```

The qualification fixture/validator tests are CPU-only and never initialize a
GPU. They establish neither execution nor protected qualification.

The tile path views flattened C as checked 1x64 memory tiles and retains the
original dynamic K recurrence, separate multiply/add, N guard and rounded
256-thread launch. The actual 60-byte explicit ABI is three `(pointer, usize
length)` pairs then three `u32` dimensions, packed through HIP's owned argument
list. It does not use the retired 64/320-byte Worker ABI assumptions. The
physical numerical matrix preserves all nine original cases, including zero
M/N/K, 255/256/257 output edges, rectangular shapes and the FMA distinguisher,
and adds six full nonquarter cases. Every logical output is compared bitwise
to the retained ordered F32 oracle, including positive zero when K is zero.
