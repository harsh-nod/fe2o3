# fe2o3 Verification Model

Status: living source-level verification contract. Bounded Verus models and
proof-record schemas implement portions of this contract; no general reviewed
source-to-machine or Verus-to-machine refinement exists.

This document defines what fe2o3 may call verified, what Verus is expected to
prove, and what remains trusted. It is deliberately independent of AMDGPU
instruction selection so the same contracts can apply to future device
targets.

## Verification Claim

The initial fe2o3 claim is:

> A verified kernel satisfies its stated functional, memory-safety,
> initialization, race-freedom, and launch properties in the fe2o3 abstract GPU
> model, assuming correct proof erasure, compilation, runtime execution, and
> hardware behavior.

Verus verifies source-level executable Rust and specifications. Verus does not
verify rustc, the MIR importer, Pliron passes, AMDGPU lowering, LLVM, ROCm, HIP,
the driver, or the GPU. Documentation, diagnostics, manifests, and command
output must not shorten the claim to "the compiled GPU binary is formally
verified."

## Assurance Levels

Every kernel artifact and launch reports one level:

| Level | Meaning | Required evidence |
|:--|:--|:--|
| `Verified` | Declared source-level properties were proved under this model | Accepted proof record bound to the exact executable semantic identity |
| `Checked` | Safe API and compiler analyses passed, but there is no complete Verus proof | Valid IR, ABI, capability, launch, and static-analysis records |
| `Unsafe` | One or more obligations are delegated to the caller | Explicit unsafe API plus machine-readable and documented obligations |

The levels are not optimization grades. A kernel cannot become `Verified`
because tests or a sanitizer passed, and cannot remain `Verified` after a proof
identity mismatch.

## Current Production Evidence Boundary

The production compiler retains one exact aggregate Verus execution for the
safe-reference MIR-to-live-PLIRON boundary. A canonical V4 association, carried
inside the frozen V3 capsule envelope, binds the exact semantic MIR,
middle-end V5, verified Kernel IR, MIR-to-KIR correspondence, formal-memory
evidence, signed receipt, embedded verifying key, proof binding, toolchain, and
execution identities. Worker V3 independently decodes those preimages,
reimports the signature, and checks that the signed PLIRON identity is the
identity of the carried middle-end evidence.

This has three deliberately separate meanings:

1. The embedded key and signature establish internal receipt consistency; they
   do not by themselves establish that a trusted production compiler emitted
   the receipt.
2. Signed compiler-currentness evidence supplies a separate protected-origin
   join. The move-only compiler and proof owners are retained together through
   the host HSA lifecycle but still grant no load or launch authority alone.
3. The receipt covers safe-reference MIR to live PLIRON formulas. It does not
   prove rustc/MIR extraction soundness, PLIRON-to-LLVM lowering, LLVM/ISA or
   final-machine refinement, dynamic launch preconditions, runtime behavior, or
   hardware correctness.

`fe2o3-host` exposes the exact canonical V3 verification and signed attestation
retained from the inherited FD195 endpoint only through a lifetime-bound,
non-clone view of an already admitted audit or compiler-execution lane. Both
typed identities and the expected verification challenge are derived from
those same retained records. An external protected verifier can consume a
typed caller-owned challenge in the one-use FD195 audit, retain the original
expected bytes under its own replay policy, and compare them with the view. The
view lets it decode and authenticate the full records instead of trusting an
identity echo, but it neither transfers the host's one-use
currentness custody nor supplies signing keys, protected deployment trust,
replay exclusion, verification authority, load authority, or launch authority.

Until a crate-owned production verifier consumes those owners together with
the missing machine and launch evidence, ordinary generated applications fail
closed. Synthetic integration tests demonstrate composition and hostile
rejection only and never upgrade this assurance boundary.

## Abstract Execution Model

### Launch domain

A launch defines a finite set of participating threads:

```text
Thread = (grid_x, grid_y, grid_z, local_x, local_y, local_z)
```

