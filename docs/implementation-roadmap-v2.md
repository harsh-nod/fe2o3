# fe2o3 Implementation Roadmap v2

Status: execution plan for parallel implementation.

Implementation checkpoint: `ceb0e4675173866a50fb737108e6a9b04827691d`.

This roadmap turns [architecture-v2.md](architecture-v2.md),
[verification-model.md](verification-model.md), the
[GPU safety contract v1](gpu-safety-contract-v1.md), the
[general typed dispatch V1 contract](general-typed-dispatch-v1.md), and the
[cuda-oxide parity matrix](cuda-oxide-parity-matrix.md) into independently
owned work with staged integration gates. Gates are evidence-based; calendar
dates depend on staffing and hardware availability.

## Program Rules

1. Implement vertical slices. A feature includes frontend, IR, lowering,
   diagnostics, tests, and manifest changes where applicable.
2. Keep the existing elementwise emitter runnable until G1 passes all current
   examples through the new path.
3. Put shared schemas and interfaces behind versioned tests before parallel
   agents build against them.
4. Do not mark a parity row complete from source presence or compilation alone.
5. Keep raw APIs explicitly unsafe; safe APIs must derive ABI and launch facts
   from the artifact manifest.
6. Treat Verus proof results and compiler correctness as separate evidence.
7. Merge small changes with one primary owner. Avoid long-lived branches that
   each edit shared registry, dialect, or manifest files.
8. Unsupported semantics fail at compile time with a source span and call
   chain. No approximate lowering is accepted.

## Parallel Workstreams

These workstreams can be staffed by separate agents once their input contracts
are frozen.

| Lane | Owns | Depends on | First deliverable |
|:--|:--|:--|:--|
| A: Frontend | rustc driver, kernel metadata, mono-item/call-graph collection, layout extraction | Kernel metadata schema | One non-generic kernel and helper serialized as typed frontend fixtures |
| B: IR | `mir.*`, `gpu.*`, verifiers, parser/printer, mem2reg, canonicalization | Versioned IR schema | Round-trip and verifier tests for control flow, memory, and GPU ops |
| C: AMD backend | `gpu.*` legalization, AMDGPU LLVM export, OCML/OCKL, HSACO finalization | `gpu.*` contracts, target capability schema | General vecadd from IR fixture to validated HSACO |
| D: Runtime/API | artifacts, ABI, generated modules, prepared launches, buffers, streams, events, async operations | Manifest v1 | Safe typed vecadd launch with retained lifetimes |
| E: Verification | contracts, abstract model, Verus harness, proof policy, proof manifest | Launch/index/memory schema | Verified map kernel with exact proof binding |
| F: Quality | parity status generator, compile tests, differential runner/fuzzer, hardware CI, sanitizer/debug jobs | Stable command/test interfaces | Reproducible baseline dashboard and current-example regression suite |
| G: Advanced AMD | atomics, LDS, waves, matrix/async operations, device linking | G1 compiler and capability interfaces | Target-gated LDS reduction and wave collective suites |

An agent owns a lane's implementation files during a milestone. Shared schema
files have a designated integrator. Other agents propose schema changes as
small patches with fixture updates, then rebase after the integrator merges
them.

## Shared Interfaces to Freeze Early

Parallel work is effective only after these contracts have golden fixtures:

1. Kernel metadata emitted by macros.
2. Frontend function/type/layout serialization.
3. `mir.*` and `gpu.*` textual forms and verifier rules.
4. Target capability registry and versioning policy.
5. Artifact manifest and bundle wire format.
6. Kernel ABI field and layout model.
7. Launch contract and `PreparedLaunch<K>` identity.
8. Proof record, assurance level, and executable semantic identity.
9. Diagnostic codes used by compile-fail tests.

Freeze means backward-compatible evolution through explicit version changes,
not permanent immutability.

## Implemented Checkpoint: `90b6fe3`

