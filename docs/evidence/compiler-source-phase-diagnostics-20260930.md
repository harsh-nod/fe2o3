# Original Source Emission Checkpoint

This is #272 M1 investigation and regression coverage, not completed production
qualification. M0 is complete; M1-M7 remain incomplete; strict production-to-safe
GPU launch coverage remains **0/47**. Existing tutorial execution is a separate
claim. This source candidate has not been merged into public main.

## Candidate

Local source candidate `b7a9db2f5` contains diagnostic commit `0704252f1` and two
new original-index emission regressions, atop the prior candidate `10add0374`.
One mapper now attaches retained source coordinates and a fixed failure phase
to the two otherwise context-free execution-availability/CFG errors. It preserves
precise errors and resource failures, performs no new SSA queries or claims, and
does not change admission or accounting. An independent review checked the frame,
stack and borrow mappings, including idempotence when the actual function is zero.

The new positive fixtures retain the existing ThreadIndex1d producer and guarded
store. They insert a shared borrow, ThreadIndexGet and holder StorageDead before
moving the original witness into the existing store path. One fixture includes
an extra continuation edge. Original occurrences are captured from the final
source, never transplanted from another owner. Completion assertions join source
definitions, uses and kills with the emitted producer, original loan and reader
payload. These are constructed semantic-MIR/SSA emission fixtures, not genuine
rustc-capture receipts or CPU/GPU equivalence proofs.

## Tests

All local runs use the same pinned, locked/offline, one-job guarded runner as
[the runtime packaging checkpoint](compiler-runtime-packaging-20260930.md).
The worktree is unchanged throughout each run and all GPU visibility is disabled.

| Run | Scope | Result |
| --- | --- | --- |
| r112 | Source-reference, frame and stack controls | 319 passed, 3 failed |
| r113 | Corrected controls; new regression code also compiled | 322 passed, 0 failed |
| r114 | Two positive original-index borrow emission regressions | Both failed at the promoted borrow archive check |
| r115 | Actual Rust Policy10 context/nominal matrix | All 80 sessions failed before the required consumer; every failure now has source coordinates |

Two r112 failures were old context-free diagnostic assertions. They now require
function2, block0, the exact original statement and Assign/Store stage, while
retaining injection-reach and no-completion checks. The third was a stale
independent frame-size mirror: production already included `direct_call_inputs`,
but its test mirror omitted that 24-byte field. Only the mirror was corrected;
the size-derived production budget was not increased or bypassed.

r112 stable source snapshot:
`2ea7031a2393041d0e007c18d66810b3d547d777d85649547d4c2f96493e1fdb`.
Log SHA-256:
`8a322e5bcbeeccdcfa2c7978c98d918fdfbd0a58d9feec1a2ca6d47ba397c2fb`.
r113 and r114 stable source snapshot:
`84f3a56f76e0f629fc18beee785216d1462f3fb6b5525c26fa9d6682051472ef`.
r113 log SHA-256:
`7092744e78de58c6bfd545c9f6ca946bbf0a0111e3d488b9fc0abc77166e5817`.
r114 log SHA-256:
`940bb41eacd0cd03d836cc2daa4706aba27a028919812513d4fe18813fe9a24f`.

Both r114 fixtures fail at function0/block1/statement0 with
`source reference promoted borrow archive differs from source SSA`. They do not
reach the completion observer. They remain failing positive tests: there is no
`should_panic`, ignored-test annotation or reinterpretation as a successful
negative test. This reproduces the phase identified by the earlier actual-source
MIR2 runs, but does not yet identify which archive predicate failed.

## Actual Rust Matrix

r115 rebuilt the backend at `b7a9db2f50729d24ec15e648d601a914e1a8c38b` and ran
gfx942/gfx950, backend optimization 0/3, MIR optimization 0/2, five test modes and
two fresh sessions per combination. All80 failed before the required consumer:

- All40 MIR0 sessions now identify `execution availability during Assign`,
  with actual function/block/statement coordinates. This localizes the failure
  to statement lowering, but not yet to an individual assignment predicate.
- All40 MIR2 sessions still identify the promoted borrow archive check, before
  referent resolution and payload flattening.

