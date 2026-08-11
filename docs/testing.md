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
prerequisite authentication. Canonical lease reacquisition and a durable load
envelope now exist, but Cargo does not publish that envelope and there is no
application handoff or recovered host-admission path.
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
Use the optional `GFX942-ALPHA-ZETA-HARDWARE` parity snapshot shard to archive
this exact command and artifact pin; see [Evidence Record V1](evidence-record-v1.md).

### Repository-backed compiler evidence controller

The checked `tests/fixtures/compiler-evidence/gfx942-alpha-zeta-cov6.json` and
`gfx942-mi300x-tools.json` contracts describe one exact-artifact observation
on the configured MI300X host. They pin ROCm 7.2.4, LLVM `22.0.0git`, the
nightly-2026-04-03 Cargo/rustc identities, the shell, Python, CMake, CTest,
Ninja, C++/LLD tools, core utilities and their runtime closure, the measured
Release Worker identity and executable digest, the Rust source digest, exact
`gfx942:xnack-` COV6 metadata, the canonical descriptor section, the finalized
HSACO digest and size, and the five MI300X boundary lengths. The 9 KiB binary
is regenerated and is not committed.

Run the complete controller from a clean committed checkout. Both output paths
must be absent, absolute paths with existing canonical parents:

```text
systemd-run --user --scope --quiet -p Delegate=yes \
  scripts/gfx942-cov6-compiler-evidence.sh \
  /absolute/absent/run-root \
  /absolute/absent/evidence-root
```

The launcher invokes Python with `-B` and `PYTHONDONTWRITEBYTECODE=1`, clears
the inherited environment, and rejects Python cache files in a clean checkout.
Before its first child, the
controller retains the digest-pinned loader/DSO fixture and verifies every
configured executable by canonical path, SHA-256, version output and retained
stat identity. Every child is descriptor-executed under a bounded delegated
cgroup, irreversible Landlock write confinement, and a generated PATH
allowlist. The hostile tests exercise hangs, output and memory exhaustion,
double forks, `setsid`, and attempted migration into writable sibling and
controller cgroups. Run A and run B
use separately copied immutable tracked sources, vendored registries, Rust
sysroots, LLVM/ROCm/OCML closure records, initially empty Worker build trees,
and `CARGO_TARGET_DIR`s. Generated native tests execute directly from sealed
descriptors and CTest must report 3/3 through descriptor-backed commands. Each
run independently builds the Worker and Rust integration-test executable,
records request/response/raw/final identities, runs the real `rustc ->
CompilerModuleHandoffV2 -> Worker V2` path, and must produce the checked
byte-identical Worker and HSACO. After comparison, the controller retains the
run-A HSACO descriptor, independently builds the ignored hardware test, retains
its direct dynamic-runtime closure, and requires the hardware child to compare
the direct HSACO dirent with the inherited descriptor before dispatching the
descriptor bytes.

The provider manifest includes the selected GCC C++ and target headers, system
headers, Clang resource headers, CMake modules, LLVM libraries, ROCm headers and
version files, and device bitcode. Large ROCm DSOs outside this compile path are
listed by exact path, size, and dependency justification rather than silently
omitted. Each run also retains its generated Worker objects, dependency graph,
native tests, Cargo build-script outputs, proc-macro/backend shared objects, and
the hardware test executable. Cross-run comparison is keyed by stable labels,
rejects label reordering and source-inode reuse, and canonicalizes only the
run-specific Cargo hash suffixes. Timing-only CTest and Ninja logs are excluded
explicitly as non-input observations.

For the final dispatch, the controller parses and retains the ELF interpreter,
the direct `ldd` DSO closure, and the transitive closures of the exact
ROCr-requested `libamd_comgr` and `libhsa-amd-aqlprofile64` dynamic roots. It
also retains the loader cache, NSS/passwd/timezone inputs, and exact libdrm GPU
identity file, then re-runs dependency discovery while every closure file is
retained. Landlock admits only those regular files and the HSACO. Unlisted
regular-file `dlopen` and data reads fail closed. `/proc`, `/sys`, and `/dev`
do not share one policy: `/proc` and `/sys` remain read-only observation roots,
while `/dev` has no root grant. Only stat-bound `/dev/kfd`, render nodes 128
through 184 in steps of eight, `/dev/null`, and `/dev/random` are writable; GPU nodes are
also readable. The hardware child verifies that `/dev/shm` create, read, and
`dlopen` all fail before the exact GPU dispatch succeeds.

The evidence directory retains bounded canonical Worker V2 request/response/raw
output bytes, hardware stdout/stderr, and cgroup sample/final files. Their
digests are bound through reproduction manifests and the root summary. Mutation,
omission, and oversize self-tests exercise the corresponding verifier.

The repository-golden test uses host-visible HSA pool allocations for guarded
inputs and outputs; it does not use HIP `DeviceBuffer` compilation or linking.
The hardware child remains physically capped at 8 GiB by cgroup while a
separate bounded 4 TiB `RLIMIT_AS` admits the virtual GPU mappings required by
the eight-device MI300X host.

The golden update is accepted only through the committed canonical transition,
public-key, and signature fixtures. That transition binds the old/new identities
and both external reproduction-manifest digests. This is a fixture-only Ed25519
review check with `authority=none`, not an authenticated human or production
review service.

This is only an observation of those exact artifact bytes under the recorded
tools plus an exact-artifact MI300X observation. The hardware stage loads one
executable and dispatches alpha/zeta at lengths `1`, `255`, `256`, `257`, and
`1023` with CPU oracles and prefix/suffix canaries. It does not prove which
compiler process caused the bytes, refine source semantics, authenticate the
build, create production load or dispatch authority, or issue a compiler
receipt.

The compiler-generation path uses LLVM and LLD library APIs only. It invokes
neither COMGR nor a command-line HSACO linker/disassembler. The hardware HSA
loader may dynamically load the retained ROCm COMGR library as a runtime code
object dependency; it is outside artifact generation and is not used to link
the checked HSACO. The controller output remains descriptive local evidence:
it does not construct
`AuthenticatedCompilerTransactionExecutionReceiptV1`, authenticate compiler
causality, grant load/launch authority outside the test harness, archive signed
production evidence, or promote parity.

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
