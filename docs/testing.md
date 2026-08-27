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

The generic lane validates `examples/regression-manifest-v2.txt` against Cargo
workspace metadata and the HSACO names referenced by each example. The manifest
separates source-artifact inventory from explicit qualification authority. It
does not select production or GPU execution. `verus-vecadd` remains
ordinary-rustc-only and has no artifact qualification route.

CPU example tests are the exact manifest subset with `rustc_check=true` and
`artifact_qualification=none`. The `cpu-test-raw` and
`cpu-test-wrapper-managed` queries partition that subset using the structural
source projection: ordinary packages run with raw Cargo, while every package
containing a namespace-free typed kernel runs through the feature-free binding
wrapper using exactly:

```text
cargo fe2o3 test --locked --all-targets -p <wrapper-managed-package>
```

`--all-targets` is mandatory. The host-test command rejects caller `--target`,
`--config`, Cargo-side `-Z`, `--doc`, and `--no-run` arguments. It also rejects
ambient runner and rustdoc selection plus configured runner, protected fe2o3,
dynamic-loader, and compiler selection. Configured rustdoc is overridden with
the disabled selection, and ambient loader variables are scrubbed. The lists are
sorted, disjoint, exhaustive, and both complete lists plus the full structural
projection are rescanned after the managed tests. This routing applies to any
kernel package; it does not encode package-name exceptions or require literal
namespaces.

Workspace source and configuration outside those protected selections, build
scripts, procedural macros, linkers, and test bodies are trusted and execute as
the current user. The fixed runner closes its child environment and descriptor
boundary, but it is not a sandbox. It opens and hashes Cargo's original test
executable. While Cargo's path remains stable, executing the retained original
preserves ordinary `current_exe` and `$ORIGIN` behavior and prevents
directory-entry substitution between pin and execution; the runner rechecks the
object afterward. This does not freeze same-inode writes or grant
immutable-artifact or origin authority. The protected-configuration scans before
and after Cargo are diagnostic checks for persistent changes, not an atomic
snapshot or TOCTOU proof.

This route may create ordinary Cargo host artifacts, but it has no fe2o3
backend, HSACO, publication, or artifact authority. It makes no GPU observation
or performance prediction. Because project code is trusted rather than
confined, those claims do not mean a test body is prevented from opening files,
network sockets, or device nodes.

The generic test subset runs `rustc-codegen-fe2o3` in a dedicated Cargo process.
The command-plan regression in `scripts/tests/ci-local-test-gate.sh` enforces
that separation in every generic CI run.

## Parity evidence validation

The generic lane runs legacy integrity checks plus signed row-evidence and
MI300X queue shell suites:

```text
scripts/ci-local.sh parity-evidence
bash scripts/tests/parity-evidence.sh
bash scripts/tests/parity-row-evidence.sh
bash scripts/tests/mi300x-evidence-queue.sh
bash scripts/tests/parity-dashboard.sh
```

The row suite runs class-specific test programs, signs test-domain result and
review fixtures, and exercises trust substitution, signature mutation, replay,
relabeling, stale source, duplicate identities, exact policy classes, queue
bypass, Complete authorization, and source-tree deltas. The queue suite uses no
GPU and makes no hardware claim. It consumes a signed test queue and verifies
canonical admission, lock hardening, and concurrent serialization.

See [Signed Parity Evidence V2](parity-signed-evidence-v2.md) for protected
trust provisioning, row-sharded commands, and the production MI300X queue.

## Comprehensive workspace tests

Run every workspace test target with the repository gate:

```text
scripts/ci-local.sh workspace-test
```

This is equivalent to the following required process boundary:

```text
cargo test --locked --workspace --all-targets --exclude rustc-codegen-fe2o3
scripts/ci-local.sh rustc-codegen-test
```

`rustc-codegen-fe2o3` has `crate-type = ["rlib", "dylib"]`. An `--all-targets`
Cargo process can build more than one backend variant while integration tests
link the unversioned `librustc_codegen_fe2o3.so`. A later variant can replace
that file and leave an integration-test binary expecting Rust symbols from the
earlier variant. The failure then appears as an undefined dynamic symbol even
though the test passes by itself. The repository command runs the library test
target and every integration target in deterministic separate Cargo
invocations. Each test therefore executes against the exact dylib produced for
its link, without changing compiler or crate behavior.

The comprehensive lane may link ROCm libraries through workspace packages. It
does not opt in to ignored GPU execution tests.

## Row-softmax V1 LLVM 22 release gate