The positive-case coordinates, identical within each pair of fresh sessions:

| Target | Backend Optimization | MIR0 Function/Block/Statement | MIR2 Function/Block/Statement |
| --- | --- | --- | --- |
| gfx942 | 0 | 1/1/1 | 0/7/0 |
| gfx942 | 3 | 1/2/1 | 0/2/1 |
| gfx950 | 0 | 1/1/1 | 0/7/0 |
| gfx950 | 3 | 0/2/1 | 1/3/1 |

Function coordinates alone do not identify a kernel versus helper. No ordering
relative to borrow lowering is inferred for MIR0. None of the negative modes
reached its named rejection boundary, so these are not successful negative tests.

The run terminated with status101 after a 7m05s backend build and a 271.21-second
matrix. Source/tool inventories were stable; its source snapshot equals r113's.
The pinned LLVM opt SHA-256 was
`13cb4c99d1810b4db40bca5db0759ca94c8efd3c437bb8f7fe1a94f5bda66203`.
r115 log SHA-256:
`8667bd2ee9f94a12d22971e331640b958dd710e3a77f3966520f44d04593aeb9`.
The private actual-source scratch directories were verified absent after exit.
The independent semantic repair was not included in this run.

## Repair Boundary

The isolated repair worktree has the same baseline tree as `b7a9db2f5`. Separate
owners are handling source-origin/carrier integration and loan-bound borrow/read
lowering, with independent review. No live #271 owner work is copied or changed.
A checked shared borrow may preserve its referent's existing producer origin;
it cannot erase a distinct mutable-borrow, AddressOf or unknown-alias invalidation.
A source-derived carrier recipe does not replace exact original-use, SSA archive,
loan, lifetime, index-metadata or resource-account checks. Reading a witness must
not manufacture owned index authority from an ordinary scalar.

The full actual-source matrix, reached mutation controls, finite resource
boundaries and both positive emission fixtures remain required. The following
follow-up records the first repair tests; it does not complete this gate.

## Index-Witness Repair Follow-up

The independent repair was integrated locally at `85416a5c0`, followed by
compile corrections at `e43a4e0c2`. A finalized, exact shared loan can preserve
an existing producer origin; other invalidations remain. Non-reference index
carrier recipes come from the original capability resolver, not a type-only
lookup. Borrow transport compares the current original SSA definition and
archived witness metadata. The reader validates the complete original reference
holder and returns its scalar payload, never an owned witness reconstructed from
ordinary bits. Existing account, archive and loan checks remain in force.

| Run | Scope | Result |
| --- | --- | --- |
| r116 | First combined repair build | Compile failed: two missing type imports and an invalid test-only SSA constructor |
| r117 | Corrected build and 345 focused tests | 333 passed, 12 failed |
| r118 | One positive fixture with a backtrace | Failed in the final-emission test observer, not at the old borrow-archive check |

r116 ended with status101 and stable source/tool inventories. Its source
snapshot was
`118edeec00899dabab6e4760c1b372abf0dcf669655f9da534b1e1e27d70908a`;
log SHA-256:
`9b5b692c8876e869a92e8097394a9e0cef51b047a147fdda616bf099c5b3d897`.
The correction imports the actual model types and uses an original SSA occurrence
in the unclaimed-use control rather than inventing a definition.

r117 built in 6m50s and ran its tests in 16.13 seconds. The existing 322 focused
guards remained passing. The new reader substitution test passed all nine fault
cases in both same-block and continuation forms: wrong loan, scalar substitution,
payload identity/type, owner, source, disjointness, index space and missing loan
claim. Each case requires one reached reader and its exact diagnostic, with
the original account restored. This is actual constructed semantic-MIR emission
coverage, not genuine rustc capture, proof or GPU execution.

The twelve failures do not receive acceptance credit:

- Six origin controls failed fixture admission with `InvalidTypeLayout`, before
  their intended query or resource boundary.
- The two emission positives and exact-budget positive reached a test-observer
  panic. r118 traced it to collecting only statement definitions, omitting the
  original call-edge definitions of the producer and reader result.
- The dead-parent fixture failed admission because a callable was outside the
  reachable root closure, not at the intended liveness check.
