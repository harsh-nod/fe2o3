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

This is a lowerer-only component. The default rustc backend still projects before
materializing executable KIR and does not yet use these same-owner private-array
queries. Its materialize-before-project transition and common-projector private
effect support require separate qualification, including all tutorial compile
gates; this library change does not activate that route.

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
