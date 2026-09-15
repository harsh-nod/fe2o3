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

## Integration checkpoint: 2026-09-12

The capability migration remains incomplete: 0 of 47 tutorial fixtures have
qualified through the new production path. No milestone is completed by the
component tests below.

Website publication is also incomplete. On September 12, the public compiler
`main` still lacks `config/tutorial-kernel-manifest-v1.json`, which the site's
deployment gate requires. The site validator previously matched committed
compiler HEAD while ignoring changed working-tree contracts. It now requires
both copies to agree, with modified/missing-file tests for every shared manifest,
schema and digest. Both local validators reject the site's older source-input
hashes. The live compiler manifest agrees with the frozen input used by the
source sweeps below. No committed-snapshot check may be reported as current
worktree parity, and no mismatch permits bypassing the publication gate.

Latest completed full sweep: mixed115, **47 failed exports, zero source bundles, all
47 scratch cleanups**. Report SHA-256:
`d2f95677007a4ec5a595aeb4957af785d181fa2b9953699e22cca88de52e4b19`;
compiler DSO SHA-256:
`ad71da8fe8efa3c7dffc287b5476516dcbaa5dfce8eb9ac8a4c9228733dd18fb`.
Frozen input snapshot `4e880ddab885f8e8aa38943321d5b0bc9937e21a`, tree
`795b33cdb9b41161dfeb4a29f2a977ed3e9c38ec`, remains clean after the sweep.
All 47 compiler-input identities match mixed75. This snapshot includes the
independent gradient-staging CPU output-slice reference added after mixed52.
Exporter, extractor and DSO hashes are unchanged before/after this sweep.
All 94 stdout/stderr captures and 47 raw diagnostic lines are complete and
independently hash-checked. Sixteen displayed diagnostic summaries are clipped
at their declared 2048-character limit; the full raw records are retained.
No simulator or hardware run is implied.
This diagnostic does not authenticate the compiler's complete loader closure.

Compared with mixed114, all 47 normalized stopping diagnostics are unchanged.
The compressed owner-invalidation index does not clear any full-source blocker.
Materialized attention still exceeds the lowerer's 1,048,576-unit allowance.
It now retains 67 completed loan-cache entries and 60 reachability entries,
compared with 64 and 56 before the index change. A 1,216-unit path-region debit
exceeds the remaining 380 units. Its full raw stderr is identical to the
controlled mixed112 run. The independent file ledger attributes all 1,048,196
accepted units to 20 compiler files; the bounded site ledger is still partial.
Owner-invalidation indexing accounts for 128,313 units in this failed prefix,
versus 187,509 in mixed110. These are different failed prefixes, not equal-work
timings, completed analyses or GPU performance measurements. Decode, held
fragments, interleaved stores and serial routing still fail within analysis;
earlier stopping-point regressions are not fully recovered.
Fill, typed vecadd, combine and gradient staging still stop at the unavailable
protected runtime locally. This sweep uses the compiler built from the 4,796-input
checkpoint125 snapshot, including the diagnostic file ledger and compressed
owner-invalidation index, source-ownership admission and expanded-source tests.
The 5,978 frozen fixture files remain unchanged, and all 48 scratch
paths, including the parent, are absent. No new proof, export or GPU pass is
claimed.

The full lowerer test run passes 675 library tests and all 76 public integration
tests. The full MIR-model run passes 462 library tests and 347 public tests, with
one ignored fixture-regeneration test. All 12 focused AMD compiler callbacks
pass. The final 13-library run (51dn) passes 4,810 tests, with 187 ignored;
the capability catalog and preimage targets pass 11 and three tests respectively.
The preceding run (51dm) had three ownership-fixture failures. The simulator
fixtures now use genuine shared-slice types and matching ABI/storage metadata;
backend negatives require misclassified references to reject at admission.
These test-only repairs account for the five changed paths between the 4,796-input
source-sweep snapshot and the final 4,797-input test snapshot. The production
implementation and all three source-sweep binaries are unchanged. Library tests
overlap the broader count; these results are not an end-to-end pass, numerical
proof or GPU execution.

Checkpoint126 adds explicit zero-limit claim validation and bounded Verus failure
diagnostics. The all-feature Pliron library passes 834 tests; its ranked public
target passes 55 and fails two tensor positives. Four ownership/shape fixtures
now carry actual stored values and exact per-write effect contracts, with
synthetic receipts confined to untrusted staging. The remaining tensor fixtures
retain their real `TensorResultComponent` values, which effect analysis cannot
yet consume; no proxy values or weakened positive assertions were introduced.
The full verifier suite passes 181 tests with nine runtime-dependent tests ignored.

Targeted protected-runtime run126c on MI350 retries typed vecadd, expert-rank
combine and gradient staging using the unchanged frozen tutorial sources. Vecadd
still rejects a semantic read used across blocks. Both gfx950 fixtures reach
Verus, which reports two checks verified and one assertion failure in a generated
value-equality formula. The retained stdout/stderr excerpts are complete for
those failures. This is failure to establish the formula, not an established GPU
counterexample, numerical-bound proof or runtime outage. All three exports fail;
no bundle, simulator input or GPU run is produced. The 4,798-input compiler
snapshot and binaries remain unchanged, retrieved evidence hashes agree, and the
exact owned remote directory is removed. Compiler DSO SHA-256:
`a1db711e24c6c2fdcd95de5d6b0d5772a86b44e7590a21b3f539c1187d04f95d`.
This targeted diagnostic does not replace the full mixed115 corpus sweep or
authenticate the complete compiler loader closure. End-to-end status remains 0/47.

Checkpoint127 resolves the constant-selection mismatch behind those two gfx950
value-proof failures. Original-source AMD callbacks inspect independently
translated CPU/GPU expressions after bounds checks and read-site remapping.
For gradient staging they establish:

```text
GPU: select(Bool(true), Load(event), F32(+0))
CPU: Load(event)
```

The selected loads have identical complete read descriptors, not merely matching
addresses. Combine has the same relationship on each ordered addition operand.
The shared Verus renderer now interprets literal Boolean selection. It retains
the original expressions, read events, symbols, domain checks, resource charges
and obligation hashes; it introduces no floating-point arithmetic axiom or
approximation. Dynamic selectors remain opaque. Tests reject the wrong branch,
unjustified dynamic selection, signed-zero bit equality, malformed discarded
branches and cumulative formula overflow. Receipt replay rejects a changed
discarded branch even when the newly rendered value is identical.

Protected-runtime run127 on MI350 clears both earlier Verus assertion failures,
then all three exports reject the existing same-block semantic-read restriction.
It produces no bundle, simulator input or GPU run. The 4,799-input compiler
snapshot remains unchanged; evidence retrieval and exact owned-directory cleanup
pass. DSO SHA-256:
`3d76f3b1f7d7bbcf38c3a4ac2e08fdc3aa3c9a2ef13e7b62472a9f6616b6594f`.
The final verifier suite passes 187 tests, with nine runtime-dependent tests
ignored. The final 13-library rerun (51dp) passes 4,821 tests with 187 ignored,
plus 11 catalog and three preimage tests. All 4,800 final inputs remain unchanged;
the differences from the remote snapshot are confined to verifier tests. Ten
original-source AMD callbacks and mutations pass, not GPU execution.
This is progress through the production proof path, not a completed
tutorial or a nonzero output-error proof. The full mixed115 sweep and 0/47
end-to-end status remain the corpus baseline.

Checkpoint128 closes a stored-value conservation gap. Production `ValueAccess`
and `AtomicValueAccess` now retain their exact RHS in the existing `kernel.access`
operation. The operand layout is view, rank indices, optional semantic-scalar
RHS, then optional checked-success capability. This adds no operation kind,
attribute, alternate graph or proof authority. An atomic RHS is an operand,
not a proved read-modify-write result. `FE2O3-EFFECT-011` leaves RMW final-value
refinement incomplete until actual RMW semantics are available.

After unique-write correlation and existing proof-evidence validation, effect
analysis requires `stored_rhs == contract.gpu_value` as exact SSA identity.
`FE2O3-EFFECT-010` marks a missing RHS incomplete and a substituted RHS rejected.
Even an equivalent expression cannot silently replace the claimed stored value;
binding the claim to the actual operation precedes comparing CPU/GPU semantics.
Rank/ownership checks still use only indices, and the existing structural
identity, mutation epoch and resource accounting include the additional operand.

This does not authorize tensor component equality. Tensor layout, scalar type,
root/ordinal metadata and a staged receipt are not a component arithmetic
theorem. The production component-proof generator/replay remains required before
effect analysis can consume those components; the two existing tensor-output
positive fixtures remain unresolved. No MFMA model or math-library error bound
is assumed. The user-approved numerical policy remains an explicit, proved
final-output bound; no nonzero production bound is currently admitted.

The final 13-library run (51ds) passes 4,824 tests with 187 ignored, plus 11
catalog and three preimage tests. Production binaries build successfully;
all 4,803 final inputs and the resulting binaries remain unchanged after tests.
The full analysis/dialect run reports 851 passed, five failed and one ignored.
Two collective fixtures fail a non-atomic contribution prerequisite, and three
textual fixture groups stop at missing per-write effect contracts. The three
updated textual effect fixtures pass independently. The ranked integration
suite remains 55/57 with its original tensor-output positives intact.
These suites overlap and are not summed into qualification credit. No remote
or GPU run was made in checkpoint128, and end-to-end status remains 0/47.

Checkpoint129 extends the existing live source-memory proof to bounded acyclic
control flow. It uses the actual Pliron region and native dominance, not a
second executable CFG. A producer can dominate a consumer even when its block
is listed later. Structural preflight checks definitions, parent links, exact
successor and operand use slots, tail terminators, edge argument types, and the
function signature before native graph analysis. Local ordinal checks avoid
linear same-block dominance queries during preflight; the budget also reserves
the native queries that structural identity verification still performs.

The existing invocation tracer now retains block visits and selected edge slots
alongside its unchanged memory events. Coverage requires completed, unsummarized
paths and an actual available producer for every executed consumer. A read under
`tid < 16` in a 64-lane launch has 16 instances, not 64 fabricated ones. A read
under `tid >= 48` retains invocation IDs 48 through 63. Conversely, a producer
may execute on all 64 lanes while only 16 consume it. Empty read/consumer
coverage, non-dominating diamonds and unsupported typed block-argument transport
fail closed. Eventless invocation paths remain in the trace.

Read/event joins now include both block and operation identity. Prior writes
retain their exact `AfterWrite` event identity, later writes do not change an
earlier snapshot, and conflicting writes from other invocations still reject.
The original context, function, mutation epoch and structural identity bind all
retained queries. Attribute dictionaries are limited to 64 keys of at most 128
bytes before local schema checks, including block dictionaries. Graph validation
and all invocation coverage share one cumulative work budget. A malformed
function signature can intern a missing expected type before rejection; that
mutation attempt is not reset and cannot revive existing evidence.

At checkpoint129 this was not end-to-end cross-block read support: the ranked
materializer still needed dependency-ready construction with stable source-local
IDs, and source export needed completed-CFG replay and guard-aware output coverage. Loops,
LDS/barrier memory proofs, tensor output arithmetic and nonzero final-output
error proofs are not admitted by this change. No GPU result or new successful
tutorial export is claimed; the corpus remains 0/47 end to end.

The stable-source 13-library rerun (51du) passes 4,842 tests with zero failures
and 187 ignored, plus 11 catalog and three preimage tests. This includes all
18 new CFG regressions. All 4,806 recorded compiler inputs remain unchanged.
Production binaries build, and all 148 selected shared-analysis integration tests
pass. Ranked integration remains 55/57 with the same two unresolved tensor-output
positives. The full progress/lit/collective integration suites were not rerun in
this checkpoint. These are component tests, not additional qualified tutorials.

Checkpoint130 integrates dependency-ready construction into the production ranked
materializer and source structural replay. Canonical source-local numbering is
checked separately from construction order. Fixed optional slots must all be
bound exactly once. The schedule preserves original per-block operation order,
includes every ordinary operand and retained read producer, and rejects cycles.
The shared builder creates every block first, emits each exact typed read once
at its original access, and creates terminators only after all values exist.
The pinned native verifier then checks the completed CFG's SSA dominance; build
readiness is not a dominance or memory theorem. A valid `entry -> producer ->
consumer` path works with stored block order `[entry, consumer, producer]`; a
diamond that bypasses the producer rejects at public construction.

The producer index replaces repeated whole-function scans for each read's view.
The source-reference roster also classifies the complete set of view declarations
before checking original access sites. A later Private view declaration does not
depend on physical block order; Global access correspondence remains mandatory.
Aggregate operation counts reject before walking operation contents, and expanded
expression counts reject incrementally. Dependency accounting charges occurrences
before deduplication under the existing work limit. These are weighted work units,
not an assertion about literal B-tree comparison counts.

Unsigned index casts now use their exact low-bit semantics in shared sparse-index
analysis and invocation traces. Casts preserve affine form only when their range
is proved to fit, otherwise use an exact supported remainder fact. Constants are
masked exactly; 64-bit casts are identity. Unknown inputs and source arithmetic
overflow are not converted into proved values. These integer facts grant no
floating-point error-bound authority.