The row release is a separate host-specific compiler/code-object lane. At
implementation Commit A, the manifest path below is deliberately absent. Only
a subsequent manifest-only Commit B directly above A may add it and pin A's
exact commit and tree. A compliant B's committed manifest must be reviewed out
of band, then tested by invoking the gate twice with the same reviewed digest
and four distinct paths that do not exist:

```text
export REVIEWED_ROW_SOFTMAX_MANIFEST_SHA256=<independently-reviewed-sha256>
MANIFEST_PATH="$PWD/tools/fe2o3-llvm-link-worker/row-softmax-v1-release-manifest.txt" \
EXPECTED_MANIFEST_SHA256="$REVIEWED_ROW_SOFTMAX_MANIFEST_SHA256" \
tools/fe2o3-llvm-link-worker/run-row-softmax-v1-release-gate.sh \
  /absolute/new/row-cmake-run1 /absolute/new/row-cargo-run1
MANIFEST_PATH="$PWD/tools/fe2o3-llvm-link-worker/row-softmax-v1-release-manifest.txt" \
EXPECTED_MANIFEST_SHA256="$REVIEWED_ROW_SOFTMAX_MANIFEST_SHA256" \
tools/fe2o3-llvm-link-worker/run-row-softmax-v1-release-gate.sh \
  /absolute/new/row-cmake-run2 /absolute/new/row-cargo-run2
```

The gate requires a clean manifest-only commit directly above its pinned
implementation parent. It rehashes the LLVM source/package, LLD, device
libraries, Cargo vendor tree, rustc sysroot, build tools, runtime DSOs, Worker,
probe, probe output, and real retained HSACO before and after use. Focused CTest
and Rust suites verify the exact four-explicit plus nineteen-hidden-argument
LLVM 22 metadata profile and reject field, note-view, MessagePack, identity, and
artifact substitutions. Both runs must reproduce the manifest's exact output
digests and lengths.

Passing this lane establishes operator-selected reviewed integrity for one
host-specific compiler/code-object profile. It does not authenticate origin,
run the GPU, establish source/Verus-to-machine refinement, prove memory safety
or race freedom, or grant publication, load, launch, or parity authority.

## Retained bounded MoE evidence

The obsolete MoE V1/V2 host routes and their workload-specific hardware
launchers have been removed. Validate the remaining compiler, proof, source,
oracle, and production-route-absence evidence with:

```text
python3 scripts/test-bounded-moe-docs.py
cargo test --locked -p rustc-codegen-fe2o3 --features qualification-oracles-test-only --lib collected_moe_top2_v1::
cargo test --locked -p fe2o3-verifier --test moe_expert_compact_plan_v1
VERUS=/absolute/path/to/pinned/verus \
  ./scripts/test-moe-expert-compact-plan-verus.sh
cargo test --locked -p fe2o3-host --test production_application_handoff_ui
```

These commands do not execute MoE on the GPU. A production hardware claim must
flow through the generic Worker V3 route.
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
scripts/ci-local.sh rustc-codegen-test
```

These tests cover deterministic root/helper import, exact internal-helper call
signatures, two-kernel AMDGPU lowering, one-payload bundle validation,
per-kernel proof binding, typed host selection, distinct native HSA symbol
resolution, and borrow-enforced executable lifetime. Mutation and UI tests
reject ambiguous helpers, signature changes, payload/proof/kernel
substitutions, duplicate native identities, cloning linear selections, and
unloading while a kernel set remains live.

The real-source Worker V2 integration from this checkpoint is retired: its
backend selector no longer exists. Its retained oracle logic is exercised only
inside the feature-enabled library test binary and is not current
compile/publication or hardware evidence.

## Bounded Pliron scalar-add MI300X slice

Commits `fd6520d88`, `70f9c5ad7`, `e016833d3`, `c9e8ca702`, `62efd243e`, and
`228c88ed9` close one exact backend-fixture-to-MI300X scalar-add route. The code
target is `gfx942:xnack-`; the qualifying device reports
`gfx942:sramecc+:xnack-`. The backend fixture is not Rust user source. Source
observation, policy/authority, Worker execution joining, and exact finalization
remain bounded legacy qualification evidence. The MI300X/HSA consumer is not
part of the standard generic or ROCm CI lanes. Run the ignored test explicitly
on `mi300x` with the exact qualified files and lane variables:

```text
cd /home/harsh/fe2o3-pliron-final-current
HSA_XNACK=0 \
HIP_VISIBLE_DEVICES=0 \
ROCR_VISIBLE_DEVICES=0 \
FE2O3_RUN_REPOSITORY_SCALAR_ADD_V1_MI300X=1 \
FE2O3_PLIRON_SCALAR_ADD_V1_WORKER=/home/harsh/fe2o3-pliron-integrated-worker-build/fe2o3-llvm-link-worker \
FE2O3_PLIRON_SCALAR_ADD_V1_OBSERVED_WORKER_BUILD_ID_FILE=/home/harsh/fe2o3-pliron-integrated-worker-build/fe2o3-worker-build-id.txt \
FE2O3_PLIRON_SCALAR_ADD_V1_OBSERVED_LLVM_BUILD_ID_FILE=/home/harsh/upstream-llvm-fe2o3-v1-acceptance/evidence-v6/upstream-llvm-build-id.txt \
cargo test --locked -p fe2o3-pliron-scalar-add-v1 \
  --test gfx942_repository_scalar_add_v1_hardware \
  repository_scalar_add_v1_isolated_mi300x \
  -- --ignored --exact --nocapture