- The equal-shaped alternate-loan test reached zero readers, so its intended
  substitution boundary was not tested successfully.
- The unwind control reached the reader, but expected an escaping panic where
  the existing production boundary instead returns its callback-panic refusal.

The observer correction must inspect exact original edge definitions, including
source block, successor ordinal, SSA identity, reachability and promotion. It
must retain the original producer, holder, loan, scalar-output and consuming-store
checks. Fixture corrections and both genuine disjoint-index reader positives
still require compilation and execution; none is inferred from static review.

r117/r118 stable source snapshot:
`f67e89f723374f504021587d19a94ac72a8960c6cfd7899cf9924f9aa700f54b`.
r117 log SHA-256:
`959afb3cfe59f58af2db4711b9b7246470f5966d2e296bf0592c9961008d3fa5`.
r118 log SHA-256:
`2aaec89650e56481548babce82b0d100768d86ec2bdb141982a841c8711dbfbd`.

M1 remains incomplete. No finalizer, protected proof, simulator or safe hardware
launch success follows from these tests. The source repair is not on public main.

## Actual Rust After Repair

r119 ran the same 80-session matrix on local candidate `a5e69bed7`, including
the witness repair and finer assignment/operand diagnostics. It completed with
status101, stable source/tool inventories, a 6m47s backend build and a
319.56-second matrix. All80 still failed before their required consumer:

- All40 MIR0 sessions now identify
  `execution destination definition differs from source SSA`, specifically at
  `bind_local_definition_v29`'s cursor `define` call. The individual cursor
  predicate still needs diagnosis.
- All40 MIR2 sessions passed the previous borrow-archive failure and now stop at
  `pending global native changed exact access recipe`. This is a later
  source/output/native global-memory correspondence check, not completed memory
  safety, finalization or authority.

Positive MIR0 coordinates, identical within each pair of fresh sessions:

| Target | Backend Optimization | Function/Block/Statement |
| --- | --- | --- |
| gfx942 | 0 | 1/0/1 |
| gfx942 | 3 | 1/0/1 |
| gfx950 | 0 | 1/2/1 |
| gfx950 | 3 | 0/2/1 |

MIR2's combined recipe diagnostic does not yet identify which access, guard,
root, address or result comparison differs. None of the four negative modes
reached its named consumer boundary, so no negative acceptance is credited.
The Rust fixture covers logical context issuance/ABI; its indexing still uses
the existing thread API. Context-derived invocation remains a separate M1 item.

r119 source snapshot:
`95391269757d6dae79f8e46d044b9a63ebc7adf3f212ddb60c714293e3bfa908`.
Log SHA-256:
`323b91c1dbead47bfad26700fed0a78a110717b597f3a5db6cc6116a43d19c78`.
Pinned LLVM opt SHA-256 remains the r115 value above. No proof executor or GPU
was invoked. The source repair remains an unmerged candidate, not a public
production capability claim.

## Original-Witness Emission Result

r120 compiled candidate `4c20e28eb` and ran 347 focused tests: **338 passed,
9 failed**. It completed with status101 after an 8m19s build and 17.73 seconds of
tests. Source/tool inventories remained unchanged throughout the run.

All four original-producer positives now pass: ThreadIndex and DisjointIndex
shared-borrow readers, each in same-block and continuation forms. Their final
observers retain the original call-edge definitions, loan/payload correspondence,
scalar observation, and consuming guarded store. The dead-parent control now
reaches its intended live-loan storage-death rejection. The nine reader
substitution cases in both control-flow forms remain passing.

The nine remaining failures are not accepted negatives:

- Six origin controls now pass type-layout admission but stop at
  `source enum payload differs from its original guarded value`, before the
  intended authenticated query.
- The exact-budget positive completes both admission and construction replay;
  its assertion incorrectly expected a single reader invocation. Separate
  phase counts are required, not an unbounded count check.
- The unwind control reaches its injected reader panic, but expects the inner
  reference callback diagnostic instead of the actual containing function-frame
  boundary's `source reference availability construction or callback panicked`.
- The alternate equal-shaped loan fixture still stops before the reader, at
  `typed allocation identity or representation requires its exact source contract`.