Source-output export still needs guard-aware whole-output coverage and correct
ownership placement. Loops, LDS/barrier memory, tensor arithmetic and nonzero
final-output error bounds remain separate unfinished obligations. The new public
construction tests do not count as GPU runs or completed tutorial qualification.

The stable-source 13-library run (51eb) passes 4,854 tests with zero failures
and 187 ignored, plus 11 catalog and three preimage tests. This includes all
12 new construction, budget and integer-cast regressions. All 4,810 recorded
compiler inputs remain unchanged for that run. Broader integration and protected
runtime results are recorded separately; these counts grant no tutorial credit.

All 196 selected shared-analysis integration tests pass, including sparse indices
and ranked bounds. Ranked integration remains 55/57: the same two tensor-output
arithmetic positives still fail, with no weakened assertions. The production
binaries build and retain their checksums after integration tests.

Protected-runtime checkpoint130 replays the original frozen vecadd, expert-combine
and gradient-staging sources on mi350. All three clear the cross-block read
materializer, then fail `FE2O3-OWN-001`: generated ownership metadata is outside
the entry block. The diagnostic sites are respectively `(10, 5)`, `(33, 11)` and
`(30, 6)` in their materialized graphs. The verifier still requires unconditional
entry-block ownership contracts; moving generated metadata is not permission to
move a read, write, view or branch, or to assume full output coverage.

No bundles, simulator executions or GPU runs were produced. Runtime source and
installed manifests match the protected pin; remote evidence was retrieved and
checksum-verified, owned scratch removed, and shared cache size unchanged. The
new compiler DSO is `4b04e884b85b0edfe9d486083a80c533ae2211e0c625722754b5ec2f8d2b1528`.
The end-to-end count remains 0/47. No nonzero final-output error theorem, completed
milestone, push or deployment is claimed by this checkpoint.

The preceding full sweep, mixed75, has report SHA-256
`cdd4e2379d24188356f6f9d5d47dc880593dc2300c1f5791ba2faff652798ff3`.
Compared with mixed69, FP4 and FP8 attention cleared typed-custody work limits
before rejecting missing exact SSA uses; materialized attention reached the
structural-analysis work limit since cleared above.

The preceding mixed69 includes enum joins/lifetimes, retained backend output,
shared Workgroup wrapper transport and sparse BF16 address facts. Compared with
mixed67, 43 normalized stopping diagnostics are unchanged. All four GPT-OSS
diagnostics change because Workgroup setup substantially increases charged
fact-construction work; held-fragments now exhausts the same budget in setup
rather than use analysis. This is a regression, not progress. Fill, typed vecadd,
combine-expert-ranks and gradient
staging still stop at the protected proof runtime; the other 43 stop earlier.
All 48 temporary paths, including the shared parent, are independently absent.
The streaming digest and private mutable-header candidates are not included.

Targeted mixed70 rechecks the same four GPT-OSS inputs after distinct-type reuse.
Fact-construction work drops from 235,115 to 114,503 for decode, 262,144 to
140,799 for held-fragments, 232,724 to 113,329 for interleaved stores, and 222,465
to 108,415 for serial routing. All four still exhaust the unchanged 262,144-unit
total during use analysis; no bundle is produced. This substantially reduces
the setup regression, not the remaining use-analysis cost or an end-to-end
failure. The eight complete captures, four diagnostics, three binary hashes,
189 compiler input hashes and cleanup of all five temporary paths were checked.
No compiler wall-clock or GPU performance improvement is inferred.

Targeted mixed72 profiles the same four GPT-OSS inputs after the shared Global
and BF16 source-binding changes. All four exports still fail at the unchanged
262,144-unit aggregate work limit; no bundle is produced. Complete use-analysis
prefix measurements attribute 63,834, 76,569, 64,018 and 59,054 units to global
setup for decode, held fragments, interleaved stores and serial routing,
respectively. Each prefix contains only two repeated matcher evaluations, so
duplicate matching is not the dominant measured cost. These are executed
failure-prefix costs, not whole-pass costs or measured speedups. All eight raw
captures, four complete diagnostics, three binary hashes and 5,978 frozen input
file hashes were checked; all five scratch paths are absent. Report SHA-256:
`944a995cf5767f2edb54f796e4aa6180f8a67da793ddf84f81127bc26b05018b`.

Targeted mixed73 rechecks those exact four sources with the completed-shape
cache. Global setup now costs 35,305, 46,094, 36,047 and 36,426 units in the
same order, including cache storage, initialization and misses. The original
cold traversal, exact type keys and all compiler limits are unchanged.
Serial routing completes use analysis at 133,587 units, then exhausts the
remaining allowance during CFG construction; the other three still exhaust
during use analysis. All four exports fail and produce no bundle. These are
analysis-work savings, not measured compiler speedups or GPU results. Eight
complete captures, four diagnostics, three binaries, 5,978 frozen fixture files
and 4,751 compiler inputs were checked; all five scratch paths are absent.
Report SHA-256:
`075ac30255623ccfe6ed561360dcde19372fff390d2202e8097baf347e079437`.

Targeted mixed74 rechecks the same sources with the indexed four-state memo.
Completed Global setup costs 22,147, 26,019, 21,980 and 20,906 units in the same
order, saving 13,158 to 20,075 units including retained storage and initialization.
Decode and interleaved stores now complete use analysis, then stop during CFG
construction. Serial routing completes use analysis at 118,027 units and CFG
construction at 7,246 units, then exhausts its allowance during owner-change
analysis. Held fragments still exhausts use analysis. All four exports fail;
no source bundle or GPU result is established, and no limits changed. All eight
captures, four diagnostics, three binaries, 5,978 frozen fixture files and 4,755
compiler inputs were checked; all five scratch paths are absent. Report SHA-256:
`93e6f2fb9e5066c537cd7fc7b1483b0d97aa533b90a5e136154f66c5c55eba57`.

Targeted mixed76 checks those four unchanged sources plus materialized attention
with the declaration batch and endpoint cycle index. All five exports fail;
zero bundles are produced. Completed Global setup costs 17,067, 19,654, 16,956,
8,235 and 16,094 units for decode, held fragments, interleaved stores,
materialized attention and serial routing respectively. Each saving from mixed75
is exactly N - 2 for its original local count. Decode still exhausts CFG work;
interleaved stores now completes CFG and reaches owner-change analysis. Serial
routing reaches 13,133 owner-change units. Held fragments reaches source site
(2317, 2), previously (2151, 1), but still exhausts use analysis. Its unchanged
stopping summary is not evidence of a completed use pass.
Materialized attention clears its previous structural-work failure, then rejects
because a capability reference has no exact SSA use. This remains a failed
export, not an ownership proof. All original work limits remain unchanged.
Ten complete captures, five diagnostics, three stable binaries, 4,760 compiler
inputs and 5,978 frozen fixture files were independently checked; all six
temporary paths are absent. Report SHA-256:
`ed67a8ed92e9c611bedf3a35231c155cdf6551341cc60bf8a364c346d7aad4a5`;
compiler DSO SHA-256:
`ccbb1b3188e9c8088350fdd176cad776664cd97b9d453eb2dfceb653d59b30ea`.

Targeted mixed86 checks the same five sources with checked
Bound-reference registration and streaming field-read recognition. Materialized
attention now retains its original shared-borrow SSA uses and clears the missing
use error. It stops later at the unchanged 1,048,576-unit analysis limit; this is
not an exported kernel or an accepted output proof. The original borrow events,
wrapper ownership, matrix/policy leaves and ordered lifetime checks are retained.
A shared read of the checked wrapper's Matrix field is a read sink, not a
Bound-to-Matrix alias or a new capability issuer. Recognition runs in the existing
use pass without a second body scan or a stored read-capture map.

All five exports still fail and produce zero bundles. The four GPT-OSS kernels
retain the 262,144-unit aggregate limit. Decode and held fragments stop during
use analysis, interleaved stores during CFG construction, and serial routing
during owner-change analysis. Their setup work is 115,516, 141,811, 114,341 and
109,390 units respectively. This removes most of the superseded mixed85
whole-body-scan regression, but does not fully restore mixed76's stopping points:
decode previously reached CFG construction, interleaved stores reached owner
changes, and serial routing completed more owner-change work. No budget increase
or compiler/GPU speedup is claimed. Ten complete captures, five diagnostics,
three stable binaries, 4,764 compiler inputs and the unchanged frozen fixtures
were checked; all six temporary paths are absent. Report SHA-256:
`1bcca91ee8fc49c9bc5dc602686cccc42bc19d4ae8eef57acf1c2403323bdc8a`;
compiler DSO SHA-256:
`1a482202cf56add40bad78b001c722a4afc2399660b1057c573e34389138805d`.
The later full 47-source sweep, mixed100 above, includes these changes.

Diagnostic runs mixed87/88 repeat only materialized attention with work tracing
on/off. Both still fail at the same analysis boundary and produce zero bundles.
The new default-off `FE2O3_TRACE_CAPABILITY_WORK=1` ledger records accepted
charges under at most 64 observed Rust caller locations, retaining additional
work in an unclassified total. Its storage is outside the charged query cache;
it neither changes budgets nor participates in admission. A bounded, complete
record is emitted only when a work debit fails. It describes the executed
failure prefix, not successful execution, complete lifetime analysis or CPU time.

The measured prefix consumes 1,048,074 units, leaves 502 and rejects a separate
515-unit request. All accepted work is accounted for, including 165,403 units
without individual caller attribution. The 64-row record is 13,391 bytes.
The largest identified caller is the retained-plan comparison in Global BF16
source-query setup, at 194,422 units; repeated guarded-enum analysis is another
substantial contributor. Propagated Rust caller locations can combine several
charge expressions, and unclassified work must not be omitted from comparisons.
Removing the one complete profile block leaves stderr byte-identical to the
off run; stdout, source inputs, limits and binary identities also match. Both
reports' full captures and temporary-path cleanup were checked. Six focused
tests pass, including actual constructor, source-query and cache-publication
failure emissions. No optimization or kernel pass follows from this measurement.
Report SHA-256 values, on then off:
`a21c91416683a0b83c600924caa77b0678e40b06b16263d654a5800b03839178`,
`2c8ea9bb50c7c0ff1c8b5bd7d0dc00fddafed378486bf6bb233fd15482534a34`.

Mixed89 applies a paid identity fast path to that retained-plan comparison.
The original replay, body/root checks and full comparison for distinct plans
remain mandatory. Three tests verify exact costs, equal copies, foreign or
unequal plans, body/root substitution, and both retained-plan getter paths.
Attention clears the previous work limit, then rejects a different original
Policy borrow at block 88, statement 0 because its owner lacks a promoted SSA
use. No bundle is produced. The old Bound-reference failure remains cleared.
This run emits no work-limit profile, so it establishes neither the number of
comparison scopes nor an exact measured total-work saving. Report SHA-256:
`6ee80f4e6900c647e2a20397e9676b442d478965807fcf79eced34dd0f76d63d`.
Selected diagnostic mixed90 confirms rejected Policy uses without changing
that outcome. Its observation history reaches the diagnostic cap; the final
candidate rows are complete, but they do not establish a complete invalidation
history or a proved cause. Single-site diagnostics mixed92/94/95 use an
independent, default-off `FE2O3_TRACE_CARRIER_SITE` selector with the format
`body_sha256/destination_local/block/statement`. They read existing classifier
state at one assignment, with capped operands, type details and output, without
changing proof work or admission. The eleven-field closure at 88:9 has a Math
route at field 8, a separate checked Policy reference at field 0, and two other
tracked references. No Workgroup join is present. The Math reference is already
invalid after the shared Borrow at 88:6, where the owned Math wrapper is also
classified as a Policy carrier. A later four-field closure at 385:4 has no
tracked parent for its Policy input. These are separate transport obligations;
simply exempting another sibling field would not establish them.

Each selected run emits five identical complete records. Removing those records
leaves stderr byte-identical to the mixed93 off control; stdout, inputs, limits,
binaries and the rejection also match. Four focused diagnostic tests pass.
All four source runs still fail the original Policy SSA query and produce zero
bundles; their full captures and temporary-path cleanup were checked. This
remains a source-analysis blocker, not a numerical-output proof or GPU result.

Mixed96/97 apply the Math opacity fix. Only owned bound types identified by
checked Math consumer contracts become barriers to generic carrier traversal;
their shared-reference routes remain intact. The exact Math Bind still accounts
for both internal loans, and lowering checks those loans again at each consumer.
All added registry storage and lookups use the existing analysis allowance.
In the actual attention source, the erroneous Policy alias for the Math wrapper
is gone. The original Math Borrow at 88:6 and its complete six-node component
are accepted, with a complete diagnostic history. The separate Policy Borrow
at 88:0 still rejects; neither run produces a bundle.

Seven focused carrier tests cover the admitted mixed Math/Matrix source, original
Borrow-to-SSA correspondence, unchanged events, opacity and budget boundaries.
A separate lowerer test rejects Policy-owner death after Bind but before Math
use, including live and reborrow controls. Mixed-source death variants retain
the exact preterminal SSA kills; they are not mixed-kernel lifetime proofs.
An initially unreachable helper fixture was corrected to assert admission's
existing root-closure rejection, not to relax it. These results establish the
Math transport fix, not complete mixed-closure or numerical-output verification.

