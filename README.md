# fe2o3

`fe2o3` is an experimental single-source Rust GPU stack for AMD GPUs.

The next architecture keeps the working AMD runtime while replacing the
elementwise MIR recognizer with a target-neutral compiler pipeline and adding
source-level Verus contracts. See the [v2 architecture](docs/architecture-v2.md),
[cuda-oxide parity matrix](docs/cuda-oxide-parity-matrix.md),
[verification model](docs/verification-model.md),
[GPU safety contract v1](docs/gpu-safety-contract-v1.md), and
[implementation roadmap](docs/implementation-roadmap-v2.md). The
[testing guide](docs/testing.md) defines the generic, Verus, ROCm compile, and
hardware execution lanes.

The intended end state is:

```text
Rust host + #[kernel] device code
        |
        v
rustc frontend and MIR
        |
        +--> native host binary
        |
        +--> fe2o3 device backend -> AMDGPU LLVM IR -> HSACO
                                                |
                                                v
                                     HIP module load/launch
```

## Architecture

The workspace is split into explicit compiler, artifact, runtime, and proof
boundaries:

- Device surface: `fe2o3-device`, `fe2o3-macros`,
  `reserved-fe2o3-symbols`, and `fe2o3-contracts`.
- Compiler: `rustc-codegen-fe2o3`, `fe2o3-kernel-ir`,
  `fe2o3-kernel-analysis`, `dialect-mir`, and `dialect-amdgcn`.
- Artifact model: `fe2o3-artifacts`, `fe2o3-kernel-descriptor`, `fe2o3-hsaco`,
  `fe2o3-hsaco-finalize`, and `fe2o3-artifact-transaction`.
- Runtime: `fe2o3-core`, `fe2o3-completion`, `fe2o3-host`, and
  `fe2o3-hip-sys`.
- Build coordination: `cargo-fe2o3`, `fe2o3-rustc-invocation`, and the
  `fe2o3-rustc-wrapper` binary.
- Verification spike: `examples/verus_vecadd` plus the proof identities and
  records in `fe2o3-contracts` and `fe2o3-artifacts`.

Safe buffer element types and their limits are documented in the
[device memory safety contract](docs/device-memory-safety.md). `DeviceCopy`
establishes structural host-side byte validity only. Safe device interpretation
also requires manifest-derived type and ABI identity, provenance/address-space,
and capability evidence.
Safe ownership of resources used by asynchronous copies is documented in
[device operations](docs/device-operations.md).

## Current Status

### Working end to end

- `cargo-fe2o3 build` builds and loads the custom backend, delegates host
  codegen to `rustc_codegen_llvm`, discovers strict versioned registrations
  emitted by `#[kernel]`, collects device-reachable MIR, and emits HSACO
  sidecars. Registration identifies compiler semantics; it is not package or
  artifact authentication.
- For a public kernel, `#[kernel]` emits a public, doc-hidden marker with the
  deterministic symbol `__fe2o3_kernel_marker_<function>` and an unsafe
  `KernelMarkerV1` implementation tied to the exact Rust function type and V1
  registration. The marker does not authenticate an executable or establish
  its full packed ABI and semantics; generated binding remains an unsafe
  compiler/runtime boundary.
- The default `legacy-v1` AMDGPU emitter supports the repository's `f32`/`f64`
  elementwise examples. It recognizes scalar float arguments and literals,
  read-only slice loads, `DisjointSlice<T>` or indexed mutable-slice stores,
  `+`, `-`, `*`, `/`, unary negation, read-before-write, and the documented
  constant/affine one-dimensional index forms.
- Setting `FE2O3_CODEGEN_PIPELINE=kernel-ir-v1` routes the exact `fill` or
  three-slice `vecadd` kernel through imported MIR, canonical target-neutral
  kernel IR, verification, exact-shape legalization, G1 AMDGPU lowering, and
  the normal transactional LLVM/object/HSACO publication path. The selector,
  ABI, witness dataflow, bounds control flow, and accepted kernel shapes are
  fail closed: invalid values and unsupported kernels remove stale generation
  artifacts and never fall back to `legacy-v1`.
- The HIP runtime provides contexts, streams, device buffers, pinned host
  buffers, events, synchronous transfers, event-backed borrowed and owned
  asynchronous transfers, module loading, and kernel launch.
