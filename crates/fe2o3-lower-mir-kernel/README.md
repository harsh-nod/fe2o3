# fe2o3-lower-mir-kernel

`fe2o3-lower-mir-kernel` owns a bounded in-memory detached lowering core from
the feature-gated `mir` dialect shell to target-neutral
`kernel.algorithm_root` operations. One verified kernel algorithm root is
materialized for each supported MIR function, in source order. The raw Pliron
core is private to this crate.

The accepted source is deliberately narrow. A source must be one verified
`mir.module`; its direct children must be `mir.func` operations; and every CFG
block may contain only its canonical `mir.block` marker followed by
`mir.return`. All traversal is bounded before recursive Pliron verification.
Any unsupported operation, malformed structure, exhausted source bound,
unsupported rank, or exhausted rewrite bound is a terminal typed error. The
service has no fallback path and never reports a result after failure.

The public V1 conformance facade accepts only pointer-independent module and
function recipes. It creates, registers, validates, and destroys a private
Pliron context for each run. Successful results retain only the immutable
configuration and a pointer-independent observation of module identity,
function identity and ordinal, argument type references, canonical block
identifiers, and admitted MIR operation order. They expose no `Context`,
`Ptr<Operation>`, registration hook, target operation, or structural replay
admission. The record is not a durable MIR identity, equivalence proof,
artifact identity, or authorization.

Inside the crate, postcondition validation rechecks both live source evidence
and every emitted kernel operation before a pointer-independent observation can
escape. Raw results and registration markers are bound to a private,
context-owned `fe2o3-pliron` identity anchor. Equal arena slots, transplanted
markers, and erased handles fail closed with typed errors before foreign or
stale pointers can be dereferenced.

The crate does not choose a GPU or physical target and contains no AMDGCN,
COMGR, `pliron-llvm`, compiler, linker, artifact publication, loader, launcher,
tuning, proof-authority, runtime, filesystem, process-execution, or unsafe-code
surface in its own source. Pinned Pliron remains part of the memory-safety
trusted computing base.

This crate deliberately does not expose or implement Pliron's `Pass` trait.
The internal lowering core materializes detached operations outside the source
root, which is not a legal in-tree pass rewrite. Tests use the versioned
pointer-independent conformance facade; production compilation uses the
separate semantic KIR APIs exported by this crate.

## Production Source Launch Facts

`ProductionSourceLaunchRosterV1` checks the complete ordered kernel-root
association and exact required workgroups before ranked operations are built.
It retains the semantic module identity, full root bindings and source-derived
execution layouts, including finite grid extents and the existing dynamic
sentinels. Logical names need not equal export symbols. Root association uses
ordered sets, with O(R log R) comparisons and O(R) storage for R roots, plus the
cost of comparing logical-name bytes.

The backend adapts its validated `LaunchContract` into detached input fields and
retains the original contract. Semantic MIR supplies root identity and required
workgroups; it does not independently authenticate the supplied grid limits.
The roster grants no artifact or launch authority. A later stage must retain
the originating contracts and bind the aggregate semantic identity, not treat
a copied row as authentication of another module.

