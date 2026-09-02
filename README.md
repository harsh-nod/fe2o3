# fe2o3 GPU

`fe2o3` is a developer-preview GPU programming stack for writing kernels in
Rust, compiling them through a typed intermediate representation, and building
a direct-KFD runtime for AMD GPUs.

The project is investigating a deliberately integrated model: source
semantics, compiler evidence, artifacts, runtime authority, deterministic CPU
simulation, and debugger/profiler observations all carry explicit identities
instead of being joined by filenames or convention.

> **Developer Preview**
>
> fe2o3 is under active development. It is not ready for production workloads,
> does not yet provide a supported source-to-GPU first-run experience, and makes
> no API, wire-format, or compatibility stability promise before the first
> preview release. Current direct-KFD qualification is bounded to MI300X
> `gfx942:xnack-` lanes. See [What works](#what-works) and
> [Known limitations](#known-limitations) before evaluating it.

The project currently ships only from a source checkout and makes no crates.io
installation promise. The first developer-preview release remains blocked on
the conditions in the [release process](docs/release-process.md) and
[launch issue #267](https://github.com/harsh-nod/fe2o3/issues/267).

## Why fe2o3

- **Single-source Rust kernels.** Kernel functions use ordinary Rust syntax,
  typed kernel arguments, and explicit device APIs.
- **Direct KFD.** The production runtime direction is the Linux KFD interface;
  HIP and HSA are not fallback execution paths.
- **Typed compiler contracts.** Source, semantic MIR, Pliron, Kernel IR,
  LLVM, artifact, and runtime boundaries are represented explicitly and fail
  closed when a required association is unavailable.
- **CPU simulation without a GPU.** A deterministic Kernel IR V7 simulator can
  execute the supported semantic subset and expose logical work-item, wave,
  workgroup, memory, atomic, fence, and barrier observations. It does not
  predict GPU performance.
- **Semantic debugging and profiling.** Agent-facing JSONL protocols preserve
  provenance and distinguish declared, observed, inferred, and unavailable
  facts. CPU replay supports reverse navigation and structured diagnosis;
  bounded ROCgdb control and rocprofv3 planning/import workflows are
  implemented. Admitted stopped-GPU state, protected real-dispatch capture, and
  ATT decoding remain incomplete.
- **Evidence-aware verification.** Formal contracts and qualification evidence
  are kept separate from compiler, publication, load, and dispatch authority.

## Kernel example

This is the complete kernel body from
[`examples/fill`](examples/fill/src/lib.rs):

```rust
use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel(
    typed,
    namespace = "3f959016b22cc527afdf32bf2ed9b043947c2147348f1ab939488dab760220e5",
    launch(required = [64, 1, 1], max = [64, 1, 1]),
)]
pub fn fill(mut out: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    let Some(value) = out.get_mut(idx) else {
        return;
    };
    *value = 42.5;
}
```

`DisjointSlice` makes the output partition explicit. `thread::index_1d()` is a
typed logical index, and the bounds check remains part of the admitted kernel
semantics.

## Why Write Kernels In fe2o3

The unique bet in `fe2o3` is that a kernel should ship with its intended
semantics, not just its device code. A contributor writes the GPU kernel and a
safe Rust CPU reference or oracle for the same bounded behavior, then the stack
records how mismatches are caught:

- Proof-facing examples use Verus source models and negative fixtures to reject
  wrong value, ownership, bounds, or frame behavior before promotion.
- Runnable examples compare the GPU result, state, or metadata against the safe
  CPU reference and fail the bounded runner on a mismatch.
- Promoted compiler-time claims must go through the compiler-owned
  `SafeReferenceMirToLivePliron` join, generated per-compilation Verus replay,
  and PLIRON structural reconciliation before KIR lowering.

That does not mean arbitrary Rust CPU and GPU semantics are equivalent at
compile time today. Unsupported reference MIR, tensor-component replay, finite
numerical error replay, LLVM/ISA behavior, launch, hardware execution,
performance, and full-model integration still fail closed unless a specific
evidence record says otherwise.

## CPU quick start

### Requirements

- x86-64 Linux with Bash and GNU `realpath`
- Git
- `rustup`; the repository pins `nightly-2026-04-03` and its required
  components in [`rust-toolchain.toml`](rust-toolchain.toml)
- Enough disk space for a Rust compiler workspace build

Clone the repository and run the source-to-CPU quick start:

```console
git clone https://github.com/harsh-nod/fe2o3.git
cd fe2o3
bash scripts/quickstart.sh no-gpu
```

The command exports the ordinary Rust `fill` kernel through the production
source/MIR/KIR stages, creates a temporary authority-free simulation bundle,
executes its embedded KIR on the CPU, and removes the bundle. The result is
deterministic JSON with `"status":"ok"`, copied-back argument bytes, execution
counts, and an explicit statement that no hardware was observed or validated.

Bundle export and simulation do not load a GPU, silently fall back from a
hardware command, authenticate compiler execution, or establish equivalence
with GPU execution. See the [getting-started guide](docs/getting-started.md)
for the component commands, exact-KIR fixture, debugger, and cleanup behavior.

## GPU evaluation status

For compiler engineering only, `cargo fe2o3 engineering hsaco` can extract one
explicitly selected device crate and run its inert handoff twice through an
exactly measured native worker. It writes only a fresh
`fe2o3-engineering-v1/<content-id>/` observation with `"authority":"none"`.
This command does not contact the production compiler-execution supervisor and
its output cannot be adopted as a production generation, publication, load, or
launch owner. Non-path dependencies require an explicit `cargo vendor` tree;
the command measures, retains, and revalidates that tree while Cargo runs with
a fresh home, frozen resolution, no ambient configuration, and no network.
Run `cargo fe2o3 engineering hsaco --help` for the explicit tool, provider,
target, COV, resource-bound, vendor-source, and Cargo-selection arguments.

There is currently no supported copy-and-paste source-to-GPU quick start.
`cargo fe2o3 build` enters the production compiler transaction, but ordinary
example applications still lack the production Worker V3 verifier and release
deployment needed to authorize load and dispatch. For example,
`fe2o3-vecadd` and `fe2o3-fill` intentionally fail closed before dispatch.

Hardware development and CI exercise bounded direct-KFD mechanics and exact
`gfx942:xnack-` qualification lanes on MI300X. These tests demonstrate specific
compiler/runtime properties; they are not a supported general application
workflow. Publishing a clean-checkout source-to-MI300X example is a blocker for
a future GPU-ready preview, not a claim made by this source/simulator preview.

## What works

| Area | Current developer-preview state |
| --- | --- |
| Rust kernel surface | Typed kernels, device indexing, checked buffer views, bounded scalar/control/memory subsets |
| Compiler | Source/MIR through typed Pliron and verified KIR; bounded `gfx942` LLVM/HSACO vertical slices |
| CPU simulation | Deterministic execution of admitted canonical KIR V7, including supported helpers, barriers, workgroup memory, atomics, fences, floating point, and seeded schedule exploration |
| CPU debugger | Work-item, logical wave, workgroup, operation, stack, SSA, allocation-relative memory, break/watch, reverse replay, and structured diagnosis over retained simulator evidence |
| Live debugger | Bounded direct-KFD observation/control and ROCgdb MI integration; hardware lane/register/PC/source state remains incomplete |
| Profiling | Bounded rocprofv3 dispatch import with strict JSON/CSV admission and agent-facing observation queries; real-dispatch and ATT coverage remains incomplete |
| Runtime | Pure-Rust KFD/AQL foundations and bounded MI300X execution diagnostics; public application authorization is incomplete |
| Verification | Verus contracts and evidence-bearing compiler/runtime boundaries for bounded slices; not an end-to-end proof of general kernels |

The [support matrix](docs/support-matrix.md) separates implemented,
qualified, experimental, and unsupported combinations. Detailed historical
milestones are retained in the [project status archive](docs/project-status.md).

## Known limitations

- The only currently qualified production direct-KFD profile is the bounded
  MI300X `gfx942:xnack-` profile. Other AMD targets are not implied.
- An ordinary external project cannot yet compile and dispatch a general Rust
  kernel through one supported public command.
- The simulator accepts a defined KIR V7 semantic subset. Unsupported types and
  operations fail closed; CPU results are not timing or performance predictions.
- CPU logical waves model semantic collectives and visualization partitions,
  not physical GPU wave scheduling or `EXEC` state.
- Live KFD debugging does not yet expose general wave/lane PC, registers,
  target memory, source stepping, or breakpoints.
- ROCgdb integration is bounded by what the installed debugger exposes and is
  not a source of fe2o3 compiler or runtime authority.
- Profiler import has not completed a protected real GPU-dispatch rocprofv3
  round trip. ATT decoding is unavailable without a mutation-proof decoder.
- Multi-GPU distributed kernels and communication/computation overlap are not
  a supported execution surface.
- The compiler and protocols are evolving. Do not treat crate APIs, KIR, bundle,
  debugger, profiler, receipt, or evidence formats as stable unless a document
  explicitly freezes a version.

## Architecture

The intended production flow is:

```text
Rust source
  -> semantic MIR
  -> typed Pliron
  -> verified Kernel IR
  -> AMDGPU LLVM / HSACO
  -> authenticated artifact publication
  -> direct-KFD load and dispatch
```

CPU simulation branches after verified Kernel IR and never becomes runtime or
hardware evidence. Debugger and profiler services join observations through
content identities and typed provenance rather than native paths, descriptors,
or addresses.

Start with the [architecture overview](docs/architecture-v2.md), then use the
[documentation index](docs/README.md) to find compiler, runtime, debugger,
profiler, simulator, verification, and evidence contracts.

## Repository layout

| Path | Purpose |
| --- | --- |
| `crates/fe2o3-device` | Kernel-facing Rust APIs and types |
| `crates/rustc-codegen-fe2o3` | rustc integration, production lowering, and source-to-simulator export |
| `crates/cargo-fe2o3` | Cargo orchestration, inspection, debug, and profile commands |
| `crates/fe2o3-kfd` | Direct Linux KFD boundary |
| `crates/fe2o3-kir-sim*` | Deterministic CPU simulator and CLI |
| `crates/fe2o3-debug-*` | Debug protocol, simulator debugger, and live-tool adapters |
| `examples/` | Kernel source, host-boundary, proof, and qualification examples |
| `docs/` | Architecture, contracts, evidence policy, testing, and status |
| `scripts/` | CI and qualification entry points |

## Development

Run the bounded contributor preflight before opening a pull request:

```console
cargo fmt --all -- --check
bash scripts/tests/quickstart.sh
bash scripts/quickstart.sh source-check examples/vecadd/Cargo.toml
```

Compiler, runtime, proof, and trust-policy changes must also run their
applicable broader lanes, including `bash scripts/ci-local.sh generic-core`
where required. The full validation matrix adds codegen shards, policy checks,
Verus, compile-only AMDGPU checks, and hardware lanes. See
[testing](docs/testing.md) for trust boundaries and required environments.

External contributions are welcome once they satisfy the repository's
fail-closed authority and evidence boundaries. Read
[`CONTRIBUTING.md`](CONTRIBUTING.md), the
[`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md), and
[`SECURITY.md`](SECURITY.md) before submitting work. Support expectations are
documented in [`SUPPORT.md`](SUPPORT.md); maintainer and decision ownership is
documented in [`MAINTAINERS.md`](MAINTAINERS.md).

## Project resources

- [Getting started](docs/getting-started.md)
- [Support matrix](docs/support-matrix.md)
- [Documentation index](docs/README.md)
- [Debugger and profiler tutorial](https://harsh-nod.github.io/fe2o3-kernels/#/debugger/profiler-import)
- [Implementation roadmap](docs/implementation-roadmap-v2.md)
- [Current parity dashboard](docs/generated/cuda-oxide-parity-dashboard.md)
- [Issue tracker](https://github.com/harsh-nod/fe2o3/issues)

## License

Except for third-party-derived or file-specific material identified in
[`THIRD_PARTY_LICENSES.md`](THIRD_PARTY_LICENSES.md), fe2o3-authored source is
licensed under either of

- Apache License, Version 2.0 ([`LICENSE-APACHE`](LICENSE-APACHE)), or
- MIT License ([`LICENSE-MIT`](LICENSE-MIT))

at your option. Contributions are accepted under the same dual license unless
explicitly stated otherwise.
