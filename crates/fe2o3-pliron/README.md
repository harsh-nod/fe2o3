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
`9de42fc6ca7b8f3500ccf2346d69ebbb36e889cd`. It is a strict descendant of
upstream v0.17.0 commit `2610651306ea3ba670f68d5d8b1e1159bcd521ed` and adds
private context provenance on upstream pointers and the mutation-attempt epoch
used by the production analysis boundary.
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

The resource receipts cover the implemented phase policies, not a complete
worst-case proof for every upstream operation. In particular, generic verifier
allocation, retained hash-table behavior, and same-block ordering scans still
need resource closure. A fresh scoped dominance tree avoids repeated tree
construction during one production structural capture, but does not establish
formal compiler verification. Native Switch and vector memory operations are
not admitted by this migration.

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

### Typed read boundary

Ranked semantic load leaves now refer to one `SemanticTypedReadOp` immediately
after their original ranked access, rather than free scalar symbols. Repeated
uses share that SSA result; distinct source reads remain distinct. The producer
retains exact view, indices, allocation, scalar type and source volatility.
Construction uses a bounded dependency schedule and single-assignment local
slots, so a producer may be listed after its consumer's block. The resulting
graph retains the original block order, per-block operation order, numbered
locals, and source sites. Read address operands are checked at their original
access, not at the consumer. Native verification checks the completed CFG:
construction readiness is not dominance, and missing or cyclic producers reject.

Mandatory bounds admission now checks that each producer has an adjacent ordinary
read with exactly the same SSA view and ordered indices. Both volatility modes
retain the original access as the single memory event. Detached, guarded,
checked, atomic, or mismatched companions reject at the mandatory bounds gate.

This is memory-access correspondence, not memory stability or value equivalence.
Original access bounds and the remaining safety and semantic checks still apply;
live read-value semantics and volatile call-result binding remain unsupported
here. Read labels and expression commitments alone never establish CPU/GPU
equality or an error bound.

Scheduling and source-MIR regression tests establish construction behavior only.
They do not authenticate a Rust callback or prove final output equivalence. In
particular, some cross-block index bounds remain unproved and reject before the
semantic gate. Approximate math still requires explicit finite absolute and
relative limits, a checked input domain, and compiler-proved composition into
the final output error; construction does not relax that contract.

### Stored value boundary

`ValueAccess` and `AtomicValueAccess` now materialize their actual RHS as an SSA
operand on `kernel.access`. This includes typed scalar roots, tensor components,
and expressions derived from paired reads. The existing dependency scheduler,
native dominance verifier, owner snapshot, structural identity and input census
all observe that operand. Address analyses still see exactly one index per
dimension; a retained RHS does not add a second memory event.

The mandatory effect-refinement gate first correlates a contract with a unique
write by block, view and ordered indices. It then requires the contract's GPU
value to be that write's exact SSA operand. A missing RHS is incomplete; a
different RHS is rejected even when the expressions normalize identically.
Matching atomic writes can proceed through the existing proof checks only with
the required target capability. The public target-agnostic staging entry still
rejects atomics at the earlier atomic-legality gate. A matching
atomic read-modify-write update remains incomplete because its input does not
establish the final stored value. No RHS can disambiguate competing writes.

These are necessary custody checks, not new final-output theorems. They do not
prove physical store conversion, live read values, or CPU/GPU math-library
equivalence. Existing ownership, reference evidence and numerical requirements
remain mandatory. In particular, test agreement is not a compiler-proved error
bound and a clean effect report grants no artifact or launch authority.

### Conditional dynamic output coverage

The production ownership pass can now derive a conditional coverage record
from its existing verified graph snapshot. The supported slice is a rank-one,
acyclic prefix kernel with one output write, guarded read-only inputs, exact
invocation indexing and a retained nontrapping scalar value DAG. No additional
raw-graph entry point or independent structural verifier is introduced.

For `out[i] = a[i] + b[i]`, guarded by all three lengths, it records
`out.len <= a.len`, `out.len <= b.len`, and
`out.len <= actual_global_workitems`. The record retains the exact branch
conditions, graph identity and mutation epoch. Each ranked view still needs
a compiler-owned binding to the physical allocation extent; the launch needs
a binding to the actual dispatch. Ranked argument ordinals are not ABI indices.

These are outstanding conditions, not runtime observations or unconditional
ownership. The original ownership failure, findings and proved counts remain
unchanged. Bounds and race checks still run first. Unsupported control flow,
potentially trapping arithmetic and missing stored values fail closed. A
conditional record does not admit code generation or grant launch authority.

The intended numerical contract is separately explicit:
`abs(GPU - CPU) <= A + R * abs(CPU)` for finite nonnegative limits `A` and `R`
over a checked input domain. Both implementations' errors, rounding and
accumulation must compose into each final output; exceptional values require
an explicit policy or exclusion. There is no default tolerance. The production
`ErrorBounded` path is still unsupported: this coverage work does not prove
CPU/GPU `exp` implementations equivalent or supply a final-output error bound.

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
