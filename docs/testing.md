# Testing fe2o3

The test matrix separates target-independent checks, Verus proofs, code-object
compilation, and hardware execution. Every pull request should run the generic
lane. Proof, compiler, and runtime changes should also run their applicable
lanes.

## Generic validation

This lane does not require ROCm or a GPU:

```text
scripts/ci-local.sh generic
```

The generic lane validates `examples/regression-manifest-v1.txt` against Cargo
workspace metadata and the HSACO names referenced by each example. The manifest
is the authoritative package selection for ordinary Rust checks, ROCm
compilation, and GPU smoke. `verus-vecadd` remains ordinary-rustc-only and is
never selected for ROCm compilation or GPU execution.

## Verus proof coverage

Run the two positive vecadd and fill proof harnesses plus all twelve
expected-rejection mutations with an explicit Verus installation:

```text
VERUS=/absolute/path/to/verus scripts/ci-local.sh verus
```

This lane requires Verus; an unavailable binary is a failure rather than a
skip. The production `f32` vecadd and Verus expand the exact same control,
index, guarded memory-access, and write body. The three real-body mutations
require exactly one primary Verus error, one verification result reporting one
error, and the expected diagnostic, failed source clause, and marker. The
remaining mutations require their expected obligation marker and diagnostic.

The lane proves source-level models under the documented thread-witness
contracts. Verus uses a total arithmetic adapter for vecadd; it does not prove
that production IEEE `f32` addition, ordinary Rust, the compiler pipeline,
HSACO, HIP, or GPU execution refines that model.

## ROCm compile coverage

Set an explicit LLVM target and compile every supported example:

```text
FE2O3_TARGET=gfx1151 scripts/ci-local.sh rocm-compile
```

Use the target reported by `rocminfo` on the machine under test. Compilation
does not execute a kernel. This lane also compiles the trusted-device marker
fixtures. `#[kernel]` generates a typed `KernelMarkerV1` with deterministic
marker symbol; public kernels expose the marker publicly but doc-hidden.
Genuine and renamed dependencies must emit, while local lookalikes, the
same-name unmarked external crate, local markers, and duplicate markers must
fail closed. Rejected fixtures also preseed generated artifacts and require
transactional invalidation of the complete artifact triplet. Markers identify
compiler semantics; marker and executable authenticity plus full ABI semantics
remain unsafe artifact-provenance and generated-binding responsibilities.

The lane also compiles and inspects the hardened G1 fill code object, compiles
the real three-slice vecadd through `kernel-ir-v1`, validates its exact ABI and
bounds-control-flow LLVM shape, and checks that invalid selectors or
unsupported inputs fail without fallback and remove stale artifacts.
After each example build, the lane requires every artifact declared by the
manifest. The pipeline example declares both `scale_stage.hsaco` and
`bias_stage.hsaco`.

## Hardware smoke

Hardware execution is deliberately opt-in:

```text
FE2O3_ALLOW_GPU_SMOKE=1 scripts/ci-local.sh hardware-smoke
```

Native Linux requires read/write access to `/dev/kfd`. WSL uses `/dev/dxg` and
also requires `HSA_ENABLE_DXG_DETECTION=1` with AMD's WSL HSA runtime installed:

```text
HSA_ENABLE_DXG_DETECTION=1 \
FE2O3_ALLOW_GPU_SMOKE=1 \
FE2O3_TARGET=gfx1151 \
scripts/ci-local.sh hardware-smoke
```

The smoke suite builds and runs all supported examples. Each example copies its
result back to the host and checks it against a CPU-computed expected value. It
also runs both `fe2o3-fill` and `fe2o3-vecadd` through
`FE2O3_CODEGEN_PIPELINE=kernel-ir-v1`, so the integrated verified-IR paths are
exercised independently of the default legacy emitter. Generated HSACO
inspection uses a strict pipeline-specific metadata profile.

## Guard tests

The native-Linux and WSL device-node selection logic has a host-only test:

```text
scripts/tests/ci-local-hardware.sh
```