r120 source snapshot:
`926f14f16b2790f12abcc8f8ee2e1190689e8979af02d4b630a3222dc576a907`.
Log SHA-256:
`81673ed8632550dbffcd78960f8434d78530b473096d6bc4ed1c64b64f1c5adf`.
These are constructed original semantic-MIR emission results. They do not
establish a passing genuine Rust matrix, proof, simulator or hardware launch.

## Exact Native Predicate

r121 ran the actual80 Rust matrix on `a335a38ff`, changing only the seven-way
native correspondence diagnostic. It ended with status101 after a 6m35s backend
build and a 279.57-second matrix, with stable source/tool inventories. All40 MIR2
sessions identify `pending global native changed exact data root`; all40 MIR0
sessions retain the destination-definition refusal. No named negative consumer
was reached. Private actual-source scratch was verified absent after termination.

The issued-pointer replay retains both a descriptor's ABI root and its transported
metadata receiver and checks their CFG ancestry. The pending native endpoint had
retained only the root and demanded direct equality for metadata operands. The
isolated repair retains both coordinates while preserving ancestry, guard,
access, pointer-transport and native-owner checks. r121 localizes the failing
predicate; it does not test that repair or establish a completed memory proof.

r121 source snapshot:
`b789be716161a9dd47f80d68cd9a5533f2491e8b6c492763a9e2ab885a001fd5`.
Log SHA-256:
`2194d499e8a0f6cf7b6036e5fbfc2bd1793f3ec9c42fe7770477c376d17c33cc`.

## Receiver Repair And Definition Diagnostics

r122 attempted the combined original-witness, destination-definition and native
receiver controls on `d5a0d654f`. Compilation stopped at one test-fixture type
error: the secondary reader block tag was `u32`, but its helper requires `u8`.
No tests ran. Correcting that fixture produced candidate `b3dbf2c7d`.

r123 compiled that candidate and ran 416 selected lowerer tests: **409 passed,
7 failed**, with no ignored tests. It completed with status101 after a 7m33s
build and 38.16 seconds of tests, with unchanged source/tool inventories.
Original ThreadIndex/DisjointIndex positives, the nine reader-substitution
cases in both control-flow forms, dead-parent rejection, exact/one-short budgets,
and the injected reader unwind now pass. Five destination diagnostic controls
and the existing global-memory/issued-pointer controls also pass.

The seven failures remain unaccepted:

- Three new native-receiver fixtures fail deterministic block identity ordering
  before native correspondence is tested.
- The new elided-definition positive fails SSA admission before its claimed
  elision; its access-result Option lacks the required branch.
- Two origin fixtures still lack a closed shared-reader consumer, leaving the
  original witness unpromoted.
- The equal-shaped loan positive reaches its observer, whose expected single
  read ignores the two reads recorded across overlapping active shared loans.

The fixture corrections preserve genuine original consumers, Option dominance,
deterministic identities, and exact effect counts. They still require execution;
static review is not acceptance.

r124 then ran the actual80 Rust matrix on the same `b3dbf2c7d`. It completed
with status101 after a 7m22s backend build and a 276.59-second matrix. All80
sessions still fail before their intended consumer:

- All40 MIR0 sessions now identify the cursor's ordered-claim predicate:
  `execution destination claim is duplicate or out of order`.
- All40 MIR2 sessions pass the previous native exact-data-root failure and stop
  later at `slice Store guard transport differs`.

No negative mode reaches its named consumer boundary. Source and tool inventories
remain unchanged, and private actual-source scratch is absent after termination.
These results localize the next two production repairs; they do not complete
source verification, proof execution, simulation or GPU launch. The source
candidate remains unmerged pending reconciliation with the #271 dependency.

r122 source snapshot:
`af307985d634429c37cc0c835ae940e5023a1aa37ec14b32183a4b619e0ed07e`.
r122 log SHA-256:
`996e359e551e38d9013caf38373fe190a6c4e3a56d5a624a434feca9cf7a7ca1`.
r123/r124 source snapshot:
`3ec417ca160ace1406e5727dee7d95b9e887ad41b18ba20693e289024ac19c7c`.
r123 log SHA-256:
`f8af70206d8d0ba2d5794340439e63c5b48fc0c55cfac4ff37e4bb16b5d06729`.
r124 log SHA-256:
`da450f6abd928f18925e086c29f32fcb57772be260d8a295aa5a89af6d013c7c`.

