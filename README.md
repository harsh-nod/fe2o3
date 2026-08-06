# fe2o3

`fe2o3` is an experimental single-source Rust GPU stack for AMD GPUs.

The next architecture keeps the working AMD runtime while replacing the
elementwise MIR recognizer with a target-neutral compiler pipeline and adding
source-level Verus contracts. See the [v2 architecture](docs/architecture-v2.md),
[cuda-oxide parity matrix](docs/cuda-oxide-parity-matrix.md),
[evidence-backed parity dashboard](docs/generated/cuda-oxide-parity-dashboard.md),
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
  `fe2o3-kernel-analysis`, `fe2o3-rustc-front`, `dialect-mir`, and
  `dialect-amdgcn`.
- Artifact model: `fe2o3-artifacts`, `fe2o3-kernel-descriptor`, `fe2o3-hsaco`,
  `fe2o3-hsaco-finalize`, and `fe2o3-artifact-transaction`.
- Runtime: `fe2o3-core`, `fe2o3-completion`, `fe2o3-host`, and
  `fe2o3-hip-sys`.
- Build coordination: `cargo-fe2o3`, `fe2o3-rustc-invocation`, and the
  `fe2o3-rustc-wrapper` binary.
- Verification: `fe2o3-contracts`, the bounded `fe2o3-verifier` driver model,
  `examples/verus_vecadd`, and proof records in `fe2o3-artifacts`.
- Test and release evidence: `fe2o3-differential`, the Cargo inspection/tool
  commands, `scripts/parity-evidence.sh`, and the deterministic claim gate in
  `scripts/parity-dashboard.sh`.

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
- `#[kernel(typed)]` implements one exact generated profile for a public, safe,
  non-generic unit function with signature
  `pub fn(&[f32], &[f32], DisjointSlice<f32>)`. It emits
  `<kernel>_gpu::{Kernel, Prepared}`. The backend packages the finalized LLVM IR
  identity, native HSACO payload, target, exact 48-byte read/read/write slice
  ABI, canonical rustc-derived type/layout identities, and fixed
  one-dimensional launch contract into a canonical `ArtifactContainerV1`, then
  embeds those immutable bytes in the host link. Extraction uses normalized,
  monomorphized rustc types, so token aliases such as `type f32 = f64` fail at
  the compiler boundary.
- The typed vecadd V2 profile records a domain-separated canonical source shape,
  rustc ABI class, pointer width, size, alignment, and ordered physical
  components for all three arguments. The host independently reconstructs the
  same evidence from its actual slice and `DisjointSlice` layouts.
  `Kernel::load` recomputes an identity over the profile, full kernel binding,
  names, source and executable digests, ABI fields, effects, type/layout
  identities, and launch contract before loading the embedded payload.
- Before embedding, the backend parses the finalized HSACO and binds its ELF
  entries to AMDHSA descriptors. The fixed profile requires one normal kernel,
  the exact target and symbol, no printf/init/fini entry, and six 8-byte
  pointer/length kernargs at offsets `0, 8, 16, 24, 32, 40`, followed by the
  runtime-populated implicit argument region at offset 48.
- Typed V2 registrations carry full SHA-256 crate and kernel binding IDs. The
  Cargo wrapper derives the crate ID from rustc's crate name and ordered
  `-C metadata` values; the macro and backend independently derive and validate
  the kernel ID. Private host functions and artifact accessors use that ID, so
  same-named kernels in separate rlibs cannot silently resolve to one archive
  member. Direct compilation without the wrapper fails closed unless source
  declares an explicit 256-bit fallback namespace.
- The generated vecadd API safely prepares equal, nonempty `f32` buffers,
  performs context, geometry, and alias admission, and retains all typed
  resources through either synchronous `Prepared::launch` or non-escapable
  `Prepared::launch_scoped`. The vecadd example uses only this generated API; it
  contains no artifact pathname, raw parameter pack, or unsafe user launch.
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

