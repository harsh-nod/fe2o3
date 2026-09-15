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
The backend now materializes the legacy executable before ranked verification,
then borrows that same graph for canonical assertion facts and attaches checked
ranked custody without lowering it again. Replacing ranked analyses with views
of an optimized graph remains part of
[issue #271](https://github.com/harsh-nod/fe2o3/issues/271).

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

The default rustc production transaction and its existing simulation/verification
entries now use this materialize/query/attach component. They retain the same
authenticated source/launch/reference bindings and all later checks. Canonical
assertion analysis has its own phase-local inventory/sparse ledger with G+A
reserved; source/projector work remains separately limited and excluded. Earlier
materialization failures can now precede ranked diagnostics that were formerly
reported first. This adds no NativeSource/native contract emission, native scalar
bindings, optimizer, or alternative target/host route.
Origin custody and connected structural admission do not grant source equivalence,
formal Complete, artifact or launch authority.

The default ranked producer and roster receipt recheck derive existing V2 `u32`
induction facts for the exact selected body using that source owner's retained
SSA plan. Unreachable source statements do not participate in those facts. The
receipt compares the complete rederived report, including producer-only SSA
scope work; source-only canonical evidence replay remains separate. This does
not make an absent assertion true, relax final effect correspondence, or add
authority. Existing analysis limits and non-induction proof checks are unchanged.

Codegen introduces a private fixed policy for these two separate canonical phases:
18,014,398,509,481,984 logical work units and 2,147,483,648 logical storage bytes
per fresh phase budget. These are new codegen phase-policy constants, not an
inherited optimizer policy or a serialized V4 API. They preserve the exact values
selected for this transaction; work conversion to the host ledger remains checked.
Library callers still supply their own budgets. The limits neither measure
allocator overhead/RSS nor bound all compiler work or memory, and do not activate
an optimizer.

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

Materialization records actual retained-array counted allocations and indexed
ordinary memory effects with their source roles, original index definitions and
physical operation coordinates. A fixed borrowed PreRanked query uses the same
owner's sealed selected-body and SSA plans. It returns false only for a supported
proven-unretained occurrence; missing required rows and unsupported index evidence
are errors. Codegen preserves private source rows only after this query succeeds.

The exact translation and retained-check discharge consumers compare the original
constant index, counted
allocation, element layout/access and GEP with the actual ranked private view and
index. The initial final rule covers ordinary constant-index writes only. Matched
writes remain subject to duplicate, order and control checks, and every correlated
retained-array source row must be consumed. This is address/effect correspondence,
not stored-value equivalence or a census of unrelated unsourced private operations.
Read/mixed owners, initialization scatter, whole-value gather/scatter, dynamic or
lossy index transport, atomics and volatile accesses gain no final proof here.

The recorder activates only for selected retained arrays and shares a new monotone
checked R*M proof-work phase, where R is selected root count and M the existing
operation limit. Its subset census counts actual operations of array-bearing
function instances before shared-helper deduplication; it does not impose S<=M.
Failed or rolled-back proof work remains charged. New typed occupied/capacity
admissions and fallible geometric buffers are separate from G+A; inherited trace
payload, allocator capacity, relocation and RSS remain outside that receipt.
Queries use the caller's canonical ledger with the same owner's G+A reserved,
and final attachment retains its independent 64*M correlation phase. New proof-work
exhaustion is explicit; no configured limits, Native gates, formal or launch
authority change.

## Checked Output Memory Analysis

`analyze_checked_output_formal_memory_v1` derives fresh memory obligations from
the actual fixed-rule checked V12 output and returns a report borrowing that
owner. Every actual kernel must have complete extraction and no inter-invocation
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
source block/control aliases and transported assertion outcomes; fresh sparse
facts are computed on O. Failure selection is not successful assertion discharge,
and physical unreachable placement is distinct from checked omission.

This B0 connection does not activate a backend or admit a final executable. Its
private codegen assertion entry explicitly refuses transformed private-memory
queries until checked operand/allocation transport is connected. Native pipeline
catalog transport, final ranked/formal correlation, and serialized final evidence
remain separate obligations; old N-only assertion transport keeps its pointer
contract unchanged. Canonical equality of separately admitted B owners is checked
using complete bytes, never a digest alone.

New indexes, queries, and temporary payloads share the caller canonical ledger.
The caller retains N graph/source rows, B, checked O/history, and transfer receipts;
the constructor checks only the stated N-plus-O floor lower bound. Source replay
uses its inherited source limits outside that ledger. Row receipts describe
logical requested payload under the existing allocation convention, not portable
allocator capacity, RSS, or a whole-compiler memory bound.

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
not allocator capacity or RSS. The source tests cover six actual event calls,
the ordinary allocation/barrier/no-marker census before and after real target
binding and checked optimization on both AMD profiles, hostile allocation
catalogs, graph-only limitations, nonauthority and an isolated four/three getter
boundary. A separate structurally admitted V12 graph component preserves all six
physical marker placements and missing-binding rejection through the actual
checked optimizer. That component has no semantic source owner and is not
NativeSource admission. Neither scope establishes a whole-constructor resource
bound or completed source-to-final proof.

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
also carries independent reachability. The private projector adapter refuses
either non-executable outcome instead of converting it into a live index hint.
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

### Checked Output Ordinary Global Accesses

The unactivated source/output view can retain ordinary nonvolatile Global
Load/Store occurrences from an exact kernel-entry source span. Each row keeps
its source root, body, statement and original access ordinal separately from
the actual checked O operation, pointer/value uses and Load result. The actual
O function-argument definition must descend from the original N parameter;
only that original parameter value enters N's existing source/component lookup.
The checked B/O operand and definition relations preserve intermediate pointer
ancestry. No O value is installed in N tables and no O span is guessed.

Mixed private/global spans, other memory effects, source/generated calls,
non-entry bodies and unsupported allocation ancestry return explicit refusal
states. Unreachable omission is distinct from physical retention and is not a
proof discharge. These gaps must be closed before an all-kernel consumer can be
activated. This library query grants no ranked/formal, lifecycle or launch
authority and changes no default backend route.

The existing external-allocation pointer worklist is shared through two narrow
adapters. Its SliceData/GEP/Cast/Select and incoming-block-argument rules and
traversal order are unchanged. Legacy correlation retains its original visited
set and one charge per nonempty pop; its source ordinals and private-storage
exemptions are untouched. New capture uses the caller canonical ledger, not
that old allowance, and prepays every new traversal and buffer.

For D original definitions and E incoming edge arguments, new scratch is built
once lazily per view constructor: 2D+E numeric cells and a 2D+E+1 value worklist.
Generation marks avoid whole-definition resets between accesses, while direct
incoming links avoid rescanning all edges at each phi. Per-access pointer traversal is
bounded by O((D+E) log D); source-binding scans and checked descendant-range
binary searches are additional charged work. Source aliases use the existing
sorted origin index. Two borrowed passes count only eligible Global effects,
so private-only and memory-free kernels reserve no Global effect-row buffer.
Source-span sorting and binary queries remain separately charged. Both retained
Global Vec headers are prepaid before their buffers become live and subtracted
from the later full view-header reservation; cached storage includes the full
retained payload. Existing enclosing cleanup restores the entry storage floor
after failure without refunding work/history. Receipts count requested typed
payload, not allocator capacity, a hard RSS bound or source replay allocations.

### Checked Source Successor Selection

The inert source/output view can map an independently checked Boolean input
edge to the exact replayed source successor occurrence. It checks the actual
N conditional branch, Boolean source type, singleton explicit 0/1 target and
both source target identities. Explicit/otherwise ordinals remain distinct
even when their target block IDs are equal. No checked selection means no
selection fact, not proof that every source edge is executable.

Selected assertions, generated splits and other unsupported shapes are refused;
assertions retain their separate success/failure transport and proof rules.
The query reuses existing source/control rows, allocates no storage and charges
every lookup plus fixed mapping work on the caller ledger. This prerequisite
does not skip source effects, prune source CFG edges, change the real root
projector, or grant effect/assertion/final-executable authority.
