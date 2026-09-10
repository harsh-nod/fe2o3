# GPU execution capabilities V1

Status: normative architecture and milestone contract for
[#272](https://github.com/harsh-nod/fe2o3/issues/272). This document refines
[#134](https://github.com/harsh-nod/fe2o3/issues/134),
[#175](https://github.com/harsh-nod/fe2o3/issues/175), and
[#271](https://github.com/harsh-nod/fe2o3/issues/271). It does not create a
second compiler, IR, proof, artifact, or launch path.

This document specifies the contract and completion gates. It does not claim
that any #272 work package or milestone is implemented. A milestone is complete
only when its linked issue dependencies and its end-to-end acceptance tests are
complete on the one production route.

## Integration checkpoint: 2026-09-10

The capability migration remains incomplete: 0 of 47 tutorial fixtures have
qualified through the new production path. No milestone is completed by the
component tests below.

A clean source-export sweep of `75a5778ed` on mi350 attempted all 47 exact
manifest selections: 10 gfx942 and 37 gfx950. All exports still rejected before
producing a Bundle V8; no hardware command or qualification adapter ran. The
first observed boundaries are now:

| Source boundary | Fixtures |
| --- | ---: |
| Other core helpers lack reviewed source-safety authentication | 29 |
| Reachable panic or precondition path | 7 |
| Mutable-slice descriptor admission | 3 |
| Closure environment count exceeds the bounded profile | 2 |
| Numerical-policy terminal lacks production expansion | 2 |
| Dynamic launch/output coverage | 1 |
| Retained capability borrow requires private-slot lowering | 1 |
| RustCall helper is unsupported by checked call expansion | 1 |
| Dereferenced memory access lacks ranked index projection | 1 |

The 29 helper failures comprise `Result::from_residual` (12),
`usize::checked_sub` (8), `usize::wrapping_sub` (4), `usize::checked_add` (3),
`Option`'s `Try::branch` (1), and `PartialEq::ne` (1). These require exact
authentication and subsequent semantic lowering, not a blanket core-library
exemption. This table reports first failures, not a complete inventory of
remaining obligations. Nine fixtures reach descriptor, semantic import, call
expansion, ranked analysis, or KIR lowering after collection; none qualifies.

Device closure transport now retains exact caller, operand, callee, MIR, ABI,
monomorphization, and target custody. The admission identity is carried in the
rustc identity transcript. Borrowed helper environments and zero-sized constant
closures require admitted caller custody; helper status alone grants none.
Large environments share the existing aggregate capture/byte maxima; the
eight-environment limit remains. Host references, escapes, changed call edges,
dropping captures, and exhausted budgets still reject.

Closed MIR checks now authenticate the bounded core checked-multiply and
`Option::unwrap_or`/`and_then` profiles, including dead blocks and cleanup.
Authenticated generated `FnOnce` adapters remain in recursive collection;
shared receiver reborrows and RustCall source tuples are represented explicitly.
Canonical local ordering is preserved. Wave64 consequently reaches checked
call expansion, which still rejects its RustCall helper. Fill passes its earlier
invocation receiver boundary but retains a different capability borrow.

Named constant `for` ranges now use bounded CTFE expansion with primitive
integer typing, a shared nested-unroll budget, and rejection of executable
expressions embedded in endpoint paths. Mutation tests cover identities,
operands, effects, cleanup, receiver types, source roles, and resource limits.
On mi350, 1,149 selected library tests and 11 integration tests passed. One
integration test checks constant-only closure profiling, not whole-kernel
semantic admission or GPU execution. Formatting passes for changed packages;
unrelated existing workspace formatting differences remain untouched.

The diagnostic report retains exact candidate/binary identities, commands,
content-addressed logs, and 47 successful scratch-cleanup observations. Its
SHA-256 is `555564e4c33c9ec4667a42b138e7019435b5ff3e866198546e5c556a4682584e`.
Input-manifest hashes and qualification statuses are unchanged. Final machine
refinement, required negative cases, and protected GPU/CPU comparison remain
mandatory and incomplete.

### Previous source baseline: 2026-09-09

A source-export sweep of committed revision `742c20641` on mi350 attempted
all 47 exact manifest selections after repairing ten standalone lockfiles and
the sparse/compressed-attention feature gates. All 47 exports rejected; this
was not a hardware run. The first observed blockers were:

| Source boundary | Fixtures |
| --- | ---: |
| Borrowed closure capture lacks allocation/completion provenance | 25 |
| Other cross-crate helpers lack reviewed source-safety authentication | 12 |
| Closure capture budget exceeded | 3 |
| Reachable panic path | 3 |
| Dynamic launch/output coverage | 1 |
| Retained private-slot lowering | 1 |
| Named-constant `for` range unsupported by macro lowering | 1 |
| Host-only dependencies included in an AMDGPU build | 1 |

The Wave64 dependency issue was then fixed in `4045b8bd7` using the existing
host/device cfg split. A separate exact-fixture rerun passed dependency and
Rust compilation, then rejected at semantic MIR import: function 1 has invalid
local roles. It still produced no bundle or hardware observation.

The diagnostic runner retains exact commands, candidate and binary identities,
exit status, bounded content-addressed logs, and cleanup observations. Its
`--source-export-report` mode does not create qualification evidence. Earlier
checked-in `productionExport` observations remain historical; refreshing input
hashes does not rerun those observations or turn them into passing results.

The real context-based vecadd now passes CPU-reference binding and semantic
MIR admission. Integration fixes preserve the borrowed invocation receiver,
its root provenance, nominal disjoint index-space dependencies, and the exact
physical payload of transparent typed memory views. Canonical V17 records
retain the invocation-index operation; older wire versions reject it.

Vecadd now passes checked direct-call expansion and SSA planning. The compiler
retains the original admitted MIR and a separate, content-bound execution view
shared by ranked analysis and KIR lowering. Expanded calls have distinct local
and block coordinates, with exact argument-transfer, return, and source-origin
records. Moves, borrows, context issuance, physical bindings, and control flow
remain explicit. Recursive calls, unsupported call contracts, and exhausted
resource budgets reject.

Ranked projection now represents authenticated `Global<ReadOnly>` loads and
identity-mapped `Global<DisjointWrite<Index1D>>` stores as indexed effects with
exact allocation, extent, index, access, and predicate relations. Physical-view
metadata remains tied to the authenticated allocation. Shared scalar slices
retain read-only access even when rustc's ABI record omits a frozen-pointee
flag; this refinement does not apply to raw pointers or interior-mutable data.
Exclusive read/write views now preserve their authenticated mutable borrow,
allocation, access, and source relations in this ranked path. A bounded
reaching-write analysis now correlates a mutable load with initial memory or
one preceding store at the exact same invocation coordinate. It retains
unchanged-memory paths, requires exact bounds guards and dominance, and rejects
conflicting joins, aliases, unknown effects, barriers, and cyclic control flow.
This is not yet a model for tiled, cross-invocation, or loop-carried mutable
state. An exclusive allocation does not imply disjoint
invocation accesses: stores still require
an exact invocation-derived index, and ranked race and ownership checks remain
mandatory. Blocked stores and unsupported compact arithmetic index mappings
still reject.

The real vecadd extraction now passes write-guard and expression matching and
CPU-reference bounds discharge. For input arrays `a` and `b`, the CPU reference
writes only when `point < a.len() && point < b.len()`. The GPU's checked-load
branches independently produce those conditions as bounded disjunctive normal
form. Input guards remain explicit even when their lengths happen to match.
Only an exact output-bounds condition is discharged under the declared
output-coordinate domain. Each CPU bounds assertion must follow from that
domain or its own preceding CPU path conditions, never from itself, a later
write's guard, or the GPU's guards.

The protected functional-refinement runtime now admits on mi350 in a private
mount namespace with the unchanged byte pins and root-owned installation at
`/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5`.
A read-only directory overlay supplies the pinned loader only inside that
namespace; the shared host's loader and libraries are unchanged. A leaf-file
bind mount correctly rejects at the retained runtime's no-cross-device check.
No production protection check or manifest pin is relaxed.

Real vecadd extraction now executes and imports its local reference proof.
The controller's bounded polling backoff removes a fixed-sleep bottleneck:
the previous run hit its 60-second proof deadline; the new run reaches ranked
ownership admission in 13.14 seconds with the same deadline. Ownership then
rejects the dynamic launch dimension (`FE2O3-OWN-002`). No Bundle V8, HSACO,
protected launch, or hardware qualification was produced.

Completing that boundary requires guard-aware output coverage and an
authenticated relationship between runtime launch dimensions and output
extents. The current source join requests `TotalView`, whereas the CPU reference
can leave outputs unchanged when an input is shorter. Merely replacing that
contract with `ExactEffectDomain`, or treating a tested grid as a universal
compile-time bound, would not establish whole-output equivalence.

A separate ignored production-runtime test now executes and imports matching
integer and IEEE operator-congruence formulas and rejects wrong operators
through the normal root-protected lease. Its four proof cases passed on mi350
in 24.85 seconds. The earlier retained-runtime smoke and formula tests on
mi300x use the test-only ownership policy and produce no production lease.
The retained controller also verifies generated integer and floating-point
operator-congruence formulas, rejects wrong-operator mutations in both models,
and verifies a generated IEEE aggregate formula. These are explicit test-only
executions, not tutorial source compilation or receipt publication.
They exposed and fixed an unconditional assumed-function declaration that was
incompatible with `--no-cheating`. The generator now universally quantifies
the operator interpretation and forwards it through aggregate effect proofs.

Separately tested KIR value correlation accepts a guarded load as a source
load only when its exact authenticated recipe is intact, the load precedes
the consuming write, and every possible path or write predicate excludes
using the fallback value. Unknown paths are retained conservatively; cycles
and exhausted budgets reject. This component has not yet been exercised by
the real vecadd extraction, which stops at ranked ownership admission. Mutable escapes
invalidate scalar and physical-view provenance; a copied allocation length
cannot silently retain its old meaning after rebinding through a borrow.

SSA and KIR initialize authenticated ambient workgroup scopes at each helper
call's exact frame-entry marker, not at the root entry. Distinct call instances
retain distinct locals and definitions. Expanded SSA diagnostics report the
original function, block, local, and call instance alongside execution-view
coordinates, including synthetic argument and return transfers.

SSA also recognizes the exact borrowed context, view, and blocked-witness
receiver positions of typed global-memory intrinsics. Physical bind arguments
and by-value indices do not become transparent borrows. Reference escape,
multiple consumers, wrong argument positions, and wrong receiver types retain
storage or reject; intrinsic admission still authenticates the full ABI,
ownership, and capability contract.

Execution-view replay detects changes to the retained derivation; it is not an
independent semantic-equivalence proof. Canonical call-expansion evidence V1
retains instance ancestry, local/block origins, parameter/return transfers,
and frame lifetime markers. Induction evidence V2 binds the complete recomputed
report to the original source, aggregate expansion, and selected execution
view. Decoding these inert records does not prove the retained claims; replay
checks exact source-derived records and rejects report subsets or substitutions.
The induction analysis now certifies helper-local bounds transferred from exact
`u32` constants or unchanged parent arguments. It checks each call instance,
unique definitions, dominance, storage and move availability, and frame
reinitialization. Reassignment, ordinary aliases, projected bounds, and
unsupported loop shapes still reject.

Correspondence V6 composes original source, checked expansion, complete per-root
V1/V2 induction, SSA, and exact KIR through mandatory live replay. Native V13
lineage retains distinct physical-root, selected-body, and execution-view
identities in a versioned source envelope. Original-coordinate induction V1 and
correspondence V4/V5 continue to reject expanded coordinates; no old wire format
is reinterpreted. These checks do not independently prove CPU/GPU equivalence.

Three native backend tests exercise V13, optimizer V6, target lowering, the
existing opaque receipt, and independent KIR-to-LLVM replay for gfx942 and
gfx950. Source, target, and final-graph substitutions reject. This receipt path
already existed; no new outer capsule version was needed. The tests do not
establish source-proof execution, machine refinement, or a GPU observation.

Native V5 has a typed continuation from pending machine-refined finalization
through authenticated compiler completion to publication. The finalizer binds
the checked machine evidence to the exact worker request, response, optimized
bitcode, generated object, and raw HSACO. Explicit capture-required requests
now transport bounded linked bitcode, optimized bitcode, and generated-object
bytes and require exact agreement between bootstrap and replay. Existing V2
requests retain their previous wire bytes, V4 responses, and size behavior;
the production caller still selects V2. These contents are
inputs to checking, not a machine-equivalence certificate. Complete pass
occurrence, instruction-selection, and decoded-ISA correspondence are still
missing, so the independent machine-evidence gate remains fatal.

The hardware protocol now admits compiler-only preparation separately from
signed hardware observations. Rust verifies the newline-inclusive prepared
record identity and preserves authenticated payload bytes. Python gathers
driver/runtime facts on the target host. A real sealed V5/Bundle V8 archive
has not yet passed the combined Python-to-Rust positive path. The Python
producer now consumes the current compiler-only preparation schema and leaves
driver/runtime facts to the hardware observation. A compiler-owned negative
replay binds the required declaration roster and first checks the unchanged
MIR and executable KIR as positive controls. Supplemental mutations remove a
source root, change unwind behavior, or introduce an unknown KIR callee;
import independently replays them. None substitutes for a required declared
case: the receipt explicitly reports zero required cases satisfied and no
Rust-source recompilation. Caller JSON claiming that negatives passed cannot
authorize promotion.

Python and Rust now hash package sources in the same component order, excluding
only `target` directories. A shared digest vector covers prefix collisions and
creation order; all 47 checked-in fixture input contracts pass the Rust
production admission check without rewriting their expected hashes in the test.
Input-only contract refreshes do not create fresh production observations:
retained export diagnostics are explicitly historical and all kernels remain
unqualified. `--check-inputs` validates current inputs without promoting them;
`--committed-parity` additionally requires exact shared contract bytes in both
repositories' committed HEADs.

The preceding component checkpoint passed 1,601 library tests: 658 compiler, 128 MIR
model, 166 Pliron, 200 lowering, 51 AMD model, 130 kernel analysis, 104 KIR,
39 shared lineage, and 125 verifier tests. Six verifier tests are ignored by
default: three subprocess helpers and three provisioning-dependent runtime
tests. All three runtime tests passed in explicit test-only runs on mi300x.
The selected native capability verifier integration suite passes 20 tests,
including three shared historical-fixture tests and rejection of unchanged
historical evidence with a stale work report.
The transaction producer and batch-verifier CLI suites pass another 14 tests.
That checkpoint's finalizer library suite passed 111 tests, including three machine-binding
tests and three historical-fixture regressions. Genuine V8 fixtures captured
from compiler revision `149019b40` repair the stale fixture dependency without
projecting V13 into V8. Their original bytes stay frozen; a separate current
induction replay preserves every certificate and correspondence coordinate.
Worker-admission integration passes 16 tests, but 12 still fail at the missing
machine-refinement gate before their later finalization/publication assertions.
Two real-worker integration tests remain ignored. No missing proof was replaced
with fixture authority to make these tests pass.
This update reruns 1,187 selected library tests successfully: 671 compiler,
228 artifact transaction, 120 finalizer, 126 verifier (seven ignored), and
42 runtime protocol tests. The 124 tutorial Python tests also pass. Wave64
host tests run through `cargo fe2o3 test --all-targets`: 46 pass and three are
ignored. Direct `cargo test` is not admitted for its typed kernel.
The new C++ codec tests pass against explicitly unqualified LLVM 18; the full
pinned LLVM 22 worker pipeline has not been built or executed. Finalizer-only
strict Clippy passes; this is not a workspace-wide Clippy result. The three
new wrapping-helper tests verify the closed origin/signature contract and
reject changed MIR operators, operands, effects, and return shapes. The helper
waiver applies only to the exact reviewed safe-core wrapping bodies; collection,
intrinsic authentication, MIR admission, and lowering still run normally.
This is not a full-workspace test result or all-kernel equivalence evidence.
Strict Clippy is not clean: existing style diagnostics remain in the MIR model
and proof-contract dependency.

The tutorial website passes 198 unit tests with one worker, corpus validation,
lint, type checking, and its production build. A two-worker run hit the existing
debugger UI test's five-second timeout; no timeout was relaxed. The page also passes
desktop/mobile browser checks and overflow checks. Qualification remains 0 of
47; neither these component results nor contract parity establishes a complete
source-proof, machine-refinement, artifact, and hardware chain. No deployment
is claimed by this checkpoint. No qualification requirement was relaxed.

## Decision

fe2o3 represents GPU execution authority as compiler-issued Rust capabilities.
For each admitted entry, the attributed function signature is the logical
argument bundle. Its `KernelContext` and typed memory arguments are siblings
and carry the exact same nominal `Kernel`, `Target`, and `Launch` identities.
A memory argument also carries its own allocation, extent, access, alias, and
initialization identities. The compiler binds it to the context's brand at the
authenticated entry shim; context possession alone does not grant access to an
allocation. V1 deliberately has no public `KernelArguments` wrapper or value
that user code can construct.

One logical `KernelContext` is the execution root within that bundle. Grid,
workgroup, subgroup, invocation, LDS, synchronization, collective, matrix, and
target-operation capabilities derive from the context or from values already
derived from it. The macro supplies a unique nominal `Kernel` marker; a Rust
lifetime alone is not a kernel identity.

The capabilities are ordinary Rust types with private representations and
compiler-recognized semantic identities. Safe code cannot construct, copy,
clone, send, substitute, or extend their lifetime. A kernel context is a
logical compiler input. It contributes no caller-controlled bytes to the
physical kernel argument segment.

The user-facing shape is intentionally Rust rather than an IR-builder DSL:

```rust,ignore
#[kernel(typed, launch(required = [64, 1, 1]))]
pub fn reduce(
    context: KernelContext<'_>,
    input: Global<'_, f32, ReadOnly>,
    mut output: Global<'_, f32, DisjointWrite<Index1D>>,
) {
    let invocation = context.invocation();
    let index = invocation.index_1d();
    // ordinary Rust control flow and device operations
}
```

The concrete source spelling may evolve under the versioned device contract.
The invariants in this document may not. The example uses the current source
API; compiling it on a host or in isolation is not evidence that the complete
production authority path accepted it.

## Capability hierarchy

```text
#[kernel] logical function signature
  +-- context: KernelContext<'kernel, Kernel, Target, Launch>
  |     +-- Invocation<'kernel, Kernel, Target, Launch>
  |     |     +-- work-item coordinates and extents
  |     |     +-- global coordinates and extents
  |     |     `-- checked ownership/index witnesses
  |     +-- Grid<'kernel, Kernel, Target, Launch>
  |     +-- Workgroup<'kernel, Kernel, Target, Launch, Epoch>
  |     |     +-- workgroup barrier/fence/atomic authority
  |     |     `-- workgroup collective authority
  |     +-- Subgroup<'kernel, Kernel, Target, Launch, Width>
  |     |     +-- lane identity and active mask
  |     |     +-- subgroup collective authority
  |     |     `-- matrix-fragment authority
  |     `-- TargetCapabilities<Target>
  |           +-- numerical and instruction contracts
  |           +-- address-space and atomic contracts
  |           +-- matrix/collective/async-copy contracts
  |           `-- ABI, resource, and artifact requirements
  +-- memory argument 0: Global<'kernel, Kernel, Target, Launch, T, Access, Alias>
  +-- memory argument N: another compiler-issued memory capability
  +-- scalar argument data
  `-- launch-time dynamic precondition capabilities
```

Workgroup memory is obtained under a workgroup capability and shares its
workgroup and epoch identities, but externally supplied global arguments remain
bundle siblings. No safe conversion may erase or substitute
`Kernel`, `Target`, `Launch`, allocation, workgroup, or epoch identity.

Coordinate values may be copied after extraction. Values that authenticate the
current invocation, execution scope, allocation, epoch, target, or launch may
not. A checked integer remains data; it does not become authority merely by
matching a lane, workgroup, address, or target identifier.

The portable abstraction is `Subgroup`. `Wave`, wave32, wave64, and AMD
wavefront terminology belong to the AMD target adapter and target-gated AMD
APIs after exact target binding. A compatibility alias may aid migration, but
it cannot appear in generic source contracts or neutral KIR and cannot weaken
the subgroup-width requirement.

## Source and ABI rules

`#[kernel]` continues to authenticate one ordinary Rust function body. It may
generate metadata, a device entry shim, proof harnesses, and typed host code.
It must not translate the body into builder calls or maintain a second
executable implementation.

`KernelContext` is absent from the physical kernel argument ABI, and no
`KernelArguments` value exists in either ABI. The individual physical
memory/scalar arguments retain their ordinary kernarg representation. The macro
authenticates the logical signature and kernel marker; the frontend verifies
the exact compiler provider, type, and ignored ABI pass mode; and the production
importer materializes context acquisition and use as canonical execution
operations. The generated host interface never asks a caller to construct or
pass the logical context or its branded memory views.
ABI inspection must independently confirm exact kernarg size, offsets,
alignment, storage, descriptor metadata, and emitted LLVM parameters.

Every compiler-issued capability acquisition is an authenticated semantic
operation. If its source body is a trap stub, the importer must replace it
before executable lowering; optimization may not erase the acquisition before
authentication. A lookalike path, matching layout, helper name, caller-supplied
value, or ordinary constructor is not a provider. Encountering an unrecognized
provider, an unsupported operation, or a surviving trap stub is a compilation
error, never executable fallback.

## Memory capabilities

The safe target-neutral source contract exposes allocation constructors only
for `Global`, `Workgroup`, and `Private`. Canonical KIR has exactly five neutral
address spaces: `Private`, `Workgroup`, `Global`, `Constant`, and `Generic`.
`Constant` represents read-only device-visible memory; `Generic` represents a
pointer whose concrete address space is not statically known. Neither is a V1
safe source allocation constructor. Target binding must legalize `Generic` to
an exact space or reject it.

Across those source and KIR layers, the contracts distinguish:

- safe source address space: global, workgroup, or private;
- access: read-only, write-only, or read-write;
- alias authority: shared, exclusive, disjoint mapping, or explicitly unsafe;
- allocation and dynamic-extent identity;
- initialization state;
- execution scope and synchronization epoch; and
- element layout, alignment, and numerical representation.

Rust borrowing and typestate enforce local construction and use. They do not
prove that two GPU invocations derive different addresses. Output injectivity,
cross-invocation aliasing, initialized-before-read, and race freedom remain
whole-kernel obligations over the exact launch domain and canonical KIR.

Unsafe raw-pointer construction is an escape hatch, not silent authority. MIR
admission records its source scope and emits explicit provenance, extent,
alignment, access, alias, and lifetime obligations. A proof-required build
rejects if any required unsafe obligation remains unresolved.

## Synchronization and epochs

Source typestate may make local protocols clear, for example by consuming an
LDS staging state and returning a published state after a barrier. It cannot
prove that all work-items execute the same dynamic barrier. The production
compiler must establish, for the exact optimized graph:

- the participant execution domain;
- uniform arrival at each collective barrier;
- identical dynamic barrier order;
- loop trip-count and early-exit compatibility;
- acquire, release, and happens-before relationships for covered spaces;
- LDS initialization before reads;
- reuse ordering between epochs;
- atomic operation, scope, and ordering legality; and
- collective membership and active-lane requirements.

Dropping a phase token cannot make a divergent barrier safe. Conversely, a
valid barrier does not imply functional correctness or general deadlock
freedom. These properties remain separately named and evidenced.

## Target capabilities

The target-neutral layer reasons about semantic capabilities rather than
backend spellings. At minimum the versioned target model covers:

- execution hierarchy and supported subgroup widths;
- address spaces and memory-order scopes;
- atomic operations and types;
- barriers, fences, and collectives;
- matrix and tensor instruction contracts;
- asynchronous transfer and wait semantics;
- numerical behavior;
- ABI and calling convention;
- register, LDS, scratch, occupancy, and launch limits; and
- artifact and object-format requirements.

AMDGPU profiles such as `gfx942` and `gfx950` implement that model after exact
target binding. AMD opcodes, address-space numbers, target IDs, HSA metadata,
and code-object rules do not appear in neutral source capabilities or neutral
KIR. Target-specific operations remain explicit, capability-gated escape
hatches and fail closed on every other target.

A synthetic non-AMD conformance target tests architectural separation. It does
not claim a production non-AMD backend.

## Exact capability closure

For an exact kernel build, the compiler derives a
`CapabilityRequirementClosureV1`; no caller supplies or edits it. The closure is
the least fixed point of the versioned capability-dependency relation:

1. Seed it from every reachable monomorphized operation, type, effect, address
   space, unsafe obligation, logical and physical ABI fact, launch constraint,
   numerical policy, resource use, and target-specific escape operation.
2. Add every direct and transitive prerequisite declared by each seeded
   requirement, target query, legalization, lowering rule, object rule, and
   required property.
3. Repeat until no requirement is added; then sort and deduplicate by canonical
   requirement identity.

Each member records its reason, originating operation or ABI coordinate, source
coordinate when available, and dependency edges. The closure identity binds the
kernel marker, target profile, launch contract, source/semantic-MIR identity,
final optimized KIR identity and epoch, compiler policy, analysis policy,
target-model revision, and canonical member/dependency bytes. An identity match
without those rederived bytes is not sufficient.

The exact target adapter must answer every member. `Unsupported`,
`Incomplete`, `Unreviewed`, an omitted answer, an extra unrequested
legalization, or a dependency cycle outside the bounded schema rejects the
production build. A transformation that can affect the closure must carry a
checked preservation result or invalidate and rederive the closure and every
dependent analysis. Target legalization may add backend requirements only by a
versioned rule whose output is included in the final closure and revalidated
against the inspected artifact.

The compiler-derived closure is called *authoritative* only in the narrow sense
that it is the complete requirement input accepted by the existing protected
receipt path. The record itself grants no publication, load, or launch
authority. Caller capability sets, requested feature lists, target-advertised
bitsets, and `WorkerV3SafetyPropertiesV1` are comparison inputs or summaries;
they never replace the rederived closure or independently authorize anything.

## Canonical production flow

```text
authenticated Rust source and final monomorphized MIR
  -> unified semantic MIR owner
  -> canonical target-neutral mixed-SSA KIR
  -> fixed target-neutral optimization policy
  -> complete verification of that exact optimized graph
  -> exact target binding and target-specific optimization
  -> replay of every invalidated analysis
  -> frozen verified target-KIR snapshot
  -> LLVM lowering, object generation, and linking
  -> inspected artifact and descriptor
  -> owned refinement and capability receipts
  -> generated checked host preparation
  -> sealed production admission
  -> typed asynchronous completion
```

The mixed-SSA KIR owned by #271 is the only executable graph optimized and
verified. Ranked facts and proof inputs are immutable projections keyed to the
exact graph epoch. They are not independently editable programs. A mutation
either carries an independently checked preservation record or invalidates and
recomputes every affected result.

No stage selects behavior by kernel name, source text, tutorial identity,
recorded transcript, or exact-profile route. Unsupported behavior rejects
without a legacy, unoptimized, or workload-specific fallback.

## Responsibility matrix

| Invariant | Owning enforcement stage |
|---|---|
| Rust type, lifetime, local borrow, and local typestate validity | rustc and `fe2o3-device` |
| Kernel marker, provider identity, reachable calls, unsafe scopes, logical context ABI | `fe2o3-macros`, unified rustc frontend, and MIR admission |
| Operation typing, SSA/CFG, execution domain, address space, effects, and local legality | canonical KIR and Pliron dialect verifiers |
| Bounds, initialization, ownership injectivity, race freedom, uniformity, barrier convergence, epochs, and atomic legality | fixed production analysis pipeline over the exact graph |
| Functional and numerical refinement required by a profile | identity-bound compiler analysis and accepted proof receipts |
| Target support, legalization, and resources | target model, target binding, target verifier, and final artifact inspection |
| MIR-to-KIR relation | #106 refinement boundary |
| KIR-to-LLVM/ISA relation | #107/#214 applicable machine-refinement boundary |
| Semantic capsule contents and canonical content identity | #209 |
| Authentication of the compiler execution occurrence | #218 |
| Finalized publication, restart recovery, load envelope, and application handoff | #212 |
| Independent publication/currentness and rollback anchoring | #238 |
| Owned receipt consumption and authority promotion | sealed #213 verifier only |
| Dynamic dimensions, strides, extents, aliases, geometry, context, and resources | generated checked host preparation |
| Borrow and resource release after asynchronous dispatch | typed completion and runtime ownership |

A successful earlier row cannot promote a later row. In particular, type
safety is not race freedom, a CPU oracle is not a proof, source refinement is
not machine refinement, artifact inspection is not hardware correctness, and a
hardware run is not universal semantic evidence.

## Issue ownership and integration

#272 owns the capability-specific contracts: source capability shape,
requirement and result schemas, dependency closure, target query/answer adapter,
legalization association, diagnostics, and migration tests. It integrates those
records into mechanisms owned elsewhere; it does not fork, replace, or weaken
those mechanisms:

- #176 owns the unified semantic-MIR importer, #177/#134 own canonical KIR
  lowering and dialect integration, and #271 owns the single optimized and
  verified mixed-SSA graph and analysis invalidation.
- #106/#87 own source/MIR-to-KIR refinement, while #107/#214 own applicable
  KIR-to-LLVM/ISA and final-machine refinement.
- #180 owns generated host preparation and inspected-ABI agreement, #181 owns
  migration and removal of legacy/exact-profile routes, and #216 owns the
  deterministic simulator/runtime mechanism.
- #209 owns semantic capsule contents and identities. #218 owns authenticated
  compiler occurrence. #212 owns publication, recovery, load, and application
  handoff. #238 owns independent currentness. #213 alone owns the sealed join
  that may promote complete owned receipts.
- #175 owns overall pipeline convergence and #267 owns the community-release
  gate and public-repository parity.

An implementation issue continues to own its mechanism, codec, custody type,
and authority boundary. #272 may define a capability-specific payload or
adapter consumed by that mechanism, but a #272 record cannot stand in for an
owner-required receipt or bypass an owner-required check.

## Result and authority model

Three independent vocabularies must not be collapsed:

**Analysis disposition.** `CapabilityAnalysisDispositionV1` describes whether
one checker completed its bounded job on one exact input:

- `Clean`: the pass discharged its documented obligations for its exact input;
- `Rejected`: a bounded counterexample or violated invariant was found; or
- `Incomplete`: the operation, model, budget, solver result, or evidence was
  insufficient.

Both `Rejected` and `Incomplete` stop a proof-required production build.
`Clean` is authority-free and does not select a property status.

**Property status.** The existing `PropertyStatusV1` describes the exact kind
of evidence reported for one independently named property: `Proved`,
`Validated`, `Contracted`, `Checked`, or `Unsupported`. These variants have no
ordering and do not imply one another. A clean static analysis may support an
exact `Checked` record when the property contract allows it; it cannot silently
be relabeled `Validated` or `Proved`. `Unsupported` is a property-evidence
classification, not the same event as an analysis returning `Incomplete`.

**Artifact assurance and admission.** Artifact- or stage-local assurance names
what an exact producer record establishes for exact bytes and explicitly named
checks. It does not follow from either vocabulary above. Publication, load, and
launch admission arise only when #213 consumes the complete move-only receipt
set, exact final-artifact view, #218 occurrence evidence, #238 currentness, and
per-dispatch preconditions required by policy. No global assurance lattice is
implied.

Canonical capability results therefore enter the existing owned receipt
architecture with exact stage identities and are consumed only by the sealed
#213 verifier. No public boolean, caller-built report, caller-provided
capability set, `WorkerV3SafetyPropertiesV1` bitset, mutable `verified`
attribute, or matching digest grants compiler refinement, publication, load,
or launch authority. Worker safety bits may summarize a decision already owned
by the sealed path; they are never the source of that decision.

## Identity and versioning

The following names are reserved as the stable V1 schema identifiers. Their
identity-domain bytes are exact ASCII including the shown trailing `\0`.
Implementations may choose different Rust module/type names, but may not reuse
an identifier or domain for a different byte grammar.

| Contract | Stable schema identifier | Identity domain |
|---|---|---|
| Device capability vocabulary | `fe2o3.capability.device-api.v1` | not an identity-bearing record |
| Compiler-issued argument bundle | `fe2o3.capability.kernel-arguments.v1` | `FE2O3/CAPABILITY/KERNEL-ARGUMENTS/IDENTITY/V1\0` |
| One requirement and dependency edges | `fe2o3.capability.requirement.v1` | `FE2O3/CAPABILITY/REQUIREMENT/IDENTITY/V1\0` |
| Exact transitive requirement closure | `fe2o3.capability.requirement-closure.v1` | `FE2O3/CAPABILITY/REQUIREMENT-CLOSURE/IDENTITY/V1\0` |
| Analysis disposition/result | `fe2o3.capability.analysis-result.v1` | `FE2O3/CAPABILITY/ANALYSIS-RESULT/IDENTITY/V1\0` |
| Target query and answer | `fe2o3.capability.target-query-answer.v1` | `FE2O3/CAPABILITY/TARGET-QUERY-ANSWER/IDENTITY/V1\0` |
| Target legalization association | `fe2o3.capability.target-legalization.v1` | `FE2O3/CAPABILITY/TARGET-LEGALIZATION/IDENTITY/V1\0` |
| Static evidence association | `fe2o3.capability.static-evidence-association.v1` | `FE2O3/CAPABILITY/STATIC-EVIDENCE/IDENTITY/V1\0` |
| Per-dispatch dynamic evidence | `fe2o3.capability.dynamic-precondition-evidence.v1` | `FE2O3/CAPABILITY/DYNAMIC-PRECONDITION/IDENTITY/V1\0` |

The schemas version these concerns independently:

- source device API and diagnostic-item roster;
- logical kernel signature and launch contract;
- semantic MIR capability vocabulary;
- canonical neutral and target KIR schemas;
- analysis policy, obligation, checker, and result schemas;
- target capability and legalization models;
- MIR/KIR and machine-refinement receipts;
- compiler lineage and artifact associations; and
- generated host preparation and completion interfaces.

Canonical identities use owned, bounded records and explicit domain
separators. They never include process addresses, Pliron handles, traversal
order, debug formatting, or printer text. A schema change is side-by-side
unless its owning compatibility policy explicitly permits an in-place
extension. Decoders reject unknown required fields, duplicates, noncanonical
ordering, trailing bytes, oversized input, and cross-version substitution.

`ProductionSemanticCapsuleV3` and `SemanticCompilerModuleHandoffV3` are frozen
#209 contracts. #272 must not add fields, reinterpret padding/reserved values,
or change either V3 identity preimage. Compile-time capability evidence is
encoded separately as `fe2o3.capability.static-evidence-association.v1` and is
carried only through a new side-by-side outer schema or a separately versioned
opaque association mechanism explicitly admitted by #209/#212. Older decoders
continue to decode the exact old grammar; the protected route rejects a version
that cannot carry policy-required capability evidence rather than projecting or
falling back.

Static evidence binds the exact source/MIR, final optimized KIR and epoch,
capability closure, analyses, target answers/legalization, compiler policy,
artifact association, and applicable refinement receipts. Dispatch dimensions,
pointer values, concrete allocation extents/alias relationships, selected
device/context/stream, current publication occurrence, and other launch-time
facts are not compile-time capsule contents. Generated host preparation derives
a fresh `fe2o3.capability.dynamic-precondition-evidence.v1` for each dispatch,
binds it to the static association and exact artifact/entry, and passes it to
the #213 join. Dynamic evidence is never written back into or treated as a new
identity for the frozen compile-time V3 capsule.

## Diagnostics

The stable #272 diagnostic namespace is `FE2O3-CAP-*`:

| Code | Meaning |
|---|---|
| `FE2O3-CAP-AUTH001` | capability provider is unauthenticated, forged, caller-supplied, or a lookalike |
| `FE2O3-CAP-BRAND001` | kernel, target, launch, allocation, scope, lifetime, or epoch brand mismatch |
| `FE2O3-CAP-ABI001` | logical capability changed or disagrees with the physical ABI/artifact |
| `FE2O3-CAP-CLOSURE001` | requirement closure is missing, malformed, cyclic, stale, or not exact for the graph epoch |
| `FE2O3-CAP-TARGET001` | exact target reports a required capability unsupported |
| `FE2O3-CAP-TARGET002` | target support/legalization is incomplete, unreviewed, or unanswered |
| `FE2O3-CAP-ANALYSIS001` | analysis rejected a required property and has a bounded witness |
| `FE2O3-CAP-ANALYSIS002` | analysis could not complete the required property contract |
| `FE2O3-CAP-EVIDENCE001` | capability evidence is missing, stale, downgraded, omitted, or cross-boundary substituted |
| `FE2O3-CAP-DYNAMIC001` | a per-dispatch capability precondition failed before GPU side effects |

Existing subsystem diagnostics such as bounds, race, barrier, importer, ABI,
or artifact codes remain owned by those subsystems. A compiler may attach such
a code as the lower-level cause; it must not renumber it into this namespace.

Every capability diagnostic identifies:

- the kernel root and reachable helper call chain;
- the primary source span and relevant secondary span;
- the failed invariant and enforcement stage;
- whether the result is a counterexample, unsupported operation, or incomplete
  proof;
- launch, execution scope, target, and memory/epoch context when relevant; and
- a bounded witness when one is available.

Diagnostics must not say that a kernel is incorrect when analysis only failed
to prove it. Failed compilation produces no new descriptor, handoff, object,
HSACO, receipt, publication, or launch authority, and stale outputs from the
failed attempt cannot be selected.

## Milestone dependencies and status

This table is the #272 status snapshot at this document revision
(2026-09-06). All referenced issues are open. "Not complete" means the
end-to-end milestone contract is unmet even if component code or test fixtures
exist.

### Baseline kernel inventory

M0 pins the community tutorial corpus to
`config/tutorial-kernel-manifest-v1.json` in both `fe2o3` and
`fe2o3-kernels`, rather than to a prose kernel list. The manifest, schema, and
digest records must be byte-for-byte identical in both repositories. At this
revision the schema requires one record for each of the 47 tutorial kernels and
forbids promotion when any required capability, negative fixture, simulator
result, hardware result, or evidence join is missing.

- raw manifest SHA-256:
  `6216d17b801a841357da03e89cd93fc796d174e5419aef616c6e355f06283810`;
- canonical corpus SHA-256:
  `7d31c3e24315ddaec4138526c9cc21a52b32a5a6b91392dde4a371ce9eb0b99f`;
- schema SHA-256:
  `984d1637cb9b2eb76e9a2e3312c828172dabd91b686f34e3770c434795fb033a`;
- semantic expectation schema SHA-256:
  `7d907d17c594fbe2d344bdc54525de3bc2eba7e932350bb2c5fa5b27aa632638`;
- semantic qualification evidence schema SHA-256:
  `b5e598a3e1f280be27e9e47866bb4740e5a959e3c0e72025607fac72feea8c56`;
- 25 entries are classified `legacy-compiler-produced` and none is classified
  `compiler-produced` through the V1 capability path;
- every capability closure is `not-produced`, every capability production path
  is `legacy-only`, and every required capability-negative fixture is
  `missing`;
- three simulator commands and all 47 hardware commands describe legacy-only
  qualification; the remaining simulator commands are explicitly unavailable;
  and
- legacy fallback is forbidden for any future `compiler-produced` promotion.

The manifest validator rejects missing entries, noncanonical status values,
AMD terminology in neutral requirements, stale commands or evidence joins, and
unsupported promotion. These hashes are a baseline identity, not production
evidence. They must change as entries migrate and may be promoted only by the
M1-M7 gates below.

Compiler CI runs `python3 scripts/tutorial_kernel_manifest.py`. A coordinated
checkout also verifies repository parity with:

```console
python3 scripts/tutorial_kernel_manifest.py \
  --site-repository /path/to/fe2o3-kernels
```

| Milestone | Required integration dependencies | Status |
|---|---|---|
| M0: ADR and frozen contracts | #272 W0; current manifest baseline coordinated with #181; compatibility with #134/#175/#271 | In progress: normative ADR, canonical schemas, diagnostic taxonomy, TCB, migration policy, and the exact 47-kernel baseline inventory exist; the two-repository content/parity gate passes, while source/API freeze review remains pending |
| M1: minimal vecadd vertical | M0; #176 importer; #177 canonical lowering; #271 final graph/optimization; #180 host/ABI; #209/#212 carriage; #218 occurrence; #238 currentness; #213 sealed admission | Not complete; source/API drafts do not constitute a production vertical |
| M2: hierarchy and synchronization | M1; #271 exact-graph analyses/invalidation; #216 simulation; #272 W4 synchronization proofs | Not complete |
| M3: general memory and control flow | M1-M2; #176/#177 reachable generic helpers; #271 loop/memory/interprocedural transformations; #180 dynamic host checks | Not complete |
| M4: structured compute and targets | M3; #272 W5 neutral target model and adapters; #271 neutral/target-specific phase split; #107/#214 applicable machine refinement | Not complete |
| M5: advanced kernels | M2-M4; #181 migration; #216 simulator; target-matched hardware and applicable refinement gates | Not complete |
| M6: authority, migration, removal | M1-M5; #106/#87 source refinement; #107/#214 machine refinement; #209/#212/#218/#238/#213 authority chain; #181 legacy removal | Not complete |
| M7: documentation and release gate | M0-M6; #267 release/parity requirements and identical public-repository revision | Not complete |

Completing an upstream component narrows a dependency; it does not
automatically complete a #272 milestone. Each row also requires its positive,
compile-fail, hostile, simulator, artifact, and applicable target-hardware
acceptance matrix.

## Migration and qualification

Migration follows #181:

1. fill and vector kernels;
2. scalar control flow, loops, helpers, cross-crate generics, and multiple
   kernels;
3. global, private, and workgroup memory;
4. barriers, portable subgroups, target-specific AMD waves, collectives, and
   scoped atomics;
5. scalar and tiled GEMM;
6. reductions and softmax;
7. attention; and
8. MoE routing, expert computation, combine, and other advanced kernels.

The machine-readable kernel manifest records production classification,
capability closure, required properties, target matrix, simulator command,
hardware command, and negative-fixture coverage. Every current or future entry
classified as `compiler-produced` uses the one production transaction. An
unsupported entry remains explicit; it cannot select a legacy or exact-profile
fallback.

A capability counts as implemented only when ordinary attributed Rust reaches
the fixed production transaction, the exact final optimized graph is verified
and lowered, exact evidence reaches the existing sealed gate, the generated
host path is safe, and the applicable generic, hostile, simulator, artifact,
and target-matched hardware tests pass.

## Trusted boundary

The first production profile trusts the reviewed Rust compiler/toolchain
closure, fe2o3 compiler implementation outside mechanically checked
boundaries, accepted proof checker/tool binaries, LLVM and LLD outside exact
translation-validation coverage, the operating system and runtime interfaces,
the GPU driver/firmware/hardware, and protected deployment/currentness roots.

Each receipt states the narrower boundary it actually crosses. The project does
not claim that Verus proves rustc, Pliron, LLVM, LLD, the runtime, driver, or
GPU. The long-term direction is to reduce this set through independent
translation validation and formal refinement without changing the public
capability model.

## Deliberately deferred mechanism choices

The exact Rust method names and migration aliases, the internal trait shape of
target adapters, and the implementation technology of each bounded analysis
remain implementation choices. The static capability association's concrete
carrier also remains with #209/#212: it may be a new outer handoff version or
an owner-approved separately versioned opaque association. None of these
choices may change the brands, closure, authority split, frozen-V3 rule,
diagnostic identities, or fail-closed milestone gates above.

## Rejected alternatives

- Independent `current()` capabilities as the permanent public model: they
  fragment one invocation identity and make cross-capability substitution
  harder to state.
- Rust typestate as the complete synchronization proof: it cannot establish
  uniform dynamic participation across GPU invocations.
- A proof-only shadow graph: correspondence can drift after optimization and
  introduces a second editable program.
- An AMD-shaped neutral API: it makes later backends semantic emulations of
  AMD details rather than implementations of shared GPU concepts.
- CPU differential testing as compile-time equivalence: it tests selected
  inputs and remains valuable evidence, but does not prove all admitted
  executions.
- A clean-analysis boolean as launch permission: it is forgeable, loses stage
  custody, and bypasses the existing production authority architecture.