The launch contract defines active dimensions, grid and workgroup limits,
exact or bounded workgroup shape, dynamic workgroup memory, cooperative launch
requirements, and target capabilities. Linear or tiled index spaces are pure
functions from a thread coordinate and contract parameters to logical indices.

An index witness is valid only for the launch and index space that created it.
It cannot be copied, transferred to another thread, stored for a later launch,
or reinterpreted as a different layout.

### Memory

An abstract location is:

```text
Location = (allocation_id, byte_offset, byte_width, address_space)
```

An allocation records its extent, alignment, element layout, mutability,
provenance, lifetime, and visibility scope. Pointers and views retain the
allocation identity; integer-to-pointer conversion loses safe provenance and
therefore requires an explicit unsafe contract.

The model includes these address spaces:

- global device memory;
- host/device shared or managed memory;
- constant/read-only device memory;
- workgroup memory (AMD LDS);
- thread-private memory.

Target backends may have more spaces, but they must refine one of these or add a
versioned model extension.

### Effects

For each dynamic thread `t` and synchronization epoch `e`, a kernel has read,
write, and atomic effect sets:

```text
R(t, e) = non-atomic locations read
W(t, e) = non-atomic locations written
A(t, e) = atomic actions with operation, ordering, and scope
```

Effects include the byte range and allocation provenance, not just an element
index. This catches partial overlap between differently typed views.

### Synchronization epochs

A workgroup barrier closes one epoch and opens the next for all participating
threads. The barrier establishes only the visibility and ordering promised by
its scope and memory semantics. A wave barrier does not order unrelated waves;
a workgroup barrier does not order separate workgroups.

The verifier must prove that every required participant reaches barriers in a
compatible order. A barrier under divergent control is rejected unless the
model and primitive explicitly support that participation mask.

### Atomics

An atomic action records:

- target location and value type;
- read/modify/write operation;
- success and failure ordering where applicable;
- synchronization scope;
- abstract state transition and linearization point.

Overlapping atomic accesses are allowed when scope and ordering cover every
participant. Atomic and non-atomic conflicting accesses still require an
ordering proof.

## Core Proof Obligations

### Memory access

Every safe load or store proves:

```text
allocation is live
pointer has valid provenance for the allocation
address is aligned for the operation
0 <= byte_offset
byte_offset + access_width <= allocation_size
address arithmetic does not overflow
required initialization has occurred
```

Safe slice indexing normally discharges these obligations through bounds
checks. Proof-carrying views may discharge one check for an entire static tile.
Unchecked indexing and raw pointers expose the missing obligation as `unsafe`.

### Race freedom

For distinct concurrently executing threads `t` and `u` in the same unordered
epoch, non-atomic accesses require:

```text
W(t, e) intersection W(u, e) = empty
W(t, e) intersection R(u, e) = empty
R(t, e) intersection W(u, e) = empty
```

The rule is applied to byte ranges and respects synchronization scope. Read/read
overlap is allowed. Conflicts proved ordered by valid barriers, stream/event
dependencies, or atomics are checked under the corresponding rule instead.

Common kernels have concise proofs:

- elementwise map: output indexing is injective;
- gather: output writes are injective and all input indices are in bounds;
- stencil: input is read-only, output writes are injective, halo accesses are
  guarded;
- transpose: the output permutation is injective;
- reduction: each epoch has disjoint workgroup-memory ownership, then a barrier,
  then a smaller active set or a scoped atomic update.

Data-dependent scatter is not automatically race-free. It needs an injectivity
lemma, an ownership partition, or atomics.

### Initialization

A read must observe initialized storage. Global arguments carry initialization
state from the host type. Thread-private values follow Rust initialization
rules. Workgroup memory starts uninitialized unless a target contract says
otherwise and is tracked per byte or logical element across epochs.

### Barriers and convergence

For every barrier instance, prove:

1. the declared participant set is correct;
2. all required participants reach the same dynamic barrier instance;
3. participants reach barrier instances in the same order;
4. no participant exits an epoch while another waits for it;
5. the barrier scope and memory semantics cover the intended communication;
6. workgroup memory read in the next epoch was initialized in an ordered prior
   epoch.

