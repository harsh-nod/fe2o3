# fe2o3-pliron

`fe2o3-pliron` is the issue #134 D0 boundary around the pinned Pliron
workspace. It provides:

- construction of a real Pliron `Context`;
- opaque process-local context identities backed by Pliron's private uniqued
  store rather than transferable auxiliary marker data;
- opaque operation handles whose upstream pointers remain in a private
  session registry;
- exact graph snapshots, mutation epochs, and session-owned analysis caches;
- byte- and tree-guarded textual operation import that requires exact
  end-of-input and recursive verification before returning an owner handle;
- owner-scoped dialect-registration services with bounded typed actions;
- deterministic, bounded pass plans over real Pliron `Pass` values;
- a typed canonical-KIR bridge for executable `gpu.*` SSA; and
- a sealed optimizer executor for the fixed fe2o3 pass vocabulary; and
- the fixed production analysis pipeline, with no public raw-graph entry point.

The context identity and typed dialect-registration primitives are implemented
once in the lower-level `fe2o3-pliron-owner-core` crate. Admitted dialect
adapters depend on that core rather than this full session shell. This crate
depends on the core and publicly re-exports its existing API for downstream
compatibility.

The dependency is pinned to reviewed Pliron v0.17.0 fork commit
`7ebf6e6638c2a3bcec179423993b01211a9689b4`. It is a strict descendant of
upstream v0.17.0 commit `2610651306ea3ba670f68d5d8b1e1159bcd521ed` and adds
private context provenance on upstream pointers and the mutation-attempt epoch
used by the production analysis boundary.

The fork also exposes a nonallocating byte-length query for live SSA value
names. It reads borrowed debug metadata and checks length arithmetic, allowing
analysis preflight to measure names without constructing diagnostic strings.
The query is linear in defining arity plus attribute count; it provides no
semantic proof or resource-admission authority by itself.

The fork also provides opt-in rewrite observers and occurrence notifications for
CFG merging, unreachable-block removal, and dead block-argument removal. The
existing pass entry points and scheduling listeners remain available. Builtin
function DCE preserves entry arguments and removes each dead internal argument
once, including when duplicate uses enqueue it repeatedly. These observations
are not semantic proofs; consumers must check the resulting graph independently.
The `pliron-derive` dependency used by `pliron` is sourced from that same Git
workspace revision and retains the reviewed source-tree identity. This crate
does not construct or lower LLVM operations. The workspace pins `pliron-llvm`
at the same revision for the isolated dialect smoke crate, and the first #144
slice defines a Pliron-independent canonical LLVM handoff.

The target integration permits `pliron-llvm` only for transient `llvm.*`
dialect construction, transformation, and verification.
Ordinary compiler crates must use `default-features = false` so the optional
`llvm-sys` dependency is not linked into their processes. fe2o3-owned bounded
canonical records, identities, receipts, and evidence form the finalizer
handoff; Pliron handles, printer text, and diagnostics do not. The isolated
pinned upstream LLVM 22.1.8 target machine and in-process LLD remain the sole
machine-code and HSACO authority.

## Immutable Canonical Analysis Scope

`with_canonical_analysis_scope_v1` borrows one connected V12 owner, derives its
inventory once, and lazily caches sparse scalar facts and conservative
single-partition MemorySSA independently on first request. It
cannot replace the owner or reuse facts by matching hashes. Mutation requires
ending the scope and deriving fresh analyses for the changed graph. Returned
values cannot borrow the local inventory or cached report.

The caller reserves the graph's retained payload. Inventory and cached-report
payloads remain reserved on the shared ledger until dropped; success, Result
errors and unwinding restore the incoming floor without rewinding work, peak
storage, or failure history. Each cache request prepays six custody/lookup checks;
the first retained report also pays two transfer checks. Replacing the Work
ledger or losing any live floor poisons both caches, even when a consumer catches
the error. Foreign storage is not released and missing storage is not recreated.
Callback scratch must be dropped before returning or unwinding; output retained
by callers must be reserved before scope entry. Stack-only framing and unrelated
caller allocations remain outside this logical-payload contract. Fixed checks
cannot observe violations completely repaired inside a callback.
This API is not a pass, a preservation proof, or production-pipeline activation.