Hardware smoke coverage includes a local `gfx1151` Radeon 8060S and a remote
`gfx942` MI300X. On both targets the suite generated and inspected real HSACO,
launched every runnable example, copied results back, and compared them with
CPU results. These runs cover the current narrow executable paths; they do not
turn the foundations below into end-to-end features.

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
- A bounded rustc-front record models canonical collected function signatures,
  source locations, and CFG edges, and the rustc backend can construct those
  records for monomorphized functions. Reducible-CFG analysis and
  block-argument-to-LLVM-phi lowering are implemented and tested, including
  loop-shaped Kernel IR. The production device pipeline still does not consume
  these records generally, and most Rust MIR operations remain absent.
- The G2 type foundation records validated semantic scalars, pointers,
  references, slices, tuples, arrays, structs, direct and niche enums, padding,
  and rustc ABI facts. A bounded rustc-private extractor obtains those facts
  for fully monomorphized types. Separate bounded foundations now model
  semantic constants/data relocations and manifest-driven scalar/slice packing,
  but neither is a general rustc-to-artifact integration. Dedicated fixtures
  make the current generic, const-generic, aggregate, integer-match, loop, and
  cross-crate collection/lowering frontiers explicit; generic registered kernel
  roots remain unsupported.
- Versioned artifact manifests, ABI layouts, launch contracts, bounded
  containers, payload digests, native-kernel selection, and proof records have
  canonical encoders, decoders, and adversarial tests.
- G3 adds a canonical multi-kernel bundle index, validated compiler-generated
  argument-packing plans, and explicit asynchronous operation lifecycle
  records. These are bounded data and typestate foundations; no general
  manifest-to-host-code generator or composable cancellation API consumes all
  of them yet.
- Canonical AMD target IDs, HIP-observed device properties, HSACO metadata and
  descriptor inspection, kernel-descriptor binding, and bounded post-link
  finalization are implemented as separate validation layers.
- The G4 model includes capability tables for supported AMD targets, branded
  3D invocation and wave-lane witnesses, canonical Kernel IR for static and
  dynamic LDS, scoped atomics, fences, and convergence-bearing workgroup
  barriers. The experimental AMD lowering emits LDS, scoped integer atomics,
  fences, workgroup barriers, and explicit wave32/wave64 lane, ballot, vote,
  and bounded shuffle operations. These paths have produced target-specific
  code objects, and a branded dynamic-LDS API enforces bounded disjoint
  typestates. Dynamic-LDS launch-byte plumbing, broad atomics and wave
  collectives, GPU semantic execution, and general source-to-IR integration
  remain fail-closed gaps.
- `fe2o3-host` has a `PreparedLaunch<K>` geometry/resource checker and a
  `LoadedKernel<K>` authority that owns the exact HIP module and function and
  can bind only matching prepared launches. Argument admission reserves
  context-scoped allocation ranges and rejects overlapping mutable or
  mutable/shared aliases. The exact generated vecadd adapter assembles these
  pieces behind its safe API. The general doc-hidden generated-code SPI still
  exposes an unsafe compiler boundary: backend/linker association of a marker,
  complete ABI and effects, and executable semantics must be correct before its
  sealed launch can be treated as safe.
- Compiler artifact publication is transactional and generation-owned. Typed
  generation results contain bounded immutable IR and HSACO snapshots captured
  through exact staged file descriptors and validated after publication while
  the transaction lock is still held. Later publication or pathname
  replacement cannot mix generations. Build-attempt and canonical rustc
  invocation descriptors are versioned and bounded.
- Linux-only rustc and codegen-backend primitives use descriptor-backed procfs
  paths. The backend is copied into a rehashed, immutable sealed memfd and can
  be inherited only by a prepared child command. Neither primitive is connected
  to compile execution.
- `examples/regression-manifest-v1.txt` is the authoritative package/artifact
  inventory for ordinary checks, ROCm compilation, and GPU smoke tests.
