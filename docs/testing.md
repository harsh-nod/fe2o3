# Testing fe2o3

The test matrix separates target-independent checks, Verus proofs, code-object
compilation, and hardware execution. Every pull request should run the bounded
fork-safe preflight:

```text
cargo fmt --all -- --check
bash scripts/tests/quickstart.sh
bash scripts/quickstart.sh source-check examples/vecadd/Cargo.toml
```

Proof, compiler, runtime, and trust-policy changes should also run their
applicable broader lanes. GitHub reports the bounded preflight first, then runs
the full generic core and codegen shards before merge and again on protected
push. The full matrix is intentionally hour-scale; the preflight provides the
fast feedback path without weakening the required generic qualification.

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

Passing this lane establishes operator-selected reviewed integrity for one
host-specific compiler/code-object profile. It does not authenticate origin,
run the GPU, establish source/Verus-to-machine refinement, prove memory safety
or race freedom, or grant publication, load, launch, or parity authority.

## External rollback crash and replay coverage

The external monotonic service and the local compiler Worker ledger exercise
opposite sides of the same restart protocol:

```text
cargo test --locked -p fe2o3-external-anchor-service --all-targets
cargo test --locked -p fe2o3-broker-authority-service --lib -- --test-threads=1
bash scripts/build-static-external-anchor-service.sh
```

The first suite interrupts the real atomic state replacement before and after
cleanup, create, write, file sync, rename, and directory sync, and interrupts
the daemon before and after receive, durable exchange, and send. Every restart
must recover exactly the prior or proposed state and exact request replay must
terminate at one proposed head. The broker suite covers the complementary local
journal orderings, queued-response recovery, exact challenge re-emission,
anchor-committed restart, and every retained-record boundary. These tests prove
deterministic same-host protocol recovery. Production rollback authority still
requires the root-managed distinct-UID anchor to be wired into the deployed
supervisor and qualified as an independently administered service.

## Retained bounded MoE evidence

The obsolete MoE V1/V2 host routes and their workload-specific hardware
launchers have been removed. Validate the remaining proof, source, and production-route-absence evidence with:

```text
python3 scripts/test-bounded-moe-docs.py
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

## Historical alpha/zeta Worker V2 observation

Commits `daf0b459ced07a25376670c83b1474eaebcd1a68` and
`dc9738e367c392f7716eacb8459ca73fa32abbbb` recorded compiler, raw HSA, and
fake-authority generated-safe observations for one exact two-kernel COV6
artifact. The Worker V2 host admission graph, exact alpha/zeta adapters,
hardware harness, and optional parity shard have since been deleted. Those
observations remain historical evidence and their commands are intentionally
not current test instructions.

Current feature-free host coverage uses the generic Worker V3 contract and the
direct-KFD composition boundary:

```text
cargo test --locked -p fe2o3-host
cargo test --locked -p fe2o3-host --test generated_spi_ui
cargo test --locked -p fe2o3-host --test worker_v3_verification_admission_ui
cargo test --locked -p fe2o3-host --test production_application_handoff_ui
cargo test --locked -p fe2o3-macros --test typed_kernel_fixtures
```

The deprecated HIP-buffer/HSA-lifecycle surface is retained only for explicit
qualification and differential coverage. It is absent from the default host
dependency graph and must be selected by name:

```text
FE2O3_HIP_SYS_DISABLE=1 FE2O3_HSA_RUNTIME_DISABLE=1 \
  cargo test --locked -p fe2o3-host \
    --features qualification-legacy-hip-hsa
FE2O3_HIP_SYS_DISABLE=1 FE2O3_HSA_RUNTIME_DISABLE=1 \
  cargo test --locked -p fe2o3-host \
    --features qualification-legacy-hip-hsa \
    --test hsa_executable_lifecycle_ui