## Checked Occurrence Execution

The additive connected V12 bridge borrows an already admitted canonical owner.
Its one-shot neutral execution lease observes the existing literal seven-pass
scalar/CFG roster and extracts the actual successor with complete operand,
successor, edge-argument, block-chain, and value-descendant candidate rows.
The independent kernel-analysis checker must accept those rows against both
actual graph inventories before consuming checked-output custody is returned.
The old value map and optimizer report are not semantic rule oracles.

These checks establish only the named input-to-final local rewrite rules.
They do not seal every intermediate pass, supply a general equivalence theorem,
verify source/ranked memory, or grant artifact or launch authority. The checked
owner retains one executable output and historical input bytes. Optional owned
origin callbacks cannot replace either checked graph or bypass the checker.

Input, session growth, observed output, inventories, rows, and returned owners
have distinct logical storage transfers on the shared work/storage ledger.
Drop restores the appropriate floor without refunding accepted work or failure
history. Capture, rule, resource, extraction, or panic failures discard the
candidate; there is no fallback to an unchecked map or source graph. The native
bridge profile bounds structural/signature work and is not NativeSource compiler
proof admission. Allocator overhead and upstream primitive execution costs are
not claimed as exact measurements.

This library surface does not activate default backend optimization or change
the existing target optimizer's pass/report policy. Source-origin transport,
final optimized-graph mandatory checks, and compiler publication remain separate
integration obligations.

## Boundary

This crate does not define fe2o3 dialect operations, select a production
compiler, publish artifacts, grant proof or launch authority, or use COMGR. It
does execute a closed, owner-authenticated optimization plan selected by
`fe2o3-kernel-opt`; it never accepts an arbitrary caller-provided pass.
Pliron pointers, arena identities, printer text, and diagnostics are never
used as durable canonical fe2o3 identities. A session-local presentation digest
detects graph drift; its replay projection is diagnostic optimizer metadata,
not artifact, proof, or launch authority.

The shell bounds registration count before collecting caller input, each
dialect hook to 128 typed registration actions, pass-plan count, names, and
diagnostics. A registration service accepts only types, attributes, and
operations owned by its assigned dialect namespace. Its private context and
dialect fields cannot be extracted or retained by safe callers. Operation
creation returns an opaque handle containing only a process-local owner
identity and session-local registry ID.
Textual operation import is a noncanonical construction bridge for dialect
integration. Input bytes, parser text, and printer output cannot become
artifact, cache, proof, publication, or runtime identities. The bridge bounds
input byte length and the delimiter syntax understood by the pinned, audited
parser set before parsing, then bounds the complete returned operation tree.
Successful import bytes are charged monotonically to the session because
interned parser data can outlive an erased operation. A parse or verification
rejection poisons the session because upstream arena allocation is not
transactional.

Every linked or registered Pliron `Parsable` implementation is trusted code at
this transitional boundary. The source and tree guards do not meter arbitrary
parser CPU time, temporary allocation, interning, comments, literals, or
private delimiter syntax. A deployment that adds a parser must audit or
process-contain it independently; the public registration action limit does
not make parser execution bounded. Consequently this API makes no resource
containment claim for an open third-party parser registry.
This bridge is transitional: a production compiler path that depends on a
printer/text round trip remains unsupported until it uses an owner-held typed
dialect construction service.
The corresponding upstream `Ptr<Operation>` remains in the private session
registry. Every query or erase authenticates the context anchor, owner, live
registry entry, and upstream pointee in that order. Erasure removes the
registry entry, so cloned stale handles cannot be revived by later arena
allocation. Handle identities and their debug representations are not
canonical data.

The crate does not execute a generic pass plan. Invoking an arbitrary
caller-provided Pliron `Pass` would give that pass a raw pointer and
`&mut Context`. `PlironOptimizationPlanV1` instead accepts only the closed
fe2o3 pass enum, authenticates the root, bounds structural work, recursively
verifies every changed checkpoint, and poisons the private session on failure.
Hook and upstream diagnostic text is not copied into stable diagnostics; the
shell emits fixed fe2o3 codes and messages instead. Hook and upstream unwinds
remain contained by the session-construction boundary.