- The Verus vecadd, fill, active-wave, and LDS harnesses prove bounded
  source-model properties under documented assumptions. The exact control,
  index, guarded memory access, and write body of the production `f32` vecadd
  kernel is mechanically shared with Verus through explicit thread and
  arithmetic adapters. Positive harnesses and paired expected-rejection
  mutations run in the required proof lane; the three real-body mutations
  additionally require one exact primary diagnostic and failed source clause.
  Verus uses a total model arithmetic adapter and does not prove that
  production IEEE `f32` addition,
  compiler output, HSACO, or GPU execution refines that model. Proof-record
  matching rejects incomplete or mismatched identities, but the records remain
  synthetic evidence rather than authenticated compiler-refinement evidence.
- The G5 contracts now describe bounded independent-thread reads and writes,
  allocation provenance, bounds, injective writes, and deterministic proof
  obligations. Paired copy, gather, and affine elementwise bodies have positive
  and negative Verus harnesses. `fe2o3-verifier` canonicalizes bounded tool,
  policy, invocation, and result records, has a bounded shell-free process
  executor, and can convert validated results into descriptive proof records.
  It still has no reviewed Verus adapter, authenticated binary measurement,
  compiler or machine-code refinement, or runtime authority.
- G6/G7 includes canonical multi-input AMDGPU link plans and a standalone
  direct LLVM/LLD worker with bounded Rust/C++ protocols. Device FFI macros and
  compiler validation bind import/export symbols, physical ABI, address spaces,
  effects, target, and code-object version. Cooperative and peer capabilities
  retain exact contexts, streams, and cleanup ownership. The opt-in
  `kernel-ir-worker-v2` flow now carries one real Rust source module through
  rustc collection, verified kernel IR, an attempt-scoped textual LLVM handoff,
  exact compiler-produced symbol-role manifests, Cargo wrapper consumption,
  and byte-identical GenericLink/V2 execution in a measured direct LLVM/LLD
  worker for `gfx942`. This path uses LLVM and LLD library APIs directly; it
  does not use COMGR or command-line linking. Cargo independently admits the
  returned raw HSACO, derives publication only from retained inspection
  evidence, and durably publishes it with an attempt-bound provenance receipt.
  The durable transaction has adversarial and crash-boundary coverage, while
  exact retry after the handoff has been consumed is currently limited to the
  same Cargo process. Compiler identity and origin are not authenticated, no
  Verus proof is authenticated or bound to the executable, canonical artifact
  finalization is not performed, and the output grants no HSA load or launch
  authority. A release-worker `emitObject` stack-smashing failure on the MI300X
  still blocks successful `gfx942` hardware validation of this vertical slice.
- G8 adds deterministic model generation/reduction and a bounded conformance
  harness that executes fill, vecadd, and affine kernels against an independent
  HIP/CPU oracle. `cargo fe2o3 inspect` performs bounded read-only decoding.
  `sanitize` and `debug` retain plan mode and can execute descriptor-pinned
  native ROCgdb under bounded supervision. ROCgdb precise-memory diagnostics
  are not a race, API, initialization, synchronization, or safety proof.

### Not yet integrated

- General MIR to kernel IR to AMDGPU lowering is not complete; `kernel-ir-v1`
  accepts only the exact fill and vecadd shapes, and the elementwise recognizer
  remains the default emitter.
- The generated typed path supports only
  `pub fn(&[f32], &[f32], DisjointSlice<f32>)`; general typed signatures,
  arbitrary rustc layouts, aggregate arguments, multi-profile module
  generation, and multi-kernel bundles are not implemented. Canonical
  rustc-derived evidence currently covers only the fixed two shared slices and
  one `DisjointSlice<f32, Index1D>` profile on 64-bit targets.