## Corrected Fixture Rerun

r125 ran 416 selected lowerer tests on `564bc006c`: **413 passed, 3 failed**,
none ignored. It completed with status101 and stable source/tool inventories.
The shared-index origin controls, original elided-definition positive, and
equal-shaped alternate-loan control now pass. All previously passing selected
controls remain passing.

The three native-helper receiver tests pass source block ordering but stop
before their intended native correspondence boundary. All report
`execution availability during defined call suspension` at source function 0,
block 1, terminator. The positive reaches no native callback; the two mutation
controls likewise fail before mutation. No narrower claim about the failing
predicate follows from this diagnostic, and none is accepted as a negative pass.

r125 source snapshot:
`c179e5b9fb60ff6b883f12541958aee01c2be30f768abe9b1cf4f4d038c490df`.
Log SHA-256:
`3f0a116305bfede9b985ecc08f61d7c12c5efca9d9df88b4595d068c22e9c75d`.

## Immutable Compiler Reconciliation

The separate integration candidate reconciles source work `905e76107` with the
published, immutable #271 checkpoint
`7776f9d9b2c8af5a260a0c234886f207398235d4` and public runtime checkpoint
`102be5b1b07058c6d804c36f081d809c7bfbc415`. It does not copy the compiler owner's
live worktree. The integrated source commit is
`54f15da57ff0fad547bb5fa94f26f32b72732c0e`; it remains unqualified and is not on
the public main branches.

The reconciliation preserves the owner's original SSA/rvalue archives,
same-type slice-reborrow source-use ordering, store-guard checks, and single
index-reader implementation. It composes the exact V40 importer and source
commitments with the retained source/loan checks, rather than adding a second
production route. Independent source review found no merge-specific blocker;
execution is a separate gate.

Guarded lowerer run `r129` compiled the candidate and completed with status 101:
**442 passed, 7 failed, none ignored**, from 449 selected tests. This selection
also includes the newer owner's rvalue and reborrow regressions, so it is not
the same denominator as r125. The source and tool snapshots stayed unchanged.

- The three native-helper receiver fixtures now stop specifically during
  `execution availability during defined child request`, before the native
  correspondence callback or intended mutations.
- Two same-type slice-reborrow tests panic inside their audit callback's exact
  diagnostic assertion. Their positive end-to-end test outcomes are still failed.
- One shared-index origin test observes retained storage above its expected
  no-scratch floor; the accounting discrepancy requires investigation.
- One original shared-index mutation test receives the localized
  `source reference promoted borrow archive differs from source SSA` diagnostic,
  rather than its expected rejection detail. This is not accepted as passing
  coverage without reconciling the intended boundary.

A subsequent mechanical split separates shared-borrow origin queries, their
carrier tests, and test-only expression helpers. Expanding the includes and
formatting reconstructs the original three files exactly. The combined delta
then passes the existing hygiene policy, without suppressions or removed checks.
Compilation and actual-source validation of that successor remain separate
gates. Neither reconciliation nor these component results advances M1 or 0/47.

Subsequent independent review traced the 7,169-byte origin-test delta to fixed
query headers intentionally retained by the enclosing emission scope, not a
failed scratch refund. A separate test correction derives that exact delta
from the original statement roster and concrete header types, while still
requiring the scratch-only query to refund its full frame. It also updates the
two localized-diagnostic expectations and requires reached mutation sites and
unchanged rejected cursor state. These corrections have not yet been executed;
they do not retroactively turn r129 into a passing run.

Frontend run `r130` at successor
`c3d15de67cf6df9334371a3e0d7b3d209af51364` passed **128 tests**, with **18 ignored**
and none failed. It covers selected source-schema commitments, context custody,
fixed provider policies, source observers, trusted-item controls and the default
entry's refusal to fall back to a conditional continuation. The ignored genuine
source callbacks and repeated-mutable pointer-cast matrix were compiled but not
executed by this selection. Source and tool snapshots remained unchanged.