```

`generic-core` executes feature-free host boundary tests. `rocm-compile` builds
the sealed qualification Cargo driver and runs bounded rustc-codegen artifact
oracles; it does not enable an alternate host execution graph or a scalar-add
runtime lane. The live MI300X test above remains a separate explicit opt-in.

The qualified Worker executable has SHA-256
`12c06e0da5d812c1db6f33450f99a8d70087c585eec552f7f8616077704361fd`
and embedded build identity
`fe2o3-worker-v1-sha256-a33996e00d152954305779c30174d7644f3fb8a54dd06f38d97c0f824aac6181`.
It uses upstream LLVM identity
`upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1`.
The expected HSACO is 4,984 bytes with SHA-256
`011671a80384051232fb684c90afadd9b5e9d81c13d216238f15af55dd3880b1`;
the pinned ROCr HSA 1.18 image SHA-256 is
`7010eba894569c044749b71b63ff782080c4a91e19ff24d6dc93e857045ab37e`.
The COV6 descriptor declares the 280-byte kernarg at alignment 8, while the
runtime reports and supplies alignment 16.

Success prints a marker beginning
`FE2O3_REPOSITORY_SCALAR_ADD_V1_MI300X_OK`. The sealed, move-only consumer is
the only typed route from the finalized receipt to load, dispatch, checking,
and terminal unload. The marker is a canonical self-consistency record binding
the bounded policy, artifact, runtime image, device, dispatch, result, canaries,
and unload observations. Its fixed-order serialization is stable,
but process-local runtime, agent, executable, dispatch, and kernarg identities
may differ between runs. It is not an external signature, trusted CI
attestation, general memory-safety or race-freedom proof, or CUDA-Oxide parity
claim. The compile-time checkout policy likewise relies on repository/build
provenance and is not separately authenticated. This test changes no parity
row or dashboard count.

The [general typed dispatch V1](general-typed-dispatch-v1.md) gate requires an
archived run from a single commit that includes all focused commands above
plus strict Clippy, the ignored Worker V2 test, and an opt-in MI300X execution
test. That hardware test must load one executable, resolve and dispatch two
differently typed kernels through the descriptor-driven path, compare outputs
with independent CPU oracles and canary checks at boundary lengths, and record
commit, rustc, worker, LLVM, ROCm, driver, and `gfx942` device identities. It
must also run the contract's generated-expectation, packing, alias, artifact,
proof, HSA identity, hidden-kernarg, queue, and lifetime rejection suite.
Compilation or symbol resolution alone does not pass this gate.

## Historical alpha/zeta Worker V2 observation

Commits `daf0b459ced07a25376670c83b1474eaebcd1a68` and
`dc9738e367c392f7716eacb8459ca73fa32abbbb` recorded compiler, raw HSA, and
fake-authority generated-safe observations for one exact two-kernel COV6
artifact. The Worker V2 host admission graph, exact alpha/zeta adapters,
hardware harness, and optional parity shard have since been deleted. Those
observations remain historical evidence and their commands are intentionally
not current test instructions.

Current host coverage uses the generic Worker V3 contract:

```text
cargo test --locked -p fe2o3-host --all-targets --all-features
cargo test --locked -p fe2o3-host --test generated_spi_ui
cargo test --locked -p fe2o3-host --test hsa_executable_lifecycle_ui
cargo test --locked -p fe2o3-host --test worker_v3_verification_admission_ui
cargo test --locked -p fe2o3-host --test production_application_handoff_ui
cargo test --locked -p fe2o3-macros --test typed_kernel_fixtures
cargo test --locked -p fe2o3-hsa-runtime --all-targets --features hardware-test-hooks --no-run
```

The macro fixture suite also checks the generated pure-KFD argument boundary:
safe code cannot implement its unsafe generated trait, mutable host outputs
remain exclusively borrowed, and HSA-backed and KFD-backed `Arguments`
specializations cannot satisfy each other's generated trait.

The next hardware evidence must enter through the production Worker V3
application and verifier, generated dispatch, completion, and a KFD runtime
path. A fake verifier or externally selected legacy route cannot satisfy that
gate.

### Archived compiler evidence controller

The checked controller and fixtures preserve the hardening and identity model
used by the historical bounded alpha/zeta Worker V2 observation. New captures
are disabled because their generator depended on a retired backend selector.
`scripts/gfx942-cov6-compiler-evidence.sh` therefore fails closed in capture
mode; its mutation self-test remains available for the archived format.

A replacement compiler-evidence controller must consume a production Worker V3
artifact and cannot grant runtime authority. New hardware qualification must
use KFD rather than the historical HSA harness.

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

Set the admitted LLVM target and run the bounded production compiler gates:

```text
FE2O3_TARGET=gfx942 scripts/ci-local.sh rocm-compile
```

Compilation does not execute a kernel. The lane runs the selector-free
production extraction, general-matrix, transaction, ranked-bounds, and barrier
drivers, including their fail-closed cases, then compiles and inspects the
independent hardened G1 code-object fixture. It builds one private,
content-addressed `cargo-fe2o3` production driver for `doctor`; neither that
driver nor any backend child receives a qualification selector.

## Hardware smoke

Hardware execution is deliberately opt-in:

```text
FE2O3_ALLOW_GPU_SMOKE=1 FE2O3_TARGET=gfx942 \
  scripts/ci-local.sh hardware-smoke
