# Pliron Wave 0 Architecture and Proof Boundary

Status: normative architecture decision for issue
[#134](https://github.com/harsh-nod/fe2o3/issues/134) Wave 0, updated through
the bounded scalar closure at `fd6520d88`, `70f9c5ad7`, `e016833d3`,
`c9e8ca702`, `62efd243e`, and `228c88ed9`. Issue #134 remains open. The current
repository implements bounded contracts, representation shells, and one exact
backend-fixture-to-MI300X scalar route described below. The repository does not
implement the complete Pliron pipeline, general Rust-source lowering, proof
coverage, or performance qualification.

This decision refines, and does not replace, the existing
[architecture](architecture-v2.md), [verification model](verification-model.md),
[GPU safety contract](gpu-safety-contract-v1.md), and
[general typed dispatch contract](general-typed-dispatch-v1.md). Existing
artifact formats, Kernel IR V1-V5 encodings, proof records, publication rules,
and runtime authority remain unchanged until a separately reviewed stage
explicitly versions and migrates them.

The words MUST, MUST NOT, SHOULD, and MAY are normative in this document.

## Decision

fe2o3 will use one Rust-first production compiler pipeline with Pliron as its
in-memory transformation framework. Rust remains the source language and user
API. Pliron operations, arena handles, textual syntax, parser order, and
printer output are not public source concepts, durable identities, or artifact
authority.

The Wave 0 dependency baseline is the single Pliron workspace release
`v0.17.0`, commit `2610651306ea3ba670f68d5d8b1e1159bcd521ed`. Every
implementation dependency on `pliron` or `pliron-derive` MUST resolve to that
same source revision through one centralized workspace configuration. The
workspace now pins `pliron-llvm` at that revision with
`default-features = false`; the optional `llvm-sys` converter is excluded from
all production components, including the isolated worker. Selective use is
confined to the reviewed LLVM dialect/lowering layer. Any such dependency may
land only after the D0 license, advisory,
build-closure, duplicate-package, and LLVM-integration review. Updating the
revision is an architecture migration with golden identity and pipeline tests,
not a routine semver update.

The permanent decisions are:

1. A `#[kernel]` item has exactly one source-level executable algorithm body.
2. rustc performs ordinary Rust checking and concrete monomorphization before
   fe2o3 imports behavior.
3. The default device path is the verified Pliron ladder defined below. There
   is no second production lowering route from Rust to LLVM.
4. fe2o3-owned canonical records, not Pliron objects or text, define durable
   identities, cache keys, finalizer handoff, and evidence.
5. Proof is a non-executable, separately serialized overlay. Proof evidence
   grants authority only for its exact property statement and covered boundary.
6. Unknown, unsupported, unauthenticated, unmodeled, or over-budget input fails
   closed and transactionally.
7. Existing artifact publication and launch authority cannot be broadened by
   Wave 0 scaffolding, shadow output, a proof result, or a descriptive manifest.

## Scope

Wave 0 freezes the source, compiler, identity, proof, and authority boundaries
needed for D0-D11. It deliberately does not:

- add a user-facing kernel language or Pliron builder API;
- translate a kernel body into macro-generated IR-builder calls;
- select schedules by recognizing arbitrary optimized Rust loops;
- claim that Verus proves rustc, Pliron, LLVM, LLD, ROCm, the runtime, driver,
  firmware, or hardware;
- modify current artifact authority or make Pliron the default compiler;
- alter the byte encoding or meaning of canonical Kernel IR V1-V5;
- qualify a kernel for performance or remove a legacy path.

Syntax shown below is illustrative. Wire records, operation schemas, and Rust
API syntax become stable only through their owning versioned stage.

## Current Implementation Boundary

The following infrastructure is implemented:

- `fe2o3-mir-model` owns the canonical, Pliron-independent MIR semantic model,
  executable schema, wire validation, control-flow analysis, and mem2reg
  implementation that previously lived behind `dialect-mir`.
- `fe2o3-compiler-api` defines bounded target-neutral requests, snapshots,
  receipts, diagnostics, outputs, and the `Legacy`, `PlironShadow`, and
  `PlironV1` selectors. `fe2o3-compiler-driver` routes exactly one configured
  backend and revalidates its output. `fe2o3-legacy-compiler` is a dormant
  adapter contract only; it neither contains the existing compiler nor appears
  in production selection.
- `fe2o3-proof-contracts` defines solver-neutral property, status, obligation,
  TCB, and correspondence records. Structural validation does not authenticate
  evidence, run a solver, or promote proof authority.
- `fe2o3-pliron` constructs a real bounded Pliron context with a private,
  process-local identity anchor and validates dialect registration and pass
  plans using v0.17.0 commit
  `2610651306ea3ba670f68d5d8b1e1159bcd521ed`. It does not expose generic pass
  execution because upstream `Ptr<T>` values carry no owner provenance.
  The workspace policy rejects another Pliron revision, duplicate Pliron
  package identities, unexpected packages from that source, `llvm-sys`, and
  COMGR. The exact dialect-only `pliron-llvm` package is now required.
- Seven target-neutral Pliron shells implement bounded `kernel.*`,
  `schedule.*`, `tile.*`, `gpu.*`, `proof.*`, `dispatch.*`, and `autotune.*`
  types, attributes, operations, interfaces, registration, and verification.
  `dialect-mir` additionally exposes a bounded Pliron `mir.*`
  module/function/block representation only under its non-default `pliron`
  feature. Without that feature it remains the compatibility facade that
  re-exports `fe2o3-mir-model`.
- `fe2o3-kir-pliron-bridge` keeps canonical KIR V1-V5 bytes as the only durable
  record, wraps each redundant Pliron projection in an opaque context-bound
  envelope, and requires exact expected-record agreement before recovery. This
  is bounded D2 envelope coverage, not completion of the full D2 semantic
  bridge gate.
- `fe2o3-lower-mir-kernel` accepts a deliberately narrow verified `mir.*`
  subset and emits bounded `kernel.*` roots. `fe2o3-lower-kernel-gpu` converts
  bounded kernel roots into target-neutral `gpu.*` operations. Both are
  detached services rather than in-tree Pliron passes; their results are bound
  to the owning context and erased handles return typed errors. Both expose
  terminal unsupported errors and no fallback, target, or artifact authority.
- The existing strict AMDGPU vocabulary and lowering moved into
  `fe2o3-amdgcn-model`. `dialect-amdgcn` is now a compatibility re-export, not
  an implemented `amdgcn.*` Pliron dialect. Canonical target contracts remain
  separate in `fe2o3-amd-target`.
- The closed scalar slice structurally parses its embedded backend fixture,
  constructs real `pliron-llvm` load, strict add, store, and return operations,
  and extracts operations, operands, results, types, and CFG from the live
  graph into canonical handoff V2. Because upstream v0.17.0 cannot represent
  the AMD calling convention, target attributes, module metadata, or fe2o3
  evidence, an exact validated V1 sidecar supplies only those properties;
  every mismatch rejects. The fixture is not Rust user source.
- The canonical V2 serializer emits deterministic bounded LLVM assembly with
  its source identity, and the scalar bridge binds those exact bytes through an
  attempt-scoped compiler handoff into a sealed Worker V2 request. This record
  is inert: it authenticates no compiler executable and grants no worker,
  object, link, publication, load, or launch authority.
- The graph-derived extractor (`62e66209e`), serializer (`3a3b43e90`), bridge
  (`cb571012f`), hardened Worker (`fd6520d88`), exact inspector (`70f9c5ad7`),
  measured-HSACO gate (`e016833d3`), move-only execution evidence
  (`c9e8ca702`), sealed join (`62efd243e`), and runtime-alignment correction
  (`228c88ed9`) are implemented.
- `fe2o3-pliron-scalar-add-v1` is the dedicated join crate for this exact
  profile. It combines compile-time checkout policy, finalization, and a sealed
  one-shot HSA consumer; it is not a general backend, approval service, or
  reusable runtime-policy layer.
- `fe2o3-host-api` defines inert target-neutral compile/admit/load/dispatch/wait
  records. For issue
  [#135](https://github.com/harsh-nod/fe2o3/issues/135), which also remains
  open, `fe2o3-service-model` defines executable-free P0 service semantics and
  `fe2o3-service-host` defines an authority-free P1 borrow-retaining typestate
  adapter. Neither crate executes a persistent service.

These components make later implementation and parallel ownership possible.
They do not connect general rustc MIR extraction to the Pliron ladder, complete
the D1-D11 gates, replace the current compiler selector, publish an artifact,
execute a general host operation, or create a persistent GPU scheduler. The
bounded #159 finalization and #161 MI300X execution slices are closed. Existing
low-level HSA adapters were reused, but the old runtime route was not
sufficient by itself; the dedicated join crate owns the one-shot policy,
artifact, device, ABI, dispatch, result, canary, and unload checks. The COV6
descriptor records a 280-byte 24+256 kernarg segment with alignment 8, while
ROCr reports runtime alignment 16; the consumer requires the stricter value.

The measured run records
`evidence=69238ad704470649b9811b41cf0194bb392be8116a1b0618adb1dcbe7e1bbd4f`
against ROCr 1.18 runtime image
`7010eba894569c044749b71b63ff782080c4a91e19ff24d6dc93e857045ab37e`.
The embedded checkout policy and marker are self-consistent repository
evidence, not an external signature or CI attestation. They make no
CUDA-Oxide parity, general memory-safety, or race-freedom claim.

The existing production-directed GPU finalizer remains separate: an isolated
worker uses pinned upstream LLVM 22.1.8 target-machine APIs for object emission
and in-process LLD library APIs for HSACO. It is the sole machine authority.
It does not use the `pliron-llvm` `llvm-sys` converter. COMGR is not used.
Shell-mediated GPU compilation/linking is historical compatibility behavior,
not the target architecture.

## Rust-First Source Contract

### Kernel item and instance

`#[kernel]` authenticates an ordinary Rust function as a GPU entry item. It
does not reinterpret the function body. Modules, visibility, generics, const
generics, trait selection, associated types, ownership, borrowing, pattern
matching, and cross-crate monomorphization retain their normal Rust meaning
within the admitted GPU Rust profile.

A generic kernel definition is a kernel family. A final-crate monomorphization
with concrete type arguments, const arguments, and relevant `cfg` values is a
target-neutral concrete kernel instance. Binding its semantic operation graph
and numerical contract creates an algorithm identity; pairing that algorithm
with a schedule and target plan creates a concrete executable instance.
Library crates MAY define kernel families and ordinary device helper functions.
Device extraction and artifact finalization occur where the final crate graph
and concrete instances are known.

Reachable device helpers are ordinary Rust functions. They are authenticated
and admitted by their identity and semantics, not by symbol substrings,
displayed rustc paths, method names, or user-implementable marker traits.

`KernelContext<'grid>` is an unforgeable compiler-provided capability. It is
not a user-bound hardware ABI argument. Safe constructors for execution,
address-space, region, tile, fragment, and protocol values are private or
sealed. The compiler and registered device APIs create them only when their
executable semantics, effects, required capabilities, and proof contracts are
known together.

### One executable body

For every kernel instance there MUST be one reachable executable algorithm
body shared by ordinary rustc device extraction and the pinned Verus source
view. The macro and compiler MAY generate only:

- reserved authenticated item metadata;
- a device entry and ABI shim that delegates to the item body;
- a typed host descriptor and marker;
- a proof harness that refers to the same body; and
- deterministic source, contract, and erasure records.

They MUST NOT generate, copy, synthesize, or maintain a second algorithmic
implementation for host reference, proof, device execution, or optimization.
CPU reference execution is an explicitly separate oracle and cannot be the
kernel's executable or proof body.

The entry shim may unpack the admitted ABI, construct `KernelContext`, and call
the one body. It may not contain algorithm-specific control flow or memory
effects. A kernel item is not an ordinary host-callable function or a device
helper. Any emulation API must be explicit and independently named.

Macro ownership ends at authenticated metadata and generated surfaces. rustc
owns parsing, typing, borrowing, trait resolution, layout, MIR, FnAbi, and
monomorphization. The fe2o3 frontend owns kernel authentication, reachable-call
collection, GPU Rust admission, stable origins, and canonical import. Pliron
passes own only transformations over admitted operations.

### Host descriptor and launch lifetime

The compiler-generated descriptor is a typed host value bound to the exact
kernel family and concrete instance. Preparing a launch MUST join that
descriptor with an admitted artifact entry, inspected physical ABI, observed
device/context, launch geometry, and typed arguments.

A prepared or submitted launch retains every allocation borrow and ownership
witness until an admitted completion event. Dropping a future may stop
observation but MUST NOT release device-visible storage early. Cross-stream
conflicts require typed event dependencies or an explicit unsafe obligation.

No Pliron type or operation appears in a public Rust kernel or host signature.

## Target Production Pipeline

The intended production-directed pipeline is shown below. At this checkpoint, the
landed Pliron crates represent bounded pieces of this ladder but do not compose
or execute the full route.

```text
Rust crate graph
  |
  +-- authenticated #[kernel] family and reachable Rust device functions
  +-- typed capabilities, contracts, and proof-only state
  |
  v
rustc check + concrete monomorphization + MIR/FnAbi extraction
  |
  +-- proof projection and deterministic proof erasure check
  +-- typed host descriptor and launch ABI manifest
  v
mir.*        admitted Rust behavior and stable source origins
  |
  v
kernel.*     structured algorithm, indexing, mask, and numerical semantics
  | \
  |  +-----> proof.* source, algorithm, and refinement obligations
  v
schedule.*   non-executable tiling, fusion, mapping, and pipeline plan
  |
  v
tile.*       distributed tiles, regions, masks, LDS, and register layouts
  | \
  |  +-----> proof.* region, initialization, race, and schedule obligations
  v
gpu.*        executable target-neutral SIMT CFG, memory, barriers, and epochs
  |
  +--------> canonical fe2o3 Kernel IR snapshot and effect/capability closure
  v
amdgcn.*     selected AMD semantics and target legalization
  |
  v
llvm.*       selective pliron-llvm dialect representation
  |
  v
fe2o3 canonical V2 handoff -> bounded deterministic LLVM assembly
                           -> pinned LLVM 22.1.8 target machine
                           -> object -> in-process LLD -> HSACO

dispatch.*   optional admitted graph around exact kernel variants
autotune.*   inert selection among already admitted variants
```

`kernel.*` preserves semantic operations such as contraction, reduction,
online softmax, scan, top-k, gather/scatter, and epilogues until scheduling is
complete. `schedule.*` describes transformation choices and is not executable.
`tile.*` materializes distribution, masks, regions, and physical layouts
without premature scalarization. `gpu.*` is the target-neutral executable
boundary and has a lossless, versioned bridge to `fe2o3-kernel-ir`.
AMD-specific operations first appear in `amdgcn.*`.

The dependency closure contains reviewed, dialect-only `pliron-llvm` uses with
`default-features = false`. The optional `llvm-sys` converter is absent from
the producer and worker. That layer may build, transform, and verify transient
`llvm.*` operations; it does not own stable identity, evidence, LLVM code
generation, object emission, or linking. fe2o3 canonical records, receipts,
and the bounded serializer own the handoff to the isolated worker. Pinned
upstream LLVM 22.1.8 and in-process LLD remain the sole machine authority.

The bounded scalar implementation has landed structurally parsed backend-
fixture-to-live-graph V2 extraction, deterministic LLVM assembly, an inert
attempt-scoped request bridge, exact Worker/finalizer inspection, move-only
execution evidence, and one sealed MI300X runtime consumer. Its V1 sidecar
remains necessary for the AMD calling convention, target attributes, module
metadata, and evidence missing from the upstream dialect. General Rust-source
production lowering remains incomplete.

Every operation family defines its typed operands and results, effects,
capabilities, canonical identity payload, verifier, lowering contract, source
origin, and proof semantics together. An unknown semantic is never retained as
an unstructured string.

Every pass runs the verifier on its input and output. A pass emits a bounded,
canonical receipt that names input snapshot, output snapshot, pass and
configuration identity, preserved/replaced/discharged/delegated obligations,
and diagnostics. Losing an obligation or source origin is a verifier error.

`pliron-v1` is the explicit artifact-producing selector. Once selected it MUST
either complete this path or reject; it MUST NOT fall back to a record sketch,
raw-MIR recognizer, direct LLVM template, command-line compiler, or legacy
artifact. `pliron-shadow` is inspect-only, may compare canonical results, and
has no publication, load, proof-promotion, tuning, or launch authority.

## Canonical Identity Model

### Canonicalization rules

Durable identities are digests of domain-separated, versioned fe2o3 canonical
records. Every record declares its schema and digest algorithm. Encoders MUST
have one byte representation; decoders MUST be bounded, reject unknown
mandatory fields and noncanonical encodings, and never preserve trailing data.
Sets and maps have specified order. Optional fields distinguish absence from an
empty value.

The following MUST NOT affect an identity unless explicitly represented by a
versioned semantic record:

- Pliron context, arena, operation, block, value, or attribute handles;
- allocation or traversal order;
- Pliron parse/print spelling or diagnostic formatting;
- process IDs, paths, build directories, timestamps, or hash-map iteration;
- source comments and whitespace after an audited semantic normalizer exists.

Stable source origins are canonical fe2o3 records, never raw Pliron pointers.
They identify the authenticated crate/item, monomorphized source construct,
and semantic value or operation. Diagnostic presentation may include paths and
line/column spans without making those presentation values durable identity.

`fe2o3-kernel-ir` remains buildable without Pliron. Existing V1-V5 canonical
wire records and their identities remain byte-for-byte frozen. A future KIR
version is additive and requires explicit conversion policy; importing through
Pliron cannot restamp an old record.

### Identity chain

The canonical identity chain is:

```text
KernelItemId  = authenticated crate/item identity + generic definition
KernelInstId  = KernelItemId + concrete type/const/configuration identity
AlgorithmId   = KernelInstId + semantic operation graph + numerical contract
ScheduleId    = AlgorithmId + tiling/layout/mapping/pipeline parameters
TargetPlanId  = ScheduleId + target capabilities + legalization choices
ExecutableId  = TargetPlanId + LLVM/toolchain/object/HSACO identities
ProofInputId  = semantic snapshot + obligation/model/tool identities
```

Each `+` means inclusion by canonical identity reference in a domain-separated
record, not byte concatenation. The records have these responsibilities:

| Identity | Required canonical payload and invalidation rule |
|---|---|
| `KernelItemId` | Authenticated package/crate identity, item identity, generic signature/definition, kernel role, and source-profile version. A semantic definition or authentication change invalidates it. |
| `KernelInstId` | `KernelItemId`, concrete type/const arguments, relevant features and `cfg`, monomorphization/toolchain profile, reachable executable closure, proof-erasure profile, FnAbi, and launch-contract references. Any concrete source or configuration change invalidates it. |
| `AlgorithmId` | `KernelInstId`, canonical `kernel.*` operation graph, iteration/index maps, masks, semantic layouts, functional contract, and numerical policy. Schedule-only choices are excluded. |
| `ScheduleId` | `AlgorithmId`, schedule schema, tiling, distribution, physical layouts, mapping, fusion, specialization, unroll, and pipeline choices. An optimization changes this identity without pretending to change the algorithm. |
| `TargetPlanId` | `ScheduleId`, target and feature identities, code-object policy, target primitive contracts, legalization choices, and resource policy. Target aliases are forbidden. |
| `ExecutableId` | `TargetPlanId`, deterministic LLVM export, pinned compiler/finalizer/linker identities and options, object identity, inspected HSACO identity, exported symbols, physical ABI, and origin map. It states exact output identity, not correctness. |
| `ProofInputId` | Exact covered source/semantic/KIR snapshot, obligation set, launch/effect/target/numerical contracts, theorem statement, model package, checker, Verus, solver, and proof-policy identities. It is a proof branch, not a mutable field on executable IR. |

ABI, launch, effect, capability, source-projection, proof-erasure, model,
artifact, and benchmark records retain their own versioned identities and are
referenced at every applicable point above. Sharing a payload does not merge
kernel-entry identities. Sharing an `AlgorithmId` does not authorize proof
reuse across schedules unless an admitted parameterized refinement theorem or
checked mapping covers the new `ScheduleId`.

A cache entry is valid only when its complete identity key and owning schema
match. A cache hit is inert evidence until the consumer revalidates the record.

## Proof Boundary

### Proof is an overlay

`proof.*` is a solver-neutral obligation graph over stable fe2o3 semantic IDs.
It is not executable, cannot be targeted, and is serialized separately from
executable KIR. There is no `verified = true` operation attribute. A proof
result is an identity-bound sidecar admitted independently under the existing
evidence policy.

Every transformation must classify every incoming obligation as exactly one
of:

- preserved with a checked identity map;
- replaced by a versioned equivalent or stronger obligation;
- discharged by accepted, exact evidence; or
- delegated as a named unsafe or trusted contract.

Dropping, silently weakening, or implicitly discharging an obligation rejects
the transformation. A compiler boolean, successful type check, matching
digest, test result, or unsafe block does not become `Proved` evidence.

### Deterministic proof erasure

The frontend classifies source constructs as executable, contract, or
proof-only under one versioned erasure profile. Classification is based on
reserved authenticated metadata and admitted APIs, not spelling conventions.
For each concrete kernel instance it records:

1. the authenticated monomorphized item and reachable helper closure;
2. the proof-source snapshot consumed by the pinned Verus view;
3. the deterministic erased executable projection consumed by rustc import;
4. the erasure profile, relevant `cfg`, macro, rustc, and Verus identities;
5. a correspondence map from proof-source origins to executable origins; and
6. the proof that, or accepted checker result showing, erasure satisfies the
   restrictions below.

Erasure MUST NOT change or select executable control flow, values, memory
effects, calls, panic/unwind behavior, ABI, target, schedule, or numerical
policy. Ghost state MUST NOT flow into an executable branch, address, loop
bound, call target, synchronization decision, return value, or device effect.
An executable fact originating in a proof witness must be re-derived at
runtime, proved in the generated theorem, or carried by a small versioned
certificate checker whose identity is in `ProofInputId`.

After erasure, the executable projection contains the same one algorithm body
and reachable executable helper closure authenticated for `KernelInstId`.
Entry/ABI shims remain non-algorithmic. The binder compares source origins,
control dependence, calls, effects, contracts, and semantic identities, not
only token text or a digest supplied by one producer.

A mutation to proof-only text that cannot affect executable semantics may
change `ProofInputId` without changing the executable projection. A mutation
to executable behavior, a reachable helper, a contract premise, erasure
classification, or correspondence map invalidates all affected identities and
evidence. Proof evidence can be erased from the payload only after its identity
and executable/proof correspondence have been recorded and checked.

### Property claims

Verification is a property matrix, never one unqualified `Verified` bit. Each
admitted artifact records exactly one of `Proved`, `Validated`, `Contracted`,
`Checked`, or `Unsupported` for each applicable property:

```text
memory_safe, bounds_safe, initialized, race_free, barrier_convergent,
deadlock_free, functionally_refined, numerically_bounded, deterministic,
machine_refined
```

Each entry binds the exact statement, identities, covered compiler boundary,
preconditions, retained dynamic checks, trusted contracts, model/tool inputs,
and certificate references. Statuses do not promote or imply one another.
Source evidence never becomes `machine_refined` without the separately
admitted MIR/KIR and KIR/LLVM/object/ISA correspondence coverage.

For a `Proved` property, the instantiated theorem quantifies over all launches
satisfying the recorded preconditions, all valid initial states, and all
execution traces permitted by the named model. Safety is required for every
trace; functional refinement is required for every terminating trace.
Termination, progress, determinism, numerical error, and machine refinement
remain separate statements. `deadlock_free` remains `Unsupported` unless the
instantiated theorem includes and proves or contracts its scheduling,
residency, and fairness assumptions.

Proof timeout, missing output, unclassified premise, prohibited shortcut,
model gap, or incomplete property coverage fails a proof-required build. A
non-proof build may expose an accurately weaker status only when policy allows
it; it cannot silently relabel the result.

### Trusted boundary

Every proof result publishes a trusted-computing-base manifest naming the
exact Verus and solver, rustc extraction, erasure checker, semantics package,
certificate checkers, compiler-boundary validators, target contracts, and
environmental assumptions. Verus reduces uncertainty about the named model
and property. It does not remove these components from the trusted boundary.

## Authority Chain

No record below grants the authority of a later step by itself. Each transition
consumes or borrows exact upstream evidence and creates a new sealed state:

```text
authenticated kernel metadata + final rustc monomorphization
    -> admitted KernelInstId and typed host expectation
    -> verified mir/kernel/schedule/tile/gpu snapshots and receipts
    -> property-specific proof evidence, when required
    -> target plan + deterministic finalizer + inspected executable
    -> transactionally published artifact occurrence
    -> descriptor/manifest/payload/proof/ABI/effect/target/currentness join
    -> loaded module bound to observed context and device
    -> selected typed kernel entry
    -> prepared launch with geometry, arguments, aliases, and retained borrows
    -> single dispatch authority
    -> admitted completion and resource release
```

The authority rules are:

- Macro metadata authenticates an item role; it grants no compile, proof,
  publication, load, or launch authority by itself.
- MIR admission authorizes only construction of an admitted semantic snapshot.
- Pliron verification authorizes only the next named transformation.
- Proof evidence authorizes only its exact property statement and boundary.
- A target plan, LLVM module, object, HSACO, inspection result, benchmark, or
  manifest is descriptive evidence until the artifact binder admits the exact
  join.
- Artifact publication creates a current immutable occurrence, not a safe Rust
  signature or launch.
- Only compiler-generated host expectations may define a safe Rust signature.
  Manifests are untrusted input until matched to that expectation and physical
  code-object inspection.
- Loaded modules own executables. Loaded kernels borrow their module and bind
  one entry. Prepared launches borrow all referenced resources through
  completion.
- Dispatch consumes one launch authority. Replay, cross-context substitution,
  stale publication, and cross-entry reuse reject.
- Autotuning and performance evidence never grant legality, proof,
  publication, or launch authority.

Any identity, version, currentness, ABI, effect, capability, target, model,
proof, resource, context, or lifetime mismatch fails before the next authority
state is created.

## Fail-Closed Conformance

### GPU Rust matrix

D0 owns a versioned conformance matrix covering every reachable Rust feature
and supported `core` API. Every entry is exactly one of:

| Class | Contract |
|---|---|
| `Supported` | Rust semantics are preserved by importer, lowering, model, and tests. |
| `Restricted` | A documented subset is accepted; all other forms receive a source-spanned diagnostic. |
| `Lowered through contract` | A reserved authenticated API has executable effects, capabilities, model status, and target/numerical contracts registered together. |
| `Rejected` | Reachable use is unsupported and compilation stops with kernel root, call chain, and source span. |

Missing entries are `Rejected`, not experimental support. Adding support
requires importer, verifier, lowering, proof-policy, conformance, and target
evidence appropriate to the feature. Rust or Pliron upgrades rerun the complete
matrix before adoption.

Proof-sensitive protocol types are non-`Copy`, privately constructible, and
subject to exact-consumption MIR admission because Rust ownership is affine,
not linear. `mem::forget`, `ManuallyDrop`, invalid `Drop`, duplication,
transmute/raw reconstruction, brand mixing, escape, and failure to consume a
required token reject in the safe profile. An unsafe API must emit a named,
identity-bound obligation; `unsafe` syntax alone satisfies nothing.

### Transactional rejection

The pipeline rejects, without fallback or partial authority, on:

- unauthenticated roots, helpers, device APIs, metadata, or proof-only markers;
- unsupported reachable Rust, panic/unwind, FFI, intrinsic, or target behavior;
- unknown dialects, operations, types, effects, capabilities, contracts,
  versions, mandatory fields, or numerical semantics;
- malformed CFG/SSA, dominance, type, layout, region, permission, epoch,
  barrier, ABI, origin, or resource state;
- missing stage receipts, dropped obligations, invalid proof projection, stale
  evidence, proof timeout, or policy downgrade;
- target feature, wave width, instruction, address-space, layout, alignment,
  resource, object, code-object, symbol, or ISA disagreement;
- parser, graph, recursion, type-instantiation, obligation, proof, pass, memory,
  diagnostic, worker-output, or time budget exhaustion; and
- any authority-chain mismatch or stale artifact occurrence.

On rejection, the attempt publishes no artifact, proof promotion, cache
promotion, tuning winner, launch token, or parity/performance status. Owned
temporary outputs are invalidated according to the existing artifact
transaction. Diagnostics are deterministic and bounded. A failing explicit
Pliron build never searches for a legacy output.

## D0-D11 Stage Contracts

These contracts are cumulative. Stage outputs are immutable, versioned, and
inert outside the named consumer. Parallel implementation is allowed when
inputs are exact, but no stage may bypass an earlier gate. D6 proof work
branches from semantic snapshots and rejoins only through property-level
evidence admission.

The sections below are acceptance contracts, not a completion ledger. The
infrastructure listed in the implementation-boundary section satisfies parts
of D0 and creates shells for later stages; issue #134 remains open and no
D1-D11 production route is claimed complete by those crates.

### D0: Architecture and dependency baseline

Input: this ADR, existing architecture/safety/evidence contracts, the pinned
Rust/Verus/LLVM environment, and Pliron commit
`2610651306ea3ba670f68d5d8b1e1159bcd521ed`.

Output: centralized exact Pliron dependencies; `fe2o3-pliron` context,
identity, registration, and non-executing pass-plan shell; versioned GPU Rust conformance matrix,
identity schemas, theorem schema, erasure profile, proof-sensitive-type policy,
and minimal cross-crate `#[kernel]` metadata/descriptor prototype.

Gate: no duplicate Pliron crate identities; bounded construct/parse/print/
verify/destroy behavior; deterministic canonical fe2o3 records and diagnostics
across fresh contexts; one executable body accepted by rustc extraction and
the pinned Verus view; stable generic and concrete IDs; no symbol-spelling
authentication; no artifact-authority behavior change.

### D1: Authenticated MIR admission

Input: final rustc monomorphizations, reachable MIR, FnAbi/layout data,
authenticated metadata, source origins, and the D0 conformance profile.

Output: a validated `mir.*` module preserving calls, CFG, places/projections,
aggregates, assertions, exact arithmetic/casts/failure behavior, layout,
provenance, brands, typestate, unsafe scopes, and source origins; one slot per
non-ZST followed by verified mem2reg.

Gate: collect only the concrete reachable cross-crate closure; parse no display
strings; reject spoofed or unsupported providers transactionally; report root,
call chain, and source span; enforce exact consumption of proof-sensitive
values; verify before and after every owner-authenticated transformation;
explicit Pliron selection has no legacy recognizer fallback.

### D2: Lossless executable KIR bridge

Input: verified `gpu.*` or canonical V1-V5 `fe2o3-kernel-ir` modules.

Output: lossless adapters in both directions and independently recomputed
types, CFG/dominance, effects, capabilities, and targets.

Gate: every V1-V5 fixture round-trips byte-for-byte; fresh Pliron contexts,
allocation order, and traversal order do not affect canonical IDs; malformed
or mutated semantics reject; `fe2o3-kernel-ir` still builds and tests without
Pliron.

### D3: First-class Rust and structured algorithms

Input: D1 admitted Rust behavior plus authenticated structured device APIs,
source/launch/numerical contracts, and concrete kernel identity.

Output: `kernel.*` algorithms with iteration domains, indexing maps, masks,
semantic layouts, and numerical contracts; sealed Rust capability/tile/
fragment APIs; typed kernel descriptor and prepared-launch surface; source and
CPU reference semantics.

Gate: ordinary Rust reaches structured GEMM, row softmax, fixed-shape
FlashAttention, and bounded top-2 routing; one cross-crate generic item becomes
a concrete typed host value; two schedules retain one applicable
`AlgorithmId`; unsupported providers and implicit layout conversions reject;
safe signatures reject wrong address space, role, variant, target, wave,
layout, or initialization; no scalarization occurs before scheduling unless an
explicit reviewed path requests it.

### D4: Scheduling, tiles, and layouts

Input: one `AlgorithmId`, legal schedule parameters, layout definitions,
target-independent resource bounds, and reusable proof preconditions.

Output: canonical `ScheduleId`, `schedule.*` plan, distributed `tile.*`
snapshot, and checked transformation receipt from algorithm to tiles.

Gate: reference, vectorized, LDS, and double-buffered GEMM schedules share one
algorithm identity; FlashAttention structure survives fusion; tails use masks
rather than cloned scalar bodies; invalid divisibility, alignment,
distribution, layout, swizzle, stage, fusion, or resource combinations reject
before target lowering.

### D5: Permissions and executable `gpu.*`

Input: D4 tiles, branded allocations/execution scopes, region algebra,
initialization and async typestate, and schedule obligations.

Output: executable target-neutral SIMT CFG with explicit regions, effects,
permissions, barriers, atomics, completion, and epochs; canonical KIR snapshot;
proof obligations and checked local-fact witnesses.

Gate: permissions scale by symbolic regions/tiles/stages, not elements;
designated tokens are exactly consumed; uninitialized read, incomplete async
read, expired epoch, invalid split/join, brand mixing, and host early release
reject; compiler booleans grant no proof authority; dynamic bounds,
disjointness, convergence, races, and visibility are proved, certificate-
checked, or exposed as named obligations.

### D6: Formal semantics and same-source Verus

Input: the authenticated monomorphized source/proof projection, D1/D3/D5
snapshots, exact launch/effect/numerical/target contracts, obligation graph,
semantics package, proof policy, and tool identities.

Output: deterministic theorem and proof unit, `ProofInputId`, proof result,
property-level evidence matrix, erasure/correspondence record, and trusted-
computing-base manifest.

Gate: every admitted construct and safe primitive has a model/contract entry;
all compiler premises are re-derived, certificate-checked, or accurately
reported weaker than `Proved`; prohibited shortcuts and unclassified premises
reject; source/proof/executable mutations invalidate evidence; parameterized
proof reuse names exact schedule instantiations; unsupported numerical and
progress semantics remain `Contracted` or `Unsupported`; evidence states its
last covered boundary.

### D7: AMD legalization and finalization

Input: verified D5 `gpu.*`, exact `ScheduleId`, property policy and any proof
results that policy requires, versioned target feature/primitive tables,
resource policy, and pinned LLVM/LLD closure.

Output: `TargetPlanId`, verified `amdgcn.*` and dialect-only `llvm.*`, a
fe2o3-owned canonical finalizer handoff and evidence record, object, inspected
HSACO, stable origin map, resource/ISA reconciliation, and `ExecutableId`.

Gate: unsupported instruction/type/wave/async/numerical combinations reject;
the intended transfer, wait, barrier, and matrix sequence is present; estimates
and observed resources disagree loudly; unexpected scratch, spills, stack,
calls, symbols, or control flow block qualification; finalization uses the
existing isolated pinned upstream LLVM 22.1.8 target-machine and in-process LLD
as the sole machine authority, with no `pliron-llvm` converter or code
generation, COMGR, or shell compiler fallback; `machine_refined` ends at the
exact validated boundary.

### D8: Autotuning and variant dispatch

Input: a bounded candidate schema containing only D4-D7 legal, policy-
compliant variants and an exact benchmark protocol/environment identity.

Output: bounded candidate records, static pruning receipts, raw benchmark
evidence, deterministic selection record, generated dispatch plan, and a
predeclared safe fallback.

Gate: illegal, proof-required-but-unproved, target-invalid, or resource-invalid
variants are never compiled for timing or selected; stale cache keys reject;
ties and noise follow a versioned deterministic policy; tuner output remains
inert until dispatch admission revalidates every selected identity; tuning
failure selects only the already admitted fallback.

### D9: Graphs, finite megakernels, and persistence

Input: admitted kernel variants, exact property matrices, typed workspace and
buffer lifetimes, dependency/event contracts, graph policy, and D8 selections.

Output: canonical `dispatch.*` graph, workspace/dependency plan, admitted
unfused execution, and separately identified finite-fusion or persistent-worker
variants where supported.

Gate: routing, scan, permutation, grouped GEMM, combine, and attention graphs
enforce capacities, empty cases, dependencies, lifetimes, and variants; a fused
graph receives new identities and explicit refinement evidence; invalid queue,
residency, synchronization, accounting, or progress state rejects; an admitted
finite fallback remains available.

### D10: Performance qualification

Input: exact D7/D9 artifacts that have passed correctness, numerical, canary,
proof-policy, resource, ISA, and target gates, plus pinned ROCm baselines and a
versioned measurement protocol.

Output: identity-bound raw measurements, profiler/resource/ISA reports,
baseline records, kernel-only and end-to-end summaries, and a qualification
decision.

Gate: at least 10 warmups and 30 recorded samples with a declared variance
rejection policy; thresholds are committed before tuning results; the default
throughput target is at least 80% of the fastest pinned applicable
rocBLAS/hipBLASLt/CK baseline on declared core shapes; median regression above
5% blocks qualification without reviewed rebaseline; performance evidence is
empirical and grants no correctness, proof, or launch authority.

### D11: Shadow rollout and legacy removal

Input: supported examples and identities from both the legacy path and the
complete D0-D10 Pliron path.

Output: inspect-only `pliron-shadow` comparisons, explicit fail-closed
`pliron-v1` artifact selection, migration dashboard, default-selection change,
and separately reviewed legacy removals.

Gate: shadow mode reports every KIR/effect/capability/ABI/LLVM/output difference
and grants no authority; explicit Pliron mode passes without fallback; generic
CI, proof, finalizer, and required hardware lanes pass; at least one qualified
GEMM, fixed-shape FlashAttention kernel, and bounded MoE graph meet D10; Pliron
becomes default only after those gates; legacy code is removed in separate
reviewable changes without reducing archived coverage.

## Acceptance Tests

The following tests are architecture acceptance contracts. Each MUST pass
before its owning D-stage is complete and then remain a regression gate. The
`W0` prefix identifies this ADR, not D0 ownership. D0 implementation readiness
requires the subset whose tested surface D0 introduces; later stages add the
remaining tests and their stage-specific exit fixtures above. Test names are
stable intent labels; owning crates may choose local Rust test function names.

| Test | Required observation |
|---|---|
| `W0-DEP-001-single-revision` | The resolved graph contains exactly one source revision and one crate identity for each Pliron workspace package; a mixed revision fails CI. |
| `W0-DEP-002-closure-review` | License, advisory, feature, transitive dependency, proc-macro, and LLVM linkage reports are complete and identity-bound. |
| `W0-DEP-003-selective-llvm` | Any target graph containing `pliron-llvm` confines it to reviewed dialect/lowering crates at the pinned revision, uses `default-features = false`, and contains no optional `llvm-sys` converter in the producer, worker, or any other production component. |
| `W0-SRC-001-one-body` | Macro expansion contains one executable algorithm body; generated entry, descriptor, and proof harness contain only permitted delegation/support code. |
| `W0-SRC-002-cross-crate-mono` | One generic library kernel and helper graph produce one stable `KernelItemId` and distinct deterministic `KernelInstId` values for concrete type/const configurations in a final crate. |
| `W0-SRC-003-authentication` | Forged attributes, symbols, paths, marker traits, device operations, helper metadata, and proof-only markers reject with stable source diagnostics. |
| `W0-SRC-004-same-source` | The same minimal body is accepted by ordinary rustc extraction and the pinned Verus view after deterministic erasure. |
| `W0-ERASE-001-no-influence` | Ghost-to-branch, address, loop, call, barrier, ABI, target, schedule, and numerical-policy influence fixtures reject. |
| `W0-ERASE-002-mutation` | Mutating executable code, a reachable helper, erasure classification, contract premise, or correspondence map invalidates affected proof and executable bindings. |
| `W0-ERASE-003-proof-only-change` | An admitted proof-only change leaves the executable projection stable while changing proof input/evidence identity as required. |
| `W0-ID-001-fresh-context` | Fresh processes, Pliron contexts, arenas, build directories, allocation orders, and traversal orders produce identical canonical fe2o3 records. |
| `W0-ID-002-text-independence` | Parse/print spelling, printer order, arena IDs, and diagnostic presentation cannot alter artifact, proof, or cache identity. |
| `W0-ID-003-mutation-matrix` | Each source, algorithm, schedule, target, toolchain, proof-model, ABI, launch, and artifact mutation changes exactly the dependent identities and rejects stale reuse. |
| `W0-ID-004-context-ownership` | Foreign same-slot arena handles, transplanted public markers, erased operations, and repaired locators reject or preserve the original context identity before traversal. |
| `W0-KIR-001-frozen-wire` | Every canonical KIR V1-V5 fixture round-trips byte-for-byte through fresh Pliron contexts; `fe2o3-kernel-ir` tests without a Pliron dependency. |
| `W0-CONF-001-complete-matrix` | Every reachable Rust feature/API fixture maps to one conformance class; an absent class rejects. |
| `W0-CONF-002-call-chain` | Unsupported cross-crate behavior reports the kernel root, complete reachable call chain, and source span without partial output. |
| `W0-CONF-003-linear-protocol` | Copy, forget, `ManuallyDrop`, invalid `Drop`, reconstruction, escape, brand mixing, and unconsumed proof-sensitive token fixtures reject. |
| `W0-PASS-001-pre-post-verify` | Every pass verifies input and output; malformed output, lost origins, or a dropped obligation aborts the transaction. |
| `W0-PASS-002-resource-bounds` | Oversized parser, graph, type, proof, diagnostic, and pass-work fixtures fail deterministically within configured bounds. |
| `W0-PROP-001-no-promotion` | A proof, check, test, benchmark, manifest, unsafe block, or compiler success cannot promote another property or covered boundary. |
| `W0-PROP-002-proof-required` | Timeout, missing evidence, prohibited shortcut, stale tool/model identity, and unclassified premise fail a proof-required build. |
| `W0-AUTH-001-complete-join` | Every authority-chain mismatch rejects before a loaded kernel or prepared launch exists; no individual manifest, proof, payload, or inspection record suffices. |
| `W0-AUTH-002-lifetime` | Context, cross-entry, stale-publication, replay, alias, and early-resource-release fixtures fail before dispatch or release. |
| `W0-SEL-001-no-fallback` | `pliron-v1` failure publishes nothing and never invokes a legacy recognizer/template or consumes stale legacy output. |
| `W0-SEL-002-shadow-inert` | `pliron-shadow` can report differences but cannot publish, load, launch, select a tuner winner, promote evidence, or update qualification status. |
| `W0-TXN-001-rejection-cleanup` | Failure at every stage leaves no current artifact, proof promotion, cache promotion, tuning winner, or launch authority from that attempt. |

In addition to focused tests, D0 requires two clean executions to produce
identical canonical records and bounded diagnostics, strict format/lint checks,
dependency duplicate/license/advisory checks, and malformed-input/fuzz corpora
for every newly introduced canonical decoder.

## Adoption and Change Control

This ADR is accepted when its terminology and invariants are used by D0
interfaces and the Wave 0 acceptance suite passes. It does not mark D0, issue
#134, or any parity row complete by documentation alone.

Changes to the one-body rule, identity composition, proof erasure, property
statuses, authority transitions, no-fallback policy, V1-V5 independence, or
stage gates require an explicit architecture revision and migration tests.
Operation syntax, internal pass decomposition, and implementation crate names
may evolve when the same ownership and canonical boundaries remain intact.

The first qualification target remains the repository's admitted `gfx942`
profile. `gfx950` and RDNA targets require separate feature, schedule, proof,
artifact, and performance identities; they are not aliases of `gfx942`
evidence.