The `90b6fe3` checkpoint establishes a bounded `gfx942` multi-kernel spine:

- one external Cargo fixture declares two kernel roots and one reachable shared
  helper;
- MIR import assigns the helper one canonical source identity, and Kernel IR
  lowering validates calls against the collected helper's exact signature;
- AMDGPU lowering is deterministic for two kernels, one shared helper, and
  shared OCML declarations;
- Worker V2 compiles the real Rust fixture into one independently inspected and
  durably published HSACO through the sealed Cargo backend path;
- the V1 artifact wire format carries exactly two canonically ordered kernel
  entries that reference one digest-validated native `gfx942` payload;
- each kernel has an independently keyed proof binding over its own ABI,
  effects, launch contract, source identity, and the shared executable;
- host admission can select two distinct compiler-generated kernel markers from
  the same authenticated executable without allowing marker, target, layout,
  payload, or executable substitution; and
- the reviewed HSA adapter can resolve and linearly retain a fixed set of
  distinct symbols while borrowing the loaded executable.

This is compilation, artifact, proof-binding, selection, and lifecycle
evidence. It is not general typed dispatch. The generated safe launch surface
and reviewed HSA argument initializer still implement only the exact vecadd
profile, and the second host selection is deliberately inert. No parity row is
promoted by this checkpoint.

## Implemented Source/Unit Checkpoint: `ceb0e46`

The post-snapshot implementation through
`ceb0e4675173866a50fb737108e6a9b04827691d` advances G3.1 foundations without
passing its vertical-slice exit gate:

- `#[kernel(typed)]` preserves the exact vecadd V2 expansion and emits an
  expectation-only V3 registration for bounded scalar, shared-slice, and
  `DisjointSlice` signatures outside that compatibility profile;
- rustc validates the V3 registration against semantic primitive types and
  genuine trusted `DisjointSlice<T, Index1D>` identities, then derives variable
  physical layouts and COV6 descriptors. The alpha and zeta fixtures are
  `40/296` and `56/312` explicit/complete kernarg bytes;
- host argument binding validates canonical scalar and slice identities,
  retains allocation borrows in lifetime-branded packed values, and requires a
  backend-issued semantic witness before general V3 authority can exist;
- the witness wire contract, reserved symbols, parser, and fail-closed tests are
  implemented, but the rustc backend witness host-object emitter is not;
- Worker V2 canonically finalizes descriptor-bearing COV6 before publication,
  preserves descriptor-free COV5 compatibility, and recovers exact raw and
  finalized publications across process crashes with legacy-marker migration;
  and
- native worker tests preserve two COV6 entries, both `.kd` symbols, and one
  shared helper. `.fe2o3.kd.v1` authentication and artifact-container
  construction remain downstream.

These are source/unit and native-worker boundary results. There is no generated
alpha/zeta wrapper, production two-kernel container/load/dispatch, alpha/zeta
MI300X execution, or Verus result at this checkpoint. No parity row is Complete.

## G0: Baseline and Safety Boundary

### Objectives

- Turn the parity document into machine-checkable data or a checked generated
  view without losing the pinned 94-row baseline.
- Record current examples, expected artifacts, compiler toolchain, and hardware
  targets as regression fixtures.
- Make low-level launch APIs and arbitrary argument packing explicitly unsafe.
- Define kernel metadata, capability, launch contract, ABI, artifact, and proof
  schema version 1.
- Establish CPU-only CI and named CDNA/RDNA hardware queues.

### Parallel assignments

- Lane A replaces kernel name substring discovery with generated metadata while
  retaining compatibility tests.
- Lane D introduces unsafe raw launch methods and a minimal kernel brand plus
  `PreparedLaunch<K>` skeleton.
- Lane F ports current examples into a manifest-driven smoke list and adds
  compile-fail infrastructure.
- Lane E builds an erased-contract compatibility spike that compiles the same
  tiny kernel under Verus and ordinary rustc without asserting a proof-to-code
  theorem.