This factoring removes launch-layout computation from ranked construction.
The backend does not yet move executable KIR materialization before ranked
verification or replace ranked analyses with views of the optimized graph; those remain
part of [issue #271](https://github.com/harsh-nod/fe2o3/issues/271).

The `production_semantic_kir_v1` integration target covers source launch
agreement without ranked IR, including sparse roots with reachable helpers,
full-binding substitutions, per-root workgroup failures and retained identity.

## Internal Correspondence Replay

The existing correspondence wrapper always replays the same immutable source
SSA owner before entering its private structural checker. The private split
preserves all checks and error ordering, including for an owner with captured
occurrences. It adds no public skip-replay API, native-source activation, proof
authority, resource-accounting claim or reduction in replay work on this path.

## Library PreRanked Materialization

`ProductionPreRankedKirOwnerV1::try_materialize_with_budget` consumes the
admitted source SSA owner and complete source launch roster, performs the
existing legacy lowering and correspondence checks, then retains one immutable
connected V12 executable. Borrowed assertion-origin queries describe the actual
emitted condition use/definition and ordered success/failure edges, including
the existing assertion-elision rule. They do not prove the condition true.

`ProductionMaterializedRankedModuleReceiptV1` validates the complete ranked
root/layout roster using the existing replaying candidate checks. Consuming
`ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks` checks
the same retained graph against the ranked inputs without lowering it again.
The graph and canonical-byte buffers move into the existing owner. Its explicit
`verify_equivalence` reconstruction audit and both older Legacy constructors
remain unchanged in meaning; the older constructors expose no PreRanked views.

The phase-local ledger transfers the connected graph receipt G and retained
origin receipt A, restoring the incoming floor on ordinary Result return.
Reserve both before another allocation and retain those reservations while the
materialized owner or attached successor lives. Source MIR/SSA, launch rows,
legacy bytes/correspondence, translation checks and other lowering allocations
are excluded. An optional preexisting SSA occurrence-capture receipt C stays
separately caller-reserved through receipt validation and attachment; release it
only after the consumed source owner is dropped, including on failure. This
legacy path performs no new capture and promises neither full-memory accounting
nor unwind cleanup. Existing origin capacity-growth/top-up rules are retained.

This is a closed library materialize/query/attach component, not activation of
the rustc backend's pre-ranked route. It adds no NativeSource/native contract
emission, native scalar bindings, optimizer, target or host route. Origin custody
and connected structural admission do not grant source equivalence, formal
Complete, artifact or launch authority.

## Enum SSA Facts

Enum facts follow simultaneous block-argument assignment. Each edge reads
its incoming SSA snapshot before replacing target facts, so a stale loop-header
variant cannot overwrite a fresh backedge definition. Discriminator relations
are transported with their referenced enum values; overwritten or ambiguous
relations become unknown. This analysis does not replace runtime discriminants
with constants merely because a variant fact is available.

A dynamic downcast needs an authenticated variant edge for the exact source
local. Concrete payload storage alone is not that evidence. Shared switch
targets, including an otherwise edge, cannot identify a variant. Known
constructors and variant-refined SSA bindings retain their existing behavior;
unknown aliases and nested values require their own authentication.

Edge transfer and meet use the analyzer's bounded logical work and map-entry
accounting, including temporary reservations and failure cleanup. These are
not allocator-byte or whole-compiler peak-memory bounds. Component tests cover
changing and invariant loops, simultaneous transfers, ambiguous edges and
exact resource limits; admitted lowering tests preserve the runtime selector
and variant-guarded payload paths.

## Same-Type Slice Reborrows

An exact single-Dereference reborrow can preserve an ordinary slice reference
when the source local and result have the same semantic reference type,
SliceLength metadata, address space zero, and 64-bit pointer width. Shared
reborrows retain ReadOnly access; mutable reborrows retain ReadWrite access.
The lowerer checks and transports the same whole `Type::Slice` value, including
its data and length, through existing SSA definitions and block arguments.

This narrowly tightens admission: a thin `Borrow` result for an actual Slice
place is rejected rather than dropping its length. Non-Slice thin borrow rules
are unchanged. Raw address formation, fake borrows, extra projections, distinct
source/result reference types, and either access-changing direction remain
outside this rule. Mutable-to-shared narrowing requires a separate preserving
slice operation; it is not implemented by relabeling a carrier. This support
does not add capability elision, general Rust place coverage, or native scalar
proofs, and does not discharge pipeline, formal, or host correspondence gates.

## Internal Slice Helper LLVM ABI

Materialized internal helpers with bodies can pass immutable Global slices of
supported scalar elements as a data pointer and an i64 length. Definitions,
calls and block arguments use the same pair, including duplicate-edge joins
and loop backedges, on gfx942 and gfx950. Existing scalar and thin-pointer ABIs
are unchanged. Mutable or non-Global slices, foreign/declaration-only slice
parameters, and slice results remain outside this helper ABI.

This does not widen source helper effect admission. Helpers that read or write
element data still need independently admitted call summaries. Source aggregate
and capability-bearing results need their own semantic transport and effect
authorization; scalar KIR result vectors do not supply it. LLVM assembly checks
establish IR validity, not source refinement or hardware execution.

## Internal Scalar Result LLVM ABI

Defined `InternalHelper` functions can return 2 through 256 ordered, supported
scalar KIR results. One named LLVM struct per validated unique helper symbol is
shared by the definition, calls and returns; each component is extracted or
inserted in KIR signature order. The gfx942 and gfx950 model paths use this same
internal convention without changing existing void or single-scalar/pointer
results. Foreign exports and external declarations cannot use multiple results;
pointers, slices, Unit, vectors and capability-bearing aggregates are not admitted
as multi-result components.

This is an internal KIR-to-LLVM transport, not an external Rust FnABI or general
Rust aggregate support. It does not scatter retained aggregate destinations to
memory, widen helper purity or read/write admission, or establish source
correspondence, hardware execution or tutorial compile coverage. Those require
separate source and effect checks.

## Issued Wave Lane Reborrows

An already issued wave-lane binding can pass through an exact reference
Dereference without issuing another lane or loading memory. The source must be
a thin 64-bit reference in address space zero, with the exact projected pointee
registered as the existing 64-lane compiler-issued type. Both admitted reference
mutabilities preserve the same typed binding and numeric value; an ordinary U32
or a caller-chosen type cannot create that capability. Raw-address results remain
rejected, and other capability projection rules are unchanged.

This does not generalize reference-alias promotion. The tests cover reborrows,
issued lane-value Copy/Move and lane-value SSA joins, not arbitrary reference
alias joins. Existing storage classification, admission, operation limits and
later correspondence gates remain in force. The new predicate and projection
branch add no emitted operations or retained owner rows, and make no whole-work
or whole-memory accounting claim.

## Issued LDS Scope Reborrows

An already authenticated workgroup LDS scope binding also survives an exact
thin Reference Dereference in address space zero with 64-bit pointer width and
the same nominal pointee registered as WorkgroupLdsScope. Shared and mutable
references preserve the original scope binding without issuing another scope,
loading memory, or changing its SSA transport. Raw input pointers, raw-address
results, different pointees and other capability classes are not admitted by
this rule. The existing wave-lane checks and diagnostics are preserved.

The admitted-source regression uses the published PreRanked materializer and
an actual PipelineCreate consumer, with same-block and successor-block
reborrows. It does not require NativeSource or backend activation and does not
relax source admission, capability consumption, allocation geometry, resource
limits, or later ranked/formal/launch checks.

## Fixed Retained Arrays

Existing storage-observable locals may use counted private storage for nonempty
fixed arrays with exact rustc size, stride and alignment and modeled scalar or
thin-pointer elements. Partial writes do not establish whole-array initialization;
whole-value reads gather the current storage, and projected moves or deinitialization
conservatively invalidate it. Call destinations capture their element address before
argument evaluation. Array arguments, escaping array addresses, nested aggregates
and whole-array volatile accesses remain outside this storage rule.

Counted arrays are not covered by the scalar-slot effect exemption. Source bounds
control and later memory/correspondence checks retain their existing duties. This
does not add read-driven array storage selection, ranked array-transport evidence
or symbolic guard inference. Element admission uses the existing analysis extent
limit; each whole-array gather or scatter separately checks its three-operations-
per-element expansion against the existing total and per-block operation limits
before reserving a gather buffer. No limits are raised or whole-memory bound added.

## Private Array Correspondence

Lowering records actual retained-array counted allocations and indexed ordinary
memory effects with their original source roles, index definitions and physical
operation coordinates. Fixed borrowed PreRanked queries use the same owner's
selected body and sealed SSA plans. The boolean query returns false only for a
supported proven-unretained occurrence; the constant-index query returns None
for the same case. Missing required rows and unsupported relations are errors.
The returned index is inert, not an independently transferable proof.

The exact translation and retained-check discharge consumers compare the original
constant index, counted allocation, element layout/access and GEP with the actual
ranked private view and index. Ordinary materialized attachment uses the translation
consumer; the second consumer remains a separate retained-check discharge path.
The initial final rule covers ordinary constant-index writes only. Matched writes
still pass the existing duplicate, order and control checks, and every correlated
retained-array source row must be consumed. This is address/effect correspondence,
not stored-value equivalence or a census of unrelated unsourced private operations.
Read/mixed owners, initialization scatter, whole-value gather/scatter, dynamic or
lossy index transport, atomics and volatile accesses gain no final proof here.

The recorder activates only for selected retained arrays and shares a new monotone
checked R*M proof-work phase, where R is selected root count and M the existing
operation limit. Its subset census counts actual operations of array-bearing
function instances before shared-helper deduplication; it does not impose S<=M.
Failed or rolled-back proof work remains charged. Typed occupied/capacity
admissions and fallible geometric buffers are separate from G+A; inherited trace
payload, allocator capacity, relocation and RSS remain outside that receipt.
Queries use the caller's canonical ledger with the same owner's G+A reserved,
and final attachment retains its independent 64*M correlation phase. The index
query adds one fixed work unit before the unchanged boolean-query relation.
No configured limits, Native gates, formal or launch authority change.

These are correspondence components, not acceptance of the complete optimized
output pipeline. Default activation of the new checked optimizer policy and
complete optimized private-effect support require separate qualification,
including all tutorial compile gates.

## Checked Output Memory Analysis

`analyze_checked_output_formal_memory_v1` and the separately typed
`analyze_checked_output_formal_memory_policy3_v1` derive fresh memory obligations
from their actual checked V12 output and return reports borrowing those exact
owners. The Policy3 report retains the real eight-pass execution owner without
converting it into the historical owner. Both APIs use one complete-only formal
analysis implementation. Every actual kernel must have complete extraction and no inter-invocation
conflict under the existing structural witness: exact static extents, extent two
for dynamic active axes, and 64-bit indices. No old ranked or compiler discharge
is accepted, and the executable is not copied.

This is an analysis prerequisite, not final source/ranked admission. The formal
engine does not model Private accesses, its witness does not authenticate runtime
launches, and memory completeness does not exclude assertion traps. Its work,
scratch and obligation payload retain the existing formal-analysis allocation
policy and are not included in the canonical optimizer ledger or receipt.
Source-qualified output occurrence transport, final ranked/formal consumers,
and explicit V12 lineage/evidence migration remain required before activation.
Existing final owners, wire formats and authority gates are unchanged.

## Source-Qualified Checked Output Occurrences

The library checks complete V12 executable-coordinate preservation across the
exact existing AMD target-binding metadata delta, then reconnects sealed source
N to actual admitted B and independently checked optimizer O. It retains numeric
source block/control aliases and transported assertion outcomes. The codegen
connection that computes fresh sparse facts on O is not included in this library
publication. Failure selection is not successful assertion discharge,
and physical unreachable placement is distinct from checked omission.

This B0 connection does not activate a backend or admit a final executable. Its
source-qualified library queries do not replace the published codegen consumer.
The catalog and private-array placement prerequisites below do not complete
pipeline lifecycle/effect correspondence, final ranked/formal correlation, or
serialized final evidence. Old N-only assertion transport keeps its pointer
contract unchanged. Canonical equality of separately admitted B owners is
checked using complete bytes, never a digest alone.

`derive_source_output_occurrences_policy3_v1` connects the actual checked
Policy3 output to the same source/block/assertion/catalog/private-array queries.
It and the existing `derive_source_output_occurrences_v1` share the exact
custody and replay checks; neither accepts a receipt in place of a live checked
owner. The private-array ranked receipt also has an explicitly typed
`with_checked_private_array_output_policy3_v1` scope. It compares the complete
closed Store subset with the actual output, including stored-value uses, under
the original Work identity and storage floor. Scope records cannot escape the
callback. The actual tagged borrowed-owner headers are included in their
existing storage accounting.

These occurrence/report APIs do not themselves attach final checks or authorize
default Policy3 output.
Unit-local helper source/output replay and unsupported private reads/effects
remain refused; complete final source/control/memory/refinement validation is
still required. Target-binding tests use the real binder and exact metadata
checker for both profiles, but are semantic/ranked component tests, not rustc
source, protected artifact, or GPU qualification.

New indexes, queries, and temporary payloads share the caller canonical ledger.
The caller retains N graph/source rows, B, checked O/history, and transfer receipts;
the constructor checks only the stated N-plus-O floor lower bound. Source replay
uses its inherited source limits outside that ledger. Row receipts describe
logical requested payload under the existing allocation convention, not portable
allocator capacity, RSS, or a whole-compiler memory bound.

## Closed Policy3 Output Admission

`ProductionCheckedOutputOwnerPolicy3V1::try_admit` consumes the materialized
ranked receipt, exact B and actual checked Policy3 O. Its first admitted subset
is deliberately closed: root-only modules, unsigned integer/boolean scalar
inputs and locals, Unit returns, one returning source/KIR block per root, and
total scalar recipes. The ranked projection may contain a finite unconditional
chain covering every block, including its synthetic entry block. A bounded
linear walk checks that chain independently of stored block order. Source/KIR
branches, assertions, calls, borrowed interfaces, private addresses, memory
effects and ordered execution are refused, including in unused bodies.
Source/ranked/N/B/O censuses and exact endpoint checks precede fresh O obligations;
all allocation/access/bounds/alias/conflict rows must be empty.

The owner retains historical N separately, and `output()` exposes only O. It
cannot replace the legacy formal owner or be constructed from a borrowed formal
report. Revalidation reruns the closed checks and fresh obligations. New census
work uses the shared ledger; existing source reconstruction/ranked attachment,
formal scratch/results and wrapper bookkeeping remain outside its canonical
payload accounting. Input reservations are caller-owned and must outlive the
consuming owner, including failed transfers.

The backend has an inert descriptor consumer joining this owner with exact
target metadata, typed source ownership/ABI, retained launch fields and O
geometry. The default backend route and legacy V8/V9/V11 lineage are unchanged.
General final admission, V12/Policy3 lineage, ordinary-source tutorial coverage,
protected artifacts and hardware qualification remain separate gates. This is
an M5/M7 prerequisite, not completion of either milestone or formal verification
of the compiler.

## Guarded Global Output Admission

`ProductionCheckedOutputOwnerPolicy3V1::try_admit_general_v1` consumes the same
real materialized source/ranked receipt, target-bound B, and checked Policy3 O.
It adds a separate, privately tagged admission contract; the older scalar
constructor keeps its original restrictions. Revalidation dispatches by that
private tag, not by a caller-supplied mode or a fallback.

The new grammar includes signed/unsigned scalars, booleans, indices, F32/F64,
global scalar slices/pointers, ordinary global loads/stores and supported guarded
accesses. It admits branches, switches and source-authorized assertion
failure traps. Exact source/ranked replay, N/B coordinates, independent B/O
transition checking and complete source-block/assertion transport are required.
A retained conditional assertion still executes at runtime; it is not a
statically discharged bounds check. Existing ranked progress checks still apply;
this admission adds no blanket acyclic requirement or general loop-carried
address support. A supported source-loop positive through final admission remains
to be qualified; an unconditional nonterminating source loop is refused before
the required ranked receipt can be constructed.

Memory obligations are freshly derived from actual O for every kernel. Only
the existing structural guarded-access exception can be resolved by checking
the actual output predicate, address and slice extent. All other incomplete
reasons fail closed. A full physical-access census matches each output access
to its fresh formal location, kind, address space and alignment. Runtime bounds
and alias requirements remain obligations, not authenticated launch values.

Fixed-cell private scalar storage has a separate complete allocation, address,
use, initialization and source-lifetime census. Each retained read requires a
same-block latest store to its exact cell. Allocation alignment and constant
byte offsets must justify access alignment. Pointer escape, transported private
pointers, dynamic cells, unknown element widths and unsupported projected source
destinations are refused. The global formal-memory report omits private accesses
and is never used as their safety proof. The lifetime census builds one sparse
source invalidation index and uses logarithmic interval queries. Private-array
correspondence still uses bounded per-read source effect/statement scans; this
is not general cross-block memory dataflow.

Helper interfaces, collectives, atomics, volatile or ordered effects, unsupported
address provenance and unsupported arithmetic remain outside this contract.
The source/ranked and formal engines retain their
existing bounded accounting domains; new inventory, transport and census
scratch uses the shared ledger and restores its incoming floor.

### Integer Arithmetic Admission

General checked-output admission accepts ordinary signed/unsigned 8-, 16-,
32-, and 64-bit add/subtract/multiply only through the source materializer's
existing checked pair. Result zero is the modular numeric value; result one
is the overflow flag. Source/N and N/B correspondence and the independently
checked optimizer transitions must preserve those exact operands, types and
result roles. An explicit source overflow assertion is still a runtime
assertion. Plain native integer arithmetic, unchecked MIR operations and
128-bit source arithmetic are not admitted by this extension.

Unsigned 32-/64-bit division and remainder have a separate, graph-bound
nonzero-divisor check. It runs independently on actual B, C, and final Policy4
O using the retained source target. A previous graph's success flag cannot
authorize a changed operation. Signed division, floating division and narrow
unsigned division remain outside this contract.

The checker recognizes nonzero literals, exact equality/inequality guards,
integer-switch edges and supported zero-preserving casts. Its backward search
tracks a block and an exact SSA definition. It substitutes each predecessor
edge's actual block arguments before testing that edge's guard, so duplicate
edges and loop-carried values cannot borrow another value's proof. All reachable
incoming paths must establish nonzero; the function entry has an explicit
unknown-input seed even when it has a backedge. A repeated state only bounds
work and never establishes a fact by itself.

Work and vector capacities use the shared admission ledger. With no division
candidates, only the operation bitmap is retained. For dynamic candidates the
search uses at most the largest per-function block-count times definition-count
workspace, not a product across unrelated functions; it is bounded, not a
claim of globally sparse complexity. The enclosing transaction restores its
incoming storage floor on both success and failure.

This is admission of partial arithmetic at its actual execution site, not
permission to hoist, speculate, forward across, or classify division as pure
and total. Ranked semantic-expression definedness and backend source projection
remain independent gates. Component tests do not establish whole-corpus,
default-pipeline, protected-proof, or hardware qualification.

## Checked Policy4 Composition

`ProductionCheckedOutputOwnerPolicy4V1` consumes the real ranked/source receipt,
target-bound B and the optimizer's sealed Policy4 owner. Policy4 runs the unchanged
eight-pass Policy3 B/C sequence, then checked private store-to-load forwarding
C/O. It retains both intermediate C and final O. The Policy3 occurrence map
describes B/C only; the independent forwarding relation describes C/O. No map or
proof is relabeled as a different endpoint.

Admission qualifies source/B/C, independently replays the fixed composition,
then derives fresh private and global-memory facts from actual O. Forwarding is
limited to ordinary same-block integer memory with exact pointer SSA identity
and access attributes. It retains the store and replaces the load with an
equivalent value expression; unsupported effects stop forwarding. Separate GEP
values are not assumed to be the same pointer. A genuine semantic-source fixture
requires one real forwarding row, but does not claim that ordinary Rust retained
arrays necessarily trigger that exact-pointer rule.

The backend's move-only checked-output stage consumes the Policy4 owner, lowers
actual O through native V12 LLVM lowering, constructs its descriptor/handoff and
independently replays the final LLVM and descriptor relation. It retains the
authenticated source bindings, ranked roster and complete transferred logical
storage floor. Its test-only rustc callback compiles ordinary `fill` and `vecadd`
source through this stage without adding a shipping policy selector.

Native source replay reconstructs semantic MIR, SSA and N through normal
constructors, compares the complete N bytes and checks the exact source catalog.
It does not infer launch origin from equality. The consuming backend additionally
joins the complete ordered retained ranked texts and requires real aggregate
proof execution; missing signatures do not become unsigned success. The verifier
rehashes the aggregate obligation from its claims and complete ordered staging
commitments and compares it with the imported signature's binding. This binds
the exact Middle identity, contracts, generated source and staging order; merely
comparing enclosing claims would not. Embedded keys establish signature
consistency, not trusted signer origin. The compatibility commitment-only API
still explicitly does not reconstruct ranked/source correspondence.

The consuming native backend additionally exports every typed ranked recipe and
its access/effect source maps from the retained source owner, and transports the
actual per-effect signed receipt bytes and keys from local proof execution.
The stronger verifier independently imports those receipts, reruns staged ranked
compilation, recreates exact V5 evidence, and derives semantic/parallel contracts
and the aggregate proof source with the same pure preparation used by execution.
It then consumes the fresh results into the reconstructed source/N owner and
replays correspondence. Missing typed roots or signed effect receipts fail;
there is no commitment-only fallback. This typed in-process handoff is not yet
an autonomous artifact codec. Existing translation checks still exclude complete
indexed-address and whole operational equivalence. Reconstructed correspondence
does not establish final-output proof provenance or protected compiler origin.

New transport vectors, maps and names use the canonical work/storage ledger;
legacy ranked compilation, semantic reconstruction and proof-source generation
retain their own bounded resource domains. This is not complete allocator/RSS
accounting. Public test-key consistency fixtures do not certify Verus execution.

The default backend still uses its existing policy. Protected native lineage and
full corpus admission are still required before default activation; this stage
does not relabel old proof receipts or publish an artifact.

## Source-Qualified Pipeline Catalogs

The B1 library extension constructs pipeline contracts from the actual sealed
source declarations and replay-checked create-call spans. It binds the resulting
catalog to the same admitted B inventory, transports it through the independently
checked B/O transition, and checks the fresh catalog against actual O. Exact
source coverage comes from the preceding full source replay, not from a catalog
digest or the isolated graph catalog checker.

The catalog covers pipeline allocation layout/geometry and, when actual graph
markers exist, their contract, storage alias and epoch type. Ordinary pre-ranked
materialization emits the pipeline allocation and Wait barrier, but no V12 event
markers. Its source-qualified catalog therefore has zero marker placements;
this is not complete source-event transport. The isolated graph checker may
accept missing bindings when no actual markers require them; source catalog
completeness instead relies on the sealed constructor and full source replay.
The catalog does not establish lifecycle safety, payload
Load/Store value correspondence, ordinary Global parameter ancestry, generated
scalar/enum-storage exemptions, call timing, final ranked/formal admission, or
NativeSource authority. Those remain separate consumers; no backend is activated.

Source scratch, input catalog/check, transported output catalog/audit and retained
view share the same caller canonical ledger. Existing catalog logical row,
comparison and swap charges are preserved; borrowed getters charge one dispatch
and do not prepay caller traversal. Receipts describe requested typed payload,
not allocator capacity or RSS. The included lowerer constructor tests exercise
empty source/output catalogs and their header/payload accounting. The nonempty
source-pipeline, both-target catalog/getter, and six-marker graph-component tests
remain in the held codegen integration and are not part of this publication.
Nonempty source-catalog coverage therefore remains a publication test gap; empty
catalog success is not source-event or lifecycle coverage. No included test
establishes a whole-constructor resource bound or completed source-to-final proof.

### Checked Output Private-Array Writes

The source/output occurrence view can retain the five checked operand uses for
an ordinary retained-array write: allocation count, GEP base/offset, and Store
pointer/value. Original source type, role, unsigned index definition, whole-value
initialization policy, and N allocation/span correlation remain checked at N by
the unchanged source query. Output checks use actual checked O coordinates and
use-specific definition ancestry, not N ordinals or an invented N-shaped graph.
The original unsigned source definition may have no O descendant.

Required missing output uses refuse; a checked-unreachable omitted write is
distinct from a proven-unretained source access. A physically retained write
also carries independent reachability. The codegen adapter that consumes these
outcomes is not included here; the published projector is unchanged.
Other retained memory forms remain unsupported. These inert queries do not
activate the backend or establish final ranked/formal, lifecycle, artifact, or
launch authority.

All additional capture, sort, lookup and physical checks use the same caller
canonical ledger. One fixed-size numeric row is fallibly reserved per original
effect while the B/O inventories and checked control index are still live. Its
payload and the added view headers remain reserved with the existing view;
failure drops temporary rows before the enclosing constructor restores its
incoming storage floor. Legacy N query and 64*M correlation allowances do not
pay for added O work. Existing source replay/trace and allocator/RSS exclusions
are unchanged; fresh-vector requested payload is a logical storage receipt,
not a whole-process memory guarantee.

This library-only packet adds 21 Rust tests: four coordinate checks, two actual
target-binder checks, eight optimized source-assertion tests, one independently
checked retained-unreachable assertion component, and six checked-output array
query/header tests. The array tests use genuinely admitted source and actual
fixed optimization, including sparse coordinates and Boolean-selected omission.
They do not cover a physically retained unreachable Store query or final ranked
attachment. The held codegen source/output session and its tests are excluded.