Grid-wide barriers additionally require a cooperative launch contract accepted
by the runtime and target.

### Launch and host lifetime

A safe launch proves or dynamically checks:

- kernel identity and payload identity match;
- argument types and layouts match the manifest;
- all buffers belong to the selected context or an allowed peer/managed domain;
- immutable and mutable aliases satisfy the kernel effect contract;
- launch rank, dimensions, workgroup shape, and resource sizes satisfy the
  kernel contract;
- target capabilities are available;
- borrowed or owned allocations remain live until completion;
- cross-stream conflicting effects are ordered by events or dependencies.

The host borrow does not end when work is enqueued. It ends only after a
completion event, synchronization, or returned ownership proves that the GPU
can no longer access the allocation.

## Proof-oriented Device Types

### `ThreadIndex`

The target type is conceptually:

```rust,ignore
ThreadIndex<'kernel, IndexSpace>
```

It is opaque and must be non-`Copy`, non-`Clone`, non-`Send`, and non-`Sync`.
Its executable value is a hardware-derived integer. Its ghost view records the
thread, launch brand, logical index, and index-space relation.

Constructors prove dimensional assumptions from a `LaunchContext<K>` generated
by a prepared launch. An unsafe raw launch may create a context only by assuming
the documented geometry obligations.

### `DisjointSlice`

The target type is conceptually:

```rust,ignore
DisjointSlice<'a, T, IndexSpace>
```

Its executable representation is pointer plus length. Its ghost view includes
allocation identity, element layout, logical contents, writable region, and
the mapping accepted from `IndexSpace`.

Safe mutable access requires a matching `ThreadIndex`. The central theorem is:

```text
valid launch context
+ unique valid thread witness
+ matching DisjointSlice index space
+ successful bounds check
=> a live in-bounds element unique to this thread in this epoch
```

More general views express tiles, strided regions, and multidimensional
layouts. They must prove disjointness at construction or carry an unsafe
obligation.

### Barriers

Managed barriers use typestate for executable lifecycle and ghost state for
participant count and epoch transition. Typestate alone prevents invalid API
sequences; Verus proves cross-thread participation and memory facts.

## Verus Integration

### Source structure

Kernel contracts use a small `fe2o3-contracts` vocabulary that has two views:

- Verus sees specifications, ghost views, invariants, and proof functions.
- ordinary rustc sees the executable API after proof erasure.

The exact attribute syntax is an API design task. The architectural requirement
is that there is one executable body and that erasure is deterministic and
testable. A macro may generate an entry shim, launch marker, and proof harness,
but may not generate a second algorithmic kernel body.

### Verification units

The verifier operates on concrete kernel instances plus reusable generic
lemmas. A proof unit includes:

- kernel definition and reachable executable functions;
- monomorphized types and const arguments;
- launch contract;
- memory and capability contracts;
- imported specifications and approved axioms;
- verification model version.

### Proof policy

Release `Verified` artifacts must reject unchecked proof escapes by default.
Any use of `assume`, admitted lemmas, external bodies, or trusted specifications
must appear in an explicit, reviewed allowlist recorded in the proof manifest.
The empty allowlist is required for the initial Tier 1 kernel suite except for
the fe2o3 model axioms named by version.

Solver timeouts, incomplete verification, or missing proof records fail a
`--require-proof` build. They may produce `Checked` output only when the build
policy explicitly permits downgrading.

## Proof Manifest and Binding

`cargo fe2o3 verify` emits a machine-readable proof record containing:

```text
format and verification-model versions
kernel ID and concrete type/const arguments
source tree and erased executable semantic hashes
launch, memory, effect, and capability contract hashes
crate features and relevant cfg values
Verus and solver versions
result and proved property set
approved axiom/trusted-item list
reproducible invocation metadata
```

Artifact finalization accepts the record only if it matches the compiler's
independently computed executable semantic identity and contract hashes. The
bundle embeds the record or its cryptographic digest. The runtime reports the
assurance level but does not re-run the proof.