### Exit gate

G0 passes when:

- every current fe2o3 example still builds through the old emitter;
- raw launch and raw pointer packing require an explicit unsafe call site;
- a safe prepared vecadd launch rejects wrong rank, wrong kernel brand, wrong
  context, and insufficient resource declarations before HIP launch;
- metadata and manifest v1 have round-trip, malformed-input, and unknown-field
  tests;
- CI verifies that changes outside approved generated directories do not
  rewrite golden schemas;
- the pinned cuda-oxide and fe2o3 commits are shown by the parity status tool.

## G1: General Compiler Spine

### Objectives

- Add the explicit device extraction driver while keeping a compatibility
  adapter in `rustc-codegen-fe2o3`.
- Import Stable MIR through `rustc_public` into typed `mir.*` operations.
- Add IR structural/type verification, memory-form translation, mem2reg, and
  basic canonicalization.
- Lower baseline control flow, calls, arithmetic, comparisons, loads/stores,
  slices, pointer offsets, and thread coordinates into `gpu.*` and AMDGPU LLVM.
- Emit validated HSACO without elementwise shape recognition.

### Required vertical slices

1. Scalar return helper.
2. Branching fill kernel.
3. Vecadd with three slice ABI values.
4. Stencil with multiple indexed loads.
5. Pipeline with two kernels and shared helpers.
6. Cross-block loop fixture that requires correct SSA promotion.

### Parallel assignments

- Lane A owns extraction and serialized frontend fixtures.
- Lane B imports fixtures and owns `mir.*`/`gpu.*` verifiers and passes.
- Lane C lowers checked fixtures to AMDGPU and finalizes HSACO.
- Lane F compares old/new outputs for every current example and runs negative IR
  fixtures against verifiers.

The lanes integrate through serialized fixtures before linking crates together.
This permits backend progress without a live rustc frontend and frontend
progress without AMD hardware.

### Exit gate

G1 passes when:

- every current example builds and executes through the new compiler path on
  one supported AMD machine;
- CPU-only tests cover every accepted `mir.*` and `gpu.*` operation;
- malformed dominance, types, address spaces, barriers, and capabilities fail
  verification;
- `cargo fe2o3 pipeline` shows each IR stage and `cargo fe2o3 inspect` shows the
  selected payload and metadata;
- the old and new paths agree on kernel ABI for current examples;
- the elementwise recognizer is disabled by default but retained temporarily as
  a differential oracle.

## G2: Rust Semantic Coverage

### Objectives

Implement the target-neutral language rows before advanced GPU features:

- rustc layout fidelity for structs, tuples, arrays, enums, variants, and ZSTs;
- constants, statics, promotions, and supported pointer relocations;
- generics, const generics, closures, function items, drop glue, and cross-crate
  calls;
- complete baseline control flow, iterators, matches, loops, break/continue, and
  supported unrolling;
- integer/float operations, checked operations, casts, pointer distance,
  volatile access, and bulk copy;
- device panic-as-trap and unsupported-call diagnostics.

### Test corpus

Port tests by semantic category from the pinned cuda-oxide checkout. Do not copy
CUDA-specific expected IR. Each test has one or more of:

- a frontend/IR golden fixture;
- a compile-pass or compile-fail assertion;
- a CPU reference and AMD execution comparison;
- layout bytes independently produced by host rustc;
- an LLVM/HSACO metadata assertion.

Priority order is control flow and calls, aggregates/layout, constants, then
closures and uncommon pointer cases. This order unblocks runtime ABI work while
the long tail continues.

### Exit gate

G2 passes when:

- Exact compiler rows 02-25 and 31-35 meet their pinned acceptance targets;
- a generic closure map, enum match, nested loop, padded struct array, const
  relocation, and cross-crate kernel execute correctly;
- all supported host/device layouts compare equal by size, alignment, field
  offsets, discriminant, and parameter bytes;
