# fe2o3

`fe2o3` is an experimental single-source Rust GPU stack for AMD GPUs. The goal
is to let kernel authors write Rust, keep explicit compiler/runtime evidence for
what was accepted, and connect bounded kernel behavior to source models and
Verus-facing contracts.

This repository contains the implementation: compiler crates, runtime crates,
examples, proof models, evidence records, debugger/profiler tools, and hardware
qualification lanes. The public learning path lives in `fe2o3-kernels`.

## Start Here

If you are new to `fe2o3`, start with the kernel learning workbench:

- Deployed guide: <https://harsh-nod.github.io/fe2o3-kernels/>
- Source repo: <https://github.com/harsh-nod/fe2o3-kernels>

`fe2o3-kernels` is the authoritative guide to runnable bounded `fe2o3`
kernels. Every kernel lesson should identify the exact source, reference or
oracle, runner command, target, evidence status, and non-claims.

Use this repository when you are ready to inspect or change the implementation.
When adding a kernel, keep the source, reference, runner, and evidence shape in
`fe2o3-kernels` synchronized with the implementation here.

## What Works Today

The community launch surface is a set of runnable bounded examples. These are
real kernels with explicit boundaries, not broad library or serving claims.

| Kernel family | Community status | Boundary |
| --- | --- | --- |
| Fill / Vecadd | Runnable bounded starter examples | Introductory shapes and typed host paths |
| Row softmax / GEMM / FlashAttention / MoE | Runnable bounded operator examples with evidence labels | Specific shapes, targets, and runners |
| gfx950 KDA/GDN | Runnable bounded gfx950 attention examples | Teaching decode/prefill slices, not a full model layer |
| Kimi K3 KDA | Runnable bounded decode-core example | Not full Kimi K3 serving, batching, cache plumbing, or full-model equivalence |
| GPT-OSS layer tile | Runnable bounded layer-tile example | Not a complete GPT-OSS layer or whole-model decode |
| Debugger / simulator | Bounded CPU and live-host tools | They report unsupported capabilities instead of inferring success |

## Production Compiler Status

The runnable examples above do not mean the full production compiler is done.
That is a different claim.

The full production compiler is incomplete because `fe2o3` does not yet accept a
broad Rust GPU kernel surface and carry every accepted program through one
general, authenticated, evidence-bearing path from source to safe launch.

The remaining work is mainly:

- General source coverage for more Rust control flow, memory patterns, layouts,
  synchronization, math, and model-kernel shapes.
- General lowering from Rust/MIR through semantic IR, Kernel IR, LLVM, and HSACO
  without workload-specific admission.
- Source-to-machine authority that connects accepted source, proof/model facts,
  generated artifacts, and machine code for each promoted kernel.
- One protected publication, currentness, load, and launch chain for ordinary
  safe application use.
- Production verifier or attestor paths that replace qualification-only,
  historical, or test-issued records.
- Library and serving integration for arbitrary shapes, performance tuning, and
  model-scale execution.

So the launch message is precise: `fe2o3-kernels` contains runnable bounded
examples; `fe2o3` is still completing the general production compiler and
authority path.

## Quick Start

For the guided path, use `fe2o3-kernels` first. To run the generic validation
lane in this repository:

```bash
git clone https://github.com/harsh-nod/fe2o3
cd fe2o3
scripts/ci-local.sh generic
```

Useful local commands:

```bash
cargo run -p cargo-fe2o3 -- doctor
cargo run -p cargo-fe2o3 -- inspect target/fe2o3/kernel.hsaco
cargo run -p cargo-fe2o3 -- clean --dry-run
```

GPU compile and hardware lanes require a ROCm/KFD-capable AMD host and explicit
target selection. The exact runnable kernel commands are documented in
`fe2o3-kernels`.

## Repository Map

- `crates/`: compiler, runtime, host API, verifier, debugger, protocol, and
  evidence crates.
- `examples/`: runnable kernel examples and bounded operator slices.
- `docs/`: architecture, evidence, roadmap, parity, runtime, and verification
  design documents.
- `scripts/`: local validation, ROCm compile lanes, proof lanes, parity tools,
  and hardware evidence helpers.
- `perf-evidence/`: benchmark and performance-evidence scripts.
- `deployment/`: service and deployment notes for protected components.

## Evidence Model

`fe2o3` uses evidence labels to avoid overclaiming:

- Runnable means there is a command for a bounded slice.
- Bounded means the shape, target, runner, and assumptions are explicit.
- GPU-observed means a particular hardware run was recorded.
- Source-model verified means a model or proof covers the stated source-level
  property.
- Production authority means the compiler, artifact, currentness, load, and
  launch chain grants safe application use.

These labels compose only when the corresponding records compose. A runnable
kernel is not automatically a verified kernel. A verified source model is not
automatically machine-code refinement. A GPU observation is not automatically a
performance or full-library claim.

## Contributing Kernels

For community-facing kernel work, include:

- The exact Rust source path.
- A reference implementation or oracle.
- A runner command and required target.
- Evidence records or tests for the claimed status.
- Explicit non-claims.
- A matching `fe2o3-kernels` lesson or operator-cookbook update.

Do not promote a status claim without the matching implementation and evidence
record in this repository.

## Canonical Docs

- [Architecture](docs/architecture-v2.md)
- [Implementation roadmap](docs/implementation-roadmap-v2.md)
- [Testing guide](docs/testing.md)
- [CUDA-Oxide parity matrix](docs/cuda-oxide-parity-matrix.md)
- [Evidence-backed parity dashboard](docs/generated/cuda-oxide-parity-dashboard.md)
- [Debugger/profiler architecture](docs/debugger-profiler-architecture-v1.md)
- [Verification model](docs/verification-model.md)
- [GPU safety contract](docs/gpu-safety-contract-v1.md)
