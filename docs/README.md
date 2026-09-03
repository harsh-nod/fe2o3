# fe2o3 documentation

This index separates first-use documentation from implementation contracts and
historical evidence. fe2o3 is a developer preview; the
[support matrix](support-matrix.md) is the compatibility authority for public
documentation.

## Start here

- [Getting started](getting-started.md): run the CPU simulator, export a Rust
  kernel bundle, and inspect the debugger transcript.
- [Support matrix](support-matrix.md): hosts, targets, execution modes, tools,
  and stability boundaries.
- [Project status archive](project-status.md): detailed milestone narrative and
  open engineering boundaries formerly kept in the root README.
- [Testing guide](testing.md): generic, formal, ROCm, and hardware lanes.
- [Implementation roadmap](implementation-roadmap-v2.md): planned compiler,
  runtime, verification, debugger, profiler, and multi-GPU work.

## Architecture

- [Architecture V2](architecture-v2.md): end-to-end design and ownership model.
- [Workspace layers and ownership](workspace-layers-and-ownership.md): crate and
  dependency boundaries.
- [Production pipeline convergence](production-pipeline-convergence-v1.md):
  the single production compiler route.
- [Pliron Wave 0 architecture](pliron-wave0-architecture.md): typed IR
  integration.
- [Verification model](verification-model.md): what proof and evidence do and
  do not establish.
- [GPU safety contract](gpu-safety-contract-v1.md): memory, launch, artifact,
  and runtime safety obligations.

## Compile and execute

- [General typed dispatch](general-typed-dispatch-v1.md)
- [Production KIR V7 structural bridge](production-kir-v7-structural-bridge-v1.md)
- [Production compiler convergence review](compiler-convergence-review-2026-08-20.md)
- [Worker V3 load envelope](worker-v3-receipt-bearing-load-envelope-v2.md)
- [Pure-Rust runtime](formally-verified-pure-rust-runtime-v1.md)
- [Runtime identity oracle](runtime-identity-oracle-v1.md)
- [Device operations](device-operations.md)
- [Device memory safety](device-memory-safety.md)

## Simulate and debug

- [Source-to-simulator bundle V1](simulation-bundle-v1.md): production
  extraction into an authority-free simulation bundle.
- [Production semantic CPU conformance V3](simulator-production-conformance-v3.md):
  exact ordinary-source Bundle V5/KIR V10 output checks and typed producer gaps.
- [Semantic schedule V1](semantic-schedule-v1.md): deterministic schedule
  recording and replay.
- [Authority-free virtual runtime V1](virtual-runtime-v1.md): bounded
  allocation, copy, queue, dependency, dispatch, completion, and failure-model
  composition over admitted KIR.
- [Simulator diagnosis agent V1](simulator-agent-c5-v1.md): fresh-process,
  read-only diagnosis and bounded witness paging for exact races and virtual
  host-lifetime incidents.
- [Debugger and profiler architecture](debugger-profiler-architecture-v1.md)
- [Profiler Variant V3](profiler-variant-v3.md): finalizer-replayed production KIR comparison.
- [Production profiler KIR archive V1](production-profiler-kir-archive-v1.md): bounded self-contained structural-owner replay.
- [Semantic debug transformation map V2](semantic-debug-transformation-map-v2.md): exact
  cross-layer cardinality separated from producer-authenticated optimization classification.
- [Debugger and profiler task matrix](debugger-profiler-task-matrix-v1.md)
- [Production multi-function semantic debug](production-multifunction-semantic-debug-v1.md)
- [Decoded ATT interchange](decoded-att-interchange-v1.md)
- [Debugger and profiler qualification](debugger-profiler-qualification-v1.md)
- [Direct-KFD runtime profiler](kfd-native-profiler-v1.md)
- [Debugger and profiler reference archive](debugger-profiler-reference-archive-v1.md)
- [Observed GPU target profile](observed-gpu-target-profile-v1.md)

The interactive tutorial is published separately at
[harsh-nod.github.io/fe2o3-kernels](https://harsh-nod.github.io/fe2o3-kernels/).
Its fixtures are demonstrations with explicit evidence boundaries, not a
replacement for the support matrix.

## Proof and evidence

- [Evidence Record V1](evidence-record-v1.md)
- [Parity evidence policy](parity-row-evidence-v1.md)
- [Signed parity evidence](parity-signed-evidence-v2.md)
- [Functional refinement receipt](functional-refinement-receipt-v2.md)
- [Production middle-end evidence](production-middle-end-evidence-v5.md)
- [Production total-output refinement](production-total-output-refinement-v2.md)
- [CUDA-Oxide parity matrix](cuda-oxide-parity-matrix.md)
- [Generated parity dashboard](generated/cuda-oxide-parity-dashboard.md)

Evidence documents describe exact qualified observations. They do not silently
upgrade an experimental target or path into a supported public interface.

## Compiler authority and deployment

These documents specify the fail-closed protected compiler-execution boundary:

- [Compiler execution subject](compiler-execution-subject-v1.md)
- [Compiler execution attestation](compiler-execution-attestation-v1.md)
- [Compiler execution issuer admission](compiler-execution-issuer-admission-v1.md)
- [Durable issuer state](compiler-execution-issuer-durable-v2.md)
- [Compiler execution service](compiler-execution-service-v1.md)
- [Compiler execution deployment](compiler-execution-deployment-bundle-v1.md)
- [Protected publisher service](protected-publisher-service-v1.md)

They are protocol and implementation references, not an installation guide for
a generally available release service.

## Status vocabulary

Public documentation uses these terms consistently:

- **Supported:** intended for users and covered by the stated compatibility
  policy. No GPU execution target currently has this status.
- **Qualified:** passed a named, reproducible lane for an exact revision,
  target, and environment.
- **Experimental:** implemented but incomplete, unstable, or not yet admitted
  as a public workflow.
- **Unavailable:** intentionally rejected or not implemented. fe2o3 should
  report this state instead of inferring success.
- **Authority-free:** content may be valid and useful for observation but does
  not authorize compiler publication, loading, dispatch, or a hardware claim.

## Contributing

Read the repository-level [contribution guide](../CONTRIBUTING.md),
[code of conduct](../CODE_OF_CONDUCT.md), [security policy](../SECURITY.md),
[support policy](../SUPPORT.md), and [governance](../MAINTAINERS.md). New design
documents should link from this index only when they define a maintained public
entry point or a durable subsystem contract.