- unsupported `std`, allocation, unwind, dynamic dispatch, and relocation cases
  produce stable diagnostics with reachable call chains;
- no G2 test depends on the old recognizer.

## G3: Artifact, ABI, and Runtime Contract

### Objectives

- Make a versioned target-neutral artifact bundle the only source of entry,
  payload, ABI, launch, capability, and proof metadata.
- Generate typed module loaders and launch methods from kernel declarations and
  validate them against the finalized manifest.
- Implement structurally host-valid `DeviceCopy`, manifest-derived type/ABI,
  provenance, address-space, and capability gates, layout-safe buffers, pinned
  memory, events, and context ownership.
- Add lazy typed async operations with borrowed and owned forms.
- Retain resources through completion, cancellation, callback failure, and
  stream error paths.
- Add deterministic cache keys and local clean behavior.

### G3.1: General typed multi-kernel dispatch

This remains the next critical vertical slice. It replaces the exact vecadd-only
packing and dispatch bridge with one path generated from each admitted kernel
entry. Its normative scope and authority transitions are frozen in the
[general typed dispatch V1 contract](general-typed-dispatch-v1.md): by-value
scalars, shared slices, and exclusive `DisjointSlice` arguments already
represented by the bounded ABI model. Aggregates, return values, asynchronous
launch, and language coverage not yet accepted by G2 are not silently added
here.

Parallel ownership is split at frozen records:

| Slice | Owns | Produces |
|:--|:--|:--|
| G3.1-A: compiler ABI | rustc layout extraction, physical parameter expansion, effect/alias declarations | Canonical per-entry ABI descriptor fixtures |
| G3.1-B: artifact/module | multi-entry bundle validation, descriptor-to-payload binding, generated module declarations | One module descriptor with two independently typed kernel entries |
| G3.1-C: host packing | generated argument views, checked offset/alignment writes, prepared geometry, retained borrows | Kernel-specific packed arguments that cannot be exchanged |
| G3.1-D: HSA dispatch | multi-symbol resolution, reviewed COV6 hidden arguments, queue submission, completion, unload ordering | Generic synchronous dispatch for an admitted kernel descriptor |
| G3.1-E: adversarial tests | UI tests, mutation tests, CPU oracles, MI300X execution evidence | Reproducible positive and fail-closed evidence |

Progress at `ceb0e46` is deliberately partial. G3.1-A has V3 registration,
rustc-semantic reconstruction, and alpha/zeta descriptor fixtures. G3.1-C has
typed scalar/slice binding, retained packing lifetimes, and the semantic-witness
consumer contract. Worker V2 contributes canonical COV6 publication and restart
recovery to G3.1-B, while native worker tests cover its COV6 link boundary.
Still missing are the backend witness emitter, production two-entry container,
generated per-kernel wrappers, checked subranges and equal-length admission,
one-executable alpha/zeta dispatch, and the G3.1-E MI300X evidence.

The compiler ABI descriptor is the integration boundary. Runtime code may
compare an untrusted manifest with a compiler-generated descriptor, but it may
not synthesize a safe Rust argument interface from manifest bytes alone. Each
packed argument value is branded by kernel, executable, context, and descriptor
identity and retains all referenced resources until quiescence.

G3.1 passes only when all of the following are true:

1. One ordinary Cargo project declares two kernels with different nontrivial
   signatures and one shared Rust helper; the sealed backend emits one `gfx942`
   HSACO containing exactly both entries and one helper definition.
2. The backend emits canonical ordered physical fields, offsets, sizes,
   alignments, address spaces, mutability/effects, launch contract, target, and
   code-object identity for each entry. Repeated clean builds are byte-identical.
3. One V1 bundle references the shared payload from both entries, and
   independent HSACO inspection matches each entry to exactly one descriptor.
4. Generated host declarations expose distinct safe argument and prepared
   launch types for both kernels. No kernel name, signature, offset, or byte
   count is special-cased in `fe2o3-host` or `fe2o3-hsa-runtime`.