Context identities protect fe2o3-owned envelopes and results from being
validated against a different context, including when public Pliron auxiliary
marker boxes are moved between contexts. The pinned upstream `Ptr<T>` also
retains private context provenance; equal arena slots from different contexts
do not compare equal. fe2o3 keeps those pointers inside the trusted compiler
boundary and exposes only owner-scoped handles. `ContextIdentity` intentionally hides its numeric
debug value but remains only process-local in-memory provenance; equality or
hashing must never become an artifact, cache, proof, publication, or runtime
identity.

## Production semantic SSA resource envelope

Production semantic SSA applies two independent deterministic resource
policies. Every function first remains bounded by the unchanged planner caps:
262,144 variables, 262,144 blocks, 65,536 edges, 1,048,576 events, 65,536 edge
definitions, 1,048,576 output items, 2,097,152 logical storage words, and
67,108,864 work units. Passing that check does not authorize a module to retain
an unbounded number of individually valid plans.

The cumulative module summary is separately bounded by a fixed compiler
envelope of 1,048,576 variables, 1,048,576 blocks, 262,144 edges, 4,194,304
events, 262,144 edge definitions, 4,194,304 output items, 8,388,608 logical
storage words, and 268,435,456 work units. These constants are four times the
per-function ceilings to retain a small bounded set of substantial plans. The
factor is not multiplied by function count, is not selected by a compilation
request, and is not an accommodation for any particular source program.
Checked accumulation rejects arithmetic overflow before comparing the summary
with the fixed module envelope. A stricter validated module policy may lower
these ceilings, but no public limit value can exceed them.

Optional source-to-SSA occurrence capture performs one full replay on the same
move-only SSA owner. Its sealed ordered rows preserve original event and edge
occurrences, constants as source locators, and actual elision/entry categories.
Only borrowed descriptive views escape; these rows do not prove scalar,
lifecycle, output, or formal equivalence. Ordinary construction/replay does not
allocate capture arrays, and native materialization is not activated by this API.

Capture uses a separate logical byte/work ledger for its count/fill/join actions,
not for inherited source checks, classification, planning, hashing, or allocator
overhead. Fresh fixed-row Vecs use one prepaid exact reservation and never grow
or convert to boxes. Exact capacity relies on pinned nightly-2026-04-03
RawVec/Global behavior; a mismatch fails without retrospective top-up, and a
toolchain change requires re-audit. Success transfers a receipt at the caller
entry floor; errors/unwind drop pending rows before restoring that floor.
An already captured owner is rejected without work or ledger changes.

## Closed generic kernel-check production path

The implementation lives in private `src/production_analysis/` modules. The
public surface exports reports and model types, not functions accepting raw
`Context`, `FuncOp`, or operation pointers. `fe2o3-kernel-analysis` retains the
Pliron-independent optimizer analyses and Presburger math; the live-IR
Presburger adapter lives beside the other production analyses here.

`compile_ranked_kernel_for_lowering_v1` is the single closed owner path for the
target-neutral ranked-memory schema. It admits only bounded data recipes,
constructs the module and function inside `ProductionPlironSessionV1`, performs
recursive Pliron verification, and consumes a `ConstructedGraphStageV1`
through the fixed tensor-layout, memory-bounds, atomic-legality, race-freedom,
hierarchical-ownership, barrier-convergence, pipeline-protocol,
workgroup-memory, and semantic-refinement passes. Atomic accesses must
retain explicit ordering and scope, and cannot pass without a bounded matching
target capability; system scope additionally remains incomplete until
coherent-allocation provenance is authenticated. The complete fixed sequence
produces one private `KernelChecksVerifiedGraphStageV1`; there are no public or internal
per-pass stage transitions that could be reordered or partially consumed. Only
that aggregate transition can create the move-only
`ProductionRankedKernelLoweringInputV1`; an empty module, a foreign root, a
same-session substituted root, a changed graph, or any rejected report cannot
be relabeled as verified.