The Global sibling extension now records occurrence-specific obligations in
the existing escape audit. A temporary handle borrows that audit exclusively;
it accepts only the same original assignment and a complete checked Global
reference leaf, not another role in a mixed operand. Registration is not a
successful check: the complete audit must validate both endpoints before the
field can remain exempt from sibling invalidation. Failed fields invalidate
the original operand and destination before main-component publication. The
original Global Borrow-site set and downstream SSA/lifetime obligations remain
unchanged; no issuer, replacement SSA value or separate proof path is added.

Fourteen new tests cover exact subjects, wrong-family/foreign-body rejection,
late poison, unchanged roots and charged allocation/finalization. Two of them
run production candidate discovery through returned Borrow sites and the exact
shared-budget boundary. Their capability metadata is a component fixture, not
authenticated kernel issuance. Admitted-source Global/Workgroup-join
coverage remains incomplete. No source export,
numerical proof or GPU pass follows from these component tests.

Checkpoint119 retains a separately unique Policy and Workgroup path alongside
the primary carrier path. It joins their exact original operands into the same
candidate graph and checks every retained role on shared-wrapper extraction.
Ambiguity is retained as a failed obligation even when duplicate definitions
remove its operand from the candidate map. An unchecked sibling, failed owner,
late escape or repeated-node component prevents publication of the joined sites.
The existing Global audit and original SSA/lifetime checks remain required.

All 329 focused borrow-flow tests pass. Fifteen new tests cover admitted
Math/Policy capture and reordered transport, original owner queries and unchanged
events, duplicate/missing parents, late escapes, three-role Workgroup components,
high field indices, edge deduplication and exact analysis budgets. The three-role
graph fixtures are inert classifier inputs, not authenticated kernel issuers.
The production rebuild passes. Its targeted materialized-attention run (mixed101)
clears the previous Policy/SSA failure: the original Borrow at `(88, 0)` is
accepted and the previously untracked local639 now has its original carrier
parent. It stops later at the lowerer's 1,048,576-unit analysis limit, while
walking a path region over the 1,216-block body. No limit was raised.
The full 20,600-byte stderr is retained; the selected diagnostic's history
allowance is exhausted, so it is not a complete component-history record.
Report SHA-256:
`770b9a166543877391627511755a94ccfe5525335c5a48282480145017d1acd5`.
The broader suite passes 4,732 tests; full-source mixed102 confirms the changed
attention stopping point with zero exports. Actual AMD compiler callbacks in
51br pass nine cases and fail three. Although the counts match 51bq, those three
fail earlier: the wrong-Bind negative gets a missing Workgroup SSA use before
the positive lane query that previously exhausted analysis work. This is a
regression, not unchanged coverage. The reusable closure also captures shared
Global references; integrating their shared-carrier escape audit is in progress.
No callback result is GPU execution or a nonzero numerical output proof.

Checkpoint120 adds one canonical shared-reference edge to the existing Global
escape audit. The pointee must be an already selected by-value carrier, and
extraction permits one leading dereference followed by exact fields. Raw or
mutable pointers, malformed layouts, reference chains and projected aggregate
moves reject. The Global audit does not independently publish the wrapper's
Borrow: doing so could override another role's veto. Wrapper publication remains
with the common custody graph. A late escape still poisons the connected group.
All 349 focused borrow-flow tests pass, including 19 new Global tests and an
independently derived four-unit secondary-failure query boundary.

The same checkpoint reuses one completed CFG forward cone, bound to the exact
immutable body and SSA plan, while constructing a fresh destination region.
It neither caches loan authority nor raises the analysis allowance. The
production build passes. Targeted mixed103 still fails the same overall
1,048,576-unit limit, now in the later loan-liveness scan rather than the
path-region traversal. Its report SHA-256 is
`e9b33f4fa27e8d37797310808bdd83523efafd34a22d970108b3bc433b630bb3`.
The work-profile site roster is incomplete, with unclassified charged work;
there is no complete cost attribution or kernel speedup claim. The broader
suite initially passes 4,765 tests and fails one new fixture that incorrectly
expects SSA promotion after storage-observable operations. The corrected fixture
preserves the original rejection and separately checks loan invalidations.
All 15 forward-cone tests and all 4,767 component tests now pass (51dg).
AMD callbacks in 51bw pass nine
and fail three: all three earlier SSA regressions are repaired, but their positive
lane queries still exhaust the original 1,048,576-unit allowance. The full-source
result at that checkpoint is mixed104; no source bundle or GPU pass follows.

Checkpoint121 retains completed shared-reference eligibility in a separate
packed type memo. The by-value memo never reads it, so warming a wrapper cannot
admit reference chains or aggregates containing only wrappers. Both memo
allocations, misses, hits and publication are charged; any failed query still
poisons the common owner gate. All 353 focused borrow-flow tests pass, including
four new cache/isolation regressions and every-shorter-budget cases. The
thirteen-library suite passes 4,771 tests with no failures and 187 ignored.
Full-source mixed105 shows the partial budget recovery above, still with zero
bundles. AMD callbacks in 51bx remain nine passed and three failed at the later
positive lane queries' analysis-work cap; the earlier SSA regression does not
return. This is compiler-analysis reuse, not a numerical proof or GPU speedup.

Checkpoint122 replaces the definition-query B-tree with a lazy dense table using
the SSA plan's existing definition-ID bound. Successful source checks alone
populate entries, and warm queries require the exact same body and SSA pointers.
Entry/phi values, invalid IDs and malformed source definitions retain their
original checks. Allocation, initialization, reads and publication are charged;
failed queries never publish partial entries. No loan authority is inferred
from a cached definition. Eight new tests cover these boundaries, every-shorter
budgets and amortized query cost. All 4,779 component tests pass (51di).
AMD callbacks improve to 11 passed and one failed (51by): both target-specific
workgroup-lifetime tests now pass under the unchanged allowance. The remaining
gfx950 Bind-only test reaches a later shared-Subgroup repeated-instance negative,
which lacks an exact SSA use instead of reaching its expected loan rejection.
That assertion is not weakened, and the complete test still fails. Full-source
mixed107 remains zero bundles; neither callback success nor cached loan entries
constitute GPU execution or a final-output proof.

Checkpoint123 seeds closure-carrier routes with the Subgroup reference from an
already validated MatrixAccess fact. The Epoch reference is not a carrier seed;
competing primary references, opaque bounds and secondary Policy/Workgroup/Global
checks retain their existing rejection rules. Three new tests cover exact seed
validation, competing leaves, registry isolation and charged registration. All
356 focused borrow-flow tests and 4,782 component tests pass. AMD callbacks now
pass all 12 tests (51bz), including eight shared-Subgroup loads with one lane and
two Bind identities, followed by the original cross-Bind loan rejections. No
analysis allowance or negative assertion was weakened. This repairs source
reference tracking, not numerical equivalence or GPU execution.

Checkpoint124 reuses completed enum-guard selection queries within one analysis
graph. The cache requires the exact body, SSA plan and type-slice identity and
keys every value, enum type, variant and use block. It retains the original
guard/dominance algorithm; payload, issuer, loan and cycle checks still run.
Both positive and negative results are reusable, but errors and partial entries
are not published. Owner checks, lookup, allocation and publication remain
charged under the existing limits. Thirteen new tests cover warm/cold rejection,
an independent 24-CFG edge oracle, all shorter budgets, identity isolation,
atomic publication and repeated-query savings. One-shot overhead is also tested.
All 665 lowerer tests and 4,795 component tests pass (51dk), and all 12 AMD
compiler callbacks pass (51ca). Full-source mixed109 still produces zero bundles;
these results do not establish a final-output proof or GPU execution.

Checkpoint125 adds an independent 32-file work ledger beside the existing
64-site diagnostic table. Both ledgers count every accepted debit, preserve
overflow totals, and reconcile against the same budget. The controlled
materialized-attention runs mixed110/111 have exactly equal stdout and
non-profile stderr. All accepted work in that failed prefix is attributed to
20 compiler files; owner-invalidation indexing accounts for 187,509 units.
This is not a completed analysis or a timing result. Six new Rust regressions
and 55 diagnostic-parser tests pass.

The owner-invalidation index now uses compressed block offsets and a flat vector
of statement positions. It retains the original full statement scan and
invalidation predicate, with constant-time access to empty block rows. Allocation,
growth, copies, writes and publication remain charged; failed construction
publishes no index. Owner SSA, loan windows, paths, cycles and terminators retain
their checks. An initial global-pair index failed an existing performance gate;
the compressed-row revision passes that gate without changing it. Four new
tests cover exact budget prefixes, cross-block windows, capacity boundaries and
storage. A specific 64-block, three-hit fixture retains 77 words instead of
2,245; this is not a universal cost or GPU performance claim. All 675 lowerer
library tests pass, as do 4,805 component tests (51dl) and 12 target callbacks
(51cb). Full-source mixed114 still produces zero bundles. The initial public
lowerer integration run exposed 14 failures; the corrected tests and ownership
admission are included in mixed115 and the newer results above.

The expanded public tests exposed an ownership-metadata admission gap: a
non-reference scalar could claim `SharedBorrow`. Defined and external callable
ABIs now share the same checked source-input walk. `SharedBorrow` requires an
immutable reference, `UniqueBorrow` a mutable reference, and `RawPointer` a raw
pointer. Other tags grant no new alias or exclusivity authority. Type lookup
and the constant-time shape check are charged before inspection. Four new tests
cover both ABI paths, pointer metadata and mutability, malformed tags, and every
shorter work budget; production limits are unchanged.

Public helper-expansion tests now follow source-to-execution-to-KIR
correspondence instead of assuming block numbers survive expansion. They retain
observable return values, alias writes, barrier effects and storage-lifetime
rejections. Four additional V6 tests check live source/SSA/KIR replay and reject
substituted owners, limits, digests and malformed bytes. The existing production
V6/V2 expanded-source route is reused; legacy V4/V5/V1 evidence still rejects
expanded coordinates. Neither decoding evidence nor passing these tests grants
proof or launch authority.

Mixed67 includes the phase integration, callable-classification cache, exact
BF16 read observations and address-formation diagnostics. Eight normalized
stopping diagnostics differ from mixed64; 39 are unchanged. All four GPT-OSS
use-analysis failures persist. Two private-flow analyses now stop on storage
instead of work; neither produces an accepted result. Fill, typed vecadd,
combine-expert-ranks and gradient staging still stop at the protected proof
runtime. The remaining 43 fixtures stop earlier. No new end-to-end pass is
claimed. The subsequently mounted two-constructor enum join and retained backend
output changes are not included in this sweep.

Mixed64 includes exact Unit RustCall flattening, retained Grid borrow consumers,
typed-custody indexing, private-flow reuse and exclusive enum-index resolution.
Ten normalized stopping diagnostics change; 37 are unchanged. MoE top-2 still
exhausts private-flow work during a join. MoE route retains the same storage
failure, with less work spent before that failure. All four large GPT-OSS
variants still exceed the unchanged use-analysis limit. FP4/FP8 attention still
exhaust typed analysis, and row-softmax still rejects the enum projection.
These changed stopping observations are not kernel passes or measured speedups.
The subsequent captured-Workgroup borrow fix and row diagnostic observer are
not included in this sweep; all 15 new central tests pass. Actual compiler-input
checks retain 15 passes and three failures; a targeted row diagnostic run is
complete. It still produces no bundle. The row index comes through a move from
an enum with two original definitions; the diagnostic stops without choosing
either constructor. All three binary hashes and the frozen source remain stable,
and the targeted run's scratch directories are removed.
Targeted MoE run mixed66 uses the subsequent join-comparison optimization:
both exports still fail and produce no bundle. Routing reaches the identical
storage failure with 15,090 fewer prefix work units; top-2 still exhausts work,
now at join block 116. Neither result establishes a kernel pass. The two source
identities, complete raw captures and three binary hashes were checked, and all
three scratch paths are absent. Report SHA-256:
`decd49bd61e0d277ad58649c20538a02d8bb3908978351588d66fcf4869647e5`.

Mixed63 measures lazy terminal validation and includes the diagnostic-only ABI
observer and bounded incoming-caller query. Three normalized stopping diagnostics
change; 44 are unchanged. All four large GPT-OSS kernels still exhaust use
analysis. Different final debit sizes do not establish reduced total work or
clearance of a work limit. No prior blocking kernel completes export.

Mixed62 includes the Grid KIR consumer, Policy carrier, row-source argument
support and two analysis optimizations. Ten normalized diagnostics change;
37 are unchanged. No previous blocking kernel completes export. MoE top-2 still
exhausts private-flow work, now in statement processing; a different stopping
coordinate is not a measured speedup. Four GPT-OSS variants still exhaust use
analysis. Row-softmax now identifies the exact unsupported enum-payload
projection. The preceding mixed61 attempt stopped on disk exhaustion without
publishing a report; it is not counted as a completed sweep.

