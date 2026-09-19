# Pliron Optimizing Middle End V1

## Decision

Pliron is the mutable in-memory IR and pass framework for fe2o3's optimizing
middle end. Canonical Kernel IR remains the Pliron-independent interchange,
identity, replay, and proof-checkpoint format.

The two representations are not concurrent mutable authorities. An
optimization transaction starts from one verified canonical KIR snapshot,
constructs a fresh session-owned Pliron graph, mutates only that candidate,
and publishes a new canonical KIR snapshot only after checked export and both
Pliron and KIR verification succeed. Dropping the candidate session aborts the
transaction.

```text
verified canonical KIR
        |
        v
bounded typed import into a fresh Pliron session
        |
        v
closed Pliron pass pipeline over executable gpu.* SSA
        |
        v
recursive Pliron verification and bounded typed export
        |
        v
verified canonical KIR + transformation and coordinate receipts
```

`fe2o3-kernel-ir` does not depend on Pliron. The owner-aware bridge and pass
executor live on the Pliron side of that dependency boundary.

## IR ownership

- `mir.*` represents admitted Rust semantics.
- `kernel.*` represents structured algorithm and indexing semantics.
- `schedule.*` represents scheduling decisions.
- `tile.*` represents distributed tiles and layouts.
- `gpu.*` represents executable target-neutral SSA, control flow, memory, and
  synchronization.
- target-bound canonical KIR carries AMD-specific legalization; the
  `fe2o3-amdgcn-model` implementation lowers it directly to deterministic LLVM
  text. `dialect-amdgcn` is currently an API facade, not an `amdgcn.*` Pliron
  dialect, and there is no `llvm.*` dialect.
- canonical KIR snapshots bind stable identities and evidence between stages;
  they are not the mutable optimization data structure.

## Pass execution boundary

The Pliron-backed V2 optimizer accepts only a closed, versioned pass plan. It never accepts a
caller-provided `Pass`, callback, `Context`, or `Ptr<Operation>`. The executor
authenticates the session and root handle, constructs audited pinned-Pliron
passes internally, and returns no graph pointer or mutable context. New
production compilation invokes this exact pipeline through a fixed entry point
with no optimizer selector or fallback. The optimizer report binds the fixed
policy, pass accounting, mutation epochs, endpoint identities, and surviving
coordinates for independent replay validation.

Every pass records its stable kind, input and output graph work, change status,
and deterministic resource charge. A changed graph is recursively verified
before the next pass. Erased descendants invalidate all corresponding
owner-aware handles. A panic, invalid graph, accounting mismatch, or failed
post-mutation check poisons and discards the candidate session.

Initial generic pass order is:

```text
sccp
simplify-cfg
canonicalize
dce
pure-cse
dce
simplify-cfg
```

The executable Kernel IR optimizer V1 has been removed. Versioned V1 names in
the dialect and bridge identify internal data/API formats, not an alternate
production optimizer.

## Dialect legality interfaces

Executable dialect operations implement pinned Pliron interfaces for constant
folding, branch folding, and dead-code removal. Potentially trapping,
convergent, memory-accessing, and unknown operations are excluded by a closed
eligibility policy. An absent interface always weakens optimization.

The first CSE pass accepts only operations proven to be deterministic, pure,
total, non-convergent, and free of memory dependence. Loads become eligible
only after alias/effect analysis and memory versioning exist. Barriers, fences,
atomics, inline assembly, wave collectives, unknown calls, pointer arithmetic,
and potentially trapping operations are ineligible by default.

## Determinism and resource limits

Optimization uses stable traversal and pass order. Current limits count graph
structure inspected at each boundary, registered handles, pass count, and
canonical bytes. Wall-clock time is not an artifact-affecting input.

Pinned Pliron passes do not currently expose internal budget hooks. Their
inputs and outputs are hard-capped and recursively verified, and the initial
pass set is non-expanding, but structural accounting is not instruction-level
CPU metering. Expanding rewrites require explicit budget hooks before
production admission. Output graph caps are checked after every pass;
canonical-byte caps are checked on typed export.

## Implemented scope

The production V2 transaction gives scalar arithmetic, compares, casts,
selects, direct calls, slices, pointer arithmetic, loads, stores, branches,
conditional branches, and returns executable dialect operations. The remaining
verified KIR V10 operation and terminator families cross the bridge through
typed, effectful preservation carriers: their SSA operands, result types, and
CFG successors remain first-class while their exact versioned payload remains
owned by the private bridge transaction. This lets SCCP, CFG simplification,
select canonicalization, conservative same-block pure CSE, and DCE rewrite the
surrounding graph without treating opaque GPU effects as pure.

The O0 bridge is byte-exact. Optimized export binds input/output identities and
a deterministic digest of every surviving bridge coordinate. Unrecognized or
malformed graph nodes fail closed, and the live transaction never falls back
to a historical or unoptimized path.