- Raw module loading and raw launch are explicit `unsafe` escape hatches. The
  caller remains responsible for artifact trust, target and ABI compatibility,
  pointer validity, aliasing, launch geometry, and resource lifetimes.
- `DeviceCopy` and its derive macro restrict safe byte transfers to supported
  layouts and have compile-pass/compile-fail coverage.

The recorded hardware run used a `gfx1201` AMD Radeon AI PRO R9700 with TheRock
ROCm `7.13.0a20260509`. The smoke suite generated HSACO, launched every current
example, copied results back, and compared them with CPU results.

### Implemented foundations

- The structured MIR importer lowers the vecadd-shaped subset, including
  scalar control flow, helper calls, and slice memory operations, into the
  target-neutral `fe2o3-kernel-ir`. Its verifier checks types, SSA uses,
  control-flow edges, memory accesses, launch axes, capabilities, barriers, and
  atomics. The IR has a bounded canonical V1 wire format. The G1
  `dialect-amdgcn` path lowers the verified 1D fill and vecadd subset to
  deterministic AMDGPU LLVM and is connected to the opt-in `kernel-ir-v1`
  fill and vecadd paths above; it is not yet general or the default. For its
  modeled effects, Kernel IR derives formal allocation identities, affine byte
  regions, bounds requirements, runtime-alias requirements, and
  inter-invocation race obligations. Unsupported index widths, arithmetic,
  calls, or memory effects make the analysis incomplete rather than silently
  granting authority. Even a complete result is conditional on an explicit
  launch extent; the extent and mappings from formal parameters to runtime
  allocations remain unauthenticated and grant no proof or launch authority.
- Versioned artifact manifests, ABI layouts, launch contracts, bounded
  containers, payload digests, native-kernel selection, and proof records have
  canonical encoders, decoders, and adversarial tests.
- Canonical AMD target IDs, HIP-observed device properties, HSACO metadata and
  descriptor inspection, kernel-descriptor binding, and bounded post-link
  finalization are implemented as separate validation layers.
- `fe2o3-host` has a `PreparedLaunch<K>` geometry/resource checker and a
  `LoadedKernel<K>` authority that owns the exact HIP module and function and
  can bind only matching prepared launches. Argument admission reserves
  context-scoped allocation ranges and rejects overlapping mutable or
  mutable/shared aliases. A doc-hidden generated-code SPI can unsafely bind a
  generated marker, load it, construct read-only and writable typed device-slice
  capabilities, and seal one exact admission, argument pack, and capability
  pack into `GeneratedAdmittedLaunch`. Its safe scoped launch retains the typed
  buffer borrows, alias reservation, loaded authority, and packed parameters
  until HIP completion or a stronger stream-quiescence fallback. Marker and
  executable authenticity, the complete ABI/effect association, and assembly
  of the sealed launch remain explicit unsafe obligations.
- Compiler artifact publication is transactional and generation-owned. Build
  attempt and canonical rustc invocation descriptors are versioned and
  bounded.
- Linux-only rustc and codegen-backend primitives use descriptor-backed procfs
  paths. The backend is copied into a rehashed, immutable sealed memfd and can
  be inherited only by a prepared child command. Neither primitive is connected
  to compile execution.
- `examples/regression-manifest-v1.txt` is the authoritative package/artifact
  inventory for ordinary checks, ROCm compilation, and GPU smoke tests.
- The Verus vecadd and fill harnesses prove bounded source-model properties
  under documented assumptions. The exact control, index, guarded memory
  access, and write body of the production `f32` vecadd kernel is mechanically
  shared with Verus through explicit thread and arithmetic adapters. Two
  positive harnesses and twelve expected-rejection mutations run in the
  required proof lane; the three real-body mutations additionally require one
  exact primary diagnostic and failed source clause. Verus uses a total model
  arithmetic adapter and does not prove that production IEEE `f32` addition,
  compiler output, HSACO, or GPU execution refines that model. Proof-record
  matching rejects incomplete or mismatched identities, but the records remain
  synthetic evidence rather than authenticated compiler-refinement evidence.

### Not yet integrated

- General MIR to kernel IR to AMDGPU lowering is not complete; `kernel-ir-v1`
  accepts only the exact fill and vecadd shapes, and the elementwise recognizer
  remains the default emitter.