r130 source snapshot SHA-256:
`93349a6457b77801848716cdd4f86ed63926b108b13942c35bc04bedae37d763`.
Log SHA-256:
`358b0a4089952a3f6c26861dbf44b1ec62152464299d23aa6ec03996ac1cd4e1`.

The subsequent actual-source run `r131` completed on that same successor and
snapshot with status 101. All **80 fresh sessions** now pass the former ordered
destination-claim and Store-guard failures, but stop later at
`Source(Binding("slice global effect census is incomplete"))`. This affects
both gfx942/gfx950 targets, both MIR levels, both optimizer settings, and both
repeats of every mode: 16 positives and 64 named negative controls. None reaches
the intended consuming callback or negative boundary. The parent result is
0 passed / 1 failed / 0 ignored; its 292.88-second matrix is not 80 passing tests.

The guard confirmed stable sources/tools, and the harness's private
`fe2o3-context-source-v29-1796803-0` scratch was absent after termination. No
protected proof, simulator or GPU workload was run. The newer published #271
checkpoint `c1b5856df54dc3c3ef8688fde2b09989e0541d7d` is a separate dependency
under comparison, not the revision tested by r131. Its live worktree was not
copied into this run.

r131 log SHA-256:
`a646ff24cc575ec713a9e088f4de8322f7876aad242c56ea40b400e8a5649a66`.

r129 source snapshot SHA-256:
`13cf593236e8deaddb38b62b958b432df8aa7c37337097fd7dc3bea8d3e19a56`.
Log SHA-256:
`cb58f1474cbd83008f5ff05f831ba9afee6e4789d5f5e0b537ffefaa0752dab5`.

## Reconciled Diagnostic and Accounting Controls

Guarded lowerer run `r132` at
`46d13316274202ca3ec7fba89c79a0df30e3cd45` executed the corrected accounting and
diagnostic controls described above. It completed with status 101:
**450 passed, 3 failed, none ignored**, from 453 selected tests. All four repaired
tests now pass, together with the new independent scratch-refund and child-call
diagnostic controls. The source and tool snapshots stayed unchanged.

The accounting oracle now derives the retained header charge from the concrete
source-operation roster and header types. It separately checks scratch refund;
it does not replace the expected charge with the observed result. The mutation
controls require the genuine original sites to be reached and rejected cursor
state to remain unchanged. Child-call diagnostics retain specific source and
resource failures while distinguishing structural argument shape from later
correspondence checks.

All three remaining native-helper receiver fixtures stop at
`execution availability during defined child structural argument shape`, at
function 0, block 1, terminator. The positive does not reach its native callback,
and the two mutations do not reach their intended substitutions. These are
failed integration tests, not accepted negative coverage. Generic transport of
the original authenticated index value into a by-value helper argument is still
required; accepting a same-shaped scalar without its provenance is not a fix.

This successor did not rerun the actual-source matrix. Its latest result remains
r131's 80 early failures at the global effect census. Neither result qualifies
context-derived indexing, protected proof execution, simulation or hardware.
The compiler candidate remains separate from public main; M1 and **0/47** do not
advance.

r132 source snapshot SHA-256:
`2e6a876e7f7564d2e45317bb8f8ea67223c06a592a022c3cf249180ee76fae95`.
Log SHA-256:
`c0dc10e9fd48080b060852228cc1a78610e586082528661b1ef58f30fc68e888`.

## Runtime Observation

A primary-session read-only SSH check reached MI350-2 on 2026-09-30 and confirmed
the pinned native image still exists. All four production host paths were absent:
the V3 client profile, policy-v2, compiler-runtime-manifest-v1 and compiler-runtime-v1
directory. A worker-only DNS failure was not treated as host unavailability.
These reads did not start containers, install files or change the shared host.
The earlier r111 container and scratch had already been cleaned.