The aggregate transition binds its reports to the exact owner, root, graph
epoch, and canonical ranked recipe. Lowering preparation independently runs
the checks again and compares the retained reports and resource receipts.
It reserves the first run's retained storage and the report-comparison cost
before admitting the second run. Ordinary optimizer passes begin and commit
checked graph mutations; changed graphs advance the epoch and invalidate
cached analyses, while verified no-op passes preserve them.

A fresh scoped dominance tree avoids repeated tree construction during one
production structural capture. Same-block dominance uses the operation-order
index retained from def-use closure, after checking the exact physical block and
operation order. Query work and the index's storage overlap with counting and
verification are prepaid. The index belongs to the exact context and root and is
consumed before final diagnostics and canonical maps are constructed.

The resource receipts cover the implemented phase policies, not a complete
worst-case proof for every upstream operation. Generic verifier and dominance-tree
allocation, along with other retained hash-table costs, still need resource
closure. These checks do not establish formal compiler verification. Native
Switch and vector memory operations are not admitted by this migration.

Concrete pipeline lifecycles may span an acyclic CFG when all normal paths
consume the same exact Create, event and access occurrences in order. A shared
control view validates actual successor ordinals, including repeated edges;
per-pipeline cursors check joins and normal returns without enumerating paths.
Empty diamonds and reordered block storage are supported. Normal bypasses,
distinct branch-local event copies, unreachable lifecycle sites and reachable
cycles are rejected. A trap may precede the lifecycle or follow Create alone,
but cannot discharge a partially executed lifecycle. Existing same-block and
recognized dynamic-loop routes retain their separate semantics.

This check retains the existing constant epoch/slot and initialized-coordinate
requirements. It does not infer values from ambiguous edge arguments, establish
new barrier convergence facts, or grant source/output refinement authority.
The fixed resource preflight includes the cached CFG and one pipeline's scratch;
standalone diagnostics reserve the same bound before discovery. These remain
logical phase bounds, not allocator/RSS or formal execution-cost proofs.

Concrete epoch/slot queries also use the existing function-bound sparse cache,
including constant affine remainders with nonzero divisors and block arguments
whose incoming facts agree. Unknown values, conflicting joins, zero divisors
and machine overflow remain unproven. Literal-only and recognized dynamic
routes do not initialize this cache on demand. Query work is prepaid; a missing
cache is admitted once with its peak overlapping live pipeline scratch, and
admission or analysis failure returns `AnalysisIncomplete`. Current nested
barrier/workgroup callers already prepare the cache before entering this check;
future callers with additional live scratch must account for that overlap too.
This adds neither a recursive evaluator nor new refinement authority.

Concrete initialized-coordinate checks compare a read only with earlier writes
in the same epoch, using the existing budgeted structural index-equivalence
checker. Separately emitted equal constants and supported equivalent expressions
can match; unequal or unproved coordinate pairs cannot. Borrowed write rows retain
source order, making resource-sensitive comparison order deterministic. The
existing access-pair query bound covers this route; dynamic-loop matching and
its separate rules are unchanged.

Dynamic staging and consuming windows also retain coordinate occurrences in
source order, so shared equivalence-memo warming and exhaustion are deterministic.
Matching remains bidirectional set membership: duplicate access counts need not
agree, but every coordinate must have an equivalent counterpart. Actual access
counters and lifecycle/empty-window checks are unchanged, and the existing
access-pair resource bound includes all occurrences without deduplication.
Matching stops at shared equivalence-budget exhaustion; a later identical or
memoized candidate cannot revive it, and the protocol report remains incomplete.

Cross-block regressions also run through the closed public entry point:

```sh
cargo test -p fe2o3-pliron --test production_static_pipeline_cfg
```

The relocated textual suites are under `src/production_analysis/tests/lit/`
and `src/production_analysis/tests/protocol-lit/`. Run them with:

```sh
cargo test -p fe2o3-pliron --lib textual_pliron_lit_suite
cargo test -p fe2o3-pliron --lib pliron_protocol_lit
```

The current `compile_ranked_kernel_for_lowering_v1` entry has no authenticated
target-context owner, so an atomic recipe deliberately stops as `Incomplete`.
The target-aware analysis entry is non-authoritative test/compiler plumbing;
production may bind it only when the retained target and allocation owners can
be consumed by the same closed stage transition.