Changing comments alone should not invalidate an executable semantic hash;
changing executable code, reachable functions, layout, configuration,
contracts, model version, or approved axioms must invalidate it. Until a
semantic hasher is implemented and audited, use a conservative normalized
source plus dependency hash.

## Trust Boundary

The initial trusted computing base includes:

- Verus, its translation, SMT solver, and fe2o3 model axioms;
- proof erasure and the executable/proof identity binder;
- rustc and the pinned nightly interfaces;
- the fe2o3 frontend, IR verifiers, transformations, and lowerings;
- LLVM and ROCm code-object tools;
- HIP/HSA runtime, driver, firmware, and GPU hardware;
- contracts for unsafe Rust, FFI, inline assembly, and external device
  libraries;
- host code that uses raw or unchecked APIs.

Verus reduces uncertainty in the source program. It does not remove any item
above merely because the build produced a proof record.

Later translation validation can reduce trust in compiler passes. Examples
include checking ABI reconstruction, comparing `mir.*` effects with `gpu.*`
effects, validating control-flow simulation, and proving selected optimized
kernels equivalent to a reference IR. These validators are separate roadmap
features and are not part of the initial `Verified` meaning.

## Static Analysis and Runtime Defense

Verification is complemented by, not substituted for, these layers:

- IR structural, type, dominance, address-space, and capability verification;
- divergence and barrier analyses;
- static effect summaries and alias checks;
- compile-fail tests for unsafe boundary violations;
- GPU AddressSanitizer where supported by the ROCm toolchain;
- ROCgdb smoke tests and debug metadata validation;
- differential CPU/GPU execution tests;
- randomized and fuzz-generated kernels;
- runtime argument, context, geometry, and resource checks.

Sanitizers find executions with bad behavior; Verus proves specified behavior
for all modeled executions. Both are required because unsafe code, compiler
defects, FFI, and model gaps remain possible.

## Staged Verification Scope

### V1: independent threads

Supported proof patterns:

- elementwise maps and zips;
- fills and copies;
- bounds-checked gather;
- out-of-place stencil;
- injective transpose and affine permutations;
- pure helper functions and generic arithmetic.

Required properties: bounds, overflow, provenance, initialization, injective
writes, functional postconditions, and prepared launch validity.

### V2: workgroup epochs

Adds static workgroup memory, uniform barriers, tiled algorithms, block
reductions, and scans. Requires epoch effects, initialization transfer, and
barrier convergence proofs.

### V3: atomics and subgroups

Adds scoped atomics, atomic invariants, linearization points, wave collectives,
active-lane contracts, and width-polymorphic subgroup reasoning.

### V4: asynchronous and advanced hardware

Adds asynchronous copies, managed barriers, matrix operations, cooperative
grid synchronization, and host operation graphs. Each target-specific
primitive needs a reviewed semantic contract before it can participate in a
`Verified` artifact.

## Explicit Limitations

- Arbitrary pointer-heavy kernels will require unsafe contracts and may remain
  `Checked` or `Unsafe`.
- Race freedom for data-dependent scatter is not generally decidable and may
  require substantial user lemmas.
- Floating-point proofs follow the selected model and contraction policy; real
  arithmetic is not silently substituted for IEEE floating point.
- External libraries and inline assembly are trusted according to declared
  contracts unless separately validated.
- A proof for one wave width, target capability set, or feature configuration
  cannot be reused for another unless the theorem and manifest are explicitly
  parametric over that difference.
- A hardware fault or compiler miscompilation can violate a source-level proof.

## Acceptance Criteria for `Verified`

A release artifact may carry `Verified` only when:

1. all declared properties are proved with the configured proof policy;
2. the proof manifest and executable semantic identity match;
3. no unrecorded trusted escape is present;
4. IR and artifact validation pass;
5. safe launch construction succeeds from the same manifest;
6. required dynamic checks are retained or proved redundant;
7. the reported claim includes the compiler/runtime trust assumption;
8. the kernel has at least one differential hardware test for every supported
   target family, unless the artifact is proof-only and cannot be launched in
   that environment.