- The generated contract identity authenticates compiler declarations and the
  exact payload bytes; it does not inspect machine code to prove that declared
  read/read/write effects match every executable memory access. The fixed
  lowering, Kernel IR checks, host alias admission, and tests provide separate
  defenses, but general illegal-access and race freedom still require complete
  analysis and Verus/refinement evidence. Trusted rustc diagnostic-item
  classification also remains part of the compiler TCB.
- The generated vecadd API has synchronous launch and a scoped asynchronous
  callback that cannot return the in-flight operation. Generalized returnable
  borrowed or owned generated async APIs, cancellation, and composition are not
  complete.
- Generated artifact embedding currently supports only the
  `x86_64-unknown-linux-gnu` host. V2 binding IDs close same-name cross-crate
  archive aliasing, but marker-to-artifact association remains part of the
  trusted compiler/linker contract and does not prove executable semantics.
- `cargo fe2o3 verify` and `build --require-proof` are roadmap commands. The
  current required Verus CI lane is invoked separately and does not prove the
  ordinary Rust function, compiler, ROCm, driver, or machine-code refinement.
  Verus proof identity/refinement is not authenticated into the generated
  vecadd artifact or required by its safe loader and launch API.
- The fail-closed rustc wrapper classifies and preserves approved bootstrap
  invocations, but compile execution remains disabled until the pinned rustc
  and sealed backend primitives are composed with the validated invocation.
  Rustc-descendant descriptor lifetime, dynamic loading, transitive shared
  libraries, and non-Linux execution remain unresolved.
- General Rust language support, frontend-to-layout integration, broad atomic
  and wave collective support, production direct-link integration, general
  device FFI, occupancy-complete cooperative launch, multi-device memory
  semantics, full sanitizer/debugger coverage, broad differential fuzzing, and
  authenticated Verus refinement remain parity work. LDS, atomics, waves,
  fences, and barriers exist only in bounded experimental paths and are not yet
  generally available from ordinary Rust kernels.

The current comparison with cuda-oxide is tracked in the
[parity matrix](docs/cuda-oxide-parity-matrix.md) and the generated
[evidence dashboard](docs/generated/cuda-oxide-parity-dashboard.md). fe2o3 is
not yet at parity.

See [docs/implementation-plan.md](docs/implementation-plan.md) for the original
compiler/runtime plan and
[docs/implementation-roadmap-v2.md](docs/implementation-roadmap-v2.md) for the
current staged roadmap.

## Commands

Run diagnostics:

```bash
cargo run -p cargo-fe2o3 -- doctor
```

Inspect a bounded fe2o3 artifact without loading it, or print a normalized
ROCgdb execution plan:

```bash
cargo run -p cargo-fe2o3 -- inspect target/fe2o3/kernel.hsaco
cargo run -p cargo-fe2o3 -- sanitize -- ./target/debug/application
cargo run -p cargo-fe2o3 -- debug -- ./target/debug/application
```

Execution is explicit and bounded. Debug execution requires an explicit batch
or interactive mode:

```bash
cargo run -p cargo-fe2o3 -- sanitize --execute -- ./target/debug/application
cargo run -p cargo-fe2o3 -- debug --execute --batch -- ./target/debug/application
```

Sanitize fails when requested precise-memory coverage is unavailable. Race and
API coverage are reported as unsupported rather than inferred from a clean run.

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
The release-evidence collector requires a complete archive-relative row-link
map and records Git, rustc, LLVM, ROCm, driver, target, and stable lane
identities without changing matrix status:

```bash
scripts/parity-evidence.sh collect \
  --rows rows.tsv --hardware-lane mi300x-gfx942-release > evidence.tsv
scripts/parity-evidence.sh validate evidence.tsv
scripts/tests/parity-evidence.sh
scripts/parity-dashboard.sh check
scripts/tests/parity-dashboard.sh
```

`scripts/parity-dashboard.sh update` rewrites only the deterministic generated
Markdown and TSV dashboard. The check rejects stale paths, missing claims,
unsupported status upgrades, target/evidence mismatches, and generated drift.

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