Mixed60 integrates guarded Grid source/SSA support, the BF16 canonical-local
correction and the checked phase emission handoff. One normalized diagnostic
changes; 46 are unchanged. MoE top-2 clears its undefined issuer-return SSA
failure and reaches a missing Global receiver binding. Its private-flow analysis
exhausts work during a join (2 units remaining, 12 requested; 677 locals and 412
blocks). This is not an exported kernel. Phase and Grid KIR lowering remain
incomplete. Source-export records, rather than preparation records, are counted
for this source-mode report; the corrected checker independently reconfirms
mixed59 also produced zero source bundles.

Mixed59 adds the phase source/SSA fixes, retained definition-index reuse,
surviving-component and Global-reference audit optimizations, and canonical
BF16 forwarding block-order support. Eight normalized diagnostics change;
39 are unchanged. Materialized attention clears the mixed58 SSA work-limit
regression and returns to its earlier Global-origin memory-refinement blocker.
Four large kernels still exhaust use-analysis work despite lower fact costs.
Flash now stops earlier at the phase receiver borrow-adjustment check; both
Muon variants stop at the live completion relay. These earlier importer
rejections do not establish progress through the previously failing stages.
FP4/FP8 retain their primary typed-work failure. Fresh debit traces now name
old-epoch and typed-flow checks respectively, not the older reachability site.
Neither trace establishes the full cumulative cost. Guarded Grid and phase KIR
lowering remain outside this sweep.

Mixed58 adds used-lane transport, multi-field shared Global auditing and an
acyclic loan-region check. Six normalized diagnostics change; 41 are unchanged.
No complete export advances. Flash Attention and materialized attention regress
to the unchanged SSA work limit. Decode, held fragments, interleaved stores and
serial router also stop during use collection, before ordered path checks.
Reducing audit overhead remains required. FP4/FP8 attention's primary work-limit
diagnostics are unchanged; the acyclic optimization does not clear them.
The subsequent phase source/SSA integration and retained definition-index reuse
are not included in mixed58. Both compile and pass their component tests.
After the exact mutable-reborrow, replay-order and synthetic-variable namespace
fixes, both target callbacks pass complete two-phase source-carriage and
consuming-SSA validation. Both lease-escape rejection tests also pass
(actual51af: six passes). Actual51ag repeats these six passes, including the
duplicate Bind-receipt rejection in both SSA callbacks. Two separately labelled
Rust-negative callbacks also pass: they require the exact duplicate-borrow
diagnostic and verify that fe2o3's analysis callback was never entered. These
two are not semantic-proof passes. None of these checks establishes phase KIR
lowering, memory publication, consuming outer-loop support or GPU execution.
Actual51ak passes all eight callbacks with the additional ownership tests.
Both SSA callbacks check two source-derived positive controls, eight alias or
storage-restart rejections at the original Borrow queries, and one dead-storage
rejection at the exact earlier partial-move diagnostic. The emission-handoff
positive and eight exact substitution checks also pass. The earlier failures
in actual51ai/51aj were incorrect test-stage expectations, now corrected without
changing production ownership rules.
Surviving-component and Global-reference audit optimizations pass all 18 new
component tests. Mixed59 measures lower fact costs and the one restored
materialized-attention path, but no complete export.

Mixed57 adds exclusive Global capture, ordered private-state joins, descendant
path reuse, subgroup terminal-first auditing and failure diagnostics. Eleven
normalized diagnostics change; 36 are unchanged. Row softmax clears its missing
Global receiver and now requires an exact invocation-derived store index.
Four-branch residual clears private capture and reaches the missing global-write
effect contract. Neither change is a completed export. Other private-state
work/storage failures remain, including explicit-reuse attention and MoE route.
The subgroup early exit does not remove any of mixed56's four SSA work-limit
regressions: surviving components still incur the full statement audit. Held
fragments fails before ordered path checks, so its descendant-path optimization
is not reached. FP4/FP8 attention both exhaust typed analysis during a CFG
reachability edge query; this identifies the failed debit, not cumulative cost.

Mixed56 includes disjoint-sibling Move, guarded BF16 Result custody and closed
subgroup lane transport. Six diagnostics change after hash normalization; 41
are unchanged. FP4/FP8 attention clears its missing SSA-use rejection and reaches
typed-analysis work exhaustion. Decode, interleaved and serial-router regress
from lowerer SSA-use failures to the unchanged 262,144 SSA-analysis work limit.
Held-fragments still exceeds that limit, now during use collection before any
ordered proof: fact collection rises from 19,076 to 163,050 work units. Reducing
this subgroup-analysis overhead is required before claiming a budget fix.
Gradient staging still stops at the unavailable protected functional-refinement
runtime after clearing GPU guard extraction and CPU/GPU guard matching.

Mixed52 changes one primary failure from mixed51: combine now clears scalar
normalization and CPU/GPU effect matching, then stops at the unavailable
protected functional-refinement runtime. The other 46 primary failures are
unchanged. The compact definition index does not reintroduce either
content-sparse storage rejection; no peak-storage measurement is inferred.

Mixed51 changes seven primary failures from mixed50; the other 40 are unchanged.
Selecting relevant consumers before reachability queries removes three session
work-limit failures without raising the 1,048,576-work limit. Separating private
matrix-only queries from full BF16 sessions removes four omitted-lane failures;
full BF16 sessions still require complete consumption. All seven now reach the
same exact SSA-use rejection as four other matrix kernels. The eleven failures
identify the shared recursive custody resolver, not a completed memory proof.

Earlier lossless source-use storage compaction removed both content-sparse
storage regressions from mixed49, restoring their missing Global-receiver checks
without raising the 2,097,152-word limit. No completed peak measurement or kernel
pass is inferred.

Thirteen complete mixed49 source-CFG censuses find no removable single-entry acyclic
empty-Goto interiors; their source block counts remain 1,074 to 3,713. This does
not justify increasing proof limits or deleting meaningful control flow.
In mixed45, MoE route
passes its previous optional-comparison node limit and reaches a missing Global
receiver origin. Combine passes its previous exact CPU/GPU guard comparison
and reaches incomplete multiple-definition scalar normalization. Flash Attention
and both Muon variants still fail undefined-return SSA validation. The compressed
attention aggregate now retains a capture, but its later receiver failure remains.
These observations supersede older diagnostics below.

The latest successful thirteen-crate component suite (51dk) passes 4795 tests,
with zero failures and 187 ignored tests. All 665 lowerer tests pass, including
six work-profile tests and three retained-plan identity tests. The preceding
51cz suite recorded one runtime mutation-monitor parallel-setup stress failure,
on a `CLOSE_WRITE` event before its intended mutation. That test subsequently
passed isolated, full-verifier and full-component reruns. Its intermittent
cause remains unresolved; no runtime integrity check was removed or changed.
Eight new Bound-reference tests cover
Bind-only source closure, the original owner/SSA query, alias and lifetime
rejections, exact field/source identity, and independent setup/read costs at
every shorter budget. The fixtures were corrected to remove an unreachable
function and make an additional reference type source-reachable; source-admission
rules were not relaxed. Six diagnostic selector tests also pass. The separate
catalog and contract-preimage suites pass 11 and three tests respectively.
The backedge fixture now preserves
defined SSA so its intended aggregate-alias rejection executes. A separate
negative retains the original undefined-edge rejection. The aggregate variant
guard and loan-scanner operand traversals now debit their variable-sized work;
independent growth and one-short budget tests pass without changing the limits.
The restored environment clears the
fourteen previous protected-execution, descriptor-permission and OpenSSL
failures. The corrected Grid fixture now reaches and validates its intended
cached-payload rejection. All 4787 captured source inputs remained unchanged
through completion, including the metered shared-reference and definition caches.
Ignored tests and internally skipped external-runtime
checks are not proof or GPU evidence. The logical enum-tag storage regression
passes for both variants and Copy/Move. No failure is
counted as passing. Five private-capture CFG tests pass after correcting their input
fixtures. Supported predecessor/backedge cases retain the complete dataflow and
mutation checks. Unsupported assert cleanup must still reject before analysis;
that negative test is not proof of cleanup execution support.

The preceding 51cm suite completed with 4598 passes, one failure and 186
ignored tests. The path-region scratch change exceeded the existing 20,000-work
limit on the unchanged 1536-block chain; its new tests passing did not erase
that regression. A one-line successor reduces initial predecessor-row capacity
from four to one, retaining both complete graph walks and every debit. The
original regression now passes, with independently checked work of 18,473;
all 609 lowerer tests and eight new shape-cache tests pass in 51cp.
Actual51bq separately passes nine of twelve
AMD-target compiler callbacks, including both three-phase ownership tests and
their eight invalid final-phase mutants; three BF16/lifetime callbacks still
exhaust the unchanged analysis limit. All six suite outcomes match 51bp;
the complete failure-prefix work profiles are byte-identical. Neither the
retained-plan optimization nor the Math-bound opacity fix changes these
callbacks' stopping points. Binary and target metadata hashes remain stable
through the run. These are compiler tests, not GPU runs.

The acyclicity analysis now retains only paid vector capacity, resetting all
degrees and the queue before every complete Kahn traversal. Nine regressions
cover masks, cycles, duplicate edges, exact cold/warm work and failed
publication. Existing direct tests exercise the production helper; a separately
named cold implementation is only an independent test oracle. The same failing
BF16 callbacks in 51bk reached 90 retained loan rows instead of 82, but still rejected
at the unchanged 1,048,576-unit limit. Reaching a later analysis boundary is not
a completed compiler or GPU pass.
Endpoint-only loan regions now reuse a body-bound strongly connected component
index, constructed from the verified SSA plan's DFS reverse postorder and exact
incoming edges. An endpoint path intersection contains whole components, so
this index can classify cycles without repeating Kahn traversal for each pair.
Arbitrary masks still use the complete Kahn algorithm. Every distinct loan still
checks owner values, ordered invalidations and terminal effects. Ten new tests
pass, including owner substitution, failed publication, duplicate edges and the
original cycle rejections. A new 1536-block fixture initially failed canonical
block-ID ordering before graph analysis; correcting only its IDs preserves all
statements and edges. The original 20,000-unit regression remains unchanged and
passes at 18,474 units, including the new retained index pointer. In 51bl, the
same three failing BF16 callbacks reach 97 loan rows and 65 reachability rows,
then exhaust the original limit when publishing the next complete loan result.
Neither the larger validated prefix nor passing component tests establishes a
source bundle, numerical output proof or GPU execution.
An ordered-region experiment passed five differential and budget tests but
regressed the actual BF16/lifetime workloads in 51bm: the same three cases
exhausted the unchanged limit after 90 retained loans instead of 97. List
construction cost outweighed avoided membership scans. The experiment and its
test-only reference copy were removed; the original bitmap implementation and
all its tests were restored byte-for-byte. Component correctness alone was not
treated as evidence of a useful optimization.

