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

## `daf0b45` alpha/zeta Worker V2 vertical slice

The following commands exercise the implementation landed through
`daf0b459ced07a25376670c83b1474eaebcd1a68`. They cover source/unit,
compile-fail, process-recovery, native-link, and raw-hardware boundaries. The
hardware result is recorded separately below because it is not produced by the
ordinary commands in this first block:

```text
cargo test --locked -p fe2o3-core --all-targets
cargo test --locked -p fe2o3-core --test device_buffer_view_ui
cargo test --locked -p fe2o3-macros --all-targets
cargo test --locked -p fe2o3-macros --test typed_kernel_fixtures
cargo test --locked -p fe2o3-host --all-targets --all-features
cargo test --locked -p fe2o3-host --test generated_spi_ui
cargo test --locked -p fe2o3-host generated_alpha_zeta_cov6::tests
cargo test --locked -p rustc-codegen-fe2o3 --lib
cargo test --locked -p rustc-codegen-fe2o3 --test general_two_kernel_import
cargo test --locked -p fe2o3-hsa-runtime --test gfx942_two_kernel_hardware
cargo test --locked -p fe2o3-hsa-runtime --features hardware-test-hooks \
  --test gfx942_two_kernel_hardware --no-run
cargo test --locked -p cargo-fe2o3 --test worker_v2_vertical_slice
cargo test --locked -p cargo-fe2o3 --test worker_v2_vertical_slice \
  --features worker-v2-fault-injection-test-only
cargo test --locked -p cargo-fe2o3 worker_v2_artifact_container::tests
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
production `ArtifactContainerV1` publication are downstream and are not worker
claims. A Release worker built on `mi300x` passes all three CTests and reports
this measured identity:

```text
fe2o3-worker-v1-sha256-234d22f9fb347c86495e7156e53ef8eab55e939d6514973a6df373aee12f77a9
```

The exact reproducible CMake invocation and trust boundary are in
[Evidence Record V1](evidence-record-v1.md). The native CTest observation is
not itself GPU execution or an archived result record.

The backend-witness link integration requires the same pinned Worker V2
environment used by the publication tests:

```text
FE2O3_LLVM_LINK_WORKER=/absolute/path/to/fe2o3-llvm-link-worker \
FE2O3_LLVM_LINK_WORKER_BUILD_ID=fe2o3-worker-v1-sha256-234d22f9fb347c86495e7156e53ef8eab55e939d6514973a6df373aee12f77a9 \
FE2O3_LLVM_BUILD_ID=7.2.4 \
FE2O3_GFX942_ALPHA_ZETA_OUTPUT=/absolute/path/to/alpha-zeta-cov6.hsaco \
cargo test --locked -p rustc-codegen-fe2o3 \
  --test kernel_ir_codegen \
  worker_v2_general_v3_alpha_zeta_build_links_and_validate_backend_witnesses \
  -- --ignored --exact --nocapture
sha256sum /absolute/path/to/alpha-zeta-cov6.hsaco
/opt/rocm/llvm/bin/llvm-readelf --notes \
  /absolute/path/to/alpha-zeta-cov6.hsaco
