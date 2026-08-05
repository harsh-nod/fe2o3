# fe2o3

`fe2o3` is an experimental single-source Rust GPU stack for AMD GPUs.

The next architecture keeps the working AMD runtime while replacing the
elementwise MIR recognizer with a target-neutral compiler pipeline and adding
source-level Verus contracts. See the [v2 architecture](docs/architecture-v2.md),
[cuda-oxide parity matrix](docs/cuda-oxide-parity-matrix.md),
[verification model](docs/verification-model.md), and
[implementation roadmap](docs/implementation-roadmap-v2.md). The
[testing guide](docs/testing.md) defines the generic, ROCm compile, and hardware
execution lanes.

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
- Compiler: `rustc-codegen-fe2o3`, `fe2o3-kernel-ir`, `dialect-mir`, and
  `dialect-amdgcn`.
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
  codegen to `rustc_codegen_llvm`, discovers trusted `#[kernel]` items, collects
  device-reachable MIR, and emits HSACO sidecars.
- The production AMDGPU emitter supports the repository's `f32`/`f64`
  elementwise examples. It recognizes scalar float arguments and literals,
  read-only slice loads, `DisjointSlice<T>` or indexed mutable-slice stores,
  `+`, `-`, `*`, `/`, unary negation, read-before-write, and the documented
  constant/affine one-dimensional index forms.
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
  atomics. This IR is not yet the default AMDGPU emission path.
- Versioned artifact manifests, ABI layouts, launch contracts, bounded
  containers, payload digests, native-kernel selection, and proof records have
  canonical encoders, decoders, and adversarial tests.
- Canonical AMD target IDs, HIP-observed device properties, HSACO metadata and
  descriptor inspection, kernel-descriptor binding, and bounded post-link
  finalization are implemented as separate validation layers.
- Compiler artifact publication is transactional and generation-owned. Build
  attempt and canonical rustc invocation descriptors are versioned and
  bounded.
- The Verus vecadd harness proves bounds and injective writes under a documented
  hardware-thread-ID contract. Proof-record matching can reject incomplete or
  mismatched evidence.

### Not yet integrated

- General MIR to kernel IR to AMDGPU lowering is not complete; the elementwise
  recognizer remains the production emitter.
- Artifact manifests, descriptor finalization, observed targets, and proof
  records do not yet produce a sealed validated module or generated typed
  launch API. There is no `PreparedLaunch<K>` implementation.
- `cargo fe2o3 verify` and `build --require-proof` are roadmap commands. The
  current Verus harness is invoked separately and does not prove compiler,
  ROCm, driver, or machine-code refinement.
- The fail-closed rustc wrapper classifies and preserves approved bootstrap
  invocations, but compile execution remains disabled until rustc and backend
  executable pinning is implemented.
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

Check the workspace:

```bash
cargo check --workspace
```

Smoke-test the current backend entry point:

```bash
cargo run -p cargo-fe2o3 -- build -p fe2o3-vecadd
cargo run -p cargo-fe2o3 -- build -p fe2o3-add-inplace
cargo run -p cargo-fe2o3 -- build -p fe2o3-copy
cargo run -p cargo-fe2o3 -- build -p fe2o3-downsample
cargo run -p cargo-fe2o3 -- build -p fe2o3-fill
cargo run -p cargo-fe2o3 -- build -p fe2o3-gather-odd
cargo run -p cargo-fe2o3 -- build -p fe2o3-scale
cargo run -p cargo-fe2o3 -- build -p fe2o3-shift
cargo run -p cargo-fe2o3 -- build -p fe2o3-previous
cargo run -p cargo-fe2o3 -- build -p fe2o3-stencil
cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-add-index
cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-const-minus
cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-parenthesized-sub
cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-disjoint-inplace-shift
cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-disjoint-shift
cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-gather
cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-neighbors
cargo run -p cargo-fe2o3 -- build -p fe2o3-raw-output-shift
cargo run -p cargo-fe2o3 -- build -p fe2o3-saxpy
cargo run -p cargo-fe2o3 -- build -p fe2o3-axpy-inplace
cargo run -p cargo-fe2o3 -- build -p fe2o3-negate
cargo run -p cargo-fe2o3 -- build -p fe2o3-normalize
cargo run -p cargo-fe2o3 -- build -p fe2o3-pipeline
cargo run -p cargo-fe2o3 -- build -p fe2o3-vecadd-f64
```

On a machine with a working AMD GPU and ROCm driver stack:

```bash
cargo run -p cargo-fe2o3 -- smoke
```

The smoke command runs the supported backend examples in sequence. To run one
package at a time:

```bash
cargo run -p cargo-fe2o3 -- run -p fe2o3-vecadd
cargo run -p cargo-fe2o3 -- run -p fe2o3-add-inplace
cargo run -p cargo-fe2o3 -- run -p fe2o3-copy
cargo run -p cargo-fe2o3 -- run -p fe2o3-downsample
cargo run -p cargo-fe2o3 -- run -p fe2o3-fill
cargo run -p cargo-fe2o3 -- run -p fe2o3-gather-odd
cargo run -p cargo-fe2o3 -- run -p fe2o3-scale
cargo run -p cargo-fe2o3 -- run -p fe2o3-shift
cargo run -p cargo-fe2o3 -- run -p fe2o3-previous
cargo run -p cargo-fe2o3 -- run -p fe2o3-stencil
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-add-index
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-const-minus
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-parenthesized-sub
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-disjoint-inplace-shift
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-disjoint-shift
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-gather
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-neighbors
cargo run -p cargo-fe2o3 -- run -p fe2o3-raw-output-shift
cargo run -p cargo-fe2o3 -- run -p fe2o3-saxpy
cargo run -p cargo-fe2o3 -- run -p fe2o3-axpy-inplace
cargo run -p cargo-fe2o3 -- run -p fe2o3-negate
cargo run -p cargo-fe2o3 -- run -p fe2o3-normalize
cargo run -p cargo-fe2o3 -- run -p fe2o3-pipeline
cargo run -p cargo-fe2o3 -- run -p fe2o3-vecadd-f64
```