A default-off storage diagnostic records the first statement or terminator that
excludes one selected local from SSA promotion, plus a selected Borrow's
transparency flag. It observes the existing classification walk without changing
its predicates or adding an SSA use. Its statement label is not a claim about
which nested place caused the exclusion. Ten tests preserve complete classifier
vectors, actual adapter inputs and planner results, including undefined-use
rejections; they also check selector grammar and bounded output. The environment
selector is diagnostic-only and cannot grant promotion or admission authority.
Source77 identifies the attention failure more precisely: the queried borrow
at block 693, statement 2 is transparent, but block 699, statement 0 excludes
the same owner local from promotion. Source78 disables observation with identical
inputs and binaries; its complete stderr is byte-identical after removing the
five diagnostic records, and its stdout and final failure also match. Neither
run exports a bundle. Source79 selects the later statement and confirms its
Borrow references the same local and is not transparent; its non-diagnostic
stderr also matches source78 exactly. The next correction must justify the later borrow's full
use chain, not synthesize a missing SSA read or waive storage visibility.
Global carrier classification now uses a two-bit, type-indexed memo instead of
tree lookups and a front cache. The four states are unseen, pending, completed
false and completed true. Exact type-owner checks and failure poisoning remain;
cycles and invalid fields still reject. The previous implementation is retained
only as a test oracle. Twelve new tests pass, including original source-local
census comparison, packed-coordinate isolation, independently calculated nested
traversal and constructor work, every insufficient budget, and same-pointer
shortened-owner rejection. These component results do not establish a source
export or whole-kernel performance improvement.
The initial Global declaration walk now holds one exclusive classifier borrow
and checks its exact type owner once for the whole walk. Every declaration,
classification and immediate flow insertion stays in its original order; scalar
queries outside the walk retain their individual owner check. Any returned error
poisons the classifier, including a callback error after a partial flow write.
No result census, iterator or extra retained allocation is introduced. Ten new
tests pass against the original scalar walk and independent cost traces. The
completed walk saves exactly N - 2 units for N declarations; zero and one local
cost more, and that tradeoff is tested rather than hidden by a changed limit.
The separate capability catalog passes 11 tests, and three
independent V13 regressions preserve captured KIR, LLVM and target-closure bytes
on gfx942 and gfx950. The
malformed-capture diagnostic fixture now correctly requires an explicit Invalid
state, not an absent entry. Earlier checks passed 587 lowerer, 458 MIR-model and
711 PLIRON tests; the current suite passes 619, 458 and 751 respectively.
The latter includes shared Workgroup roles, independent classification
oracles and retained candidate-source classification. These do not
establish acceptance of heterogeneous owners by the real ordered-loan checker.
The new per-owner immutable invalidation index passes comparisons with the
previous loan checker, generated CFG cases, and exact work-budget boundaries.
It preserves source ownership, path, cycle and lifetime checks; it does not
increase their allowance. Its cold construction can cost more than a one-off
scan, so the component tests alone do not establish a real-kernel speedup.
Four phase-epoch tests cover exact source occurrences and reject stale operands.
The new arithmetic-model identity test and separate protected MI350 proof run
are described under explicit numerical limits below.
Recent passing tests cover phase source/SSA transport, exact mutable
reborrow handling, retained definition-index reuse and audit/observer changes.
Recent passes add two-constructor enum joins, cleanup/shared-target
rejections and retained backend output checks through worker handoff. These
changes preserve the existing exact-source, target and legacy-artifact gates;
they do not establish a numerical-error bound or a row-softmax export.
Seventeen additional tests cover copied enum payload lifetimes and shared
Workgroup wrapper transport. A new negative test was corrected to inspect
the source-attributed error's cause and exact carrier local; the compiler's
rejection behavior and all resource limits remain unchanged.
Twenty-eight further tests pass for initialized private mutable-field reads,
streaming canonical digests and retained backend replay budgets, diagnostic-only
Workgroup custody, and the full shared-carrier analysis budget. The private read
still requires its original mutable borrow and rejects uninitialized aggregate
state. The digest path preserves the old encoding and shared remaining budget.
Twenty-seven more tests pass for mixed enum lineage, distinct-type analysis
reuse and the separate namespaces of source locals and phase completion tokens.
Actual51ax confirms that the namespace fix removes the observed phase panic;
the callbacks still fail later in the checked phase emitter.
These changes are not included in the latest full source sweep above.
Earlier lane, Global transport and acyclic
loan-region tests remain passing. These do not establish an end-to-end kernel pass.
All 36 new scalar-enum, Grid inventory and BF16 source/address tests pass.
The BF16 address result is conditional; it does not establish final volatile-memory
correspondence or a floating-point output-error bound.
Eight further tests cover post-join comparison of shared immutable private
states, preserving normalization, resource accounting and source-bound reads.
Their passing result alone does not establish MoE source-export clearance.
The matrix/transpose test
fixture corrections preserve the storage and exact-error invariants. Three
additional canonical-attachment tests pass. The combined MIR25/transpose source
integration compiles. Actual51v passes both FP4/FP8 owned-source callbacks,
including source matching, Wave64/Wave16 partition checks, canonical flow
binding and downstream lifetime rejection checks. The test now requires the
exact existing promoted Workgroup Use; separate retained-source tests still
reject invented SSA values. This does not unblock the full attention kernels'
typed-analysis work failures. Actual51w
passes the same two owned-source callbacks and both exclusive
Global-capture callbacks; matrix, original BF16 and complete BF16 each fail
both target checks (four passes, six failures). The complete-source BF16 failure
identifies an unpromoted Global owner at block 5, local 20. Actual51x disables
both diagnostic flags and preserves the four passes and two complete BF16
failures, with unchanged binary and metadata hashes. These are compiler
callbacks, not hardware runs. Actual51y retains four passes and six failures:
both BF16 checks clear their prior failures and now stop at a Policy borrow
and exact constructor forwarding respectively. The matrix subgroup borrow
still fails. All binary/metadata hashes remain unchanged during that run.
Actual51ac again passes both owned-transpose and exclusive-Global callbacks;
both complete BF16 callbacks still fail the exact forwarding-wrapper check.
Its canonical block-order correction passes all three new component tests,
but actual51ae still rejects both callbacks at the same wrapper check.
Actual51ag identifies a second assumption: canonical locals are reordered,
while forwarding and receiver transfer still use Rust's original local numbers.
The diagnostic-off actual51ah run reproduces both failures with unchanged binary
and metadata hashes. The source-role-based correction passes all six new
permutation tests, including exact receiver IDs and same-typed argument-swap
rejection. Actual51ai captures all four source inputs, then both callbacks
stop at the first constructor-lane query with a missing exact SSA use.
The Policy carrier correction now preserves the exact shared reference through
closure capture and helper forwarding. Nine integrated Policy tests pass;
actual51al passes both complete-source BF16 callbacks, including all four ordered
lane queries, duplicate-query rejection and exact Global/Bind identities. The
test still rejects completion without the required memory reads: this is not a
memory proof or a kernel export. The old bind-only filter passes gfx942 and the
first gfx950 case, then fails canonical ABI validation for the repeated-call
gfx950 case; its third case is unrun. Guarded Grid source/SSA
support and four exact accounting tests are integrated. After correcting the
lifetime/type-argument distinction, actual51aj passes both Grid source/SSA
callbacks on gfx942 and gfx950, including cross-root substitutions and replay.
The Grid KIR consumer and exact source-event queries are integrated with six
representation, four source-body and three retained-event tests passing.
Actual51al reaches the ranked store consumer on both targets but then rejects
KIR construction: two retained borrows still lack authenticated consumers.
Grid KIR lowering is therefore still incomplete. All four binary/metadata hashes
remain unchanged during actual51al. These changes are included in mixed62,
which still produces no source bundles. Four geometry regressions
confirm that the singleton Grid store requires the actual rank-one launch and
rejects multidimensional launches. Four borrow-analysis regressions preserve
all escaping operands and exact work exhaustion while skipping irrelevant
capture lookups for non-assignment statements. Fifteen row-index tests cover
failure diagnostics and exact by-value source-argument ownership, ABI, overflow
and invocation-dependency checks. They do not establish complete row-kernel
support or a measured improvement on the large tutorial kernels.
Nine immutable-join regressions also pass: reuse requires equal complete states
and unchanged dead-local/escaped-reference normalization. Differential tests
retain the original meet, late-backedge invalidation and exact budget failures.
Actual51am repeats all eight phase, two complete BF16, two owned-transpose and
two exclusive-Global passes. Both Grid callbacks still fail the same retained
borrows. The broader transpose filter also runs two older source-SSA fixtures;
both fail because their publish argument is zero-sized rather than the required
owned move. All four binary/metadata hashes remain unchanged during the run.
The strict four-load assertions are unchanged.
Actual51an passes the same fourteen phase, complete BF16, owned-transpose and
exclusive-Global callbacks after lazy terminal validation. Three new regressions
preserve escaping and projected references and exact shared work accounting;
scalar-only calls no longer repeat unrelated terminal validators. Mixed63
measures this change without clearing any kernel's work limit. Five
diagnostic-only ABI tests pass.
Actual51ao still rejects the repeated BF16 gfx950 callback, now localized to a
RustCall function signature with a shared closure receiver and a zero-sized
argument tuple. The diagnostic does not change admission or prove a fix.
The subsequent Unit-as-empty-tuple ABI correction passes six component tests,
preserving exact source counts, ownership, pointer attributes and nonempty
tuple rules. Seventeen typed-analysis tests cover shared type/use inventories
and reverse structural closure; six private-state tests cover immutable join
probes. Nineteen Grid tests cover authenticated primitive-read consumers and
source custody. These changes still require the next actual-source sweep;
component success does not establish any newly exported tutorial kernel.
Actual51ap passes fifteen callbacks and fails three, with all four binary and
metadata hashes unchanged. The prior fourteen phase, complete BF16, transpose
and Global callbacks remain passing; bind-only BF16 also passes gfx942. Both
Grid callbacks clear the retained-borrow rejection and reach missing scalar
enum-payload lowering. Repeated gfx950 BF16 clears canonical ABI validation,
then its cross-instance negative stops at an unsupported shared-Workgroup
projection instead of the required owner/loan check. Neither family completes
its full callback. The unchanged strict assertions remain required.
Fifteen further tests pass for exact enum-constructor index transport, including
backedges, source occurrences, invocation dependence and checked overflow.
Seven private-state tests pass for sharing immutable state across empty neutral
blocks while preserving all original edges, joins and publication checks.
These do not establish actual row-kernel clearance or MoE memory headroom.
Actual51aq repeats the same fifteen passes and three failures after these
changes. The full Grid and repeated-BF16 rejection records are unchanged;
all four binary and metadata hashes match before and after the run.
Actual51ar again passes fifteen callbacks and fails three. The captured-borrow
change clears the prior Workgroup projection rejection, but repeated gfx950
BF16 now lacks an exact original SSA use before its required cross-instance
owner check. Both Grid callbacks retain the enum-payload failure. The same four
binary and metadata hashes remain unchanged during the run.
Actual51as retains fifteen passes and the same three failures after the enum
alias and BF16 address-binding changes. The complete BF16 source callbacks pass
their added event and binding checks, but neither the Grid enum rejection nor
the repeated-BF16 SSA-use rejection is cleared. Component coverage does not
establish full source correspondence or a completed kernel run.
Actual51at has thirteen unique passing callbacks and seven failures. The eight
original phase-source checks still pass, but both new source-to-phase-KIR checks
stop at an unranked private Workgroup header read. Both complete BF16 source
callbacks now exceed the unchanged shared analysis budget in the added live-read
handoff checks. Grid and repeated-BF16 failures remain. This run left these
regressions open; no limit or assertion was relaxed. The seven selected filters repeat the
two new phase tests, giving thirteen passes and nine failures when counted as
executions rather than unique tests. All four binary/metadata hashes stayed fixed.
Actual51au restores both complete BF16 source callbacks on gfx942 and gfx950
under the unchanged 1,048,576-unit source-session limit. The address theorem now
queries the complete ordered list of included guard predicates rather than
repeatedly scanning every formula slot. Ten new component tests preserve the
old theorem's non-budget results and exact budget failures. Source/owner replay,
all four read observations and both mutation checks are unchanged. All four
binary/metadata hashes remained fixed. This closes the added read-handoff work
regression, not the separate final-memory proof, repeated-BF16 SSA use, Grid or
phase-KIR failures. No complete kernel export or GPU execution is established.
Actual51av reruns six unchanged callbacks: three pass and three fail. Both
complete BF16 callbacks still pass. Grid now stops at a later enum-alias chain;
the repeated-BF16 callback still lacks the same exact original SSA borrow use.
Neither failure is cleared by the new component tests. All four binary/metadata
hashes stayed fixed; these are AMD-target compiler callbacks, not GPU runs.
Actual51aw adds the Workgroup-specific diagnostic and phase-header integration:
three of eight callbacks pass. Both complete BF16 callbacks still pass; the
repeated-BF16 check and two new carrier-lifetime callbacks stop at the existing
missing borrow use before the lifetime mutations can be validated. The trace
shows another capability's selected field route hiding the requested Workgroup
field; it does not justify overwriting that existing route. Both phase callbacks
get past the private-header read, then panic when a generated SSA variable is
used as a source-local index. All four
binary/metadata hashes stayed fixed; no source bundle or GPU result follows.
Actual51ax passes three of seven callbacks. The namespace fix eliminates that
panic on both targets, but the checked phase emitter now reports an input
producer rejection. Its precise failing invariant is not yet identified.
Both Grid callbacks pass the previous mixed enum lineage and
stop at a later Result payload transfer. The existing Workgroup full-import
callback and both complete BF16 callbacks pass. All 189 tracked compiler inputs
and four binary/metadata hashes remain unchanged during the run. No completed
export, numerical proof or GPU execution is established.
Actual51ay retains three passes and four failures across the same seven callbacks.
Grid passes the previous inactive-residual transfer and stops at an aggregate
payload alias. The phase diagnostic identifies Begin in phase-roster row 1 and shows a
different producer workgroup brand from the expected outer brand; this remains
an open identity/transport issue. Both diagnostic records are complete. Workgroup
import and complete BF16 callbacks still pass, with all 192 tracked input hashes
and four binary/metadata hashes unchanged. No end-to-end pass is inferred.
Actual51az tests the shared Workgroup role changes: the repeated BF16 callback
passes on gfx942. Its gfx950 counterpart and both lifetime callbacks advance
past the previous captured-owner rejection, then exhaust the existing
1,048,576-unit matrix source-analysis budget. Their later lifetime assertions
have not executed. Both complete-input BF16 callbacks pass. The six callbacks
therefore yield three passes and three failures, with all 196 tracked compiler
inputs and four binary/metadata hashes unchanged. These are compiler-source
callbacks, not GPU execution or final numerical proofs.
Actual51ba reruns eight callbacks on the latest component-tested compiler:
three pass and five fail. Both phase callbacks advance past the Begin brand
mismatch and reject with a synchronization mismatch in neutral lowering.
The gfx950 repeated-BF16 check and both lifetime callbacks still exhaust the
unchanged analysis-work limit before the later lifetime assertions execute.
The gfx942 repeated-BF16 and both complete-input callbacks pass. All 4713
captured source inputs and four binary/metadata hashes remain unchanged.
No tutorial export, numerical proof or GPU success follows from this rerun.
Actual51bb passes both original guarded-Grid callbacks on gfx942 and gfx950
after preserving ordinary aggregate enum payloads through their original
constructor and scalar storage. All 4715 captured source inputs and four
binary/metadata hashes remain unchanged. These are compiler callbacks, not GPU
runs. The subsequent guard-work correction passes its regression tests.
Actual51be retains both Grid and both complete-input BF16 passes, plus the
gfx942 repeated-BF16 pass. Both phase callbacks now pass synchronization
matching and stop at a stale-epoch verification error after a dominating
transition. The other three BF16 callbacks complete 79 loan checks before
exhausting the unchanged work budget; later lifetime assertions remain unexecuted.
The ten callbacks therefore yield five passes and five failures. Their four
binary/metadata hashes and all 4722 source inputs remain unchanged. No source
bundle, final numerical equivalence or GPU execution follows from these checks.
Actual51bf passes both original two-phase source-to-KIR callbacks, including
their stale-operand rejection checks, on gfx942 and gfx950. Both Grid and both
complete-input BF16 callbacks remain passing, as does repeated BF16 on gfx942.
The ten callbacks now yield seven passes and three failures. The other three
BF16 callbacks still stop after 79 completed loan checks on their 618-block
bodies; the next 618-unit path-region debit exceeds the remaining 546 units.
The 1,048,576-unit limit is unchanged, and later lifetime assertions have not
executed. All 4730 source inputs and four binary/metadata hashes remain unchanged.
These are compiler tests, not completed tutorial exports or GPU runs.
Actual51bg retains seven passes and three failures on the next compiler.
The extended complete-input BF16 tests now exercise the original source session
through the actual root allocation, provenance, extent and view tables on both
targets. They reject copied source bodies/types and confirm that binding inputs
does not consume guarded reads or allow root completion. Numeric lane mapping,
guarded-read materialization and final memory/output proofs remain incomplete.
The added source-issuer storage is charged: the same three larger BF16 cases now
have 366 units left before the denied 618-unit path-region debit. Their later
lifetime assertions remain unexecuted. All 4736 source inputs and four
binary/metadata hashes are unchanged through the rerun.
The shared incoming-caller query passes four integration tests. It reconstructs
the existing bounded canonical roster for ordinary closures as inert metadata;
source authentication and final emission remain separate required checks.
A separate rerun
passes all 28 import callbacks and all seven
wrapping callbacks. None establishes a completed kernel qualification.
The compact-row source/GPU coordinate, bounds and placement integration passes
its component tests. Read-index projection clears the missing GPU guard
definition. Guard-only constant normalization then equates literal 256 with
checked 16 * 16 only after excluding unsigned overflow. The unchanged actual
source callbacks now pass on both targets, through request preparation; they
permit the specific unavailable-proof-runtime result and are not proof passes.
All four source rejection tests also pass, including the smaller-lane guard
mismatch. Both BF16 original
constructor callbacks pass after correcting the provider path and the test's
raw/canonical block mapping. Complete-source tests now pass variant-sensitive
Result payload custody but reject the next exact borrow-use lookup at original
block 5, local 20. The source-session and guarded-read
obligations are not completed by passing component tests. The ordered matrix
capture scan passes nine new tests without increasing the work limit. Mixed54
measures lower use-analysis work, but the full held-fragments kernel still
exceeds the aggregate limit.
The BF16 source-capture fixture now tests an owned
construction join before one shared borrow; the original unsupported split-reference
join remains a passing fail-closed regression. Retained-source SSA queries,
six fixed-storage tests, 15 bounded CFG diagnostic tests and nine BF16 arithmetic-oracle
tests pass, but do not establish a read proof. Fixed source-use buffers replace
growing rows without raising resource limits; mixed50 verifies the two restored
content-sparse frontiers described above. Scoped matrix custody now retains Context loans,
accepts the exact mutable Context receiver and validates RustCall tuple field
transport. Four loan-retention tests pass, including the separate early SSA
rejection for deinitialization. Exact branded FP4/8 accumulator-zero imports
and their terminal-ABI negative tests now pass. New real-source matrix/BF16
callbacks still fail recursive exact SSA-use recovery in the shared custody
resolver. Observing the original blocks shows shared borrows of owned values:
the reference is defined, but the queried ordinary SSA use of its owner does
not exist. A source-bound borrow relation is still required; these diagnostics
do not prove memory,
numerical, KIR or GPU correctness. The suites still exit unsuccessfully. Catalog
integration adds 60 passing tests. The latest full actual AMD-target callback
suite has 63 passes and 13 failures across 76 tests, restoring the earlier
failure roster after correcting a width-mutation fixture that changed both CPU
references. The guarded-index integration adds no callback failures. These are compiler tests, not
GPU executions: reusable LDS, ordered Math, shared-Global products and independent
output guards pass their focused checks. Five new exclusive-output CPU-reference
binding callbacks also pass, including malformed-binding negatives. Six transpose
callbacks still fail exact move-consumption assertions. Two blocked-store callbacks
now retain their receivers and reach eight write sites without CPU-reference
contracts, still rejecting before completed KIR. The new full-projection combine
callback still requires success and now fails at the protected proof runtime;
its diagnostic is not an equivalence proof. The retained scalar-definition index
and exact store-site normalization pass their component tests, including six
guard-coupling regressions, and produce the mixed52 combine progress above.
Ten additional guarded-index tests pass, including the real path-bounds
consumer: preceding row/lane predicates can prove arithmetic slice bounds
without an output-domain assumption. A stricter write-domain mutation still
fails matching. This bounds extension is included in mixed53 but does not
enable mapped output coordinates by itself.
Seven focused real AMD-target callbacks now pass original-core wrapping helper
authentication, unsafe/impostor rejection, and constant extent arithmetic.
The exact checked value expression remains intact; only known unsigned overflow
flags and constant non-bounds assertions are evaluated. Unknown overflow still
rejects, and bounds assertions never become assumptions. These changes are
included in mixed53. Four matrix/BF16 callbacks still reject exact
SSA-use recovery; passing a retained borrow test is not a complete memory proof.
Eight CPU-reference tests pass through the binding-aware host runner. Gradient
staging now has an independent per-invocation output-frame reference, but its
targeted export cleared the core-helper locality gate and then stopped at a
constant overflow-field projection. The fix passes the focused callback and
the actual mixed53 source now reaches the mapped-coordinate rejection; it is
not a completed binding or kernel pass. A separate corrected storage-boundary integration
suite passes 805 tests, including the textual SSA suite and unchanged independent
396-case planner goldens; this repairs an older stale expected error, not a kernel.
None of these results establishes a completed tutorial qualification.