The output transitively owns the exact session graph while exposing no raw
pointer, and it grants no source correspondence, compiler refinement, artifact,
or launch authority. The rustc production route projects its admitted semantic
MIR into this path. Projection remains deliberately fail-closed for semantic
constructs that do not yet have exact ranked-IR summaries, and the detached
kernel-to-GPU service does not yet lower every ranked operation. Future lowering
must consume this owner-bound result rather than reconstructing or bypassing the
checked graph.

## Remaining trusted surfaces

`DialectRegistrationHook` no longer receives `&mut Context`; all eight current
dialect adapters depend only on `fe2o3-pliron-owner-core` and use its
`DialectRegistrationService`. Direct context access still exists at these
integration boundaries:

- `ensure_context_identity` and `require_context_identity` accept a caller-held
  Pliron context so existing owner-aware envelopes and detached services can
  authenticate their raw upstream handles.
- Cross-crate conformance uses owner-scoped facades. There is no
  `with_context_mut` method or feature that restores raw session access.
- Dialect crates retain legacy `register_dialect` or `register_mir_dialect`
  functions for direct-context lowering, bridge, compiler, and dialect-test
  callers that have not migrated to session-owned construction.
- Existing dialect builders, verifiers, and detached lowering services still
  accept caller-owned raw contexts and upstream pointers.

These are compiler-internal trusted-computing-base surfaces, not production
operation capabilities. The registration migration does not broaden them and
does not change the rule that ordinary session operation APIs return only
owner-authenticated handles.

## Upstream API findings

- Pliron constructs a real arena-owning context and automatically runs its
  linked context registrations. fe2o3 dialects are still registered through
  this crate's explicit hooks.
- `Dialect::register` is idempotent upstream. This shell preflights the complete
  registration list and rejects duplicate fe2o3 dialect declarations before
  constructing a context or invoking any hook.
- Generic shell plans remain metadata-only flat lists of leaf passes. The
  separate sealed optimization executor rejects hidden nested managers,
  authenticates its root, and constructs the fixed upstream passes internally.
- Pliron arena pointers and diagnostic display values are context-internal and
  are not suitable canonical identities. They are absent from manifests
  produced here.
- The v0.17.0 pass API has neither owner-aware operation handles nor a
  cooperative work or cancellation budget. fe2o3 now authenticates roots in
  its own session registry, but restoring execution still requires sealed pass
  access plus pass-work accounting or process containment.
- Upstream, hook, registration-input, and pointer-access
  unwinds are converted to typed errors under unwind-enabled builds. The
  registration action bound does not bound arbitrary computation inside a
  hook. As with all `catch_unwind` boundaries, `panic=abort`, allocator aborts,
  non-terminating code, and a panic in hostile destructor code cannot be
  converted into a Rust error.

The root workspace owns the exact Pliron revision so every dialect and lowering
crate resolves one audited upstream implementation. The selective
`pliron-llvm` dependency resolves that same revision and cannot broaden this
crate's authority boundary.

## Policy-verified functional-refinement receipts

V2 functional-refinement requests use an acyclic binding transition.
The validated ranked recipe first contains an unbound scalar or effect request
with supplied MIR subjects. `fe2o3-verifier` derives the proof program from that
DAG and returns a receipt verified under an explicit import policy. The consuming
`bind_functional_refinement_request_v2` method replaces only the addressed
unbound request. Production V2 compilation rejects unbound requests, V1
declarations, untrusted signers/toolchains, stale transcript hashes, duplicate
claims, missing proofs, and unused proofs.

The transcript includes formula definitions, view/allocation facts, the unique
correlated write, and ownership. A V2 receipt at the configured MIR boundary
materializes a retained `Proved` solver result only under the policy supplied to
this generic API. That result is not compiler-authenticated MIR custody and does
not itself authorize rustc production. The compiler must privately join it to
retained rustc MIR identities and a compiler-owned policy before making a
same-session refinement claim.

Verus proves the compiler-derived effect formulas conditional on the trusted
MIR-to-effect extractor and the exact numeric model encoded in those formulas.
This is not a proof of full MIR operational semantics, source-to-ISA
correspondence, artifact integrity, or launch behavior.