The remaining runtime gap is also implementation work, not only provisioning.
The public native compiler gate in
`crates/cargo-fe2o3/src/compiler_execution_boundary_native.rs` still refuses with
`RuntimeEnforcementUnavailable`: it requires enforcement retained from before
exec through completion. Root intake currently has refusal-only semantics, and
the native attempt deliberately cannot resume the compiler. Existing installed
inventory, staging, trace, proof-helper backing and retirement components do not
yet compose into an approved compiler execution with continuous source/code/output
binding. Packaging success is not that execution guarantee. M1 remains incomplete.

## Context Read and Invocation Checkpoint

The following guarded runs retained unchanged source and tool snapshots. They
are candidate validation, not production qualification or new kernel coverage.

| Run | Candidate | Result |
| --- | --- | --- |
| r136 | `3a7c9dc5c` | Compilation stopped on missing public-field documentation; no tests ran. |
| r137 | `56568c444` | Actual-source matrix executed all 80 cases; all stopped at the same incomplete global-read census. |
| r138 | `07b51e95c` | Merged compiler/helper candidate stopped on receiver-field, opaque-ledger `Debug`, and referenced-type pattern errors; no tests ran. |
| r139 | `e10a5035f` | After those source fixes, rustc/LLVM exhausted the local 12 GiB virtual-memory allowance; the build did not complete and no tests ran. |
| r140 | `87f5e7635` | Device library: 113 passed. Compile-UI matrix: 172 passed, 28 mismatches; its parent test failed. |

r137 reports root 0, function 0, recognized read/store counts `[0, 1]`, read effects
`(2, 1, 0)` and store counts `(1, 2, 0)`. Two source reads lack correspondence.
All 16 positive and 64 negative cases fail before their intended downstream
checks; none counts as successful negative coverage. The fixture still obtains
its index through the existing thread API, not context-derived invocation.

r138/r139 integrate the immutable #271 checkpoint
`0e21d8f6a2e089d24ff0486e76e53d2c138a0932`, original index transport through
helper calls, and exact aggregate receiver metadata. These changes remain
unqualified candidates. A failed build is not a passing integration result.

r140 exercises the proposed branded invocation/index API. Its mismatches are
not assumed to be cosmetic: `context_index_output_escape` reports a missing
return lifetime before reaching the intended output-escape check. The fixture
must be corrected and rerun before its expected rejection can count. Compiler
import and canonical scoped-index lifecycle wiring remain unfinished.

Log SHA-256 values:

- r137: `a9717486e336edc68a21c129e3c5674e38d9f315ad5552fdf8bed5c315814923`
- r138: `718a90df18990261e4ccb7099256fb0266b624d5c0fc58b9e6993ef04d15c32d`
- r139: `841d54de199c53b2fd00d3ac2cab2e9eb1cb2cb39fa6c888979e2dbdc58d84fa`
- r140: `669750a2922c8224e47b91e3df60570b2185a4db6e763c8bba017da2c6116709`

No protected proof, simulator or GPU run occurred in these checks. M0 remains
complete; M1-M7 remain incomplete; strict production-to-safe-launch coverage
remains **0/47**.

## Scoped Invocation API Regression Result

Device candidate `2a5483ab6cb3b8359f38ba03fa28022c55b8c679` passed the complete
`fe2o3-device` test command in guarded run r143: **170 library/integration tests
and 44 doctests passed, none failed or ignored**. Its API UI parent includes
**200 passing compile-pass/compile-fail cases**; those children are not added
again to the library/integration total. The host LLVM-shape test also passed and
removed its private generated library/LLVM scratch outputs.

This supersedes r140's UI failures, not its production-wiring limitation. The
output-escape fixture now names the output lifetime explicitly and reaches
E0515 at the returned borrow. r142 established that intended refusal, leaving
only a one-character underline mismatch; the final snapshot correction was
verified by r143. The other snapshots were individually reviewed against the
pinned compiler's diagnostics. No blanket snapshot overwrite was used.

The candidate adds a non-forgeable context-borrowed invocation and branded index,
plus a sealed disjoint-index consumer that retains the index's scope in the
returned output borrow. This does not prove GPU bounds or injectivity. Its new
source terminals remain pending authenticated compiler import and canonical
scope/lifecycle preservation; no host fallback or production launch is enabled.