The retained clean local source sweep names diagnostic snapshot
`22c46013c9e7a839ee6f434a29608638ce76fb2b`, tree
`f0563226a881658b416561958043977f6f6cf1c9`. All 47 exports failed: 10 gfx942
and 37 gfx950. The report has SHA-256
`525d50b736052b62dd08b014c9389032eb629cfd285fb5f50fc5f20dd56c3c12`
and explicitly grants no qualification authority. It records no Bundle V8,
simulator comparison, or hardware execution. These are diagnostic snapshot
identities, not a published release or the current uncommitted source tree.

The sweep recorded successful cleanup for all 47 fixture scratch directories.
Fill and vecadd reached the unavailable protected-runtime check; this does not
establish that their later gates pass. Nine fixtures stopped at borrowed
subgroup transport, eight at terminal ABI construction, five at policy-bound
matrix issuance, and three at execution-callable carriage. Eight compiler
process failures shared an invalid `fn_sig` query on an external closure.
The remaining twelve failures include
unreviewed arithmetic/shift helpers, source pointer handling, missing write
contracts, and unresolved overflow or epoch provenance.

A later targeted diagnostic reran 13 frozen fixture inputs with updated local
compiler binaries. All eight crashing kernels now reject normally at reviewed
external closure provenance. The three typed-global cases pass their previous
incorrect execution-token contract check: both gather variants reach SSA
resource checks, while gradient-shard reaches source ownership projection. MoE
top-2 passes `u64::checked_add` source authentication but still fails unchecked
arithmetic provenance. The sampled attention ABI failure is now identified as
`Math::current` with its `UnbrandedCapability` type argument. All 13 still fail;
this mixed compiler/input diagnostic does not replace the complete sweep.
The extractor SHA-256 is
`25bccfa69d8210f1d2adec51a3dd305ef1d4260a7b4deb36605d6f9802d2af27`.

A subsequent complete 47-input mixed diagnostic also records 47 failed exports,
zero bundles, and 47 successful scratch cleanups. Its report SHA-256 is
`183dd3512e7b85d91856ed1051c18fb9f9c6953b104cd2d462b6d3c7e4a77fb1`.
All eight external-closure failures and nine borrowed-subgroup source-import
failures advance to later checks. Matrix terminal expansion, arithmetic proof
transport, ranked memory projection and canonical Math records remain blockers.
Math diagnostics expose an invalid assumption that canonical local and block
IDs preserve rustc numbering. Both gather kernels still exceed the live
move-state storage ceiling; gradient-shard still rejects source ownership.

This mixed run uses the same frozen checkpoint-7 inputs with a newer compiler
DSO, SHA-256
`7d50defee22fb1f620dd252214ed1dabe027f2dddd037130d1a43ac410884794`,
verified unchanged before and after the sweep. Exporter/extractor executable
hashes alone identify only dynamically linked launchers, not that compiler
implementation. The source candidate in this mixed report identifies its
inputs, not a clean compiler build. No mixed diagnostic grants qualification
or authenticates the complete dynamic-loader closure.

The complete mixed diagnostic mixed24 records 47 failed exports,
zero bundles, and 47 successful scratch cleanups. Its report SHA-256 is
`d13b63f2ad00d76a9f59f11b3ebc967656fc288060a0fc6c17d750a924eb8fd8`.
The checkpoint-7 inputs are unchanged; compiler DSO SHA-256
`d40b3f16aa801cd914a8b6e132621a20b16e8048fadc5499a6105b4f7d16eac9`
was checked before, during, and after the run. This remains mixed-source
diagnostic evidence, not a clean compiler qualification or GPU run.

Since mixed20, all twelve unchecked-subtraction failures, both four-branch
undefined SSA returns, and five BF16-load preflight failures clear. Wave64
advances past its private scalar read to the exact write-effect contract.
Eight new SSA storage failures join recompute-prefix; five kernels now stop at
ranked numerical-policy issuance. Scoped workgroup indices expose an ABI gap.
Flash attention advances past checked division to pointer coercion. Speculative
transaction clears dataflow but still needs an exact store index; both gathers
remain at the ranked work ceiling. Matrix/collective expansion, typed memory
contracts, canonical borrow transport, and protected runtime remain incomplete.

The mixed26 diagnostic again records 47 failures, zero
bundles, and 47 successful scratch cleanups. Report SHA-256:
`a3ed7947203b8c7de22f7057deb8fff7559dd38f852b95d5e2843c491c780108`;
compiler DSO SHA-256:
`17a713dc4fde81b4fa09595dbaef191e7265254b30dc86b61cdda57c98d753d5`.
The same checkpoint-7 inputs and unchanged binary hashes were retained through
the run. Both gathers clear the dataflow work limit and now require exact store
indices. Both wrapping shifts clear and expose unreviewed narrow-width wrapping
subtraction. Two GEMM ABIs advance to legacy unbranded matrix multiplication.
KDA now hits the graph-work ceiling after adding eager helper-result range
analysis; this is not evidence that its arithmetic assertion has been proved.
There is still no clean compiler qualification, simulator comparison, or GPU run.

The mixed28 diagnostic retains the same checkpoint-7
inputs and records 47 failed exports, zero bundles, and 47 successful scratch
cleanups. Report SHA-256:
`1c60184a1baba32dc717c23a1c9cbaa52343199954c209a5051292e6e599d248`;
compiler DSO SHA-256:
`00f1f94814e1684df3d1416f03c21e341b9c33c2ea5aaabbe6607c947efb3c26`.
All three binary hashes match before, during, and after the run. First failures
change for the four exclusive-index, four ordered-maximum, both narrow wrapping
subtraction, and five numerical-policy cases. A changed first failure does not
establish that every former check passed: the new Context-entry analysis runs
before index and policy projection. Its work-limit failures mask the prior
checks in both gathers, speculative transaction, and three policy cases.
Gradient shard reaches a later Global metadata check; both four-branch kernels
reach arithmetic assertions. Seven kernels stop at the Context/SSA analysis
work ceiling; nine still stop at the
partial-move storage ceiling. Remaining failures include matrix source contracts,
exact memory/effect correspondence, two arithmetic assertions, projected paths,
caller-location ABI expansion, promoted constants, LDS phases, and protected
runtime admission. There are still no tutorial simulator or GPU comparisons.

The earlier remote report remains retained under snapshot
`cc10cf4a686a3108d8a746add5bf3eec87e0b853`, tree
`710ef722606f283cb272adc9bc3363c2829cd137`, report SHA-256
`9ef21596e3e9cb28a87a052887f02d6c7a0c54264bb453fe6bf9be39e416d3e7`.
Local and remote gate ordering must not be treated as equivalent runtime
coverage: the protected runtime is unavailable in the local sandbox.

Subsequent work addresses concrete Rust helper semantics, mutable slice
reborrows, numerical-policy transport, and final canonical hierarchy analysis.
Helper authentication does not permit dropping callback, destructor, or panic
obligations: collection and semantic admission must still inspect the retained
call graph. Tests using host core metadata do not replace tests of the actual
AMDGPU core metadata used to compile a kernel.