5. Safe packing writes every explicit kernarg field from its manifest-derived
   descriptor, preserves resource borrows and alias classes, initializes
   padding deterministically, and rejects arithmetic overflow.
6. One loaded HSA executable resolves both symbols. Each typed selection can be
   prepared and synchronously dispatched through the same generic path, and
   the executable cannot unload while either selection, packed arguments,
   launch authorization, or submitted dispatch remains live.
7. An MI300X runs both kernels from that one executable and compares all output
   bytes with independent CPU oracles for empty/rejected, single-element,
   boundary, and multi-workgroup lengths. The evidence records `gfx942`, ROCm,
   LLVM worker, rustc, and commit identities.
8. Negative tests reject swapped argument order or type, changed physical
   layout, wrong symbol or kernel marker, target/context/executable
   substitution, stale payload, changed effects or launch contract, duplicate
   HSA symbols or kernel objects, cross-kernel proof substitution, alias
   violations, and unload-before-quiescence.
9. CPU-only unit tests, compile-fail tests, package tests, strict Clippy, the
   ignored Worker V2 integration test, and the MI300X execution test all pass
   from the same commit with commands recorded in [testing.md](testing.md).
10. The exact vecadd public API either uses the new descriptor-driven path or
    remains explicitly marked as a compatibility profile. Async operations,
    cross-crate finalization, and broader G2 aggregate support remain separate
    gates and are not implied by G3.1.

### Safety tests

Compile-fail tests must cover:

- constructing or mutating a kernel brand;
- wrong launch rank, block shape, context, or kernel identity;
- passing the wrong argument type/order;
- mutable aliasing between arguments;
- freeing, moving, or mutating a borrowed buffer while work may execute;
- dropping a submitted future before completion;
- using a stale proof or artifact record;
- treating arbitrary bytes as a valid bundle.

Fault-injection tests cover allocation, launch, event, callback, and
synchronization failures. The failure policy prefers a bounded leak over
freeing storage still reachable by the GPU.

### Exit gate

G3 passes when:

- all safe launches use manifest-derived typed arguments and prepared geometry;
- raw sync and async launches require `unsafe` and list complete obligations;
- multi-kernel and cross-crate artifacts are embedded and found without
  filename sidecar conventions;
- async borrowed and owned pipelines pass Miri-compatible host logic tests and
  AMD execution tests;
- cancellation/failure tests demonstrate that in-flight buffers are not freed;
- artifact parsing is fuzzed and version/unknown-capability rejection is
  covered;
- Exact runtime rows 48-51 and 78-81 plus S01-S05 meet their targets, excluding
  Verus-specific proof claims reserved for G5.

## G4: Core AMD GPU Model

### Objectives

- Complete 3D workitem/workgroup/grid operations.
- Implement static/dynamic LDS, workgroup barriers, scopes, and fences.
- Implement integer and supported floating atomics for workgroup, device, and
  system scopes.
- Implement wave32/wave64 lane, vote, shuffle, match, reduction, and scan
  operations.
- Implement workgroup reductions/scans independent of wave width.
- Add OCML/OCKL math, half/BF16 types, debug print/assert/trap, and launch
  bounds.
- Run divergence, effect, address-space, and barrier validation before AMD
  lowering.

### Target policy

Portable subgroup tests run for every supported wave width. A target-specific
test names its required architecture/capabilities and is skipped only with a
machine-readable reason. A successful fallback must satisfy the same semantics;
otherwise compilation fails.

### Exit gate

G4 passes when:

- map, 2D stencil, tiled transpose, workgroup reduction, wave reduction, atomic
  histogram, and math suites pass on the required target matrix;
- LDS allocation/alignment and launch metadata are inspected in the code
  object;
- atomics pass ordering/scope litmus tests and system atomics reject ineligible
  allocations;