```

Native Linux requires read/write access to `/dev/kfd`; WSL `/dev/dxg` is not a
KFD substitute. The lane measures the pure-Rust KFD identity boundary, admits
every visible `gfx942:xnack-` device, maps and releases host-visible memory on
each device, and creates, validates, and destroys one isolated compute AQL
queue per device without packet submission or MMIO stores. It does not execute
a kernel or claim application-level Worker V3 dispatch coverage.

Supplying both diagnostic variables extends the lane with one exact-artifact
packet in an isolated selected-device process:

```text
FE2O3_ALLOW_GPU_SMOKE=1 FE2O3_TARGET=gfx942 \
FE2O3_KFD_DIAGNOSTIC_UNIQUE_ID=0x6ced1647a296545c \
FE2O3_TEST_SOURCE_AUTH_LDS_GFX942_HSACO=/absolute/path/to/lds.hsaco \
  scripts/ci-local.sh hardware-smoke
```

The diagnostic refuses an HSACO whose SHA-256 differs from
`ab6bda1e8af05b61c22753382e75dd6a9952db8e598eaac3cb5769863a618ed0`,
expects the LDS reduction result `2080`, and checks both canary regions. It uses
the same invocation-specific runtime gate as production, but supplies an
explicitly unsafe diagnostic authority implementation. It therefore does not
claim the still missing production Worker V3 verifier authority.

The host crate enforces the same split. Its feature-free build exposes the
Worker V3 application, admission, verification, HSA load, and generated
dispatch route. Worker V2 application recovery, embedded-artifact loading,
direct HIP module/function loading, raw parameter packing, cooperative launch,
and workload-specific host adapters are deleted in every feature configuration.
The former host qualification feature is deleted and cannot restore an
alternate execution path.
`production_application_handoff_ui` compile-fails representative V2 entrypoint
and runtime imports, including the retired embedded artifact contract, raw HIP
surface, and generated vecadd `Kernel`, to guard that public API boundary. The macro fixture also
proves that every supported `#[kernel(typed)]` signature, including exact
vecadd and Scalar GEMM, emits only generic Worker V3 host code. It rejects the
retired `qualification_worker_v2` option and verifies lifetime retention,
private fields, non-cloneable arguments, hidden pointers, and one-shot dispatch
against the V3 API. The vecadd example has no embedded
execution feature; it type-checks the V3 `Arguments` surface and fails closed
before runtime dispatch until a production verifier is supplied.

The Cargo V3 vertical suite builds a dedicated V3-only static consumer. Its
dependency graph contains `fe2o3-host/default` and
`fe2o3-host/hardware-test-hooks`; the host crate has no qualification feature
or alternate execution graph:

```text
cargo test --locked -p cargo-fe2o3 \
  --features worker-v3-envelope-integration-test-only \
  --test worker_v3_load_envelope_vertical -- --test-threads=1
```

## Guard tests

The native-Linux and WSL device-node selection logic has a host-only test:

```text
scripts/tests/ci-local-hardware.sh
```