The next local component checkpoint passed eight source-admission tests against
the pinned AMDGPU metadata and three collector-to-canonical producer tests.
This covers the actual AMD `Option` equality/inequality profiles, policy-pair
source identity checks, and retained wrapping-shift lowering. The compiler-test
binary has SHA-256
`fb8ecf1c03c2330eaad0c52366b2e02906bb8358af3579c7913982011a362eba`.
Separately, 158 MIR-model tests and six prepared-final-theorem tests passed.
None executes a GPU kernel or supplies a missing protected proof receipt.

The subsequent integrated kernel-analysis library run passed 220 tests. It
includes scalar FP32 expression projection (preserving separate rounding nodes),
mutation-sensitive live read observations, and rejection of writes without
reference-effect contracts. Live read observations are not memory-equivalence
proofs. The next MIR-model run passed 169 tests, including 11 policy-math tests;
all 10 KIR policy-math tests passed, as did the 38 source-sweep orchestration
tests. The rebuilt production compiler suite passed 847 tests, with 17 ignored
and one protected-verifier failure: this sandbox's OpenSSL binary is not
root-owned. Its nine actual AMDGPU callbacks and three collector-to-canonical
tests passed (binary SHA-256
`54ec3625c305219db1d8915c08f274e670134996cac77dbf38fecc5991649b0d`).
All 12 prepared/final functional-subject tests also passed, including actual
mandatory-schedule graph owners and target-record substitution rejection.
The receipt-bearing completion path still needs the protected runtime.

The MIR V20 checkpoint passes 177 tests, including eight new tests
for borrowed workgroups and retained defined epoch projections. Thirteen live
source-reference export tests pass, including foreign-context substitution and
net-zero mutation rejection. Exported CPU reference, effect, ownership and
numerical data remain inert facts. Final-write admission and production
pipeline integration are still required; dynamic and partial output domains
are not covered by this initial load-free, static-total-view subset.

The integrated Pliron library suite passes all 203 tests after replacing
full-copy move-state reservations with shared sparse states and bounded live
storage accounting. The existing memory/work ceilings remain unchanged.
Differential tests compare move behavior with the prior map/set rules;
the later gather rerun still reaches the live-storage ceiling. Ten
AMDGPU helper callbacks and three collector-to-canonical tests also pass,
including the new retained-call arithmetic mutations. After correcting exact
registration bindings and the fixture working directories, three full-import
tests pass: typed-global carriage on gfx942 and gfx950, and borrowed-workgroup
MIR V20 transport on gfx950. Eleven AMDGPU callbacks pass, including original
Math source-body validation. Both registered policy-Math full imports still
reject with an invalid canonical function ABI. Compiler test binary SHA-256:
`5497f135808581039178376219b36aa6c61207309d66c5299200d92385e19f6c`.

That compiler checkpoint passes 876 library tests, fails 11, and ignores 25.
All eight functional-phase tests pass after repairing their real ranked-memory
fixture. Ten new fixture failures remain in source-argument conversion and
unique-slice ownership tests; the other failure is the unchanged protected
OpenSSL ownership check. The integrated MIR-model checkpoint passes 191 tests;
seven additional whole-document MIR V21 tests still need a successful rerun.
None of these component results supersedes the complete 47-fixture sweep.

The later compiler18d checkpoint builds and passes 884 library tests, with
four failures and 28 ignored tests. All eleven unique-slice source tests pass,
including the exact GPU-root ABI check. Three parameter-mapping fixtures still
fail by-value component lowering; the fourth failure is the protected OpenSSL
environment check. The actual AMDGPU tests pass: fourteen source callbacks,
three collector-to-canonical tests, and all five full imports. In particular,
all thirteen policy-Math operations now import on both gfx942 and gfx950 after
replacing fixed local/block-number assumptions with canonical roles and CFG
edges. The three new external-closure callbacks also pass. Test-binary SHA-256:
`8063f5e07405489ac11b93c76748e2dadab434ad7dca2800d10483edab28fc0c`.
These are source-import results, not Math SSA-to-KIR completion or GPU runs.
The subsequent MIR-model suite passes all 229 tests, including complete Math
documents, canonical permutations, and finite Boolean-call arithmetic facts.
Arithmetic validation follows all structural checks and shares the request
work budget; constant types, moves, joins, unknown calls, unwind and backedges
have dedicated regressions. All 203 Pliron library tests pass, including the
thirteen compact move-state tests and eight borrowed-workgroup adapter tests.
All 124 kernel-IR library tests pass, including six source-occurrence codec
tests. Both actual gather reruns and production source exports remain pending.

The lowerer library checkpoint compiles with 228 passes and seven failures.
Nine borrowed-workgroup resolver tests and eleven Math adapter/transport tests
pass. Two expanded-helper tests expose missing KernelContext borrow transport;
three loaded-value tests require live memory-equivalence proof integration,
and two write-correlation tests lack exact write/effect contracts. Loaded
values cannot be replaced by free variables to clear these failures.

Compiler20 passes 892 library tests, with 28 ignored and only the protected
OpenSSL ownership check failing. All fourteen actual AMDGPU source callbacks,
three collector-to-canonical tests, and five full imports pass. Test-binary
SHA-256: `ee85140d1f5cb15a2c2246fb6f11a7524eae2e2ad35596ab701fdd7f6b0bcb2a`.
These results include repaired parameter fixtures and bounded source diagnostics;
they do not establish runtime equivalence. The subsequent KIR checkpoint passes
130 tests, including distinct checked helper occurrences, exact replay rejection,
and inconsistent expansion-custody rejection. That KIR change is not in mixed20.

The next mounted borrowed-subgroup checkpoint passes all 136 KIR library tests,
54 AMD backend library tests and 34 simulator library tests. The lowerer passes
230 tests, with five loaded-value/write-contract failures still open; both
unchanged expanded-helper tests now pass with closed KernelContext borrowing.
All 240 MIR-model library tests pass, including five BF16 source-contract tests
and six exact unsigned-comparison/subtraction proof tests. Production compiler
integration and actual tutorial reruns for these changes remain pending.

Core25 passes all MIR (243), KIR (139), PLIRON (236), backend (57), and simulator
(37) library tests. The lowerer passes 245 with three live-load-proof failures;
the compiler passes 928 with one protected OpenSSL failure and 58 ignored.
A genuinely memory-free helper passes independent lowering; substituted ranked
verification still rejects. The effect-omission negative now uses a valid source
baseline and rejects at independent memory correspondence. Sparse capability
dataflow passes dense-representation differential, loop-revisit, and exact-budget
tests without increasing analysis limits; mixed26 confirms both actual gathers
clear that dataflow boundary.

Actual AMD callbacks now pass checked-division full import and both BF16 nominal
probes. Both Math source-to-SSA probes pass, but source-to-KIR stops at ranked
policy issuance. Borrowed Workgroup reaches a later SSA argument-transport
rejection; scoped-index full imports still reject their canonical ABI. BF16
full-import probes pass macro setup but reject canonical type layout. Seven
general wrapping-shift callbacks pass, including source-mutation negatives.
These overlapping
component runs do not change the latest full-source result: zero of 47 exports.

Core27f passes 948 compiler tests with one protected-runtime failure and 62
ignored; the lowerer still has three live-load-proof failures. MIR passes 256
with seven new positive-layout fixture failures. KIR (139), PLIRON (236), backend
(57), and simulator (37) pass. Four invocation-index tests exercise the shared
bounded unsigned-expression projector without weakening uniformity, race, or
reference-effect checks. Both AMD-source helper-result range probes pass.
Borrowed Workgroup now reaches the AMD adapter, which still confuses logical
partition size16 with hardware wave64. Matrix/BF16 source integration, canonical
RustCall argument mapping, Math origin transport, and actual tutorial reruns
remain incomplete. These component tests do not change the mixed26 result.

Core28f and the subsequent MIR-only fixture repair pass MIR (275), KIR (143),
PLIRON (236), backend (63), and simulator (40) library tests. The lowerer passes
249 with three live-load-proof failures; the compiler passes 959 with one
protected OpenSSL failure and 65 ignored. Analysis passes 229 library tests and
61 uniformity integration tests. All 11 actual AMDGPU full-import callbacks
pass, including BF16, typed Global access, borrowed Workgroup, and ordered
maximum. Two scoped-workgroup and two BF16 SSA callbacks still reject at the
defined-call ABI expansion check. These are component fixtures, not the 47
tutorial inputs. Full-wave ordered Maximum no longer infers uniformity from
width alone; ordered NaN behavior can differ between lanes. The analogous
broadcast/sum audit remains separate work. Neither GPU host resolved in the
latest connectivity check, and no remote jobs or files were created.

Core29b passes MIR 282, KIR 143, PLIRON 236, backend 63 and simulator 40 library
tests. The lowerer passes 253 with the same three memory-proof failures; compiler
tests pass 972 with one protected-runtime failure and 75 ignored. Analysis passes
233 library and 61 integration tests; all 11 raw floating-point simulator tests
pass. After two test-only fixture repairs, all 16 actual AMDGPU full-import
callbacks pass in one run, including Global FP4/FP8 constructors, legacy BF16
MMA, and reusable-phase LDS. Scoped-workgroup ranked lowering and BF16 SSA reach
later write-contract and undefined-return failures. Four graph tests validate
bounded forward/reverse Context lifetime paths instead of repeated whole-CFG
walks; mixed29 below checks the actual tutorial sources with this optimization.
No blanket semantic-equivalence or end-to-end completion is claimed.

Checkpoint30 passes 238 isolated PLIRON library tests, including dynamic writes
into readable owned arrays and single-predecessor move-state retention. Array
writes cannot repair moved elements; initialization and bounds remain separate
obligations. This feature selection excludes 13 source-reference export tests
enabled by the broader core build. Analysis passes 257 library tests and 61
uniformity integration tests; execution-capability and raw-wave simulator suites
pass 24 and 12 tests respectively. Partial-width shuffles, floating-point sums,
and scan prefixes no longer gain unsupported uniformity facts. Exact live-read
proof queries pass their component tests. The broader internal-proof-staging
PLIRON suite subsequently passes 260 tests, including source read producers,
initial-memory contracts, and rejection of volatile reads and changed CPU
operators. All seven captured-Global tests pass. Final output-contract integration
and general memory-version reasoning remain incomplete.

The mixed29 diagnostic records 47 failures, zero bundles,
and all 47 scratch cleanups. Report SHA-256:
`cd69773fde8846151e63b52b5dc0a310cd9cd9b0ef34208d53612092803172ed`;
compiler DSO SHA-256:
`fe8d028e1c88823c8135b8ebb03a4df0a4ebf65906a2df86bd20a50813062372`.
Input snapshot and before/during/after binary hashes are unchanged. All seven
Context-entry work failures disappear without raising that work limit. The
gathers and speculative transaction now hit a raw CFG block limit; four other
kernels reach typed Global memory-contract checks. Ten low-precision constructor
cases reach their Global-load terminals; Muon reaches SSA return-value checks.
These changed frontiers are not successful exports. Complete tutorial reference,
simulator, negative-fixture and target-matched hardware gates remain pending.

The earlier complete diagnostic, mixed40, records **47 failures, zero
bundles, and all 47 scratch cleanups**. Report SHA-256:
`889a57b3d77663978bc1ae0190fefb9850a6629666102e2a122787c264d53724`;
DSO SHA-256:
`79da188b9efbac4bcdc6dabc617d054f6eaacf6f2f52d4796a37d79e373f0349`.
The exact mixed38 input snapshot is reused, remains clean, and before/after
binary hashes match. The
compiler comes from the dirty development worktree, not that input commit;
this remains diagnostic evidence, not clean-revision qualification.
Compared with mixed39, seven diagnostic texts change and forty remain identical.
The explicit four-branch residual kernel clears private-capture work exhaustion
and reaches a missing exact output-write contract. Regular four-branch residual
and KDA decode still exhaust private-capture work, later in the analysis. Reused
path-query scratch allows more loan and transfer checks on four GPT-OSS variants,
but all four still exhaust the unchanged work budget. No source bundle is emitted.
The mixed40 AMD-target compiler checks record 44 passes and two ordered-context
failures; these are compiler callbacks, not GPU executions. The full compiler
suite also exposed two deep-expression work regressions, which are being repaired
without reducing test depth or raising the limit.
The previous eleven core Option::zip import failures remain cleared. The seven
array-to-slice preflight failures and five accumulator/mixed-format ABI failures
now reach later gates; this does not establish complete slice lowering or matrix
execution. Fill and vecadd now pass the repaired reference-root identity join:
the carrier compares typed source-function identities, not a registration binding
ID with a source-function ID. Both stop at protected proof-runtime acquisition,
before proof admission or artifact emission. This is not a semantic proof.
MoE routing clears its counter and bit-mask
obligations but encounters another overflow assertion at source line 130.
The caller-location checker now handles canonical block/local renumbering;
Flash Attention now clears SSA borrow-work exhaustion and reaches an undefined
zero-sized aggregate return. Tiled GEMM clears its Context-entry transfer and reaches
the same BF16 memory-origin check as GEMM autoresearch and grouped MoE.
Materialized expert clears its former Context failure and joins FP4/FP8 GEMM
at the missing ranked blocked-memory relation. Both expert-rank kernels still
lack a dominating compiler-issued MFMA context. Materialized attention clears
its borrow-work regression and also reaches the blocked-memory relation.
Four GPT-OSS variants still exhaust SSA aggregate work, now during path queries
after one ordered-owner proof. The new immutable Global-fact index reduces
their charged use-classification work from roughly 234,000 to 37,000-48,000
units without changing the full candidate checks. These are compiler accounting
units, not wall-clock speedups. The reported 262145 versus 262144 remains the
limit-plus-one rejection sentinel, not a measured total.
Actual direct-tag import
tests pass, while initialization and whole-value move checks remain.
Grouped MoE clears its matrix constructor's undefined-return failure and reaches
the missing exact Global-origin refinement for BF16 matrix reads.
Two content-sparse inputs still hit dynamic-state storage limits after packed
sparse-slot storage. DeepSeek sparse attention reaches Global binding in this
snapshot, but the same compiler on the unchanged mixed37 input reproduces its
earlier storage failure exactly. That controlled replay cleaned its scratch and
retained matching binary hashes; the storage regression is not fixed.
Twelve mixed40 inputs lack typed-global receiver bindings after
conditional-Global transport integration. Two geometry-reader diagnostics still
identify exhausted private-capture analysis work, not an admitted read.
Wave64's scalar-enum return repair clears the unranked private-read regression;
it again rejects an observable write missing its exact effect contract.
Gradient staging now passes the race check after retaining the exact unsigned
comparison from its source guard. The same input reaches EFFECT-008 instead:
its output write lacks an exact reference-effect contract. Nineteen comparison
producer and consumer tests pass, including guards with changed or missing
conditions that retain the conflicting-lane witness. Checked shared-scalar range
integration still does not clear MoE routing's source-line-130 assertion.
The limits are unchanged. Four inputs still exceed the raw source-CFG
limit, with 1310-2361 declared blocks; these are not final ranked block counts.
No changed first error establishes end-to-end success.