r143 source snapshot SHA-256:
`bb094ac4c40cfab523edcb4d567ff8a8eda346a67becdaad0dfbfc815b32bcde`.
Log SHA-256:
`01dbd6490c6f2e686a494a2b0bc223d1b427e6f6eced024666b61c0ceac51161`.
The source/tool guard, DCO and delta hygiene checks passed. The API changes are
still an integration candidate, not public-main compiler qualification.

Separately, compiler r141 stopped on three test-module visibility errors before
execution. A test-only facade and public inert-site constructor corrected those
errors without widening the production interface. The subsequent candidate also
reconciles immutable #271 checkpoint
`7bc097b0b3b32ae542d8549d3d04b824f6a424ed`; its regressions require a new run.
The r141 build used a bounded 16 GiB host virtual-memory allowance. This changes
only the local Rust build envelope, not any production work/storage/proof cap.

## Reconciled Compiler Execution Results

r144 completed at `857d7079a6699968cef7af47a1bc18278a3be0b0` with unchanged
source/tools. Its lowerer selection executed **499 passing and 18 failing tests**:

- Three aggregate-runtime cases still stop at the inherited unsupported
  private-entry SSA block argument.
- Four context-read controls panic before their intended checks because the
  ABI test helper omits F32 slice descriptors. They do not yet validate the
  context-read repair.
- Eight index-call controls and three native-helper receiver controls stop at
  `execution availability during defined child request`. Their intended
  callbacks and substitutions are not reached; no negative credit is granted.

The shared SSA-boundary selection passed one test and failed nine during fixture
construction: edge-role zero is invalid. `f00040b7f` changes only fixture role
values to nonzero values, preserving zero-based edge ordinals and the production
rejection. r145 then passed all ten boundary tests.

r144 selected zero tests in `dialect-amdgcn`, so it supplies no native-layout
coverage. r145 explicitly selected `fe2o3-amdgcn-model`: ten tests passed and one
failed because it incorrectly expected only the printed owner SHA to change.
The owner also binds the emitted pseudoprobe GUID and function hash.

`8be686cbed5e6b5c3ec704d34ff9cd2fc2c191e9` corrects that test: unchanged
executable KIR is compared structurally, while the exact owner and both probe
identifiers must change and appear in their emitted records. No production
lowering check is relaxed. r146 passed **all ten SSA-boundary and eleven native
target-lowering tests**, with zero failures/ignored and unchanged source/tools.
These are CPU-side compiler tests, not GPU execution or complete refinement.

Log SHA-256 values:

- r144: `9a1ed41d08c9d45026ad635086746b0fb21e58261375e2bc666329a2a0ab3824`
- r145: `029c1930a7e2f36560e764465ea0ce186ddf03c7a03ba5accce5db74929c3401`
- r146: `f50d061a7a5ab38ab36e16752768e6939ffc3f7fc798d531094002ffbf0c6465`

The lowerer failures still require their own fixes and reruns. No milestone,
protected-runtime, simulator or safe-launch coverage advances from these results.

## Actual Rust Data-Root Boundary

r147 rebuilt the actual-source backend at
`8be686cbed5e6b5c3ec704d34ff9cd2fc2c191e9` and finished at
`2026-09-30T09:57:01.886Z`. All 80 cases failed before their required consumer
with `pending global native changed exact data root`. This includes all 16
positive cases and all 64 negative cases. The negative cases do not count as
successful rejections at their intended boundaries.

The matrix covers gfx942/gfx950, backend optimization 0/3, MIR optimization 0/2,
five modes and two fresh sessions per combination. The fixture still uses the
legacy ThreadIndex body with a logical context argument; it does not exercise
the new `ctx.invocation()` source API. No finalizer, protected proof, simulator
or hardware launch was reached. M1 remains incomplete and the strict count
remains **0/47**.

Source and tool inventories were unchanged throughout the run. Source snapshot:
`5f706214465676dc08a11f14a10c170c81f64fdd7f5417928b4af25bba01a7a6`.
Log SHA-256:
`6c8ee1768a80e4ff3414045dda36f5a15d7caa71726532ba2fa19266e22f5989`.
The next repair must preserve the exact original data-root binding; removing or
weakening the identity check would not satisfy this gate.
