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

## Retained bounded MoE evidence

The obsolete MoE V1/V2 host routes and their workload-specific hardware
launchers have been removed. Validate the remaining proof, source, and production-route-absence evidence with:

```text
python3 scripts/test-bounded-moe-docs.py
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
cargo test --locked -p fe2o3-hsa-runtime --all-targets --features hardware-test-hooks --no-run
```

The next hardware evidence must enter through the production Worker V3
application, verifier, HSA load, generated dispatch, completion, and unload
path. A fake verifier or externally selected legacy route cannot satisfy that
gate.

### Repository-backed compiler evidence controller

The checked compiler-evidence fixtures pin the exact source, toolchain, direct
LLVM/LLD worker, COV6 metadata, descriptor section, and finalized HSACO
identity used by the bounded alpha/zeta compiler observation. The controller
runs two independent clean builds and requires byte-identical worker and
artifact outputs.

```text
systemd-run --user --scope --quiet -p Delegate=yes \
  scripts/gfx942-cov6-compiler-evidence.sh \
  /absolute/absent/run-root \
  /absolute/absent/evidence-root
```

The controller is compiler evidence only. Its retired Worker V2 hardware
capture has been removed, so it does not load or dispatch the artifact. The
canonical transaction uses versioned Worker V2 serialization names, but those
records grant no application, verifier, HSA load, or launch authority. Current
hardware qualification must use the sole production Worker V3 route.

The compiler-generation path uses LLVM and LLD library APIs only. It invokes
neither COMGR nor a command-line HSACO linker or disassembler.

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
result back to the host and checks it against a CPU-computed expected value.

The host crate enforces the same split. Its feature-free build exposes the
Worker V3 application, admission, verification, HSA load, and generated
dispatch route. Worker V2 application recovery, embedded-artifact loading,
direct HIP module/function loading, raw parameter packing, cooperative launch,
and workload-specific host adapters are deleted in every feature configuration.
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

## Guard tests

The native-Linux and WSL device-node selection logic has a host-only test:

```text
scripts/tests/ci-local-hardware.sh
```