- Artifact manifests, descriptor finalization, observed targets, and proof
  records do not yet produce a compiler-generated, user-facing typed module,
  loader, or launch API. The doc-hidden unsafe generated-code SPI and sealed
  `GeneratedAdmittedLaunch` are implementation foundations, not a safe
  authenticated artifact load: marker/executable authenticity and the complete
  ABI and executable-effect association still enter through unsafe code.
- `cargo fe2o3 verify` and `build --require-proof` are roadmap commands. The
  current required Verus CI lane is invoked separately and does not prove the
  ordinary Rust function, compiler, ROCm, driver, or machine-code refinement.
- The fail-closed rustc wrapper classifies and preserves approved bootstrap
  invocations, but compile execution remains disabled until the pinned rustc
  and sealed backend primitives are composed with the validated invocation.
  Rustc-descendant descriptor lifetime, dynamic loading, transitive shared
  libraries, and non-Linux execution remain unresolved.
- General Rust language support, LDS, atomics and barriers in emitted kernels,
  wave operations, device linking, sanitizer/debugger integration, and
  multi-device memory remain parity work.

The current comparison with cuda-oxide is tracked in the
[parity matrix](docs/cuda-oxide-parity-matrix.md). fe2o3 is not yet at parity.

See [docs/implementation-plan.md](docs/implementation-plan.md) for the original
compiler/runtime plan and
[docs/implementation-roadmap-v2.md](docs/implementation-roadmap-v2.md) for the
current staged roadmap.

## Commands

Run diagnostics:

```bash
cargo run -p cargo-fe2o3 -- doctor
```

Preview or remove only fe2o3-generated artifacts under `target/fe2o3`:

```bash
cargo run -p cargo-fe2o3 -- clean --dry-run
cargo run -p cargo-fe2o3 -- clean
```

The clean command discovers the enclosing Cargo project or workspace and
preserves the rest of its target directory. Planning opens and retains the
canonical project-root capability. Each successful no-follow component open is
authoritative: substitution completed before that open selects the current
ordinary directory, while substitution after it cannot redirect later access.
Metadata is used only after an open failure to produce a fail-closed diagnostic.

Destructive cleanup is supported on Unix, where the opened `target/fe2o3`
directory is passed to capability-relative opened-directory removal. With the
pinned capability implementation, Windows removal is pathname-based, so fe2o3
fails closed there; `--dry-run` remains available. Unix opened-directory removal
is not atomic against every concurrent rename and can fail after partially
removing the opened directory's contents.

This is intentionally narrower than pinned cuda-oxide's clean command, which
removes the project's full target directory. Parity remains partial until fe2o3
also supports complete build orchestration from an external Cargo project.

If `FE2O3_TARGET` is not set, `cargo-fe2o3` tries to infer the target from
`rocminfo` and falls back to `gfx1100`.

For explicit package builds such as `-p fe2o3-saxpy`, `cargo-fe2o3` cleans that
package before invoking Cargo so sidecar HSACO files are regenerated even if the
host crate was already up to date.

Validate the authoritative example manifest and list a lane:

```bash
cargo run --locked -p cargo-fe2o3 -- examples check
cargo run --quiet --locked -p cargo-fe2o3 -- examples list rocm-compile
```

Run the repository validation lanes:

```bash
scripts/ci-local.sh generic
VERUS=/absolute/path/to/verus scripts/ci-local.sh verus
FE2O3_TARGET=gfx1151 scripts/ci-local.sh rocm-compile
FE2O3_ALLOW_GPU_SMOKE=1 FE2O3_TARGET=gfx1151 scripts/ci-local.sh hardware-smoke
```

The ROCm and hardware lanes require a matching AMD GPU and ROCm installation.
To build or run one package directly:

```bash
cargo run --locked -p cargo-fe2o3 -- build -p fe2o3-vecadd
cargo run --locked -p cargo-fe2o3 -- run -p fe2o3-vecadd
FE2O3_CODEGEN_PIPELINE=kernel-ir-v1 \
  cargo run --locked -p cargo-fe2o3 -- run -p fe2o3-vecadd
```

The smoke command reads the same manifest and runs every GPU-selected example:

```bash
cargo run --locked -p cargo-fe2o3 -- smoke
```
