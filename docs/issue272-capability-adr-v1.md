# Issue 272 Capability Contract Adoption V1

Status: proposed freeze for primary/owner review. **Not yet approved; no issue
checkbox is closed by this patch.** This records the M0 decision separately from
the M1 production vertical and later qualification milestones in
[issue #272](https://github.com/harsh-nod/fe2o3/issues/272).

Source examined: `91d4b0a2675f5f8b4449768ae89060c06f83689e`, tree
`7812f802abbd265d10d8dc5e24308692b9fba301`. The companion
[M0 baseline](../config/issue272-m0-baseline-v1.json) records an inventory and
honest unavailable/not-run observations, not executable authority or a release.

## Decision And Existing Owners

Adopt one compiler-issued logical kernel context and one derivation hierarchy
on the existing canonical production graph. No new public authority constructor,
compiler selector, proof graph, runtime route or fallback is introduced.
The following existing contracts retain ownership of their schemas and semantics:

| Contract | Adopted responsibility |
| --- | --- |
| [GPU safety V1](gpu-safety-contract-v1.md) | Brands, uniformity, regions, permissions, effects, epochs and property-level assurance |
| [Architecture V2](architecture-v2.md#permanent-component-boundaries) | Crate ownership, target-neutral capabilities, artifact ABI and generated host boundary |
| [Verification model](verification-model.md#trust-boundary) | Source proof meaning, identity binding, approved assumptions and production TCB |
| [Production convergence](production-pipeline-convergence-v1.md) | One importer/graph/transaction, canonical identity, finalizer and migration policy |
| [Execution MIR29/KIR15/SO3](execution-capability-contract-v29.md) | Allocated context/workgroup/u32-tile roles, exact scope closure and ordered effects |
| [General kernel checks](general-kernel-check-pipeline-v1.md) | Fixed analyses, failure ownership, incomplete/rejected distinction and diagnostics |
| [Typed dispatch](general-typed-dispatch-v1.md) | Inspected ABI, generated argument preparation and retained completion ownership |
| [Stability policy](support-matrix.md#stability) | Unstable Rust surface versus explicitly versioned strict records |

The [SIMT/tile proposal](unified-simt-tile-integration.md) is a #275 integration
consumer, not blanket #272 approval. Its linked
[capability-owner decision](https://github.com/harsh-nod/fe2o3/issues/272#issuecomment-5699273088),
[numeric allocations](https://github.com/harsh-nod/fe2o3/issues/272#issuecomment-5699301299)
and [MIR role allocation](https://github.com/harsh-nod/fe2o3/issues/272#issuecomment-5700545180)
are the recorded first-slice decisions; they are not approval of this ADR or
evidence that all derived capabilities execute.

## Full Derivation Hierarchy

This is a semantic derivation relation, not a promise of new Rust method names:

```text
KernelContext(kernel instance, target, launch, invocation occurrence)
  -> Grid domain
       -> Workgroup identity and generative scope
            -> Subgroup identity and participation contract
                 -> Invocation identity and index-space mapping
  -> Memory views joined to allocation, extent, access role and owning scope
  -> Target requirements checked by the owning backend
```

Direct context queries for an invocation or workgroup must retain the same
ancestors; they cannot create an independent root. A grid domain describes
geometry, not permission to synchronize all workgroups. Device/system memory
ordering is not a grid collective. A portable subgroup has an admitted size and
participation mask; AMD wave modes remain explicit target requirements, not a
portable assumption that a subgroup is 32 or 64 lanes.

| Derived family | Required identity and lifetime | Facts not supplied by the Rust type |
| --- | --- | --- |
| Context and grid | Exact nominal kernel instance, target, launch and compiler-authenticated issuance; no physical context kernarg | Actual launch geometry, compiler authenticity, grid-wide convergence |
| Workgroup | Original context borrow, dynamic workgroup and generative occurrence | Uniform arrival, shared-memory race freedom, permitted resource size |
| Subgroup/wave | Parent workgroup, subgroup occurrence, admitted width and participation | Active-lane agreement, collective convergence, target instruction legality |
| Invocation/index | All ancestors, logical index space and invocation occurrence | Injectivity of an arbitrary mapping, bounds, cross-invocation disjointness |
| Global/constant views | Original allocation/provenance, byte extent, layout, access role and launch lifetime | Non-aliasing merely because wrappers differ; initialization or safe arbitrary indices |
| Workgroup storage | Workgroup/region, initialization state, outstanding borrows and synchronization epoch | Visibility or initialized-before-read without a valid publishing transition |
| Private storage | Original invocation and region; no cross-invocation escape | Valid raw-pointer provenance or initialized bytes by type name alone |
| Barrier/atomic/async protocol | Participants, memory spaces, ordering/scope, dynamic occurrence and epoch; exact outstanding operation | Global convergence, happens-before, atomic coherence or async completion |
| Tile/fragment/matrix | Original view/scope/epoch, masks, layout, numerical policy and applicable target contract | Schedule equivalence, full contribution coverage or machine refinement |

Authority-bearing values cannot be safely fabricated, cloned, transferred to a
different host thread, reconstructed from ordinary data, or retained beyond
their scope. Plain coordinates and returned arithmetic values are data, not
authority. Borrowing a capability preserves its original identity; matching
types, zero-sized layout, a marker trait or a digest does not authenticate it.

Memory permissions distinguish shared reads, exclusive non-atomic writes,
scoped atomics and ordered ownership transfer. `AccessMode::WriteOnly` alone
does not establish disjoint writes. `Generic` address space is unresolved, not
a wildcard authorization. Every view conversion must retain or prove allocation,
space, extent, alignment, role and index correspondence. Raw reconstruction
remains an explicitly documented unsafe obligation, never safe admission.

A synchronization transition invalidates stale descendants only for its actual
participants and memory spaces. Compiler `ScopeEnd` closes a borrow/lifecycle;
it is not a GPU barrier. Async and reusable LDS storage cannot be reclaimed
until the required completion/publication transition has occurred. Rust local
typestate cannot replace same-graph convergence, race or happens-before checks.

The staged source provider currently allocates only context/workgroup and
masked-u32 tile/fragment operations. The hierarchy above freezes the ownership
requirements for other families, not their executable admission. Unsupported
derivations reject; new source operations require their existing owners' exact
schema, diagnostic and preservation contracts before admission. Existing
unrelated `current()` providers do not become context-derived by this decision.

## Responsibility Matrix

| Enforcement layer | Owns | Must not claim |
| --- | --- | --- |
| rustc types/borrowing | Local construction restrictions, invariant brands, borrowing, moves and lifetimes | Cross-invocation uniqueness, convergence or runtime completion |
| GPU-Rust MIR admission | Genuine provider/root/helper identities, complete reachable closure, layouts/FnAbi, unsafe and source origins; logical context adds no kernarg | That a reserved diagnostic name or nominal type alone authenticates issuance |
| Canonical KIR verification | Exact graph structure, types/roles, producer/consumer lineage, affine scope/epoch lifecycle and supported control flow | Source authenticity, schedule equivalence or safe launch |
| Static analyses | Bounds, initialization, alias/ownership, races, uniformity, convergence, atomic and pipeline legality on the exact graph epoch | That incomplete coverage, exhaustion or a pre-transform result authorizes the final graph |
| External proof | Exact named source/refinement/functional properties under recorded model and assumptions | Proof of another boundary, target, numerical policy or machine code |
| Target legalization | Exact target capability closure, subgroup modes, resource, numerical and operation legality | Target-neutral validity from an AMD name or matching geometry |
| Artifact inspection/binding | Exact emitted payload, physical ABI, resources, source/evidence and descriptor correspondence | Safe launch from an internally consistent unsigned/self-asserted record |
| Checked host preparation | Dynamic extents/strides, allocation identity/aliasing, alignment, target/context, geometry and inspected ABI | Compiler proof or release of device-visible borrows before completion |
| Runtime completion | Original context/stream, retained resources, terminal completion and permitted release | That drop/cancellation alone proves quiescence or numerical correctness |

The [production TCB](verification-model.md#trust-boundary) is adopted without
reduction: Verus/translation/solver/model, proof erasure/binding, rustc,
frontend/verifiers/passes/lowerings, LLVM/LLD/admitted tools, direct-KFD/DRM
bindings, Linux/driver/firmware/hardware, and unsafe/FFI/device-library contracts.
The deployed compiler/proof service and protected origin/custody roles are also
trusted where the existing [execution service contract](compiler-execution-service-v1.md)
relies on them; this decision does not replace their deployment review.
HIP/HSA qualification oracles do not enter the production closure. Native
custody and cleanup tests do not discharge a compiler or semantic proof premise.

## Canonical Schemas And Compatibility

| Fact | Existing canonical owner and boundary |
| --- | --- |
| Source identity, layout, call/issuance origin | `fe2o3-mir-model` semantic MIR and `fe2o3-rustc-front` FE2O3KC V1 declarations; declarations alone are inert |
| Context/workgroup/tile/fragment roles and lifecycle | MIR29 roles14-17, intrinsics81/82/84/85/86; KIR15 types9-12, operations32-37; SO3 Execution family6/opcodes1-6 |
| Memory spaces, access modes, launch and target requirements | Existing KIR `AddressSpace`, `AccessMode`, `LaunchDomain`, `WorkgroupSize`, `TargetCapability` and exact admitted wire schema |
| Barriers/atomics and scoped ordering | Existing KIR `SynchronizationScope`, `MemoryOrdering`, `BarrierSemantics` and operation effects; allocation/epoch/coverage remains bound to its graph occurrence |
| Transformation and analysis results | Owner/epoch-bound canonical KIR, checked correspondence and replay under #271/#134; never mutable parallel verification IR |
| Proof/evidence | Existing verification-model property records and exact #209/#213 capsule/verifier joins; no new approval bit or capability-only receipt |
| Artifact/generated host | Existing versioned artifact/descriptor and typed dispatch contracts, independently joined to inspected ABI and runtime facts |

MIR83 is reserved, not a callable source terminal. ScopeEnd is synthetic KIR.
The inert KIR15/SO3 codecs and lifecycle checks **are implemented**; this
corrects the older codec-status sentence without granting source, schedule,
proof or launch authority. Later KIR profiles remain independently admitted;
for example, checked-storage V18 has its own exact-backedge lifecycle policy,
not retroactive permission for cyclic V15 input.

Freeze existing bytes, identity domains and independent version namespaces.
Never accept a numeric version range or reinterpret an old tag. Semantic MIR
V16-V26 and numerical V27, and historical KIR13/14, do not become admitted by
MIR29/KIR15 support. Unknown mandatory features reject. A descriptive target
extension name is not a safe capability constructor.

Stable identity uses canonical semantic records and exact bindings, not rustc
private IDs, arena slots, pointers, printer text, paths, timestamps or diagnostic
presentation. New capability operations outside allocated schemas require an
explicit owner-reviewed version/compatibility decision; this ADR allocates no
additional wire tags. Raw handles and lifetime tokens are never serialized as
authority. Rust API spelling remains developer-preview unstable; aliases must
not bypass brands, checks or the single production route.

Changes to semantics invalidate dependent analysis/proof/cache evidence. Proof
inputs, artifact evidence and generated host descriptors migrate together when
their meaning changes; retaining an old decoder never selects an old compiler.

## Diagnostic Taxonomy

Adopt the existing fixed pipeline's `Clean`, `Incomplete`, and `Rejected`
outcomes. Missing facts, unsupported operations, missing proof coverage and
resource/solver exhaustion are not counterexamples and never become Clean.
Concrete violations remain Rejected with bounded witnesses when available.
Both failure outcomes stop authority-bearing production before affected output.

| Failure class | Existing identity/owner | Required reported boundary |
| --- | --- | --- |
| Forged provider/root, unsupported source or ABI | `KernelContextFrontendContractValidationErrorV1`/`KernelContextFrontendContractDecodeErrorV1` for inert declarations, then the authenticated collector/importer; [source contract](generative-tile-source-provider.md) | Decode failure is not an authentication result; MIR admission reports root/helper/source origin, not a late machine failure |
| Invalid capability signature, duplicate issuance, wrong scope/epoch, escaping/reused descendant | KIR `DiagnosticCode::InvalidSemanticOperation`; [execution lifecycle verifier](../crates/fe2o3-kernel-ir/src/verification_execution_lifecycle_v15.rs) | Exact KIR function/block/operation and failed invariant |
| Bounds/provenance/extent | `FE2O3-BOUNDS-*`, formal-memory and bounds owners | Materialization or ranked check, labelled distinctly; no invented ranked dump |
| Ownership/alias/coverage | `FE2O3-OWN-*`, hierarchical ownership/race owners | Concrete conflict/hole versus unavailable coverage |
| Atomic ordering/coherence/target | `FE2O3-ATOMIC-*`, atomic-legality owner | Invalid contract versus missing authenticated capability/coherence |
| Convergence, initialization and epoch protocol | Barrier/workgroup-memory owners and `FE2O3-PIPELINE-*` | Participant/order/publication or stage/commit/wait/consume/release failure |
| Target/launch/resource feasibility | `FE2O3-TARGET-*`, legalization and checked-host owners | Static target violation versus dynamic host rejection before GPU effects |
| Proof, artifact, currentness and completion mismatch | Existing typed verifier/artifact/runtime errors | Exact rejecting layer, without promotion from an earlier successful layer |

This is a taxonomy adoption, not a claim that every family already has a
dedicated source UI code. The existing KIR lifecycle code intentionally groups
several invariants; message text is not a new stable subcode. New public numeric
codes or wire diagnostics require the diagnostic owner's allocation and tests,
not invented codes in this ADR. The baseline records no refusal without actual
evidence of its named boundary.

UI diagnostics must retain stable owner/code, root, available source span and
helper chain, stage, failed invariant and bounded witness. Unavailable provenance
must be labelled unavailable, never fabricated. The separate
`CanonicalDiagnosticV1` retains bounded ordered code/severity/stage/semantic
subject/text and deliberately excludes formatted spans and process-local
identity. Source presentation and canonical identity must not be conflated.

## Migration And Baseline

Adopt [selector retirement](production-pipeline-convergence-v1.md#selector-retirement):
one production transaction, terminal unsupported behavior, no workload dispatch
or alternate executable implementation. Migrate ordinary attributed source and
all consumers before deleting superseded acquisition APIs. Keep only the minimum
inert/offline differential evidence; compatibility is not safe-launch parity.

The companion schema `fe2o3-issue272-m0-baseline-v1` is a documentation snapshot,
not a source-contract extension. Its original47 cohort is 10 gfx942 and 37
gfx950 fixture selections. Three named later additions bring the current roster
to50. Feature variants can share a symbol; neither the 25 lesson entries nor
the separate #275 displayed SIMT/tile inventory is this denominator.

Classification `compiler-produced` records an obligation, not successful
compilation. Every row separately records production `not-run` for this baseline
audit, simulator `pending` and hardware `unavailable` for lack of target-matched
baseline evidence. This does not claim the hosts are physically unavailable or
that no historical source/CPU/GPU experiment ever ran. Existing source-contract
simulation status and required gates are preserved, including absent hardware
requirements; this patch neither removes nor upgrades them.

No production execution/refusal receipt is imported here. A later baseline may
record `refused` only from an exact pinned observation including source, input,
target, lane, command, stage and retained evidence. Missing evidence is
`not-recorded`, not a fabricated refusal. A nondefault source-to-LLVM corpus
observation is not a protected-production result. No aggregate success count is
created; simulator/hardware success is outside this baseline's admitted states.
V1 deliberately freezes this unevaluated snapshot: its checker accepts no
observation evidence, including a refusal-shaped record. Importing a real
observation requires an explicitly reviewed baseline revision with exact evidence
binding, not filling an arbitrary JSON field until a test accepts it.

The source manifest's null accepted compiler commit/tree, pending statuses and
no-fallback policy remain untouched. A reviewed rebaseline must reconcile input
changes explicitly and preserve historical attribution and original47
obligations; it must not silently relabel this snapshot as current execution.

## Consistency And Acceptance

`python3 -I -B scripts/tests/issue272_m0_contract.py` checks the pinned manifest,
exact cohort/selection bindings and non-promotion rules using the existing
manifest parser and original47 projection. Mutation tests reject omitted,
duplicate, foreign or substituted rows, obligation changes, fake observations
and changes to the protected source-contract baseline. The existing matrix
test harness invokes it; no compiler, simulator or GPU execution is involved.

These checks establish consistency, not human approval, execution provenance or
semantic correctness. Primary must review and record explicit acceptance of
this decision and baseline, with owner decisions linked for any schema/code
changes. Review must distinguish outstanding implementation from a missing
contract decision. Until then this ADR is proposed. M1-M7 and the issue itself
remain independently open; native startup/cleanup or a library test count
cannot substitute for their required kernel evidence.