- no portable test assumes a wave width of 32;
- rows 40, 53-58, 60-73, 75-77, and 82 meet their non-Verus acceptance targets.

## G5: Verus V1 and Safe Data Parallelism

G5 runs in parallel with G2-G4 after G0 schemas stabilize. It does not wait for
advanced GPU operations.

### Objectives

- Formalize launch domains, allocation provenance, index spaces, views, and
  per-thread effects in Verus.
- Implement branded `ThreadIndex`, `DisjointSlice`, and proof-carrying static
  views.
- Prove bounds, address overflow freedom, initialization, injective writes,
  race freedom, and functional postconditions for independent-thread kernels.
- Emit proof manifests and bind them to executable semantic identity and launch
  contracts.
- Add `cargo fe2o3 verify` and `build --require-proof`.

### Required verified kernels

- fill and copy;
- vecadd/map/zip;
- affine gather;
- out-of-place stencil with halo guards;
- injective transpose or permutation;
- generic pure helper composition.

Each kernel needs a negative mutation that Verus rejects, such as an omitted
bounds guard, zero-stride output, aliasing contract violation, or incorrect
postcondition.

### Exit gate

G5 passes when:

- the required kernels have complete proof records under the approved axiom
  policy;
- stale source, dependency, feature, contract, model, and tool-version records
  are rejected;
- `--require-proof` never silently downgrades to `Checked`;
- runtime output clearly states the source-level trust assumption;
- compile-fail tests prevent index witness transfer, copying, scope escape, and
  index-space mismatch;
- rows 48-51 and 79 meet their Verus acceptance targets.

## G6: Interop and Advanced AMD Capabilities

### Objectives

- Add AMDGPU bitcode/relocatable device linking through a pinned worker that
  calls LLVM and LLD library APIs directly, plus bidirectional device FFI.
- Keep the LLVM worker out of rustc's process, use one exact LLVM build for
  parse/link/optimize/codegen/native link, and do not use COMGR.
- Support standalone device exports and external libraries through reviewed ABI
  and effect contracts.
- Add cooperative grid launch where HIP and hardware support it.
- Add target-gated split barriers and asynchronous global-to-LDS operations
  using AMD semantics.
- Add MFMA/WMMA, FP8, supported microscaling types, LDS swizzles, and matrix
  load/store helpers.
- Add VMM, peer access, coherent shared-memory capabilities, and multi-device
  runtime tests.
- Implement AMD inline assembly boundaries and source-level debug metadata.

CUDA cluster DSMEM, cluster launch, and TMA remain N/A unless a future AMD
target provides a semantic equivalent. Native AMD extensions get separate
matrix entries rather than being mislabeled TMA.

### Exit gate

G6 passes when:

- Rust calls one external AMDGPU device function and external device code calls
  one exported Rust function;
- a cooperative kernel validates launch capability and executes a grid-wide
  synchronization test on supported hardware;
- representative MFMA/WMMA and async-copy pipelines pass numerical, ISA, and
  resource tests;
- unsupported target combinations fail during capability legalization;
- AMD-equivalent rows 01, 26-30, 39, 47, 62, 64, 74, 87-88 and supplemental
  S02, S06, S12-S13 meet their declared scope;
- N/A rows 59, 63, 83-86 still reject rather than approximate CUDA semantics.

## G7: Verus V2-V4

### Objectives

- Add workgroup synchronization epochs and LDS initialization transfer.
- Prove barrier convergence and compatible dynamic barrier order.
- Add atomic invariants, scope reasoning, and linearization points.
- Add subgroup active-lane and wave-width-parametric proofs.
- Verify async-copy/barrier protocols and host operation dependencies where a
  reviewed primitive model exists.
- Distinguish source proofs from trusted contracts for external libraries,
  inline assembly, and matrix instructions.

### Required verified kernels