```

This command builds the genuine Rust `alpha` and `zeta` kernels through the
sealed Worker V2 path, links and validates both private backend witnesses,
publishes one COV6 HSACO, and optionally exports it with create-new semantics.
The measured output SHA-256 is
`3a916cdabca05ac74d340889aab2067221d6d1252a7cde13e61c1786252565c4`.
Independent note inspection reports complete COV6 kernarg sizes of `296` bytes
for `alpha` and `312` bytes for `zeta`; their explicit prefixes are `40` and
`56` bytes respectively. The prior explicit-versus-complete finalization
mismatch is fixed.

Strict lint coverage for the Rust portions is:

```text
cargo clippy --locked -p fe2o3-macros --all-targets --all-features -- -D warnings
cargo clippy --locked -p fe2o3-host --all-targets --all-features -- -D warnings
cargo clippy --locked -p rustc-codegen-fe2o3 --all-targets --all-features -- -D warnings
cargo clippy --locked -p cargo-fe2o3 --all-targets --all-features -- -D warnings
```

The landed production infrastructure includes durable Worker V2 publication,
strict finalized-bundle host admission, currentness-lease revalidation, the
authenticated HSA load/resolve/dispatch/unload state machine, macro-generated
alpha/zeta argument and preparation adapters, generated safe dispatch SPI, and
the reviewed `fe2o3-hsa-runtime` lifecycle and implicit-kernarg adapters. Unit,
mutation, and UI tests cover the state transitions, retained borrows, packing,
alias admission, currentness, identity substitution, and terminal completion.

The production trust chain still lacks both cross-process composition and
prerequisite authentication. Cargo drops the live publication lease and has no
durable load envelope, application handoff, or recovered host-admission path.
The `cargo-fe2o3` two-entry artifact-container adapter remains `cfg(test)` and
inert. Separately, `WorkerV2PrerequisiteAuthenticatorV1` defines the reviewed
boundary for compiler, Verus, proof, Rust ABI, and executable-effect evidence,
but the repository has only test/fake implementations. Therefore the
production safe path cannot yet authentically promote those prerequisites into
load/launch authority.

The raw alpha/zeta hardware harness has CPU/unit tests for exact `40`/`56` byte
packing, equal-length rejection, boundary grids, independent oracles, and canary
corruption. Its feature-gated ignored test was invoked on an AMD Instinct
MI300X, `gfx942:xnack-`, with ROCm 7.2.4 as follows:

```text
FE2O3_RUN_GFX942_TWO_KERNEL=1 \
FE2O3_GFX942_ALPHA_ZETA_HSACO=/absolute/path/to/alpha-zeta-cov6.hsaco \
FE2O3_GFX942_ALPHA_ZETA_SHA256=3a916cdabca05ac74d340889aab2067221d6d1252a7cde13e61c1786252565c4 \
cargo test --locked -p fe2o3-hsa-runtime --features hardware-test-hooks \
  --test gfx942_two_kernel_hardware \
  gfx942_cov6_alpha_then_zeta_one_executable \
  -- --ignored --exact --nocapture
```

The digest-pinned run passed at lengths `1`, `255`, `256`, `257`, and `1023`,
including independent CPU-oracle and prefix/suffix canary checks. This is real
alpha/zeta raw-path GPU execution, but the harness manually packs arguments and
calls the reviewed unsafe HSA adapter directly. It is not an end-to-end hardware
test of the generated alpha/zeta safe path, and it was not archived by the V1
snapshot runner. It therefore does not promote a parity row or dashboard
hardware-evidence strength.

The generated-safe composition test uses the same environment and artifact pin:

```text
FE2O3_RUN_GFX942_TWO_KERNEL=1 \
FE2O3_GFX942_ALPHA_ZETA_HSACO=/absolute/path/to/alpha-zeta-cov6.hsaco \
FE2O3_GFX942_ALPHA_ZETA_SHA256=3a916cdabca05ac74d340889aab2067221d6d1252a7cde13e61c1786252565c4 \
cargo test --locked -p fe2o3-hsa-runtime --features hardware-test-hooks \
  --test gfx942_two_kernel_hardware \
  gfx942_cov6_alpha_then_zeta_generated_safe_spi_with_fake_authenticator \
  -- --ignored --exact --nocapture
```

At commit `dc9738e367c392f7716eacb8459ca73fa32abbbb` this passed on MI300X for
all five lengths through generated checked slice capabilities, typed alpha/zeta
preparation, safe dispatch, and one reviewed loaded executable. The test-only
semantic witnesses and explicitly fake authenticator mean this is not
production proof authentication or a dashboard hardware-evidence strength.

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
