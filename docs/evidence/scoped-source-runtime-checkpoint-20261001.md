# Scoped Source and Protected Runtime Checkpoint

This records progress on [#272](https://github.com/harsh-nod/fe2o3/issues/272),
not a release qualification. **M0 is complete; M1-M7 remain incomplete.**
Strict production compiler -> required proof -> safe GPU launch coverage remains
**0/47**. Component tests, authored proof adapters and runtime provisioning do
not advance that count. This does not reclassify independently runnable legacy
examples as qualified through the new production path.

## Latest Compiler Checkpoint

The actual-source rerun **r356 failed** at local candidate
`b44357ecf14370d73fc29bd95cc1eecf9c6a8b35`. Its backend built, then all 24
ordinary fill, guarded-fill and vecadd configurations failed on gfx942/gfx950:

- Sixteen fill/guarded-fill attempts emitted partial proof text but lacked the
  required store-activation clause connecting original control to memory effects.
- Four vecadd opt0/MIR0 attempts passed the earlier root-only census failure
  but stopped at `pending global read changed exact local domain`.
- Four vecadd opt3/MIR2 attempts stopped with source resource-accounting errors.

The parent result was zero passed, one failed, none ignored and 4,383 filtered.
No required protected proof or safe GPU launch completed. The independent
source/tool/runner audit passed; raw log SHA-256 is
`594e6faf2e5c4ed40897a650f90d60a874b87b617f5c5b90f4b002d9b57517e7`.
These results supersede r348, not the missing production acceptance criteria.
The preceding r355 reached only a partial matrix before its unchanged deadline;
it receives no complete-matrix credit. Its original process terminated before
r356 reused the completed build cache at the same source revision and limits.

The earlier `4ad611b1105268e6e1e119f743d8a2d5281f1377` candidate's focused
lowerer run on isolated MI350 completed with
**75 passed, 17 failed, none ignored and 4,713 filtered**. Ten tests stopped on
noncanonical source block order; six exhausted the unchanged work allowance;
one stopped before its intended foreign-owner observation. Early failures are
not credited as successful negative tests. Independent source, tool, archive,
result and cleanup checks passed. Raw log SHA-256 is
`1ddd806ed9fa5fc4e844b75e256a71f3f8f46b64649467e340caeb16efb33a8b`.
The isolated container and 3,710,394,368-byte owned cache were removed. This was
CPU compiler testing on a GPU host, not GPU execution.

Reviewed local changes subsequently connect original intrinsic clauses to the
source census, replay a handoff against its retained original source owner,
and capture immutable original source bytes under a shared storage allowance.
These are incomplete integration steps: aggregate call/return obligations,
actual compiler consumption of captured inputs, generated dependency provenance,
and the complete source/context/ABI/launch conjunction remain outstanding.
`MissingCall` and `MissingSourcePublication` remain explicit refusals.

The complete kernel-descriptor library run **r351 passed all 67 tests**, with
none failed or ignored, at local candidate
`2ea1914290fe15b3c6edeaeaf0336c73acbde01a`. This includes the new allocation-free
physical-component accessor and its exact access/alias/unchanged-wire control.
The independent source/tool/runner audit passed; raw log SHA-256 is
`db38606b5571598655d7bb8b4ec23484f7524bd267330ae833ae71aaef6840d7`.
This verifies descriptor behavior, not source proof or launch authority.

The new original-input DATA codec run **r353 passed all 12 selected tests**,
none failed or ignored, at local candidate
`45a03c1ccb17e55bed024f8c2dc082dbe12c7f1e`. It covers complete context,
ABI-component and launch-field round trips, malformed records, exact work and
storage limits, and the unchanged combined 4 MiB correspondence/DATA ceiling.
Its independent source/tool/runner audit passed; raw log SHA-256 is
`7cf366c24eee1c79b5b14ce96acf6aff77e3a275bfd907406bb59e62a6e52a3d`.
These are allocation-free DATA transport checks, not authenticated original
source recovery or semantic proof. The owning MIR/SSA constructors still need
complete allocation prepayment before parent recovery can be admitted.

Three interim isolated verifier attempts stopped before executing tests:

- Recovered G44 stopped at a missing test import and an error-conversion compile
  error. Both were repaired. Its original run was collected without restarting;
  raw log `84063649f53968f1ac13b90a5eec7a015306e448ad58878b5d3159274b599dc8`.
- Gc937 and local lowerer r352 then found three missing lifetime bounds in the
  shared descriptor-plan borrowing interfaces. Those bounds were added without
  changing ownership or budgets. Raw logs respectively:
  `ef29d4a3233e15f52267e49844c18ae3a90bf871bb9484f009fec87ced37265b` and
  `b583bc19c92130879ee17510421dfddcbd9da71563673c11ab8ea661f3b246a2`.
- G45 cleared those errors but stopped on an extra reference when iterating an
  already-borrowed function slice. A one-character correction is integrated
  locally; raw log `c75cfd0091f3e18e1125747226459f8fc8ee91c91694c24c9a6cbcb574e83f42`.
  Local actual-source r354 confirmed the same compile failure, with no test
  execution; raw log
  `a649b1fa0a2e50a0aeba69652578633ae4357959930365e0d3eee660e9f61744`.

Source, tool manifests, raw logs, terminal states and cleanup evidence were
independently audited. All three remote containers and their private scratch
directories were removed. These compiler failures receive no test or proof
credit. Reviewed follow-on work preserves original bodyless import identities,
uses original SSA reachability for the activation roster, and checks aggregate
helper-return control transport while retaining its missing value-proof
obligation.

The corrected candidate `b44357ecf14370d73fc29bd95cc1eecf9c6a8b35` then compiled
and ran the focused verifier suite on isolated MI350: **57 passed, 10 failed,
none ignored and 830 filtered**. All eight selected groups executed, including
five passing source-diagnostic tests. Six failures occur before replay because
the fixtures use an invalid receipt header; one expects the wrong foreign-account
error variant; three supply an invalid SSA edge role before their reachability
checks. These failures still count as failures until the repaired tests run.
No production check or resource limit is being relaxed to accommodate them.

The independent exact-source, tool-manifest, archive, result and cleanup audit
passed; raw log SHA-256 is
`9030714446a48c1b30c0e47ad658f3bff15d36d8f83bb535cdbd67ce51c6b490`.
The container and its 3,437,182,976-byte private scratch directory were removed.
This was CPU verifier testing, not protected proof or GPU execution.

The subsequent `480b36136b36f5eb560a66ce3706f602c695a27c` isolated run completed
with **66 passed and one failed** in the same focused verifier selection, and
**339 passed, one failed and 51 ignored** in the ordinary spawn library.
The remaining verifier failure was an incorrect test assumption about a smaller
raw work charge after a denied larger charge. Its correction retains the first
denial and requires actual admission to reject it; production limits are unchanged.
The spawn failure was `clone3` returning `ENOSYS` in the container. Its cause was
not established, and no container security restriction was weakened. All three
new namespace unit controls passed; seven namespace native tests were ignored.
The exact-source, tool, result and cleanup audit passed; combined raw log SHA-256:
`a1f3f1fc4d03f63654132f743aebcc4a4f8cd287fd653cd657b859a96406e78f`.
The container and its 3,628,052,480-byte private scratch directory were removed.

Local full spawn rerun **r358 passed all 353 ordinary tests**, with none failed
and 51 ignored, at `f905ea0502ed0756200374f382878c7874eef82c`. This includes
the atomic-clone test, the three namespace unit controls and thirteen new
mapped-input lifetime/accounting controls. It follows r357's 352-pass/one-fail
result: the remaining fixture now explicitly creates its required `0700`
scratch directory instead of depending on the process umask. The production
directory check is unchanged. Independent exact-source, six-tool, runner and
raw-log checks passed; log SHA-256:
`027ba8725b43d11f5a362314eb95a5796a998e56242dcde6c949041588c3d1cd`.
The 51 ignored native tests remain unexecuted; these ordinary controls do not
establish protected compiler input delivery, source authority, proof or GPU launch.

Reviewed local integration now retains the original source-hashing file handles
and mapped input backing across descriptor close until exact unmap or cleanup.
It also joins context-issuance value clauses to the genuine original root and
existing lifecycle insertion checker. Complete source delivery, return-value
proofs and final aggregate obligations remain unfinished. No completed milestone
or kernel qualification is claimed for these component changes.

Local source-capture runs r349 and r350 receive **no test credit**: disk
exhaustion interrupted r349's report, and an execution-environment interruption
left r350 without a final report. The incomplete logs are preserved. Storage
was recovered without deleting source or reports; a fresh private RAM cache
was created after the previous cache disappeared. No missing result is inferred.

## Component History

Reviewed local integration includes scalar-control correspondence, original
helper-call topology, instance-qualified control/memory proof composition,
pending proof-response dispatch barriers and shared V3 capsule packing. The
call topology still does not prove complete callee semantics; `MissingCall`
and `MissingSourcePublication` remain explicit refusals. Strict parent recovery
needs the actual checked optimization witnesses and original context/ABI
inputs, not only record hashes or signatures. No partial record grants authority.

The nine-package checks r333-r336 stopped on internal schema visibility, a loan
lifetime bound and two test-integration issues. Those narrow repairs are local.
The earlier combined check **r337 failed** at
`1e5299e8fe9585375bb25591a3acc2dc1ab0f891`: four compile errors in the new
scalar-control fixture prevented test execution. Its source/tool/runner audit
passed; raw log SHA-256 is
`3bc3e149992ba4b3ed898196d5727669df1bff8a1df39fc1a0cdd6abb735604c`.
Focused lowerer r338 then failed on one incorrect test-probe module import,
before test execution. Its independent audit passed; raw log SHA-256 is
`2a3619cee2261404143e5aa3d1db8fe68b5d587ccfe44c087ccaa23147ac67cb`.
The test-only scope repair is integrated without exposing private production
methods. Focused lowerer execution **r339 finished with 61 passed and seven
failed**, none ignored and 4,712 filtered, at
`0811ebca0699ab17ff76fd38cd0c41069cea82ed`. The full harness built in 11m58s;
selected tests ran in 23.96s. Failures concern two original Option-storage
transports, three invalid scalar fixtures and two control-flow visit checks.
Both new authenticated-launch-envelope/paid-replay tests passed. The independent
audit passed; raw log SHA-256 is
`4e90a26697922f49b7555ba099c7f333a7a19e2d33d7b88d79d20da00cf271b2`.
The callable-prefix fixture repair is integrated but not yet executed.

Ten-package checks r340 and r341 then exposed a call-query error-type mismatch
and a verifier tuple-constructor alias. Both have narrow local fixes; neither
check executed tests. The combined ten-package check **r342 passed**, including
test compilation, at `d39c35e2ae35e571c39654af02a24fb8b42d79a8`. Its independent
source/tool/runner/raw-log audit passed; raw log SHA-256 is
`9875f9f8bf9940a8b9a175f040807ad54f565dcddfe7e590fd31df865508700f`.
This did not execute tests, protected proof or hardware. Component execution
**r343 finished with 369 artifact-transaction tests passed and two failed** on
that same frozen candidate; Cargo stopped before the remaining suites. One
failure is an independent peak-storage oracle mismatch; the crossed-receipt
fixture exceeds the unchanged 256 MiB limit before reaching its intended guard.
The subsequent test repairs preserve that limit.

The remaining five suites **r344 finished with 1,186 passed, four failed and
71 ignored**: closure capability 293/0/4, coordinator 327/0/23, FFI 152/0/0,
lineage 82/0/0, protected spawn 332/4/44 (passed/failed/ignored). The four spawn
failures concern obsolete retained-frame expectations, late funding and the
expected first refusal. Combined top-level totals are **1,555 passed, six failed,
71 ignored**; nested subprocess summaries are not counted twice. Both independent
source/tool/runner/raw audits passed. Raw log SHA-256:
r343 `8cf7fb323b8a1f805429649963606713ccdae68e63f34d5531ff7be07fafc204`;
r344 `1247afcfcac27edccf52f6b7b33b82f8f04a793757b3b8282d53c661b0815ac7`.
Ignored native controls and the newer compiler repairs receive no execution
credit from these runs.

The newer ten-package **r345 failed** at
`f865a5b67164a1a99c19aaf5def19317ca2f3d3c`: two new LICM test sites called a
nonexistent `StorageLayoutLimitsV1::default()`. No tests executed. The independent
source/tool/runner/raw audit passed; raw log SHA-256 is
`13cd9a0582ecced52a485c9d9b6e7a5d1b328fdce62099dc84e645a9ec62641b`.
The reviewed test correction uses the existing explicit two-row layout contract,
without increasing the work or storage limits.

Local integration at `d111e0d45dfb817c6c4f725aa3844dc0ca329d91` also includes
the artifact and spawn test repairs, original call-custody veto controls,
nonconstant scalar-call/input-map controls, and the complete original-source
currentness transport chain. This retains the original Cargo capture/watch owner
through the broker and a mandatory V5 root-intake role. Sampled currentness is
not immutable compiler-read protection, generated dependency provenance, or
source publication authority. Existing execution refusal remains. The focused
lowerer run **r346 failed before test execution** on that frozen candidate:
the new scalar-call fixture repeated a non-`Copy` type in an array and left a
generic error type ambiguous, producing three diagnostics. Its independent
audit passed; raw log SHA-256 is
`db699570c29abe1531778525358d868d196e8d879a06bb1cc3ab6ad86affde52`.
The two-line test correction is reviewed and integrated locally. New lowerer
and verifier controls remain unexecuted. Four-library run **r347 passed** on
the same frozen candidate, without the lowerer test target: artifact transaction
371/0/0, coordinator 333/0/23, protocol 133/0/0 and protected spawn 337/0/44
(passed/failed/ignored). Combined top-level totals are **1,174 passed, zero
failed and 67 ignored**; nested subprocess summaries are not counted twice.
All six earlier artifact/spawn failures now pass, along with the new protocol
and coordinator currentness controls. Ignored native tests remain unexecuted.
The independent source/tool/runner/raw audit passed; raw log SHA-256 is
`c30bb53881f2091b0a26ac420a7730e435111e5fe4616c358a9cece41b8a605d`.
No protected proof or GPU execution occurred.

A separate isolated MI350 check compiled the Cargo frontend, execution protocol
and coordinator with their test targets at that same `d111e0d45` candidate.
The independent source/tool/archive/cleanup audit passed; it executed no tests.
The owned container and 2,827,980,800-byte cache were removed. Raw log SHA-256 is
`d9e91af0c7d48d52ff6529a892b6656dc48bb4d6ebb0f7d6177683e869b1d017`.

The runtime staging adapter passed **28 local controls**, including ELF W+X
rejection, nonblocking FIFO rejection, detached-descendant cleanup, and reached
post-spawn initialization/SIGINT/SIGTERM failures with original signal-mask and
subreaper restoration. An independent review found the initialization cleanup
gap after the earlier 26-test run; the two additional tests cover its correction.
These controls execute local test children, not the compiler, protected proof or
a GPU kernel. No actual runtime assembly or approval is credited.

Subsequent reviewed local integration at
`1e0a09788a6ce886a5d4370b860a801283b407c5` preserves original typed storage at
both helper-return joins, checks retained descriptor bounds branches against
the original source and optimized graph, and replays the actual carried Policy11
and LICM witnesses through the existing checked engines. These changes and their
new regressions have not yet executed. Normal-edge diagnostics now expose the
original failure before the unchanged visit-count assertion; that is not a
claimed control-flow repair. Checked optimizer replay still lacks the original
context/ABI/source-semantic conjunction required for source publication.

A fresh isolated deployment retry on MI350 passed **166 tests**, with zero
failures and eight ignored: 154 library, nine runtime CLI and three qualification
tests. Both earlier descendant-reaping failures now pass under a measured init
whose actual orphan-reaping behavior was checked before the build. Packaging
still **failed** because the static qualification usage oracle omitted two
existing V3 commands. The helper stage was not reached and no runtime was
approved. The exact usage-check repair preserves exit-status and text-equality
requirements. A fresh retry of candidate
`676484cc2a97560c588a8e8b26ecd407ef5b41a0` has now passed **both complete
build stages**: 166 tests passed, eight ignored; all five deployment binaries
passed their ELF/CLI gates, and the proof-helper passed its static ELF and
secure-entry gate. The primary independently verified the archive and all
twelve artifact payload hashes, comprising six binaries and six ELF reports.
Successful archive SHA-256:
`042e1caaff6187d7fb4e8adb59987bd40fbe1f63fe904442007caa1b1ff64c80`.
The owned container and 3,957,444,608-byte disposable cache were removed after
verification. No runtime assembly, approval, service activation or proof request
is credited. This exact runtime candidate is not the newer integrated compiler.
Earlier successful V3 provisioning below remains prerequisite evidence for its
own older bundle, not current compiler or proof execution.

Both public mains were independently read at
`8b382d7c4c812b3fcfcbd70191ee401cc1905132`. Their concurrent basic-assembly
milestone documentation is preserved and does not complete issue #272. The
earlier BF16 source authoring workflow and retained helper accounting are also
preserved. The workflow's
27 local Node controls passed in this integration; they test diagnostic
transport and do not establish compiler, simulator or GPU qualification.
The unfinished implementation remains local. Its full pending-range audit at
`2eb0d14782270b892bdd47facf60810bc79a303d` found 154 missing author sign-offs
and 96 hygiene findings: 65 file-size/growth findings and 31 test-gated panic
placements the checker does not recognize. Those 31 are not newly established
production panics, but the gate is still failing. Narrow new-commit checks do
not supersede the full-range result. **M0 alone complete; strict coverage 0/47.**

## Earlier Compiler Checkpoint: 14:35 UTC

On 2026-10-01, M0 alone remains complete and strict coverage is
**0/47**. The following results supersede the earlier checkpoint below.

The full four-library run r327 at local candidate
`82fb970729526d946e9a1df28d0b9d42ddd39485` passed **893 top-level tests** with
zero failures and 17 ignored: artifact transaction 355, compiler-execution
coordinator 317, compiler FFI 152, and lineage 69. Nested subprocess
summaries are not added again. Ignored protected-runtime/process tests remain
unrun. These component results do not establish successful kernel compilation,
protected proof execution or safe GPU launch.

The actual-source run r328 at local candidate
`7fc6303e129a95b4698764baefb5aae1e5c27c6c` built the backend but failed all
24 ordinary case/configuration/repeat attempts:

- Fill and coordinate-guarded fill: all 16 attempts lacked an authenticated
  source presence predicate.
- Vecadd at opt0/MIR0: all four attempts rejected invocation cleanup in the
  assertion failure block.
- Vecadd at opt3/MIR2: all four attempts rejected descriptor/index/extent
  correspondence.

The parent result was zero passed, one failed, none ignored and 4,262 filtered.
No source-clause frame, protected proof or GPU execution was reached; later
negative modes are not credited. Local repairs cover predicate namespaces,
optimized predicate replay and checked assertion cleanup. Descriptor correspondence,
full source-role composition, protected source-input enforcement and artifact
publication remain active implementation work, not completed acceptance.

The focused lowerer run r331 at local candidate
`8632b80ed9c0335a4a93d881030bc72042d1e47e` built the full library test harness
within the unchanged 16 GiB virtual-memory limit using 256 codegen units. The
selected tests then completed with **11 passed and six failed**, none ignored
and 4,731 filtered. This clears r330's missing-helper compile error, not the
remaining test failures or a general build-memory guarantee.

Two failures reach the assertion-cleanup check repaired in the newer local
candidate. Two normal-edge controls used the wrong scalar namespace; another
control incorrectly expected different content hashes for identical source.
Reviewed test corrections preserve original-instance and rejection checks.
The sixth failure exposes missing original Option-predicate transport across
an ordinary helper return; that compiler repair remains in progress. The
actual-source rerun r332 has started at local candidate
`c13567f2f876bc4ed9b0c5edc1e4d1017a94f904`, including assertion/descriptor repairs
and actual Worker target-owner retention controls; no result is credited yet.

The earlier r329 stopped before Cargo because the shared filesystem was full.
Cleaning an inactive owned Cargo cache recovered 2.5 GiB without removing source
or reports.

Administrator access was verified on MI350 and MI350-2. The runtime agent then
completed a fresh isolated V3 service-profile installation, native readback and
revalidation on MI350; all three stages exited zero. All three containers and
their supervisor were confirmed absent afterward. The genuine protected output
is retained privately; no host configuration or service activation changed.
The primary independently rehashed and read the terminal and cleanup evidence.
Separately, the existing pinned Verus closure on MI350-2 passed the unchanged
installed-runtime inventory audit, without executing Verus. These are prerequisite
results: the current compiler/helper release still needs packaging, independent
policy review, installation and actual protected execution. Neither service
provisioning nor an inventory audit counts as a proof or GPU run.
Both public mains were independently read at
`74914888be4b6351e72171d8b69853d18682548e`; the newer compiler candidate is local.
That concurrent retained-account implementation and its reported tests are not
credited as validation of this candidate.

The primary independently audited source inventories, actual tool identities,
runner, raw logs and frozen revisions. Raw log SHA-256 values:

- r327: `5b28c830f6229449492f400e3644a3b41f3ca5a6fb84a6ae5721d4e36906c2ef`
- r328: `bf9cb0dec50e48b31723c02ac30c6ca7e38f7974caf9e707937489c404b96656`
- r330: `6ce2336aac013e1d94412d25e17562d310280f6d96ee558360f024f4c4fb9dbe`
- r331: `c845d92a6acc35ee216548bbdc3d12b1cc2c5e50b709290ac7ac484d94d58256`
- V3 terminal report: `3bbfcb7c6cc9c9d1279530d96cebc167e4fde035a7d95066522a2c7f31a76404`
- V3 independent cleanup observation: `6b8771cf96d8aadcb798159bb4a6979132eb0a0306de1ae7c24af3671e594e14`

## Earlier Compiler Checkpoint: 13:00 UTC

The latest completed six-package `cargo check --tests`, r319 at local candidate
`c66e5f30a6f7624ee3aa3b507bd045b5bad339bd`, **passed** at 12:48 UTC. This
checks the backend, lowerer, verifier, compiler-execution client/coordinator and
protected-service spawn package with diagnostic source clauses enabled. It
compiles selected test code but executes no tests. The preceding r315 receive
assertion type error is corrected by checking both returned lengths; r313's
borrow error was already cleared in r315.

Three focused local suites also passed: r317 ran **29 process-tree cleanup tests**
with zero failures/ignored and 342 filtered; r320 ran **nine source-receipt codec
tests** with zero failures/ignored and 27 filtered; r321 ran **seven receipt RPC
profile tests**, including pinned legacy statement/execution/wire bytes, with
zero failures/ignored and 850 filtered. These are selected component
tests, not complete crate suites or successful compiler/proof executions.

The latest actual Rust source run, r312 at
`414549a7f201f6d37e179da2307fef0dadf19e08`, built the backend test binary but
finished with **zero passed, one failed**, none ignored and 4,213 filtered:

- Fill: gfx942 opt0/MIR0 and gfx950 opt3/MIR2 stop during original source
  assembly. The other two configurations reach the native-policy callback,
  then refuse an uninterpreted execution intrinsic before completed-source use.
- Vecadd: both targets at opt0/MIR0 reject execution availability against the
  original SSA instance; both at opt3/MIR2 reject inconsistent scopes at a
  control-flow join.

All ordinary configurations failed. Later planned negative controls are not
credited. No source clauses, Verus execution, protected proof or GPU run were
reached. The strict source-clause extractor was not run for r312.

Local repairs now join source events to exact value IDs independently of event
ordering, and bind genuine context-derived coordinates into scalar expressions.
The attachment census releases only its own temporary bookkeeping before
emitting retained output. Coordinate interpretation uses the already-global
cell instead of applying the invocation mapping twice. New source regression tests
are authored but not yet executed. Control-condition coordinate binding and
the proof-client/runtime changes also remain local, not end-to-end acceptance.
Complete source-bound proof execution, paired publication/finalization and
continuous protected compiler enforcement remain implementation work.

The static musl runtime-test build now passes (r316 and r318), using the unchanged
Linux UAPI request value where musl's `libc` does not export its name. At the
r318 candidate, **all twelve native process-tree controls passed on MI350** in
separate fresh processes: eight budget/accounting controls and four cancellation,
retained-descendant, selected-error and unwind controls. Every run produced one
passing selected test, consumed the original waits and removed its private
filesystem and cgroup resources. A separate read-only check found all 24 recorded
paths absent. None of these tests executed a GPU kernel or a protected proof.

Two earlier setup attempts are not counted as tests. The first stopped at a
missing C `pid_t` declaration, fixed by including `sys/types.h`. The second
exceeded the upload deadline before compiler/test execution; its partial file
was removed only after checking the recorded directory/file identities and
expected binary-prefix hash. The reviewed successor records partial uploads for
cleanup and uses SSH compression. Its ten local input/result/staging tests passed;
these do not replace the twelve actual native controls above.

The primary independently audited source inventories, tool identities, raw logs
and runner before moving each validation worktree. Raw log SHA-256 values:

- r312: `482e16514b9452bc22a9f45f964ebe6a98a72c6e474d859d1bb1a4e56603ea03`
- r313: `01cc23abaaae6b60c8537da43bdf6c6ab115623b85b9d7463e1c318e3795649d`
- r314: `e13e2c722b2af41947160143cff05885cb6320b5afe2e2723a5bebdb9f58e86a`
- r315: `242306048164a64e17b0b6a94a1483b3c9be7210868bff45750244069bed1c0a`
- r317: `dcac2b911e4042f5bdf0a6308a5cec91d7deba50e4bad883252e1980909b4e0a`
- r318: `79e84d3de455f9b9ed254614543a1562d8b04922b730d7d611d3fb79450f5da7`
- r319: `b3239ad751063505e3b31a26580ce7daebc4844d48113404f9b66e55ac9506b1`
- r320: `e07aa564c4a935a557a54a6c8c318d771f2c29e0d274956d8f3623ff49e449b6`
- r321: `f57c3611b24ff2f5d46f332018ee8e59605d22373f7ef73b811f744837fee27a`

Both public mains were read at
`ae4f0206a693b0c09a1847007c857702aea97c65` before this update. This publishes
evidence, not the unfinished compiler candidate. **M0 alone is complete;
M1-M7 remain incomplete; strict coverage remains 0/47.**

## Previous Compiler Checkpoint

At 11:34 UTC, r310 completed the five-package `cargo check --tests` at
`3dabc6f3791f6cf876f609f4be6d2ffa854d670b` with **five verifier compilation
errors**: three unavailable formation-type imports/uses and two mutable-budget
argument mismatches. No tests ran. The preceding r309 failed on three
scalar-query helper visibility errors; its narrow visibility repair is included
in r310. These are completed failed checks, not observation timeouts.

The latest actual source execution, r308 at
`87f35a3a9a0415d05333061d3425ca917daaedf5`, built the backend test binary and
finished with **zero passed, one failed**, none ignored and 4,114 filtered.
The fill parent exercised gfx942/gfx950 source configurations. gfx942 opt0/MIR0
stopped at unsupported original private-expression derivation; gfx950 opt0/MIR0
stopped at execution lifecycle during pending root emission. Both targets at
opt3/MIR2 stopped at an execution-lifecycle check during a producer call.
The parent failure does not establish that every planned negative control ran.
No actual source-clause frame was emitted; the strict extractor refused the log
and created no proof input. No Verus, protected proof or GPU execution occurred.

Original integer-cast reconstruction and source-proven consuming-Copy handling
are now integrated locally, but an actual source rerun has not yet confirmed
these repairs. Multi-effect memory tracking, scalar/read symbol binding and
typed original-source borrowing are also local work, not end-to-end acceptance.
The complete source-bound proof request, authenticated receipt consumption,
publication within the original owner lifetimes and continuous protected
execution remain implementation work.

Both source inventories, tool identities, raw logs and runner were independently
checked before changing each validation worktree. Raw log SHA-256 values:

- r308: `7a11d50b450bdb64ad2ec2eb64cbb7b23e1cd3127d03d929458580cda65c2fff`
- r309: `1d5677e13c7b3070f1c4730c56403cc4ce477c4efeb77c3baa037506cdfe4d3b`
- r310: `62fd050e64e583aca767a412b49fa5731a8dda8a8283736131e29109a495c174`

Both public mains were independently read at
`e9d283947e171346b55dc6112bc2dff568fbe135` before this documentation update.
The unfinished compiler candidate remains local. **M0 alone is complete;
M1-M7 remain incomplete; strict coverage remains 0/47.**

## Earlier Actual Source Checkpoint

At 10:54 UTC, r305 completed the actual Rust fill/vecadd parent tests at
`cb4a7f048677b89d61ea4e0189194e4055cbdbd7`: **zero passed, two failed**, none
ignored and 4,101 filtered. The backend test binary built successfully, but both
tests failed before the expected original-source consumer was reached. Their
assertion hid the underlying compiler refusal, so its cause is not yet established.
A subsequent diagnostic-only change preserves that error in the assertion;
it has not been rerun and is not a fix for the refusal.

No generated source-clause frames were reached. The strict extractor refused
the log rather than substituting proof text. No Verus, protected proof or GPU
execution is credited. Source/tool inventories stayed unchanged, and the
primary independently checked both source inventories, raw log and runner:

- r305 raw SHA-256: `775cc9ee2c8c188ad69e462f28ea15f82e04d84bace473abd378a0c481914ff9`

The preceding r304 four-package test-binary build failed with ten diagnostics:
nine from an ambiguous coordinator callback error type, and one from a scalar
test expectation for a tuple-valued receive API. Corrections are committed
locally but have not been rebuilt. r305 selected the backend alone and does
not validate those corrections. Its predecessor's independently checked raw
log SHA-256 is `967d534d369ced484db3fb3e763680259f079df0a5fb58a8bfc96decf90eed5f`.

Newer reviewed proof-client cleanup, native source-admission compatibility,
original typed-root borrowing and target/descriptor/Worker integration remain
local and untested. Full source-proof request construction and continuous
protected compiler enforcement are still implementation gaps, not merely GPU
testing tasks. Both public mains were read back at
`7cfa074cf97c51176f042fe33fd531f7ddefcb81` before this documentation update.
**No milestone advances: M0 alone is complete; strict coverage remains 0/47.**

## Focused Component Checkpoint

At 10:19 UTC, r303 passed **75 selected descriptor, KFD and host tests** at
`aa7780fc31e692e64bb1896824f9aa26dce1c418`: descriptor 29, KFD 26 and host 20,
with zero failures or ignored tests and 625 filtered tests. These are component
tests, not GPU runs, complete crate suites or protected-proof execution.

Ordinary pointer formation and its eventual memory access now retain separate
source coordinates and address envelopes in the V28 contract. Shared formation,
type and link requirements remain checked. Both envelopes are checked against
the live mapping, including zero-access formations and independent overflow
cases. The codec itself does not authenticate source correspondence or grant
compiler, artifact or launch authority.

The preceding r302 selection had 74 passes and one failure: a newly authored
test incorrectly expected an unchanged formation identity when another contract
row changed. The existing hash intentionally binds each row to the whole
contract. Only that assertion was corrected; production hashing was unchanged,
and the test still requires the decoded formation row to remain identical.

The primary independently checked both runs' source/tool inventories and raw
log hashes:

- r302: `95ffeef3b43586390dbf52c031a8775392fedaf7628f97d7e875da5465ebf2b7`
- r303: `adf15b3b6b30c79c9b58f5063b00d8f67b8cb2e5a657cf41925b071cf2610918`

The broader compiler integration is not yet validated. The r300 lowerer-test
build exhausted its unchanged 16 GiB virtual-memory allowance; no tests ran.
The r301 native-backend build then stopped with three invalid block-ID method
calls in source-control verification; no tests ran. Those calls were corrected
to use the original source block ordinals. New original-formation, borrowed
proof-client and target-consumer integration requires its own build and actual
Rust fill/vecadd execution; the component pass above does not validate it.

Protected execution still needs the complete original compiler/proof connection
and continuous enforcement. Provisioning, endpoint association, process parking
and ordinary proof-text diagnostics are not protected proof results. No required
missing source obligation is waived. Both public mains were read back at
`978379990f45c126c6191ed9517288ee695ba4a6` before this documentation update;
the unfinished compiler integration remains local. Full-range publication gates
also remain unresolved. **M0 alone is complete; strict coverage remains 0/47.**

## Earlier Compiler Checkpoint

The completed r294/r295 runs used the local candidate
`0d4f37f1e2b8d963aeb28891dc6c872c52d980fd`. The primary independently checked
the source/tool inventories and raw logs. Neither run executed a GPU kernel
or earned protected-proof credit.

| Run | Executed scope | Result |
| --- | --- | --- |
| r294 | Selected kernel-IR, lowerer, Pliron and verifier library tests | 884 passed, 47 failed, 10 ignored |
| r295 | Complete kernel-IR library suite | 1,501 passed, none failed, ignored or filtered |
| r296 | Four-crate `cargo check --tests` on the newer integrated candidate | Passed; no tests executed |

r294 includes kernel-IR 37/0, lowerer 596/47, Pliron 73/0 and verifier 178/0
(pass/fail). Its ten ignored tests include checks requiring actual installed
protected-runtime prerequisites. These are **test counts, not kernel counts**;
the selected roster is broader than r285, so totals are not a same-selection
comparison. Remaining lowerer failures include access/formation correspondence,
parent cleanup accounting and insufficient fixture work allowances. The latter
require diagnosis against actual optimizer charges, not arbitrary cap increases.

r296 checked `fe2o3-kernel-ir`, `fe2o3-lower-mir-kernel`, `fe2o3-pliron` and
`fe2o3-verifier` at `bfb609bb5b9b5ead0b6ef048f8849c8c085b031c`. That candidate
includes the completed-source clause-input adapter, read-binding/formula
components and a fix distinguishing the original pointer formation from its
forwarded memory-access operand. This passing compile check does not validate
their behavior or Rust backend tests, which need their own build and execution. Required missing
source/memory obligations still refuse before aggregate proof execution.

Independently checked raw log SHA-256 values:

- r294: `514b80f449a17d220a9083e30dd1ccde86bfc80252a77c6d244100a250a9faaf`
- r295: `7c5a483f7e879e5586a587d9c992736757ac409a60afac92028a97b858802d99`
- r296: `2e12b3aed5f03f17268b0043735349c97a87936d3ae483f87e2e0e58ac50de37`

Further expression-pair, scoped native lookup and parent-cleanup fixes are
integrated locally but have not passed an execution rerun. The genuine Rust
fill/vecadd results remain the failed r272/r273 runs below. The protected client
and helper transport are still unqualified, and the active driver lacks the
complete confirmed-execution and continuous-enforcement connection required
for protected proof execution. An endpoint, handshake or process-output record
does not replace that connection or a verified proof.

The unpublished integration also has unresolved full-range hygiene and inherited
commit sign-off findings. Passing checks on individual patches do not establish
a passing publication gate. Both public mains were independently read back at
`ca8647fc1932dece0a5b34e1b75721d482225d7e` before this documentation update;
the unfinished compiler integration is not part of that publication.

## Earlier Checkpoints

The completed full kernel-IR library run r286 passed **1,497 tests**,
with zero failures, ignored tests or filtered tests. Its candidate was
`dce057115143ba327739877aab424ff56eb96eea`. Source/tool inventories stayed
unchanged, and the primary independently checked the raw log digest:
`f7dcd87666278c67be120740e340a72d8939239ff5490b3926200e5c0ce3a51b`.

This implements scoped pointer forwarding through exact SSA block arguments,
retaining the original producer and scope lifetime. Presence guards
propagate in the direction of actual CFG edges. Mixed origins, unconstrained
entry parameters, closed scopes and unsupported escapes remain rejected. This
is a canonical-IR result, not a genuine Rust frontend or GPU result.
The scoped-memory projection now distinguishes the actual access operand from
its original pointer-formation result and checks their typed, unique SSA origin.

The selected compiler integration run r285 completed at
`820f953e1f4598ac967b3c55963ad2f2e730cedb`: **455 passed and 47 failed**,
none ignored (lowerer 410/47, Pliron 20/0, verifier 25/0). These are test counts,
not kernel runs, and the selected roster differs from earlier runs. Source/tool
inventories and the independently checked log digest remained stable:
`bc3ae6748de815c7be6e23a9cfe6765e215a1306ee7d04f8ab574de125033c74`.
Failures include source/native pointer correspondence and fixture assumptions
about generated abort edges, deferred cleanup flags and query-counter scopes.

The successor combined `cargo check --tests`, r287 at the r286 candidate,
failed with **88 lowerer compilation errors**; no tests executed. The errors
include generic lifetime coupling, source-block identifier types and a helper
still restricted to the older source profile. These are being repaired, not
waived. Source/tool inventories stayed unchanged; raw log digest:
`5987c1c01e7c71bf8573490e379d758005c3a66ef388688761ef2329cd5ac47c`.
The full kernel-IR pass does not validate this unfinished compiler integration.

The reviewed lifetime/interface repairs were integrated at
`79d670124cc2b61151bbfecac29d1ac0a6369740`. The next combined check, r288,
stopped with **three compilation errors**: two scoped-wrapper lifetime arities
and one shared native-view outlives bound. No tests ran. Further errors may
be exposed after these type errors are fixed; this is not a passing build or
evidence that every earlier error is resolved. Source/tool inventories remained
unchanged, and the independently checked raw log SHA-256 is
`7b8c7f6e85836575bbe686092c7145f864df470501f5f8627ec931258118ec76`.

The successor r289 at `db6f0c379c10a477d00c4678216eea4593f87cb2`
includes the scoped lifetime repairs and exact original-read symbol interface.
It stopped with **two lowerer lifetime errors** in the final LICM native
handoff; no tests ran. Its seven new source-read tests remain unrun, and the
complete memory theorem consumer is still being implemented. Source/tool
inventories remained unchanged; independently checked raw log SHA-256:
`fce0b69e3bda660e2e486e6b324deb575d70ed76be43a54830b7b4d22ecd114f`.

The preceding combined compiler run, r279, finished at
`af11b1f931ee622dfb2029e551660fbf05ae79b6` with **402 passed and 93 failed**:
lowerer 357/93, Pliron 20/0 and verifier 25/0. No tests were ignored. Remaining
failures included scoped-pointer CFG transport, transient enum projection and
test observers that did not handle the emitted Switch terminator. Its log digest
is `8d6510fe6aa2c3c10ac0b9c4089bdb765cf2fe0d9f4b1548e48b4f84ad3f5de9`.
The successor integration at `820f953e1f4598ac967b3c55963ad2f2e730cedb`
includes repairs for those boundaries. Its r284 build stopped with **no space
left on device** while compiling the lowerer library test; **no tests ran**.
The runner also failed to write its final inventory/report, so this run has no
source/tool-stability attestation. The retained partial log digest is
`167a3f163c46a107a72f2437b7c2804083170764e9ccad3df2d25c69005184ae`.
The latest genuine Rust fill/vecadd results remain the failed r272/r273 runs
described below. Disk recovery is not validation of the new candidate.

Both public main branches were independently read back at
`3c1205861765a99459f9a4b4513760ff17bcceac` before this documentation update.
That publication fixes target-scoped dependency policy, the missing codegen test
shard assignment, and workspace formatting. All five codegen shards passed in
the [CI run](https://github.com/harsh-nod/fe2o3/actions/runs/36826853167);
generic-core subsequently failed, so the overall run failed. Its `cargo-fe2o3`
suite had 470 passes, two failures and five ignored tests: both failures rejected
the stale reviewed workspace macro-tree pin. Review found exactly four changed
files since the accepted tree: generic typed-entry validation and its tests in
`54585b104`, and a nested fixture lockfile synchronization in `ce7bed431`.
The complete 59-file tree was independently hashed and its local pin updated;
the former pin and intermediate source-only pin are explicit negative controls.
External-source and device-source pins and all closure checks remain unchanged.
The pin repair was published as `65d0910b36c222e09598ae4d5817ee6004fdf190`.
Its five codegen shards and parity gate passed, but generic-core was cancelled;
that run does not establish a passing release gate. Both public mains were
subsequently read back at `a209ae259299069479570cff7726cec7ff0b448e`, preserving
concurrent literal-repeat and retained-storage work. That candidate's
[CI](https://github.com/harsh-nod/fe2o3/actions/runs/36831694594) has five passing
codegen shards and a passing parity gate; generic-core was subsequently
cancelled. Neither cancelled run establishes a passing release gate. The
unfinished local compiler candidate has not been published.

Subsequent independent readbacks found both public mains at
`6ea0940fb67352bbc6a49c2ffab62e1f8ce803e9`, retaining concurrent context,
descriptor and reference-storage work. Its
[CI](https://github.com/harsh-nod/fe2o3/actions/runs/36833714801) was cancelled.
The next readback matched `cefb2f6426346a63a55b25dfac2e350d0a76c406` on both
mains; its [CI](https://github.com/harsh-nod/fe2o3/actions/runs/36834002610)
is in progress at this checkpoint. No complete release gate is claimed. Those
published changes do not include the unfinished scoped-source integration
candidate.

## Compiler Evidence

The guarded runs used pinned nightly-2026-04-03, locked/offline dependencies,
one Cargo job, serial tests and disabled GPU visibility. Their complete source
and tool inventories remained unchanged. The candidate was
`3fe6788ccb28d49f3eeb5254c96c3e9a6d55472d`, not the public release branch.

| Run | Executed scope | Result |
| --- | --- | --- |
| r270 | Selected lowerer, Pliron and verifier library tests | 167 passed, 82 failed, none ignored |
| r271 | Fresh Rust compiler-backend test binary, `--no-run` | Build passed; no tests executed |
| r272 | Genuine Rust fill, fresh compiler sessions | Failed before proof or GPU execution |
| r273 | Genuine Rust vecadd, fresh compiler sessions | Failed before proof or GPU execution |

r270 includes lowerer 136/81, Pliron 14/0 and verifier 17/1 (pass/fail).
Most scoped source consumers fail at original live guarded-memory admission.
All 14 source-census tests pass; the remaining formula failure is an invalid
fixture identifier, not an observed semantic-formula mismatch.

Each genuine Rust parent tests gfx942 and gfx950 at opt0/MIR0 and opt3/MIR2,
with two fresh sessions per combination. No prior backend binary or saved child
request was substituted. The observed failures are:

| Source | gfx942 opt0 | gfx950 opt0 | Both targets opt3 |
| --- | --- | --- | --- |
| Fill | Lifecycle correspondence | Original borrow not emitted | Index operand is `Copy`, not `Move` |
| Vecadd | Execution availability | Lifecycle correspondence | Index operand is `Copy`, not `Move` |

The consuming-index repair must establish an authenticated, single-use witness
and preserve original source/SSA identity and live-borrow checks. Simply accepting
copies or fabricating move events would not establish that invariant. Rust's
[runtime MIR operand rules](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_middle/mir/syntax/enum.Operand.html#variant.Copy)
do not require the `Copy` trait for every post-drop-elaboration `Copy` operand.

Log SHA-256 values:

- r270: `cdf0e1e04c715d7921a43800227843f0d1166f739bb693aa9f5f1c41f0d77dbf`
- r271: `a4cff1dabe50da409e8a6e883f876cce6aefe9680e8dbfd84656b157ada7eb5f`
- r272: `978a6aab04c7110f4adff829a4618bd99db7efae6be6ea0aef8a30b872819b33`
- r273: `856f784812a125e9c96eb99a5a4b03eb2fb14cd41197a12441b20ca12c806c77`

The subsequent r274 candidate,
`2b80425ad549bd42a29428f51b84ad3f2bb0736c`, integrates scoped private-source
completion, precise physical-access diagnostics and fixture corrections. Its
six commits pass DCO and hygiene checks. The build stopped at one inaccessible
type alias in a new test's independent frame-size oracle; **no tests executed**.
The one-line test correction preserves the exact size/alignment assertions and
needs its own rerun. r274 retained unchanged source/tool inventories; log SHA-256:
`245b06bfa3513246d70dc4c5bc9528691cb3f9eb555990b241bda7f0132af8e3`.

## Protected Provisioning

One fresh isolated MI350 attempt completed installation, genuine V3 protected
provisioning/readback and lower revalidation. All three actual stages returned
exit 0. Readback checked fresh records, key ownership, signatures, image identity
and service identity. Prior protected state was neither reused nor changed.

The original container failed the unchanged one-byte `flistxattr` check on
`/usr`. A private derived image precreated the mount targets, avoiding OverlayFS
copy-up there. No attributes were stripped and no acceptance rule was relaxed.

The executed private runner changed exactly the fixed image literal from the
committed `088d41dfecca8b3a7343331ac8854d2684b63bea` runner. It was not
byte-identical to that committed runner or a silently changed production default.

- Executed image: `sha256:bb06b7d78df7bf67a916ecaabcdf2b232bf3855cbc636949b796ef9497e07926`
- Executed runner: `5e8387951c51c93ee94ac8d383f4c3a19f3e4d705f9a7862918ef89bd94fe4da`
- Unchanged native readback: `4702554bf9f99fe799a41468a8e60702c05fec00426b8b154f2c8e710f753f7b`
- Public report archive: `21cafc7ec30d3b8800079db8127da3cd76b85d8e5295e1b84ed0c89c5349f6f2`

The primary independently verified all 47 public archive payload/manifest hashes
and six actual container snapshots. Those are **47 archive checks, not kernels**.
The snapshots retain the exact image, private PID namespace, read-only root,
no network, restricted capabilities, two CPUs, 2 GiB and 64-process limit.
Independent cleanup records confirm all owned processes and three containers
are absent; protected records and reports remain retained. No secrets were exported.

**Not executed:** service activation, protected-runtime assembly, protected
compiler/proof execution, simulator qualification or GPU validation. The image
recipe, fixed production pin and regression tests still require source integration.
The experiment does not establish a bit-identical Docker rebuild.

## Subsequent Runtime Evidence

The actual static proof-helper build from exact
`3c505a13f780c0f8703f5128bb724edefe0644c3` completed on MI350. Preparation,
build and actual ELF inspection returned zero. The 7,193,784-byte executable
has the expected secure entry, a nonexecutable stack, no dynamic dependencies
and no undefined symbols. Artifact SHA-256:
`f548930c6a3cce88c1974c08f48e6f82df20ca7b2f375481ae08c9d0e436a8be`.

All 42 public archive payload hashes were independently verified. Owned build
processes were drained and disposable caches removed; the artifact and reports
remain. **The helper was not executed or admitted as a production runtime role.**
Protected proof requests, compiler enforcement and complete runtime assembly
remain unfinished. Successful helper compilation grants no kernel coverage.

The committed image-admission gates at the same source revision also passed
for the admitted base and derived image, and rejected the old base-as-executable
substitution. This supersedes the earlier image-integration-pending note for
that local candidate only; it does not establish service or compiler activation.

### Deployment Tests on MI350

Exact deployment source `db78a9c36abc8d8a11dfe8b2a0f79205fd9bb7a0` passed a
nonroot MI350 run in private scratch with pinned nightly-2026-04-03, locked
offline dependencies, one build job and serial tests:

- Deployment library: **143 passed, zero failed, two ignored**.
- Qualification executable: built successfully; **four unit tests passed**.
- Compile-fail doctests: **19 passed**.

The two ignored tests require native root-owned filesystem authority and remain
unvalidated. All source inventories before and after the run matched. The
primary independently checked the exact source archive, selected raw report
contents and matching local/remote report-archive SHA-256:
`d0a305b99ae6acb98932706954bd6701daa36f2a5e001d742017ee86ffa41c2e`.
All owned sessions terminated. Owned build/dependency caches and temporary
source were removed; evidence and reports remain. No other user's work was
removed.

This run does **not** validate the newer integrated compiler/helper candidate,
native service activation, the fourteen-role descriptor/main-PID join, actual
proof requests, or GPU execution. The cancellation tests exercised the existing
flag logic, not injected cancellation during admission/recovery. Those acceptance
cases remain required; these component passes do not complete M1.

## Milestone Status

| Milestone | Status | Remaining acceptance |
| --- | --- | --- |
| M0: Contracts | Complete | None |
| M1: Minimal production slice | In progress | Genuine vecadd through source verification, protected proof, artifact binding and safe launch, with required negatives |
| M2: Hierarchy and synchronization | Incomplete | Wave/LDS reductions and scan, atomics, barriers, convergence and reuse-epoch matrices |
| M3: General memory and control | In progress | Integrated memory semantics, loops, helpers, cross-crate generics, dynamic layouts and multiple kernels |
| M4: Structured compute and targets | Incomplete | Generic GEMM, numerical/resource checks and target-matched validation |
| M5: Advanced kernels | Incomplete | Softmax, attention, MoE and remaining entries through the complete path and test matrices |
| M6: Authority and migration | In progress | Complete source/optimized/artifact evidence composition and retirement of superseded paths after validation |
| M7: Documentation and release | In progress | Complete coverage gates, qualified manifest, accurate tutorials and identical final implementation on both mains |

## Remaining Acceptance Work

The immediate gate is M1: one genuine vecadd source must complete source/memory/
control verification, protected proof, artifact binding and generated safe launch
in one production transaction, including its required negative tests. M2-M5 add
hierarchy/synchronization, general memory/control, structured compute and advanced
kernels through that same path. M6 requires complete authority composition and
legacy-path retirement; M7 requires coverage-enforcing CI, qualified manifests,
accurate documentation and identical final publication to both repositories.