```

With a configured ROCm development installation, compile the deprecated native
HSA/HIP qualification API and every feature-gated hardware test target without
executing them:

```text
cargo test --locked -p fe2o3-hsa-runtime --all-targets --features hardware-qualification --no-run
```

This checks only that the legacy qualification surface and its test harnesses
compile and link. It does not initialize HSA/HIP, admit a code object, or execute
a kernel, and it is not production direct-KFD qualification.

The macro fixture suite also checks the generated pure-KFD argument boundary:
safe code cannot implement its unsafe generated trait, mutable host outputs
remain exclusively borrowed, and HSA-backed and KFD-backed `Arguments`
specializations cannot satisfy each other's generated trait.

The generated KFD composition lane now covers authenticated Worker V3 typestate,
generated dispatch, completion, and the KFD runtime with a synthetic verifier.
The remaining production hardware gate must use the reviewed production
verifier through the inherited KFD application transition and must obtain its
artifact from the same build/publication transaction. A synthetic verifier,
externally injected HSACO, or selected legacy route cannot satisfy that gate.

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

Run the proof harnesses and expected-rejection mutations with the two exact
Verus installations pinned by the runtime-model and MIR/PLIRON suites:

```text
FE2O3_RUNTIME_MODEL_VERUS=/absolute/path/to/runtime-model/verus \
VERUS=/absolute/path/to/mir-pliron/verus \
  scripts/ci-local.sh verus
```

`FE2O3_RUNTIME_MODEL_VERUS` defaults to `VERUS` for snapshots whose pin-owning
harnesses use one release. The runtime-model and MIR/PLIRON harnesses verify
their executable digest and version; the fixture harnesses reuse the latter
installation. An unavailable or substituted binary is a failure rather than a
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

The smoke suite builds and runs all supported examples. Each example copies its
result back to the host and checks it against a CPU-computed expected value.

The scalar-GEMM compiler lane extracts the real single-source
`#[kernel(typed)]` example through semantic MIR, ranked PLIRON, Kernel IR, and
the production gfx942 LLVM lowering:

```text
FE2O3_HIP_SYS_DISABLE=1 \
cargo test --locked -p rustc-codegen-fe2o3 \
  --test production_general_matrix_driver_v1 \
  scalar_gemm_kernel_reaches_gfx942_llvm \
  -- --ignored --exact --nocapture --test-threads=1
```

The test requires the pinned nightly `rust-src` component and AMD target. It
checks the exact compiler-derived crate binding, the generated scalar entry,
gfx942 target triple and thread-index intrinsic, separate `fmul` and `fadd`,
and absence of FMA or MFMA contraction. It does not link an HSACO, invoke the
protected verifier, or execute the GPU. The earlier externally injected,
scalar-specific hardware test was deleted with the workload-specific Worker V3
authority closure. A retained hardware claim now requires the ordinary generic
Worker V3 inherited application and KFD route.

The host crate enforces the same split. Its feature-free build exposes the
Worker V3 application, admission, verification, and private joined KFD
invocation. The HSA-backed generated migration route is available only through
the explicitly named deprecated qualification feature and cannot provide
production launch authority. Worker V2 application recovery, embedded-artifact
loading, direct HIP module/function loading, raw parameter packing, cooperative
launch, and workload-specific host adapters are absent from the feature-free
production host surface. `fe2o3-core` retains a separately named deprecated
unsafe-launch feature strictly for qualification.
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

The Cargo V3 vertical suite builds a dedicated V3-only static consumer. Its dependency graph contains the Worker V3 host consumer fixture and hardware test hooks, with no alternate compiler route:

```text
cargo test --locked -p cargo-fe2o3 \
  --features worker-v3-envelope-integration-test-only \
  --test worker_v3_load_envelope_vertical -- --test-threads=1
```

The suite constructs a typed V4 proof association inside the frozen V3
capsule, independently decodes all five compiler-stage preimages, reimports the
exact signed aggregate MIR-to-live-PLIRON receipt, and checks its middle-end V5
binding. Host admission reconstructs the exact finalizer derivation and the
protected verifier independently repeats that reconstruction from borrowed
canonical envelope bytes. The suite checks that the accepted move-only decision
retains matching finalizer custody and rejects a foreign compiler/finalizer
derivation even when its finalized HSACO bytes match. It also asserts that the
explicit synthetic verifier retains no production proof-input owner and cannot
report protected compiler/signature custody. On the 2026-08-30 MI300X
qualification tree, all 36 serialized cases passed, including
publication-turnover, compiler-evidence substitution, finalizer cross-splice,
seccomp, restart, load-cleanup, and descriptor-lineage failures.

## Guard tests

The native-Linux and WSL device-node selection logic has a host-only test:

```text
scripts/tests/ci-local-hardware.sh
```
