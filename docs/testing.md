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

The generic test subset runs `rustc-codegen-fe2o3` in a dedicated Cargo process.
The command-plan regression in `scripts/tests/ci-local-test-gate.sh` enforces
that separation in every generic CI run.

## Comprehensive workspace tests

Run every workspace test target with the repository gate:

```text
scripts/ci-local.sh workspace-test
```

This is equivalent to the following required process boundary:

```text
cargo test --locked --workspace --all-targets --exclude rustc-codegen-fe2o3
cargo test --locked -p rustc-codegen-fe2o3 --all-targets
```

`rustc-codegen-fe2o3` has `crate-type = ["rlib", "dylib"]`. A single Cargo
workspace test process can build more than one backend variant while integration
tests link the unversioned `librustc_codegen_fe2o3.so`. A later variant can
replace that file and leave an integration-test binary expecting Rust symbols
from the earlier variant. The failure then appears as an undefined dynamic
symbol even though the test passes by itself. Keeping the backend package in a
separate Cargo process prevents the artifact collision without changing compiler
or crate behavior.

The comprehensive lane may link ROCm libraries through workspace packages. It
does not opt in to ignored GPU execution tests.

## `90b6fe3` multi-kernel checkpoint

The checkpoint at
`90b6fe31cbb1d89b82755f194ac7950c4eef4756` has focused CPU-only coverage for
the two-kernel contracts at each layer:

```text
cargo test --locked -p cargo-fe2o3 --test general_two_kernel_project
cargo test --locked -p dialect-amdgcn
cargo test --locked -p fe2o3-artifacts
cargo test --locked -p fe2o3-verifier
cargo test --locked -p fe2o3-host
cargo test --locked -p fe2o3-hsa-runtime
cargo test --locked -p rustc-codegen-fe2o3 --all-targets
```

These tests cover deterministic root/helper import, exact internal-helper call
signatures, two-kernel AMDGPU lowering, one-payload bundle validation,
per-kernel proof binding, typed host selection, distinct native HSA symbol
resolution, and borrow-enforced executable lifetime. Mutation and UI tests
reject ambiguous helpers, signature changes, payload/proof/kernel
substitutions, duplicate native identities, cloning linear selections, and
unloading while a kernel set remains live.

The real-source Worker V2 integration remains an ignored ROCm test. Run it with
the same pinned worker and toolchain identities used to build the worker:

```text
FE2O3_LLVM_LINK_WORKER=/absolute/path/to/fe2o3-llvm-link-worker \
FE2O3_LLVM_LINK_WORKER_BUILD_ID=<measured-worker-id> \
FE2O3_LLVM_BUILD_ID=<measured-llvm-id> \
cargo test --locked -p rustc-codegen-fe2o3 \
  --test kernel_ir_codegen \
  worker_v2_real_source_publishes_two_kernels_with_one_shared_helper \
  -- --ignored --exact --nocapture
```

On MI300X this test is compile/publication evidence: it runs the sealed Cargo
backend, emits one `gfx942` HSACO containing both entries and one shared helper,
and checks the published code object. It does not dispatch either kernel. HSA
multi-symbol lifecycle tests at this checkpoint use the reviewed adapter's
host-side test boundary and likewise do not establish two-kernel hardware
execution.

The next [general typed dispatch V1](general-typed-dispatch-v1.md) gate must
archive one run from a single commit that includes all focused commands above
plus strict Clippy, the ignored Worker V2 test, and an opt-in MI300X execution
test. That hardware test must load one executable, resolve and dispatch two
differently typed kernels through the descriptor-driven path, compare outputs
with independent CPU oracles and canary checks at boundary lengths, and record
commit, rustc, worker, LLVM, ROCm, driver, and `gfx942` device identities. It
must also run the contract's generated-expectation, packing, alias, artifact,
proof, HSA identity, hidden-kernarg, queue, and lifetime rejection suite.
Compilation or symbol resolution alone does not pass this gate.

## `ceb0e46` general V3 and Worker V2 source/unit checkpoint

The following commands exercise the implementation landed through
`ceb0e4675173866a50fb737108e6a9b04827691d`. They are source/unit,
compile-fail, process-recovery, and native-link boundary checks; none is an
alpha/zeta GPU execution result:

```text
cargo test --locked -p fe2o3-macros --all-targets
cargo test --locked -p fe2o3-host --all-targets --all-features
cargo test --locked -p rustc-codegen-fe2o3 --lib
cargo test --locked -p rustc-codegen-fe2o3 --test general_two_kernel_import
cargo test --locked -p cargo-fe2o3 --test worker_v2_vertical_slice
cargo test --locked -p cargo-fe2o3 --test worker_v2_vertical_slice \
  --features worker-v2-fault-injection-test-only
cargo test --locked -p fe2o3-hsaco-finalize --all-targets --all-features
cargo test --locked -p fe2o3-artifact-transaction --all-targets --all-features
scripts/tests/run-parity-snapshot.sh
ctest --test-dir /absolute/path/to/llvm-link-worker-build --output-on-failure
```

The non-default Cargo feature is required for the raw/finalized 14-case process
fault matrix. Without it, the production binary has no fault-switch path. The
native worker CTests require a build configured against pinned LLVM and LLD,
but do not require a GPU. Their COV6 case verifies protocol version 6, LLVM
module flag 600, AMDHSA ELF ABI version 4, two metadata entries, both `.kd`
symbols, one shared helper, deterministic producer-order output, and
fail-closed descriptor mismatch handling. `.fe2o3.kd.v1` authentication and
`ArtifactContainerV1` construction are downstream and are not worker claims.

Strict lint coverage for the Rust portions is:

```text
cargo clippy --locked -p fe2o3-macros --all-targets --all-features -- -D warnings
cargo clippy --locked -p fe2o3-host --all-targets --all-features -- -D warnings
cargo clippy --locked -p rustc-codegen-fe2o3 --all-targets --all-features -- -D warnings
cargo clippy --locked -p cargo-fe2o3 --all-targets --all-features -- -D warnings
```

The source/unit checkpoint covers V3 registration and rustc-semantic descriptor
fixtures, lifetime-retaining packing, semantic-witness parsing and rejection,
canonical COV6 publication, legacy migration, and restart recovery. It does not
cover a backend-emitted witness object, generated alpha/zeta wrappers,
production two-kernel container/load/dispatch, or Verus. The remote MI300X
evidence documented above remains compile/inspection/publication evidence for
the older zero-argument Worker V2 fixture. No alpha/zeta hardware execution is
claimed.

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