Independent fill, Flash Attention, GEMM autoresearch, advanced systems, GPT
decode, grouped MoE, row-softmax and Wave64 host suites pass. The corrected
advanced-attention adapter now passes all 29 tests, including six compile-fail
and source controls. Low-precision's full adapter passes 18 tests after correcting
two stale negative expectations and adding a same-root policy control. Top2 MoE exposed
a host-runner error-trait compilation failure. Its repair passes nine oracle
tests, but the full suite rejects stale kernel-source pins in retained proof
fixtures. Those proof identities have not been changed to claim current-source
validation.
The completed host sweep also found stale GEMM compile-fail expectations, a
missing vecadd extractor in the independent cache, and a workgroup atomic
read/write argument incorrectly generated as read-only. The workgroup full host
adapter now passes 23 tests, including its eight oracles and all synchronization
UI cases; two protected production/hardware tests remain explicitly ignored.
Cache-selection/cleanup repairs pass seven unit tests. Atomic physical-entry
custody passes a real host positive and seven negative cases, two AMD imports,
and backend lowering of all six atomic operations on both profiles. The broader
backend now has a checked scalar-memory lowering owner for policy-bound strict
FP32 operations. Its 91 library and 48 selected integration tests pass, with
one explicitly ignored test. Actual imports cover all 13 supported FP32 operations
on gfx942 and gfx950. This adapter does not expand production OCML authority:
the production provider remains exp-only, and emitted text is not a loaded kernel.
GEMM's repaired full host adapter passes 39 tests, including both source-compilation
cases and all 17 phase-order UI fixtures; two protected production tests remain
ignored. The isolated device-API driver retains every fixture and diagnostic
snapshot without rebuilding the enclosing typed kernel as an ordinary library.
Vector-add's
full adapter passes twelve tests, but its mandatory changed-expression negative
now reaches the protected-runtime failure described above, before the
expected RHS mismatch. Rejecting for an unrelated reason is not a pass.
None of these tests compares an exported kernel with its CPU reference.
Tutorial simulator, negative-fixture and target-matched GPU gates remain
unexecuted. Checkpoint 39 passes 1191 compiler tests with one protected-OpenSSL
environment failure and 107 ignored tests. The checkpoint 38 lowerer passes 262
tests, and its full SSA suite passes 357 tests, including ordered Context reborrows and direct enum
tag reads that retain initialization checks after payload moves. MIR passes
340 tests. Binding queries now use bounded shared CSR storage; limits and
the independent reference-query regressions remain intact. The analysis library
passes 266 tests and its race integration suite passes 59. Actual AMD callbacks
pass 26 full-import cases, six direct-enum cases and eleven matrix or core-zip
checks, plus the actual checked-in tiled GEMM Context-entry callback. Two ordered
Context callbacks clear the invocation-anchor failure but still reject a
numerical-policy receiver without its retained shared Context origin. The repaired
enum negative fixtures now reach their intended rejection checks. All nine new
Context-transfer and 17 conditional-Global component tests pass; that does not
establish their missing whole-kernel joins.
These component results are not qualification evidence.

A new preparation API binds immutable source/final canonical bytes, epochs,
and exact target decisions without requiring an already verified final graph.
Its production contract-carriage integration remains incomplete. Generating
such a theorem is not executing it: changed output expressions can generate
a different obligation but cannot acquire a successful proof by construction.
Source argument lookup is root-scoped through retained lowering correspondence,
not inferred from a lowered parameter's ordinal.

The final-graph analysis adapter derives immutable views from canonical KIR,
retains the original executable graph, and seals each view to its function,
canonical identity, context, and mutation epoch. Local and workgroup coordinates
use the exact canonical workgroup dimensions, not a target default. Missing
dynamic launch facts still reject. Writes without exact ownership and
reference-effect contracts also reject; an empty effect-contract roster must
not turn a write into a vacuously successful refinement check.

Remaining end-to-end joins include dynamic output/launch preconditions,
reference and ownership contracts for final canonical writes, complete
machine-refinement evidence, protected runtime admission, and every required
negative and target-matched GPU comparison. Passing an isolated helper, source
import, or analysis test does not discharge these joins or qualify a fixture.

### Earlier source checkpoint: 2026-09-10

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

A fresh fill export (remote85) also passes protected runtime admission and
rejects at the dynamic-launch ownership check. Its frozen tutorial inputs and
compiler binaries are unchanged. No source bundle is produced. The temporary
remote source/build directory was removed after retrieving complete evidence;
the existing cache and host-global runtime paths were unchanged.

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

### Explicit numerical limits

The numerical comparison policy permits compiler-proved error bounds for
non-exact math, including `exp`; it does not require every
CPU/GPU math-library implementation to be bit-identical. Exact operations
retain their existing obligations. This policy is a requirement, not a claim
that general finite-error proof support is implemented.

Each approximate contract must state finite, nonnegative absolute and relative
limits, its input domain, reference value, comparison rule, and treatment of
zero, NaN, infinity, subnormals and overflow. No kernel gets an implicit tolerance.
The compiler must derive the bound for the final output under those preconditions,
including error accumulation through reductions and loops, and prove that it
satisfies the declared limits. An operation-level bound alone is insufficient.
The source reference, numerical policy, final graph and exact target arithmetic
must remain bound to the accepted proof through the existing production route.
Memory, control-flow, ownership and synchronization obligations are unchanged.

The domain must follow from authenticated kernel preconditions or the actual
write guards, not an unchecked Boolean premise. Every observable write must be
covered; out-of-domain behavior needs an exact proof or an already justified
rejection/no-write path. Approximate arithmetic cannot silently change control
conditions. A bounded-output proof must be reported distinctly from exact
equivalence and must not set an exact-output proof flag.

Local rounding bounds require a proved arithmetic model, not an assumed accuracy
axiom. That model must remain connected to the emitted operations, including
rounding modes, contraction and fast-math settings. Matching source/final graph
identities alone does not prove that the emitted arithmetic implements the model.
For a Rust CPU reference calling a math library, both the CPU and GPU library
implementations need checked numerical contracts. A GPU error bound relative to
the mathematical function alone is not a bound relative to the CPU result.

Tests must reject a tighter-than-derived limit, missing domain, unsupported
math contract, changed target/graph/policy and non-finite or negative limits;
boundary and exceptional-value cases must exercise the declared comparison rule.
Diagnostics must distinguish a proved violation from failure to prove the limit;
an upper bound larger than the requested tolerance is not itself a counterexample.
An empirical comparison tolerance or unproved library accuracy claim cannot
replace this proof. The current structural numerical certificate derives only
zero error from identical typed operator trees; general interval and accumulated
error derivation remain incomplete.
The existing ranked finite-error claim uses
`abs(actual - reference) <= absolute + relative * abs(reference)` on finite
results where its domain and precondition hold. Constructing that claim is not
a proof. The comparison denotes exact mathematical values of the represented
results and limits, not another rounded GPU computation of the tolerance.
At a zero reference the relative term vanishes, leaving the absolute limit.
Both limits may be zero: that requests numerical equality, not bitwise equality
(`+0` and `-0` compare equally). The ranked claim constructor now admits that
shape and preserves the exact limit bits in its identity. This does not admit a
proof: the downstream both-zero staging restriction and unsupported numerical
replay gates remain in place. No tolerance request can acquire exact-bit authority.
The source policy capability currently exposes only `StrictIeee`, and
the ranked Verus generator rejects error-bounded expressions without a supported
claim-specific proof. Authenticated source declarations must reach the existing
per-output numerical contract and retain exact target binding before tutorial
kernels can use these limits. Strict arithmetic settings and the permitted
output-comparison error are separate requirements; a tolerance alone does not
authorize reassociation, contraction, or fast math. Any such transformation
needs an explicitly permitted arithmetic mode and the applicable proofs.
The final-KIR theorem generator also still rejects floating-point binary
arithmetic, even under `StrictIeee`. Supporting explicit bounds requires extending
that final-output proof path as well as the earlier numerical analysis; changing
the comparison policy alone does not close either gap.
The current effect proof also requires exact output-value equality. Nonzero
bounds need a versioned output-value relation bound to the same write and
actual/reference roots, while coordinates, guards, preconditions and complete
write coverage remain exact. Adding a numerical request beside the existing
value-equality request cannot make a nonzero-error kernel admissible.

Direct proof generation and aggregate effect replay now share a private
exact/numerical-request selector. Exact coordinate, domain, precondition and
value-equality pairs retain their original ordering. Numerical requests reject
before formula construction, including coherent requests with nonzero limits;
no source codec or receipt selects a proved numerical mode yet. Nine integrated
regressions cover exact behavior, foreign subjects, mismatched writes/guards and
rejection before equality-formula construction. These checks do not authenticate
an output association or prove an error bound.

The standalone arithmetic model in
`crates/fe2o3-verifier/verus/binary32_rne_v1.rs` proves nearest-even binary32
rounding and a half-input-binade-ULP error bound for its positive-normal exact
rational input domain, through maximum finite. It compares against every finite
encoding, including negative numbers, zeros and subnormals. The original pinned
Verus run completed 63 verification checks with zero errors; wrong tie-breaking
and an overly tight quarter-ULP claim reject. A single-solver revision preserves
all 18 proof contracts and 15 specifications, replacing nonlinear subqueries with
checked arithmetic lemmas; it passes 19 verification checks. Its premises do not
cover zero, negative, subnormal or overflowing exact inputs. This is arithmetic evidence, not a proof
that CPU/GPU operations or libraries implement the model. Operand decoding,
emitted arithmetic, accumulated error and every final output still need their
own authenticated production joins. The retained test asset grants no admission
or exact-output authority.

The original multi-solver model was rejected by the protected process-tree
controller. The single-solver revision now passes the protected MI350 repository
regression: the positive model verifies, and both the wrong-tie implementation
and overly tight bound reject. Runtime pins, controller restrictions and theorem
obligations are unchanged. The ordinary verifier suite passes 176 tests with
nine runtime-dependent tests ignored; this protected regression was separately
executed successfully. These results validate the arithmetic model, not a
production kernel's output relation. No nonzero error bound is admitted for a
production kernel yet.

A proof-only composition companion derives absolute/relative CPU/GPU bounds
from independent error bounds around a common mathematical target. Its positive
binary32 adapter retains the original normal-input domain and counts both
rounding contributions. Eight integrated regression tests pass. The first
protected composition run (99) failed before a proof result because Verus's
compute interpreter requests a fixed 1 GiB thread stack. Replacing three constant
evaluations with fixed-depth SMT unfolding preserves every theorem contract and
the unchanged controller limits. Protected run 100 now passes: 24 verification
checks succeed, and a tighter-bound variant rejects only its intended assertion
with 24 checks verified and one error. The repository test checks those exact
results and revalidates the runtime after each execution.

The concrete example has two separately rounded results, `G = 1` and
`Ref = 1 + 2^-22`, around a common target `1 + 2^-23`. It proves the declared
comparison with both absolute and relative limits `2^-23`, and disproves it
when both limits are `2^-24`. This is not an emitted CPU/GPU kernel or a library
accuracy proof: connecting actual operations, domains and every observable
write to those independently proved premises remains necessary. No production
output bound is admitted by this companion.

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
