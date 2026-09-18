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
retains these timings before temporary files are removed. Policy4 is one opaque
timed call containing B/C/O checks, so its refusal alone does not identify the
failed endpoint. Instrumentation I/O failures are diagnostic-only and cannot
change the compiler result or the all-case acceptance gate. Captured jobserver
variables are removed only when replaying the standalone child, whose inherited
file descriptors are no longer valid.

For a focused diagnostic run, set `FE2O3_TEST_CHECKED_OUTPUT_ENDPOINTS_V1` to an
existing fresh directory outside the checkout. The test harness retains actual
B/C/O canonical bytes, full graph dumps and identity metadata there before
admission. Each case is capped at 30 MiB plus 16 KiB of metadata; incomplete
endpoints are marked unavailable, not emitted as truncated complete graphs.
Graph capture is off by default and adds I/O to Policy4 timing when enabled.
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
approval token or executable owner. The RawEmpty route is unchanged; UnitLocal
still encounters `LocalHelperSourceConsumerUnavailable` at the materialized
ranked-receipt boundary until the downstream private-effect consumers exist.
This stage does not establish helper termination, call purity, optimized B/C/O
private-memory safety, or executable support for local-memory helpers.

New call indexes, coverage buffers and actual capacities are charged to the
existing work/storage ledger; borrowed ranked graphs and legacy translation
engines retain their separate limits. Scope exit restores the incoming storage
floor on success, error and unwind. Tests exercise exact call coverage, source
and layout mutations, entry synchronization mismatch, foreign inventories,
callback accounting and exact/one-short resource limits. These component and
normal-projector prefix tests do not replace execution of the authenticated
ranked roster with the approved Verus runtime.

## Admission tests

General checked-output admission also censuses scalar helper closures from each
actual B/C/O inventory. Eligible helpers have a direct non-unwinding Rust ABI,
Bool, signed/unsigned 8/16/32/64-bit or F32/F64 arguments, and Unit or one scalar
return.
Source membership is checked against the reconstructed root-qualified function
correspondence. Every retained helper is checked, including an uncalled helper
left after independently checked dead-control removal.

The helper census requires complete empty memory effects and a closed scalar
opcode set. F32/F64 constants, comparisons, selects and strict add/subtract/
multiply are admitted without reassociation, contraction or fast-math permission.
Float casts, division, remainder, negation, narrower float formats, pointers,
aggregates, helper traps, external calls and recursion remain outside this subset.
Calls remain ordered and block private
store forwarding. This does not establish termination, helper-result bounds or
extent facts, optimizer purity, or general tutorial-helper support. Policy4
continues to forward only its admitted integer loads, not Bool loads. The
existing actual-inventory nonzero checks still govern division and remainder.

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