- tiled transpose using LDS;
- tree reduction with workgroup barriers;
- one scoped atomic counter or histogram;
- one wave collective parametric over supported width;
- one asynchronous copy pipeline, only if its primitive model is approved.

### Exit gate

G7 passes when:

- each required kernel proves safety and its stated functional invariant;
- divergent or misordered barrier mutations are rejected;
- wrong atomic scope/order mutations are rejected or require explicit unsafe
  assumptions;
- proof manifests list every trusted intrinsic/library contract;
- rows 52-58, 61-74 and advanced supplemental rows report honest per-property
  verification status.

## G8: Hardening and Parity Release

### Objectives

- Run differential fuzzing across MIR import, optimization, lowering, and AMD
  execution.
- Integrate available GPU memory, initialization, race, and synchronization
  checking tools through `cargo fe2o3 sanitize`.
- Integrate ROCgdb source debugging and local/argument inspection.
- Validate release behavior on supported RDNA and CDNA families.
- Compare representative kernels with equivalent HIP C++ and relevant ROCm
  libraries for correctness, generated code, and performance.
- Generate parity and verification dashboards only from archived test evidence.

### Release evidence

Archive:

- fe2o3 and pinned baseline commits;
- rustc, Verus, solver, LLVM, ROCm, driver, firmware, and hardware identities;
- matrix row status with links to tests and logs;
- sanitizer/debugger results and known tool gaps;
- correctness and performance results with commands and datasets;
- approved N/A reviews and parity exceptions;
- trusted axiom, FFI, inline assembly, and external library lists.

### Exit gate

G8 passes when the parity release rule in
[cuda-oxide-parity-matrix.md](cuda-oxide-parity-matrix.md) is satisfied, the
current recognizer has no remaining default users, and removal of migration
code does not reduce archived coverage.

## Dependency Graph

```text
G0 contracts and safety
 |\
 | +-----------------------> G5 Verus V1 --------+
 v                                               |
G1 compiler spine --> G2 Rust semantics --> G3 runtime/artifacts
        |                    |                    |
        +--------------------+------> G4 core AMD+
                                             |   |
                                             v   v
                                            G6  G7
                                             \   /
                                              G8
```

G2 and G3 overlap after layout and ABI schemas stabilize. G4 begins from IR
fixtures before all G2 language features are complete. G5 begins from contract
fixtures after G0 and integrates concrete compiler identities as G1-G3 land.

## Integration Cadence

For each milestone:

1. The integrator publishes schema fixtures and affected matrix IDs.
2. Lane agents implement against fixtures in disjoint ownership areas.
3. Each lane lands unit tests before cross-lane wiring.
4. A vertical integration patch wires frontend to IR to backend/runtime.
5. Lane F adds hardware and negative tests and updates machine-readable status.
6. The gate owner records evidence; documentation status changes only from that
   evidence.

Changes to shared schemas require a version bump or a backward-compatible
reader, all fixture updates, and approval from every consuming lane owner.

## Pull Request Contract

Every implementation pull request states:

- owned parity row IDs and gate;
- layer boundaries changed;
- new or changed unsafe/trusted obligations;
- test evidence by CPU-only, compile-fail, hardware, sanitizer, and proof
  category;
- manifest or IR version impact;
- target capability and N/A behavior;
- migration-path impact.

A pull request should normally complete one narrow vertical behavior. Bulk
mechanical generated intrinsic updates are isolated from handwritten semantic
changes.

## Completion Criteria

The architecture program is complete when:

- all parity release requirements pass;
- fe2o3 host compilation no longer depends on a custom codegen backend;
- the general IR pipeline owns all supported kernels;
- ABI and launch APIs derive from versioned bundles;
- assurance levels are mechanically bound and honestly reported;
- V1 verified data-parallel kernels are stable, with V2-V4 status explicit;
- every remaining unsafe and trusted boundary is documented and tested;
- AMD-specific capabilities are named and gated instead of presented as CUDA
  features or universal GPU behavior.