Additional scalar transformations are implemented for the canonical migration
in [#271](https://github.com/harsh-nod/fe2o3/issues/271):

- [Dominance-aware CSE](../crates/dialect-gpu/src/dominance_cse_v1.rs) reuses
  Pliron's dominator tree to replace exact eligible expressions with a
  dominating equivalent. It leaves entry-unreachable blocks and non-SSA
  regions unchanged. This is not general algebraic GVN or memory CSE.
- [Integer identities](../crates/dialect-gpu/src/integer_identity_v1.rs)
  simplify a closed family of fixed-width integer neutral operands, including
  the value and false-overflow results of admitted checked operations.
  Floating-point reassociation is not part of this transform.
- Independent canonical transition checks validate the admitted replacements,
  including [integer result semantics](../crates/fe2o3-kernel-analysis/src/canonical_kir_transition_v1/integer_identities.rs).

These raw transforms do not select a production policy or grant publication
authority. They require a caller-owned budget and a private candidate that is
discarded after any transform or observer failure. Their new traversal and
temporary-storage accounting does not meter all upstream Pliron internals.
The default seven-step policy above is unchanged. The canonical migration also
has a separately typed policy-3 execution API; backend activation and complete
verification of the optimized production graph remain #271 work.

## Canonical policy-3 candidate

[`optimize_checked_canonical_kernel_ir_policy3_v1`](../crates/fe2o3-kernel-opt/src/checked_optimization_policy3_v1.rs)
imports one exact canonical input, executes the fixed schedule below, extracts
the actual output once, and independently checks the observed transformation:

```text
sccp -> simplify-cfg -> select-same -> dce -> local-pure-cse
     -> dominance-pure-cse -> dce -> simplify-cfg
```

The [execution witness](../crates/fe2o3-pliron/src/fixed_policy_v3.rs) records that
this specific schedule ran. It is distinct from the semantic transition check:
equal input/output bytes alone cannot prove that a particular policy executed.
The [policy-3 receipt](../crates/fe2o3-kernel-opt/src/checked_optimization_policy3_receipt_v1.rs)
requires both the sealed execution witness and the independently checked
input/output relation. Its owner cannot be converted to the historical
policy-2 owner, and plain receipt bytes do not construct execution custody.

Published receipt bytes also have a separate semantic-replay API,
`decode_and_check_published_policy3_semantic_relation_v1`. It borrows the exact
already admitted input/output owners and wire bytes, validates the fixed record
framing and policy roster, and independently checks the semantic transition.
Its `ReplayedPolicy3SemanticRelationV1` retains all three borrows. It does not
rerun the optimizer or construct a sealed execution witness.

The embedded `UnauthenticatedPolicy3ExecutionClaimV1` remains an untrusted
execution claim: well-framed altered dynamic counters can accompany a valid
semantic relation. A protected producer/publication join is still required to
authenticate execution provenance. Framing, endpoint digests and successful
semantic replay alone do not provide it. The local sealed-owner decoder keeps
its stricter exact execution-record comparison.

Replay uses the original caller ledger; borrowed owners/wire remain separately
caller-reserved. The returned storage receipt accounts for owned semantic rows
and the enclosing wrapper once, and must be reserved before further allocation.
This is not whole-compiler allocation accounting or final production admission.

The candidate uses the caller's existing resource ledger and is discarded on
failure. Integer-identity rewrites are not in this schedule. This API supplies
neither final source/memory/refinement admission nor artifact/launch authority;
it is not an alternate selectable production backend or a fallback route.

## Canonical policy-4 composition

[`optimize_checked_canonical_kernel_ir_policy4_v1`](../crates/fe2o3-kernel-opt/src/checked_optimization_policy4_v1.rs)
adds a new fixed composition without changing policy-3:

```text
B -> unchanged policy-3 -> C -> checked private store forwarding -> O
```

Its move-only owner retains C's execution/occurrence history, final O and the
independently checked C/O rows. The B/C occurrence map cannot describe a replaced
load in O: its operand structure has changed. The two exact relations are
composed instead. The separate policy-4 execution record binds B/C/O and the
forwarding count. Published semantic replay still cannot mint execution custody.

The forwarding rule uses same-block ordinary private integer stores/loads with
identical pointer SSA values and memory attributes. It neither removes stores
nor treats numeric address equality as provenance. Volatile, unknown, trapping,
ordered and convergent boundaries are not crossed. This is not general MemorySSA,
alias-driven load CSE or dead-store elimination.

The [nondefault checked-output backend stage](../crates/rustc-codegen-fe2o3/src/production_pipeline_checked_output_policy4_v1.rs)
consumes the real source/ranked receipt, admits B/C, replays C/O, derives fresh O
memory facts and lowers actual O to native LLVM. It retains full source bindings
and the ranked roster. The default seven-step pipeline remains unchanged until
native protected provenance, complete admission and tutorial qualification pass.
This composition does not complete #271's expanded scalar, loop, memory or GPU
optimization milestones.

Its native source handoff retains full signed per-effect receipts, not just
staging hashes. Independent replay reconstructs semantic MIR/SSA/N, recompiles
the complete typed ranked roster, recreates exact V5 evidence, derives both
contracts and the aggregate generated source, and consumes the fresh ranked
results into the reconstructed source owner. The original and reconstructed
owners remain distinct custody objects. Embedded keys establish consistency
only; indexed-address/whole-operational equivalence, protected compiler origin,
final-output provenance and default activation are still separate requirements.
The in-process typed handoff is not a serialized artifact recipe format.

### Fixed policy-5 load continuation

The separate [policy-5 owner](../crates/fe2o3-kernel-opt/src/checked_optimization_policy5_v1.rs)
retains the entire unchanged policy-4 prefix and adds one independently checked
same-block private load-to-load continuation:

```text
B -> policy-3 -> C -> policy-4 store forwarding -> S -> load forwarding -> O
```

Its closed rule requires an initialized, aligned, nonescaping, direct private
scalar allocation, fixed signed/unsigned 8/16/32/64-bit values, identical access
attributes and the actual MemorySSA incoming version. The first load remains;
only a later eligible load becomes a value identity. The complete use census
rejects pointer escape, projected/array storage and pointer parameters. Other
loads and unknown, ordered, trapping or convergent effects clear the seed.
An unrelated verified Global Store may preserve initialization but clears the
load seed; storing the private pointer globally fails the nonescape census.
No operation is made pure and no load is speculated or reordered.

Distinct [direct](../crates/rustc-codegen-fe2o3/src/production_pipeline_checked_output_policy5_v1.rs)
and [silent-Unit-erased](../crates/rustc-codegen-fe2o3/src/production_pipeline_erased_checked_output_policy5_v1.rs)
backend consumers retain original source/N, optional separately checked E,
B/C/S/O, the ranked roster and collector bindings. Final admission checks the
full policy-4 prefix, independently checks S/O and derives fresh actual-O
formal obligations. Shared descriptor and worker checks lower actual O while
preserving original source/launch identity. Old policy-3/4 schedules, records,
receipts and public errors are unchanged. The new owning native handoff cannot
manufacture signed source proof or protected publication rights.

Coverage includes genuine semantic-source/normal-projector fixtures, exact
before/after native load counts, both AMD profiles, hostile root/native joins
and resource boundaries. It is not ordinary-Rust nonidentity qualification:
ordinary scalar locals usually promote to SSA, while reference-taking and
array/GEP retention remain outside this initial rule. No tutorial optimization,
default activation, protected runtime or hardware execution is claimed. The
existing ordinary-source corpus gate below still targets policy-4.

### Ordinary-source corpus gate

The ignored `ordinary_tutorial_corpus_requires_every_checked_policy4_output`
test reads every configuration in the tutorial manifest, preserving target,
Cargo features, lockfile/source hashes and the exact expected kernel roster.
Cargo builds dependencies with `--offline --locked`; the captured root rustc
arguments, environment and working directory then drive the real compiler
callback. Each case must reach checked Policy4 output with LLVM and a matching
descriptor. A dependency failure or compiler refusal fails the all-case gate;
neither is counted as a pass or silently removed from coverage.

With the pinned nightly and its required rustc components installed, run:

```sh
cargo test --offline --locked --no-default-features \
  -p rustc-codegen-fe2o3 --lib \
  ordinary_tutorial_corpus_requires_every_checked_policy4_output \
  -- --ignored --nocapture
```

Set `FE2O3_TEST_CHECKED_OUTPUT_CORPUS_REPORT_V1` to an absolute output filename
to retain the structured per-case report, including all refusals. The harness
cleans its temporary dependency and subprocess directories. This gate checks
the nondefault source-to-LLVM route; it does not execute Verus or kernels,
publish compiler artifacts, or establish simulator, numerical or GPU results.
The separate `ordinary_rust_fill_and_vecadd_reach_checked_native_output` parent
also exercises the missing signed-proof refusal without releasing output.
Current observations count runtime-read domains from the fresh actual-O reports
and check their inert V4/policy-3 encoding. Historical reports without this
optional observation stay unobserved, not retroactively counted as zero.

The corpus prints an external `callback-progress.json` path before each child
invocation. Its atomically replaced snapshot identifies the active compiler
phase and records monotonic durations for completed calls; the final report
retains these timings before temporary files are removed. Policy4 additionally
records bounded nested phases for target binding/B admission, the fixed optimizer,
ranked/source replay, final admission and actual-O artifact preparation. The
Direct and silent-Unit Erased routes retain their actual phase orders; the
Erased ranked/source phase includes E production and checked N/E replay.
Active subphases are published before work, with completed, refused or panicked
outcomes afterward. Historical JSON without these optional rows stays
unobserved. This locates the running API boundary, not a proof of which internal
B/C/O assertion failed. Instrumentation I/O failures are diagnostic-only and cannot
change the compiler result or the all-case acceptance gate. Captured jobserver
variables are removed only when replaying the standalone child, whose inherited
file descriptors are no longer valid.

For a focused diagnostic run, set `FE2O3_TEST_CHECKED_OUTPUT_ENDPOINTS_V1` to an
existing fresh directory outside the checkout. The test harness retains actual
B/C/O canonical bytes, full graph dumps and identity metadata there before
admission. Each case is capped at 30 MiB plus 16 KiB of metadata; incomplete
endpoints are marked unavailable, not emitted as truncated complete graphs.
Graph capture is off by default and adds I/O to Policy4 timing when enabled.
Nested optimizer timing ends before snapshot I/O. Setup, receipt adoption and
diagnostic overhead remain in the enclosing Policy4 duration, so nested times
need not sum to that duration. Observation is test-only and does not select a
pass policy or change canonical work/storage limits.
These snapshots are diagnostics, not proof receipts or artifact authority, and
do not by themselves identify which endpoint's formal check failed.

Ranked read projection groups source occurrences once, preserving exact source
ordinals, duplicate rows and per-statement cardinality. Matching costs
`O(S + R log R)` for `S` source rows and `R` eligible reads instead of repeated
whole-roster scans. The index retains `O(R)` storage and can have a higher peak
than one old per-statement temporary vector. This is an algorithmic bound, not
a measured end-to-end speedup or a new canonical resource-accounting claim.

The GPU expression resolver also indexes statement-definition positions by local
and block. A reaching-definition prefix lookup becomes logarithmic, while the
existing logical statement-visit debit, first failure, dominance fallback and
cycle handling remain unchanged. Other assertion-proof consumers keep the
original scan. The sparse index adds storage proportional to definition events
and an explicit allocation-failure boundary; it does not cache expression trees
or proof results and has no measured whole-corpus speedup claim.

## Runtime slice reads

Fresh formal-memory extraction can bound an ordinary nonvolatile global read
whose runtime Index value is not affine. The actual pointer must be exactly
`GEP(SliceData(formal_slice), index)`, and a successful `index < SliceLength`
edge for that same slice and value must dominate the actual load. Existing
all-incoming unique-origin analysis can transport block arguments; ambiguous
or changing loop origins, false-edge complements and stale pre-arithmetic
guards do not establish this relation. The rule applies only to the existing
64-bit Index model and scalar element widths of 1, 2, 4 or 8 bytes.

This is a distinct read-only access domain, not an affine-address theorem.
Its byte expression stays unbounded and its alias coverage is the whole formal
allocation. Unsupported reachable accesses keep extraction incomplete; complete
reports retain the required access coverage and conservative read/write conflicts.
The rule does not admit writes, atomics, volatile or
explicit `GuardedLoad` operations, helper call contexts, or arbitrary pointer
chains. Already-supported affine and guarded accesses keep their existing rows.
Sparse paid guard indexing avoids scanning the entire CFG for every read; work
also includes the exact local producer-result lookup and bounded origin queries.

The additive inert formal-memory receipt V4 uses extraction policy 3. The V4
facade retains V1/V2/V3 bytes and identities; the older codecs reject the new
domain. Outer formal-memory evidence policy 3 accepts only V4/policy 3, the
current 64-bit analysis basis and exact nonzero invocation witness. Decoding
numeric source coordinates does not authenticate a graph or make an incomplete
extraction complete. Live consumers still require current-owner equivalence,
complete conflict-free extraction and exact graph/root/witness binding. These
changes do not activate the candidate optimizer as the default pipeline or
establish full-corpus, protected-proof, artifact or hardware qualification.

## Local helper call coverage

`ProductionPreRankedKirOwnerV1::with_checked_unit_local_ranked_stage_v1`
checks the complete original-N ranked root roster and every retained local-helper
call. It joins the same-owner source relation, actual canonical call occurrence,
root-qualified helper association and unchanged entry translation. Repeated calls
and shared helper bodies keep distinct source-root/call coordinates. Private
allocations, loads and stores remain in the graph and are not classified as empty
effects.

The backend invokes this scoped check after authenticated ranked-roster replay,
before the existing materialized-receipt constructor. The callback retains no
approval token or executable owner. The historical RawEmpty route is unchanged;
UnitLocal still encounters `LocalHelperSourceConsumerUnavailable` at that
materialized ranked-receipt boundary. This stage alone does not establish helper
termination, call purity, or optimized B/C/O private-memory safety.

A separate checked silent-Unit route retains original source/N and constructs a
distinct erased graph E before target-bound B, checked Policy3 C and Policy4 O:

```text
original source -> N -- exact silent-Unit deletion --> E -> B -> C -> O
                  |                                  |         |
                  +-- retained source/ranked custody -+---------+
```

The deletion rule requires a no-argument, non-unwinding direct Rust helper with
an actual Unit return. It freshly checks the same-owner source relation and
initialized, bounded private-memory chain, a closed total operation grammar,
acyclic selected control and exactly one Unit return. Assertions must have an
independently known successful outcome; only their exact inactive trap sinks
may remain outside that path. The admitted source values are Unit and the
existing Bool/unsigned 8/16/32/64-bit scalar recipes, with the corresponding
64-bit Index representation. Nested or external calls, recursion, floating-point
or partial arithmetic, volatile/atomic effects, escaping memory and arbitrary
zero-sized return types do not satisfy this rule. It is not a target stack-frame
feasibility theorem or general local-helper termination analysis.

N retains all original private allocations, loads, stores and call occurrences.
The exact N/E checker permits deletion only of the certified zero-operand,
zero-result calls and their certified local helper bodies after checking every
remaining reference. Other functions, operation order, CFG edges and arguments,
kernel metadata and root effects must survive unchanged. Shared physical helpers
are removed once while their source-root associations remain distinct. Calls are
not reclassified as pure or `CompleteEmpty`, and retained RawEmpty helpers are
not deleted by this rule.

`ProductionUnitLocalErasedSourceOwnerV1` keeps original source/N, the complete
ranked roots, actual E and their receipts together. Its consuming boundaries
freshly reconstruct original N and assertion origins, then replay ranked
correspondence, silence and exact deletion. A separate typed native validator
recompiles the complete ordered original-N ranked candidates with their effect
receipts and rederives aggregate subjects. It independently admits and checks
the supplied E candidate; neither serialized coordinates nor the producer's
filter provide deletion authority.

`ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1` additionally checks actual
E/B coordinates, B/C history and independent C/O replay. All original source
functions still pass source-role and grammar checks. Fresh scoped N/E maps
classify each original operation, block and assertion as retained or explicitly
deleted; retained private lifetime sites and assertion polarity/success edges
continue through the existing B/C checks. Deleted helper assertions grant no
surviving trap permission. Actual B/C/O retain their required native,
private-memory, division and retained-helper censuses; C and O use freshly
extracted formal-memory obligations.

The explicit backend method `lower_silent_unit_checked_output_policy4_v1`
consumes the authenticated original ranked roster, preserves collector bindings,
binds E to the target and prepares LLVM and descriptors from actual O. Original
semantic and launch identities remain the source evidence. Descriptor and native
text replay check the exact actual-O obligations, target and workgroup roster.
This is not default activation or protected publication: the historical direct
constructor still refuses UnitLocal, extraction still requires extraction-only
custody, and the typed N/E proof-to-protected-native consuming join remains open.

The distinct `prepare_native_source_lineage_v1` consuming stage now retains the
original authenticated ranked roster, original N native packet, typed N/E proof,
collector bindings and actual-O artifacts together. Packet assembly is shared
with the direct route, but erased source is never converted into a direct/N-only
owner. Ordered staging commitments come from the same original source owner;
they do not independently authenticate signing or execution. A paid comparison
joins the proof's semantic identity and complete catalog bytes to actual O.
The new state is not a protected artifact or launch capability, and its final
protected consumer remains unconnected.

The ordinary private-helper source test first requires genuine silent-Unit
erasure and actual-O native output, then reruns the owning lineage producer.
Without approved signed ranked execution it must return the precise
missing-receipt error, preserve its resource floor, and release no LLVM or
descriptor output. The direct fill rejection remains covered. These unsigned
tests do not qualify successful signed production execution.

The ranked projector also represents eligible whole scalar-private root loads
and stores as one-element private views at index zero. A paid per-function
census excludes arguments, nonscalar storage, every projected use, borrows,
address-taking, volatile/atomic accesses and deinitialization. The original
loads and stores remain in N/E; the existing retained-slot translation and
final-output checks still establish their source relation and initialization.
This bounds representation does not supply memory values or reference-effect
proofs. Private writes in the reference-effect lane retain their existing
unmodeled-write refusal. Tests cover one/two roots, both assertion polarities,
actual-O forwarding/native replay, hostile eligibility and exact resource limits.

New call indexes, coverage buffers and actual capacities are charged to the
existing work/storage ledger; borrowed ranked graphs and legacy translation
engines retain their separate limits. Scope exit restores the incoming storage
floor on success, error and unwind. Tests exercise exact call coverage, source
and layout mutations, entry synchronization mismatch, foreign inventories,
callback accounting and exact/one-short resource limits. These component and
normal-projector prefix tests do not replace execution of the authenticated
ranked roster with the approved Verus runtime.

The erased route separately meters candidate/E overlap, retained maps and actual
container capacities; its returned additional receipts do not replace the
caller's original source/ranked/B/C/O reservations. Existing translation,
formal, descriptor and native engine allocation domains keep their stated
limits; these are not whole-compiler heap/RSS receipts. Semantic-source and
normal-projector component tests cover shared roots, exact deletion, live root
assertions, nonidentity private-load forwarding, hostile source/graph/history
changes and resource cleanup.

The ignored `ordinary_rust_private_unit_helper_reaches_checked_native_output`
test compiles an ordinary attributed Rust kernel whose no-argument helper uses
initialized private array storage. Its test-only dispatch selects the owning
route from the authenticated helper policy, never from a kernel name or a failed
admission. The observed normal rustc configuration retains one helper and four
private accesses in N; checked deletion produces distinct E, and actual O has
no helper calls or private effects. Native text and descriptor replay use O.
The actual-O simulator checks eight Fill boundary-size scenarios twice against
the independent reference, including backing bytes and canaries. A test-only
retained-MIR configuration is available when rustc erases a helper first; such
frontend erasure does not count as fe2o3 helper-deletion coverage.

This is bounded ordinary-source and CPU-model evidence, not whole-corpus,
approved-runtime, external LLVM execution or GPU qualification. No runtime
bypass is provided.

## Admission tests

General checked-output admission also censuses scalar helper closures from each
actual B/C/O inventory. Eligible helpers have a direct non-unwinding Rust ABI,
Bool, signed/unsigned 8/16/32/64-bit or F32/F64 arguments, and Unit or one scalar
return.
Source membership is checked against the reconstructed root-qualified function
correspondence. Every retained helper is checked, including an uncalled helper
left after independently checked dead-control removal.

The source census distinguishes kernel roots, the bodies selected by the exact
transparent Result-wrapper selector, and actual retained helpers. A selected
body is not automatically a scalar helper: if it is also retained as a helper
under another root, that occurrence must still satisfy the scalar ABI rule.
Complete original source/N reconstruction remains mandatory. Repeated
root-qualified mappings of a shared helper may identify the same physical
statement only when their semantic function, block and statement all agree.
The role census and selector scans use the existing work/storage ledger.

Ranked allocation provenance also recognizes a whole-value Copy/Move chain from
an unchanged ExclusiveOwner argument to a uniquely defined, same-type carrier.
A later direct borrow is admitted only when every receiver use is an arg0 use
of an authenticated descriptor-preserving disjoint intrinsic. Casts, projected
copies, raw address escapes, aliases, unknown calls and carrier writes do not
supply this authority. The bounded graph traversal is linear in source rows,
locals and copy edges; it is not a new alias, initialization or borrow theorem.

The helper census requires complete empty memory effects and a closed scalar
opcode set. F32/F64 constants, comparisons, selects and strict add/subtract/
multiply are admitted without reassociation, contraction or fast-math permission.
Exact F32 negation and division are also admitted for roots and retained scalar
helpers. Native lowering emits plain `fneg float` and `fdiv float`; it does not
replace division with a reciprocal or apply integer nonzero checks to F32.
Float casts, F64 division/negation, float remainder, narrower formats, pointers,
aggregates, helper traps, external calls and recursion remain outside this subset.
Calls remain ordered and block private
store forwarding. This does not establish termination, helper-result bounds or
extent facts, optimizer purity, or general tutorial-helper support. Policy4
continues to forward only its admitted integer loads, not Bool loads. The
existing actual-inventory nonzero checks still govern integer division and
remainder.

The F32 component tests retain a real global output write and exercise both
direct and retained-helper forms through source/N/B/C/O replay on both target
profiles. The actual-O CPU model uses bit-exact sign inversion for negation and
nearest-ties-to-even software division. Its vectors include signed zero,
subnormals, infinities, rounding, overflow, underflow and dynamic NaNs. Division
NaNs are compared by classification, not by a claimed native payload/sign
choice. Literal NaN constants remain refused by native emission because its
widened hexadecimal spelling cannot preserve every payload; exact source-NaN
payload mutations still fail correspondence. Native text and descriptor replay
is not LLVM execution, hardware qualification or protected artifact authority.

`ordinary_rust_f32_arithmetic_reaches_actual_o_native_and_simulator` adds four
ordinary-Rust source configurations: direct negation/division on normal MIR and
scalar-helper variants with test-only `-Zinline-mir=no`. It requires actual O's
two dynamic F32 inputs, output-slice ABI, retained arithmetic, and the expected
helper call count. Its 56 scenarios each run twice across zero-length and
workgroup-boundary output sizes, checking initialization, scalar preservation
and canaries. Strict native `fneg`/`fdiv` and numerical attributes are checked
after the ordinary actual-output handoff; no workload-name compiler dispatch is
introduced. The existing fill, vecadd and scalar-GEMM oracles are unchanged.

`ordinary_rust_shared_unit_helper_reaches_checked_native_output` exercises the
ordinary two-root Rust fixture through the same callback as fill and vecadd.
It requires the admitted Verus runtime for the earlier ranked helper-effect
join. Without that runtime, the test fails before Policy4; semantic-MIR helper
and ABI component tests do not substitute for this source qualification. No
runtime bypass is provided, and this is not whole-corpus qualification.

The focused ordinary-source fill/vecadd and scalar-GEMM parents also simulate
the exact admitted optimized O. They re-verify its unchanged V12 canonical bytes
and identity, then use the existing bounded CPU simulator with explicit test
requests, actual entry ABI and workgroup metadata. They do not substitute a
handwritten KIR graph or select compiler behavior by workload name.

`ordinary_rust_result_wrapped_fill_reaches_checked_native_output` additionally
checks the real Result-returning Rust fixture with normal optimized rustc MIR
and with rustc MIR inlining explicitly disabled in the test invocation. It
requires observing zero and one retained transparent wrappers respectively,
then runs the same exact-O Fill oracle for each. The fe2o3 pass policy is
unchanged; this frontend test variation is not a production optimizer selector.

Each finite F32 scenario compares every backing byte and initialization bit with
an independent CPU reference, including readonly inputs, unused output elements
and canaries outside the slice views. Empty and workgroup-boundary lengths,
short/long GEMM outputs and a separate-versus-fused multiply/add discriminator
run twice. Numerical comparison is bit-exact for these inputs; it is not general
IEEE, FTZ, LLVM or GPU equivalence. Observed races/conflicts refuse; incomplete
bounded race assessments remain explicitly incomplete, never race-free.

Simulation observations are optional in the test JSON. Historical reports
without them stay unobserved; focused tests require them. The all48 corpus and
missing-proof probe do not request simulation and keep their existing gates.
These model runs grant no artifact or launch authority and do not qualify other
tutorial kernels or hardware.

The production admission is maintained by the following regression gates:

1. `KIR -> Pliron -> KIR` at `-O0` is byte-identical across the verified KIR V9
   operation, terminator, type, attribute, function role, kernel descriptor,
   target capability, and source coordinate; the V10 route additionally covers
   exact semantic memory intrinsics.
2. Unsupported or malformed constructs fail closed before mutation.
3. Conservative carrier tests cover allocation, guarded memory, atomics,
   synchronization, matrix/MFMA, wave, inline assembly, and switch families.
4. Mutating-pass tests retain potentially trapping and effectful operations and
   exercise live SSA rewrites into preserved operations.
5. Production source-order checks require the fixed optimizer between target
   binding and LLVM lowering, and replay evidence validates its bound report.
6. Production has one fixed optimizer-policy entry point and no legacy or
   unoptimized fallback.

The historical V2 optimizer report proves deterministic structural replay,
not semantic equivalence. The canonical migration adds independent checks for
specific scalar/CFG transformations; those checks do not establish universal
compiler correctness or replace final source, memory and refinement admission.

## Typed integer saturation

Semantic MIR V30 adds exact signed/unsigned 8/16/32/64-bit saturating Add and
Subtract. It inherits the ordinary V28 grammar, not the inert V29 Execution
grammar. Content-based production checks and closed codec tags keep Execution
unavailable even when a request also contains saturation. Old wire bytes remain
unchanged; 128-bit, mixed-sign, floating-point and pointer operations are refused.

The rustc adapter requires actual intrinsic metadata, an exact safe Rust
`(T, T) -> T` signature and nounwind metadata. Retained primitive core wrappers
are authenticated separately and their bodies still traverse collection. KIR
lowering evaluates each operand once and uses the existing checked arithmetic
result zero, an explicit overflow predicate and a clamp select. It neither
introduces partial plain integer arithmetic nor assumes the unused overflow
result proves source correspondence. Source expression reconstruction requires
a unique unescaped call destination and bounded all-path lifetime checks.

The ordinary-source regression enumerates ten integer body types, Add/Sub and
normal/retained-wrapper MIR. The typed launch macro requires literal primitive
spellings; pointer-sized body cases use explicit i64/u64 launch arguments on
the AMD64 test lane. This does not add usize/isize launch-argument support.
Actual optimized output is checked against an independent widened-integer
clamping oracle, including backing bytes, initialization and canaries. Retained
helper transport and exact source/N/B/C/O replay are not functional reference
proofs through arbitrary Defined-helper results; that separate relation remains
required and fail-closed. No protected/default or hardware authority is added.

## Typed integer/F32 conversions

Checked-output admission accepts ordinary Rust numeric conversions between F32
and signed/unsigned 8/16/32/64-bit integers, including retained scalar helpers.
The source census checks the exact source and destination types. The native
census independently reads the actual operand definition, cast kind and result
type; a claimed opcode or result type alone is insufficient. Each inspected
numeric-cast candidate charges the existing work ledger before type lookup.

Integer-to-F32 uses round-to-nearest, ties-to-even. F32-to-integer preserves
Rust's saturating conversion, including NaN-to-zero, using the existing
`FloatToIntegerSaturating` relation and LLVM saturation intrinsic. This does
not admit unchecked float-to-integer conversion, F64, narrow floats, Bool,
Char, pointers, raw Index values, or new bitcasts. Existing source/N/B/C/O and
helper-effect checks remain mandatory.

The ordinary-source regression enumerates both conversion directions, all
eight integer types and normal/retained-helper MIR on gfx942 and gfx950, for
64 configurations. Each actual optimized O is simulated twice against an
independent host-Rust `as` oracle. Cases include
signed zero, subnormals, fractional inputs, NaNs, infinities, finite limits and
rounding boundaries; complete backing bytes, initialization and canaries are
checked over empty and workgroup-boundary lengths. Native text must retain
the appropriate signed/unsigned conversion or saturating intrinsic without
fast-math flags. This is CPU simulation and native-text replay, not execution
of the emitted LLVM, target hardware evidence or arbitrary helper-reference
proof.

## Final output worker binding

`PreparedNativeCheckedOutputWorkerHandoffV1` consumes the existing Direct or
Erased signed-source compilation stage and retains it intact. It joins the
signed proof's independently reconstructed semantic source, original N,
optional erased E, catalog and launch roster to the final output owner's
source. It then checks original collector/context bindings, target/source
coordinates, descriptor roots, workgroup order and actual O against the
already prepared native worker handoff. No second lowering or graph copy is
introduced.

The added storage receipt covers the wrapper delta; the original owners stay
in the inherited retained floor. Scratch scopes preserve work and failure
history and reject floor loss or ledger replacement. Borrowed comparison
inputs are inert and cannot create the owning stage. The wrapper has no
raw-parts constructor, publication conversion or launch authority.

Component tests cover both source routes and targets, exact/short resources,
source/target/descriptor substitutions and a genuinely different same-target
prepared handoff rejected at the actual-O/native join. These are not a full
signed-stage construction test. That positive still needs the authentic signed
source producer and approved proof runtime; the missing-receipt refusal and
protected/default activation gates remain unchanged.

## Existing F32 exponential contract

Checked-output admission recognizes the existing reserved F32 `Exp` operation
only under `OcmlAbiV1`, including ordinary retained scalar helpers. Its exact
declaration, argument/result types, empty capabilities and existing effect
contract are checked from the verified inventory. The source math context and
ordered operand remain subject to complete source/N/B/C/O correspondence.
Other external imports and other reserved math operations gain no exception.

The descriptor query is inert, allocation-free and prepaid on the existing
work ledger. Native lowering retains the exact `__ocml_exp_f32` import and
strict floating-point attributes. The target-specific compiler FFI envelope
retains the original neutral-source N identity; the separate native/descriptor
relation is checked against actual optimized O. These identities are not
interchangeable. This does not select or qualify an
external OCML provider, establish real-number exponential accuracy, or grant
linking or execution authority.

The ordinary-source regression covers direct and retained-helper forms on
gfx942 and gfx950. Component tests check operand/history/native mutations and
exact/short resources. The actual O simulator preflight must still report
`UnsupportedFeatureV1::FloatFunction(Exp)`: no host approximation substitutes
for the missing OCML simulator, and no numerical or hardware pass is claimed.

## Retained helper value correspondence

The source and native checkers independently reconstruct parameterized value
recipes for retained, acyclic `Defined` scalar helpers. Each recipe is bound to
its borrowed source owner, exact callee, direct nounwind Rust scalar ABI and
actual call occurrence. The caller must have one dominating definition of the
result; ordered actual arguments are substituted at that occurrence. Nested
calls reuse root-scoped templates, not inferred purity or an opaque call name.

The recipe census checks the complete helper body, including otherwise unused
statements and blocks. Memory effects, unknown calls, branches, loops, extra
blocks and unsupported operations do not become transparent because the return
expression looks equivalent. The native path separately reconstructs values
from actual KIR and checks the complete original source/native call relation.
Neither recipe is an independently supplied assertion or signed proof.

Construction, retained caches, argument substitution and final correspondence
replay use the existing caller work/storage ledger. Queries reject a foreign
ledger or a lost retained-cache reservation before debit. Native expression
construction enforces its node bound before recursive expansion; a later final
tree-size check is not a substitute. Temporary scopes restore the inherited
storage floor while preserving work and failure history.

The ordinary-source reference regression observes exact prepared requests for
direct, nested, swapped and alternate helper formulas on both target profiles.
It then requires the unchanged `ProofRuntimeUnavailable` refusal. This is
request-construction coverage, not signed helper-reference composition,
approved-runtime qualification or protected/default-pipeline activation.

## Private scalar-borrow transport

The ranked source projector can retain a same-block mutable reference to a
private fixed-width integer temporary initialized before its unique borrow.
Repeated direct writes before that borrow are eligible only in the same live
storage epoch; a kill or restart cannot be repaired by a later overwrite.
A complete, prepaid source census checks the original borrow, each exact read
occurrence, storage lifetime and nonescape before mapping it to the original
private allocation.
The existing provenance result remains an independent requirement. Shared/raw
references, projections, reborrows, alias copies, calls, cross-block uses,
writes after the borrow and premature storage death do not gain admission.

The census is linear in locals, blocks, statements and operand uses, followed
by constant-time occurrence queries. Borrowed source identities and the caller
ledger's retained-storage floor are checked on every query. This transports
source occurrences; it does not establish arbitrary alias or initialization
theorems and does not replace source/native correspondence or final checks.

The fixed Policy5 ordinary-Rust regression separates normal MIR from explicit
MIR opt0, where a genuine initialized private load pair must survive import.
Only the later load may be forwarded; the first load remains. A separate opt0
case places a global store between the reads and requires no forwarding.
Both target profiles compare actual optimized O with an independent host-Rust
oracle, complete backing bytes and canaries. These tests do not execute LLVM
or hardware and do not activate the protected/default publisher.

Literal observation goldens exercise V12 decode, checked load forwarding,
independent full-pair replay, stable canonical coordinates and two fresh-process
replays. Uninitialized and escaped-pointer graphs require conservative no-ops;
they are not memory-safe source qualifications. These are observation fixtures,
not a new compiler IR parser or publication authority.

## Fixed-width literal shifts

The checked scalar route admits left and right shifts of signed and unsigned
8-, 16-, 32- and 64-bit integers when the source semantic-MIR count operand is a
nonnegative integer constant strictly below the left operand's width. A
heterogeneous count type is transported only at this operand position.
Pointer-sized KIR `Index`,
128-bit values, dynamic counts and out-of-range counts remain outside this
contract; masking is not used to repair an invalid source count.

The independent native census requires an actual in-range constant producer or
the materializer's exact ordered constant-and-width-mask expression. Source
and native helper derivations preserve the real operand order, body, ABI,
lifetime and effect checks. Value correspondence still checks direction,
result signedness and width, overflow contract, left operand and exact count.
Each check uses the caller's existing work/storage budget without a new graph
scan or a reset of expression-expansion limits.

Ordinary-source regressions cover both directions, zero and final-bit counts,
all eight scalar types, direct and retained helpers, and both target profiles.
They compare actual optimized O against typed host-Rust byte oracles and inspect
the remaining LLVM shift direction. Separate raw dynamic-count cases must
refuse at the unchanged source-admission boundary. The retained-MIR setting is
test-only. These regressions do not establish protected publication, hardware
execution, dynamic-shift support or full tutorial-corpus qualification.

## Fixed checked-output extraction

`RankedVerifiedProductionCompilation::lower_fixed_checked_output_v1` consumes
the actual ranked compilation and selects the fixed Policy5 route from its
retained helper-source policy. `RawEmpty` uses direct checked output;
`UnitLocal` uses independently checked silent-helper erasure followed by the
same optimizer policy. No caller supplies a pass list or chooses a fallback.

The move-only facade retains the complete original N, optional erased E,
optimizer B/C/S/O history, ranked roster, collector bindings and native output.
Its additional header storage is checked together with the inherited retained
floor. This bounded logical accounting is not a whole-process memory bound.

`run_production_fixed_checked_output_extraction_driver_v1` performs one real
rustc transaction and writes LLVM text from actual O through the existing
extraction-only handoff checks. Its capped create-new writer refuses existing
files and verifies the output file identity. The diagnostic identities and
bytes are not protected compiler, artifact, load or launch authority.

This route does not replace `lower_production_target` or the protected default
publisher. The latter still needs a general signed source-proof producer and
a consuming final-output publication contract. Missing aggregate receipts
continue to refuse; simulator agreement and output hashes cannot replace them.

## Authenticated Wave64 capture

Semantic MIR V33 adds inert capture of the reviewed gfx942 shuffle-index
primitive for u32, i32 and f32. Collection checks the actual resolved definition,
sealed trait chain, primitive Self, provider source closure, shared context
identity and exact `gfx942:xnack-` profile. Import reconstructs the actual Rust
FnAbi, including its unwind bit. This is not permission for user-written unsafe
code, arbitrary unsafe helpers, or an unmarked collective primitive.
The existing admitted-function ABI contract still rejects an unwinding ABI;
capture must not fabricate a non-unwinding bit to pass that check.

V33 explicitly extends the ordinary V30 grammar. It does not inherit V29
Execution capabilities or the independently reserved assembly grammars merely
because its version number is larger. The shuffle uses intrinsic tag 90 and
private terminal tags 135-137; historical encodings remain unchanged.

Executable lowering deliberately refuses this captured primitive until checked
lane range, physical participation, context transport and convergence facts are
available. Source tests observe the actual semantic owner inside its single
production transaction, then preserve the subsequent compilation refusal.
Successful inert capture is not successful kernel compilation, simulation,
native execution or hardware qualification.

## Proven masked shift counts

The scalar shift contract additionally accepts an actual source count of the
form `count & (width - 1)`, with the mask in the right operand position and
the exact fixed-width integer types retained. A heterogeneous count can use
the admitted integer conversion at the count position. This does not repair a
raw dynamic source shift by inventing a mask during lowering.

Source and native correspondence independently check the real mask producer,
its operand order, type, dominating occurrence and any admitted count cast.
Ranked and typed expression-domain validation require the exact mask and
recursively validate its children; masking cannot hide a division by zero or
another undefined child. The mask establishes only that the shift count is
in range. Source identity, memory, ABI, effects and value correspondence remain
separate requirements. Wrong masks, raw dynamic counts and unsupported
expression shapes continue to refuse.

This expression-level support is not complete ordinary-Rust masked-shift
admission. Rust's retained shift-overflow assertion also needs an exact
mask/comparison/successor relation; recognizing a mask expression alone does
not discharge it. Source census, retained-helper traversal and final native
provenance remain separate admission gates.

`fe2o3-mir-model::SemanticMaskedShiftIndexV1` provides an owner-bound query for
the exact mask, optional same-width signed cast, unsigned comparison, and
assertion-successor shift pattern. Its initial contract requires an acyclic
function and adjacent producers/consumer, tracks explicit local lifetimes and
escapes, and meters construction plus later lookups. It produces inert facts,
not assertion-elision permission or source/proof authority.

The production query adapter meters construction and lookups on the caller's
actual ledger and keeps its index inside a scoped borrow. A private lowerer
table binds facts to the actual source, function and lowering plan. One decision
feeds failure-block sizing, assertion emission and origin recording. The backend
independently rebuilds its source table; a contradictory actual graph constant
overrides the source fact. Legacy assertion contexts receive no such table.
An elided origin label alone is not proof. These consumers do not replace source
SSA, initialization, use or source/native correspondence checks, and their
component tests are not ordinary-source or tutorial qualification.

The checked-output source census queries the same exact assertion-success
relation once per relevant source function. It retains the literal and adjacent
mask predicates and scans every original block, including unreachable shifts
and retained helpers. Each accepted fact names the actual source owner and
shift coordinates. Query errors preserve their resource or semantic refusal;
no query or scratch allocation escapes the scope. This does not establish
retained-helper template or final native correspondence admission.

Safe core `wrapping_shl` and `wrapping_shr` calls have a separate, closed
normalization recipe for signed/unsigned 8/16/32/64-bit integers. The collector
and closure traversal authenticate the actual inherent core item, monomorphic
signature, body and FnAbi before accepting this recipe. Source construction
reobserves those subjects and lowers the call to typed value capture, a full-U32
count mask, a shift and its original successor. Synthetic locals join the
canonical raw/receiver-local order and source coordinates remain attached.
Mixed-call scans and remapping use the same cumulative work ledger. This is
a reviewed pinned-core semantic summary, not a proof of arbitrary library
implementations; it adds no exemption for direct unchecked, unsafe or panic
calls and no semantic wire opcode.

## Checked integer continuation

Policy6 consumes the existing checked Policy5 result rather than rerunning
its prefix. Its fixed schedule extends the retained B/C/S/O history by one
physical-order integer-identity sweep and DCE, producing final I:

```text
source N -> optional checked erasure E -> materialized B
  -> checked scalar/CFG and dominance CSE C
  -> checked store forwarding S -> checked load forwarding O
  -> integer identities -> DCE -> I
  -> final-I source, memory, arithmetic, ABI and native-output admission
```

The rules use exact fixed-width semantics, including checked arithmetic
overflow results. A live overflow flag cannot be discarded just because the
value is an identity. This is one deterministic sweep, not a fixed point,
SROA, general GVN or inlining. A legal no-op still records both continuation
passes. Historical Policy5 output and its record remain unchanged inside the
consumed owner.

For example, a surviving typed integer `xor(value, 0)` can be replaced by the
same value while `xor(value, 3)` remains. The independent checker validates
the complete O-to-I operation/use relation, not only a reported replacement.
The record binds B, C, S, O and I, the exact pass roster and transformation
map. Source-to-output admission preserves real trap origins when removal
shifts an operation's ordinal.

The fixed Policy6 facade selects direct or UnitLocal handling from the
retained source owner. Its descriptors, native artifacts and extraction bytes
are newly derived from I; native O is not reused. Its original-source accessor
uses actual V12 N identity, distinct from the historical compatibility digest
retained by the Direct worker handoff.

The continuation uses the caller's work/storage ledger. Callback adoption
refuses ledger replacement on success, error or panic without refunding or
charging a substituted ledger. Retained-storage receipts account for these
stages logically; they are not process RSS or a universal peak-memory bound.

The ordinary-source regression matrix separates normal and opt0 MIR across
the eight fixed-width integer types, direct/retained helpers and both target
profiles. Only opt0 cases require identities to survive in O and disappear in
I; frontend-folded expressions are not evidence of an optimizer rewrite.
The gate compares actual source/N/O/I, root order and ABI, native owning
functions, independent extraction bytes and host-Rust byte-oracle simulation.
The test's existence is not a claim that a particular checkout passed it.

Policy6 remains a checked extraction route. It does not activate the
protected default publisher, supply a missing signed source-proof producer,
qualify every tutorial kernel or establish LLVM/GPU execution equivalence.

### Native continuation and final receipts

The fixed facade has a consuming native-preparation endpoint. It retains the
source-selected Direct or UnitLocal branch, requires the existing signed source
lineage, and transfers custody to the existing native worker handoff. Missing
signed evidence remains an error. The endpoint carries the original storage
reservation and returns only its additional retained-storage receipt.

The typed native Policy6 consumer can also bind supplied final-I KernelIr and
FormalMemory receipts to that exact retained owner. It first replays the full
signed native relation, then checks the actual graph and catalog bytes, source
root identities, launch geometry, descriptor bindings, and freshly derived
final-I formal obligations. The descriptor-canonical permutation is not assumed
to be the physical kernel order in either N or I; those indices are joined
independently. An authentic historical O receipt cannot stand in for changed I.

This in-process consumer is not F2NOUT1 wire admission or a proof producer.
The serialized heterogeneous transformation chain still requires separate
integration. Its component tests use genuine unsigned
source stages without constructing a signed native owner. A positive signed
end-to-end qualification therefore remains a distinct requirement.

A separate original-N checker derives fresh formal obligations from the actual
retained V12 N and joins its KernelIr/FormalMemory receipts to source, catalog
and descriptor roots. It never substitutes optional erased E or final I reports.
The UnitLocal path discharges a private call only inside complete retained
source/N/E replay, using the exact original operation and same-root silent-call
token. The generic formal engine still refuses those calls without this source
evidence; all other incomplete reasons and conflicts remain errors. Private
sorted root/call indices bound repeated joins without changing report coordinates.
Borrowed receipt backing remains in the caller's accounting domain, while new
decoded scratch and witness headers are charged locally. This borrowed relation
does not itself associate an input proof or serialize the final-output chain.

The separate consuming V4 association requires a genuine signed native owner,
its checked final-I receipts, original-N formal evidence and the five typed
digest/length identities for semantic MIR, middle-end evidence, original native
N, correspondence and original formal evidence. It also compares the complete
retained Verus roster bytes. Equal bytes in different identity domains are not
interchangeable. Original/final root joins use bounded, metered name indexes;
they retain independent physical root ordinals and exact duplicate refusal.
This in-process type is not a new F2NOUT frame, a signed-positive qualification
or permission to reinterpret the existing scalar-only transcript.

The integer continuation's observation goldens live under
`crates/fe2o3-kernel-opt/tests/integer-continuation-golden/`. They cover an
identity, a live overflow flag, an unchanged neighboring operation, a retained
trap after earlier deletion, and a legal no-op. Their test harness checks actual
canonical graphs and records across fresh processes; the readable observations
are not another IR parser.

### Redundant private-store service

The separate `optimize_checked_redundant_store_v1` service removes later
identical stores to a direct, nonescaping, aligned private integer allocation.
It keeps the first store and only crosses a closed set of total scalar
operations in the same block. Loads, other memory effects, calls, potential
traps and convergence operations stop the run. A complete pointer-use census
rejects escapes and derived aliases, including uses after the proposed deletion.

The producer checks the original MemorySSA Def chain and emits complete
deletion and retained-operation coordinates. An independent checker rebuilds
the actual input/output inventories and verifies every deletion and survivor
without trusting the producer's MemorySSA. Mutation, fresh V12 admission and
replay share the caller's bounded resource ledger.

`prepare_owned_redundant_store_continuation_v1` moves the actual checked output J
and copies only prepaid inert deletion/origin rows. Replay against an input
always checks the complete actual pair, even when its typed identity matches.
There is no public candidate-attachment or raw-parts constructor.

The separate borrowed source checker composes the actual Policy6 origin maps
through I into J. It rejects intervening source-only lifetime or Move kills and
freshly checks J initialization, source lifetimes, traps, helpers and formal
memory obligations. Its witness borrows both the source prefix and deletion
owner; fresh J reports are checked, not reused as old I evidence.

The consuming source-owned continuation retains the whole Direct or UnitLocal
Policy6 prefix once, actual J and its fresh formal reports. Replay rederives
those reports from J and rejects stale external-access coordinates. A private-only
deletion may leave empty external obligations byte-identical to I; exact equality
with fresh J is required, not a rule that every report must change. Its
storage receipt covers only added J, metadata and report rows; inherited caller
reservations, including separately reserved B, remain caller-owned on all exits.

This exact-slot service is not general DSE. Existing ordered-store preservation
checks are not weakened to admit its separate relation.

## Fixed Policy7 and final J

The distinct Policy7 schedule consumes the complete Policy6 prefix once and
runs one checked redundant-private-Store continuation from I to J. It prepares
fresh J LLVM, descriptors, formal reports and worker artifacts; it does not
emit I merely to obtain the prefix. Its sealed execution record binds the full
Policy6 record, I/J identities and every deleted/retained operation row.
Direct and genuine UnitLocal source paths keep original N, optional erased E,
historical I and final J distinct.

The literal fixed7 extraction driver uses one live phase budget through
preparation, replay, consuming extraction and output cleanup. Invocation-local
optional census observes that same transaction; it cannot select a policy or
change its result. Extracted J artifacts and descriptors retain a conservative
source-history floor through their callback lifetime. Metadata comparisons are
charged before use, and cleanup checks the actual budget slot and work ledger.
This is an extraction API, not default or protected publication activation.

A separate native7 owner consumes that unsigned stage and the genuine signed
original-source lineage. Typed consuming endpoints then associate final-J
receipts and original-N V4 evidence. They reuse private fixed-version checkers,
but final-J admission accepts only actual Direct7/Erased7 owners and fresh J
reports. N association borrows the sole retained prefix as source evidence;
it never uses historical I as final-J evidence. Each endpoint replays the full
Policy7 witness, moves existing owners and preserves cumulative accounting.
Missing signed source evidence remains an error, not an unsigned fallback.

Complete heterogeneous wire composition, final graph proof execution,
native-V12 protected host replay and default activation remain separate work.
Constructed source/receipt component tests do not establish genuine signed
end-to-end qualification. The fixed7 census/route tests are also not evidence
that ordinary Rust retains a redundant private Store: that qualification must
observe an actual I-to-J mutation, fresh J native output and simulator agreement.
