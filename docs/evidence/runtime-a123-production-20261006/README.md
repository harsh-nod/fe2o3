# Runtime A1/A2/A3 Production Integration Qualification

This directory records scoped qualification while the native production path is
being integrated. It does not close issue #182, any complete A1/A2/A3 milestone,
HIP/HSA parity, or protected multi-GPU qualification. No hardware-overlap result
is recorded here.

Raw archives named below are retained in private, agent-owned qualification
storage, not committed to this repository or publicly retrievable. This follows
the repository policy against committing build outputs and large traces. The
committed records retain hashes, sizes, commands, source identities and exact
scope; reproducing the maintained campaigns remains the public verification
path. Future CI artifacts have finite retention and must not be described as
durable public evidence without a separate published archive.

`results.json` indexes the earlier CPU command statuses and raw log hashes,
including unsuccessful batches; later campaign sections retain their separate
archive and source identities. Nested exact-test child summaries are retained,
so those counts must not be summed as distinct tests. `artifacts.json` records
the private archives' byte lengths and digests. These indexes are inert
provenance records, not authenticated proof or launch authority.

## A2 Live-Validation Composition

The two historical A2 campaigns below precede the final production
modularization. They qualify their archived source inventories only. The later
signed-source replay is recorded separately below; no result qualifies the
subsequent cancellation, staging or cohort changes merely by inheritance.
Interrupted later campaigns are retained as incomplete evidence in the artifact
index, not substituted for complete runs.

`a2-live-campaigns.tar.gz` contains the complete maintained campaign and the
development diagnostics, including rejected runs. Archive SHA-256:
`fc0e3a0c808179e14dd30853dc35ea1848685f3eefb13135fdbfb006bb7d28c6`.

The maintained `maintained-campaign-02` completed successfully: six whole-root
positive brackets and 108 strict logical negative controls. All 114 solver
records report absent owned process groups; statuses are six 0s and 108 1s.
Source hashes were unchanged at every stage and at closing. The final concrete
positive verified 218 obligations with zero errors. First solver start to final
solver finish was 3735.463463231 seconds. Result JSON SHA-256:
`261f3e4185d561af512fba7802fab0af676d2fb39ac738329712ed30c106ffc1`.

The archived `QUALIFICATION_SCOPE.md` gives the exact invocation, source roster,
pinned toolchain, limits, development failures, and proof boundary. In particular,
the proof covers the extracted live-validation bodies and producer composition,
not the credit-lock/Arc adapter, complete Context orchestration, OS/hardware,
protected application provenance, or runtime launch authority. Parser fixtures
and timeouts are not fresh logical proof controls. The archive is evidence, not
a signing or execution capability.

The production workflow contains a maintained reviewed-host job, but reviewed
runner capacity has not been provisioned or qualified. Source integration and a
local campaign are not evidence of a successful production CI run.

## A2 Producer-to-Planner Input

`a2-planner-input-campaigns.tar.gz` retains the complete new scoped campaign,
its failed development attempts, input inventory, commands, limits and raw
diagnostics. Archive SHA-256:
`fbc8755afacea3ba070765875e55dbef9cd362a5f513abb8fc1eeecc490d13ce`.

The successful campaign has two whole-root positives (260 verified, zero errors
each) and eleven exact logical negatives (259 verified, one rejected function
each). Both full pinned verifier-closure checks passed; elapsed campaign time was
429.561774739 seconds. Eleven checker tests passed separately. All 35 process
groups recorded across development and final runs were absent before cleanup.
The scoped input manifest SHA-256 is
`a59fd44d08081b5330794de94f562859e73d96c192dfaa2e12c54ab49111b1f4`.

This connects the actual concrete reconciliation return to the existing finite
planner input leaf. It does not prove complete mutable Context execution, DAG
traversal, settlement side effects, credit-lock/Arc identity, or GPU behavior.
See `crates/fe2o3-runtime-model/verus/PRODUCER_PLANNER_INPUT_V1.md` and the archive's
`PROOF_SCOPE.md`. The generic auxiliary gate runs the deterministic checker and
its tests; the reviewed-host lane runs the actual proof campaign. These are
different evidence classes, not interchangeable successes.

## Signed Foundation Replay

The following completed campaigns use signed commit
`49df8c3c490d0cd8d30a2ab3a56b6bd4e193d8f6`, not the later feature candidate.
Its complete remote source inventory covers 15,284 files and 220,045,019 bytes;
opening and closing byte/mode inventories agree. The pinned Verus closure has
190 files and 129,019,839 bytes. The copied public Rust toolchain and its
installation-metadata differences are recorded explicitly in the raw evidence.
No private signing key, GPU execution or protected application exchange is
part of these campaigns.

- `native49-live114-planner13-evidence.tar.gz`, SHA-256
  `a0794e1e083570f5793f38a6e179a80c0e4e9ef7f0d996b3e40d8fec81394429`:
  live validation passed six whole-root positive brackets and 108 strict logical
  negatives in 461.316 seconds; planner composition passed two 260-obligation
  positives and eleven strict negatives in 60.495 seconds. All 147 recorded
  process groups across setup and accepted campaigns are absent. The first live
  attempt remains rejected because its stderr did not meet the classifier;
  fixing the private Rust setup did not relax that classifier.
- `native49-live114-planner13-semantic-audit.tar.gz`, SHA-256
  `03b0542e499c616a97f8b0b805ab41864299665aa3e1e105e450f3608051d877`:
  an independent archived-log replay reclassified all 127 accepted solver records
  using the maintained classifiers and exact staged sources. This is an audit of
  the recorded execution, not a second solver run.
- `generic-verus-signed49-mi350-20261007.tar.gz`, SHA-256
  `2c91dd7880229078f325146330d0cb58323de8f97fedaa4704c6df36a8f0c95b`:
  the unchanged generic gate exited zero in 1065.285 seconds, including all seven
  executable mutation campaigns and all 694 distinct fixed negatives. Source and
  verifier inventories are unchanged; all 143 observed process groups are absent.
  Its failed private-tool-path setup attempt is retained separately in the same
  archive, not counted as acceptance.
- `generic-verus-signed64-timeout-20261007.tar.gz`, SHA-256
  `99cbd9efa1d9c715fe15675b23a2a372eb06b73cad088bb64054f72d822131fd`:
  the older local foundation run failed on a 120-second lifecycle mutation
  timeout. It is incomplete evidence, not an accepted prefix or a logical
  negative. The successful remote replay kept the same per-proof limits.
- `a2-signed-49-template-retained-20261007.tar.gz`, SHA-256
  `43a994012e537d03a34008b4e69e49977ba160746a6e3f4d87852fb6e40aab41`:
  all seven signed-source campaigns passed, including 102 template/preflight
  negatives, eleven routing negatives and 25 retained-credit negatives. The
  194 command records contain 21 full positive brackets, 138 intended logical
  rejections and 35 signature, closure and calibration controls. The positive
  roots verify 30, 46, 64, 64, 15, 4 and 41 obligations respectively, each in
  original, relocated and closing runs. These are component counts, not whole
  runtime theorems.
- `a2-signed49-template-retained-semantic-audit-20261007.tar.gz`, SHA-256
  `5655f0456fd1216d42161bd6a21fb44eee4a96c30a96a0a1d334451d65745627`:
  independent replay accepted all 194 raw command records, exact signed source
  files, mutated/relocated input bytes, fourteen release-closure checks and seven
  source-signature checks. All 202 recorded groups, including controllers and
  the wrapper, were absent at recorded closure. This is not a new solver run or
  a current host-wide process census. The binder/roster's explicit std/hash trust
  and the retained-credit campaign's measured-only host-tool boundary remain
  part of the claim; the audit does not remove those assumptions.

These are component-scoped logical results. They do not prove complete Context
or DAG execution, opaque identity/lock behavior, native execution, performance,
or any whole A1/A2/A3 milestone. The reviewed production GitHub job still needs
qualified runner capacity; local success does not turn a queued job into a pass.

## Successor Source Controls

`a2-successor-source38-controls-20261007.tar.gz`, SHA-256
`ceeb059562a4be0f9a75f70c9690ccba67219b545aacca87a0650719f832f65e`,
retains all 22 passing source controls and the prior failed binder-shadow control.
The latter exposed an alias check incorrectly applied to whitespace-compacted
Rust. The correction uses the existing token-preserving lexer; the negative was
retained, not weakened. All 44 recorded groups across both attempts were absent.
Before/after source and support byte/mode inventories match. The existing
executable proof closures remain byte-identical to the signed foundation, but
the broader source inventories are new and require fresh maintained campaigns.
See `crates/fe2o3-runtime-model/verus/RUNTIME_SUCCESSOR_BOUNDARIES_V1.md` for
the precise cancellation, HostStaging and cohort exclusions.

`a2-source40-constructor-controls-20261007.tar.gz`, SHA-256
`d2e66caccb22151d9e35dd5d84e9dd9f452ca899b024fd22eac1f402748ecccf`,
retains a later complete 22-control replay after adding the original-primary
cohort constructor. All controls passed in 580.18 seconds aggregate, all 22
recorded groups were absent, and complete opening/closing source and support
inventories matched. This is source-control qualification, not solver execution
or successful compilation: the constructor test privacy error described below
was still present in this snapshot. Later test and incoming-main edits require
their own refreshed source captures.

Two separately retained CPU-only preflights prepare a future private replay:
`mi350-pinned-keygen-preflight-20261007.tar.gz` (SHA-256
`235d71f16b67208ede61cc81af109020e50d55bfcb4437d566adf76a37e67576`)
and `rustup-directory-override-preflight-20261007.tar.gz` (SHA-256
`93b3b6bb37092a412925c4cde5fa5da35790bae31183a47b856786c0eb8fafc2`).
The first tests the already-pinned public SSH verifier in a disposable private
mount namespace, without changing the host executable or copying a private key.
Host dynamic libraries are measured dependencies, not historical library pins.
The second demonstrates an owned per-directory Rustup override with downloads
disabled. Its two tiny one-obligation proofs test setup only. The first setup
assertion failure is retained: `rustup toolchain list` can return zero while
attempting a channel sync. Only private Rustup settings changed; source, Verus
and installed toolchain inventories did not. Neither preflight is successor
campaign qualification, hardware execution or a configured production runner.

## Incremental CPU Qualification

### Fresh Successor Replay

`native-snapshots35-38-fresh-cpu-evidence.tar.gz`, SHA-256
`7213c717e711471cf18c6ccc78c7c09a86060207340f893dcff46cb94e900ed6`,
retains 128 members: exact source manifests and deltas, clean/build/test scripts,
statuses, complete logs, closing source checks and final test-executable digests.
Snapshot 35 was captured only. Snapshots 36 and 37 each explicitly cleaned all
145 workspace packages. Snapshot 38 then cleaned the changed runtime package and
its host/physical-differential dependents; its only Rust delta from 37 corrected
the new capacity test's mock constructor. KFD source and dependencies were
unchanged from the fresh 37 build.

The accepted snapshot 38 batch passed all-target checks, strict runtime/KFD
Clippy, 2,465 runtime tests (34 ignored), 370 host tests (three ignored), and the
physical differential library/CLI/doctests. The 53 scoped runtime tests include
the actual newly added unpublished-cancellation and graph-staging tests; both
new staging-capacity tests and the retirement observations also ran. KFD's fresh
snapshot 37 run passed 2,061 tests (three ignored) and all its doctests. Runtime
and host results are CPU qualification, not protected or native execution.
Later cohort-constructor changes require separate KFD qualification.

Failed predecessors remain explicit: snapshot 36 exposed invalid scope-permit
test assertions and two test-only Clippy findings; snapshot 37 failed runtime
test compilation on a nonexistent mock constructor. Snapshot 38 setup initially
refused nine inherited Python bytecode caches absent from the source inventory.
Their exact paths and hashes were recorded before removing only those caches in
the new owned snapshot. Its corrected full-file inventory matches exactly.
The earlier snapshots retain their recorded extra caches; no old source was
silently rewritten. The source 37 setup script printed `Snapshot36`, but all
actual source paths, manifests and hashes were for 37; that diagnostic-label
mistake is preserved rather than rewriting the executed script.

`native-snapshots39-40-verifier-cpu-evidence.tar.gz`, SHA-256
`055dc01946ad4cb26f61b3e0281f8ed3b9c3f2fc0ec912954c4e44fc1d7ed4d6`,
retains 47 members covering two failed batches and their exact source inventories.
Snapshot 39 explicitly cleaned the verifier package but failed to compile two
test-fixture API uses. Snapshot 40 cleaned both KFD and verifier; the verifier
broker tests passed (24 passed, one ignored), followed by three full runs at
16 test threads (1,493 passed, 46 ignored each). These exercise isolated child
descriptor ownership and the sealed-executable readiness handshake, without
changing production admission. KFD's new constructor test failed compilation
because it accessed three private packet fields. No KFD library test in that
batch is accepted; its passing doctests are separate results. The correction
reconstructs malformed packets using existing constructors, without widening
production field visibility.

### Historical Cache Limitation

Qualification correction: later snapshot 34 exposed reuse of an older Cargo
test binary from the shared target directory. Its runtime commands omitted the
new unpublished-cancellation tests even though the snapshot inventory contained
them. Tar overlay timestamps can be older than cached artifacts. Consequently,
an exit-zero status or historical test count below is a recorded observation,
not sufficient evidence that every changed snapshot source was compiled. The
incremental CPU records require source-to-binary requalification; none may be
used to close a successor milestone on its own. Fresh replays explicitly clean
all workspace package artifacts and check that the new test names execute.
Direct Verus campaigns have separate source authentication and are not Cargo
test-binary results.

These are source-snapshot results, not qualification of every subsequent edit.
Builds used the pinned `nightly-2026-04-03` toolchain on `mi350`, two Cargo build
jobs, private Cargo/build directories, disabled HIP linkage, disabled core dumps,
and bounded command lifetimes. No GPU execution was performed.

| Archive | SHA-256 | Scope |
| --- | --- | --- |
| `native-snapshot05-cpu-logs.tar.gz` | `75b75a397da99ee23884df7fe7797b6219a3759541854823c8a80e66f68afe9b` | Initial CPU results and rejected builds/tests, including the original static-link failure. |
| `native-snapshots08-10-cpu-logs.tar.gz` | `1a468389392da58626591d96e04b1af3275a5e1814b15090fe71618f5a850b8c` | Static image/private-root tests, native custody regressions, scoped ownership/scale tests, and rejected intermediate runs. |
| `native-snapshot11-cpu-logs.tar.gz` | `ba629031cb0c349de46cd6b3ff5ceef69b3a0eecd0812038825d3a8cd57645d8` | Complete source-file digest inventory, exact batch/wrapper scripts, eight command logs and exit statuses. |
| `native-snapshots12-13-cpu-logs.tar.gz` | `f3b23c129eae73eb5fcb4559ee5bd3d9925405e53a8db57cb6aa9b625944a3c5` | Host/Context async-lending checks, original-account guard tests, doctests, fixed coordinator tests, proof-only fixture checks, and the three pre-isolation host failures. |
| `native-snapshot14-cpu-logs.tar.gz` | `ec664338c756c568f49a69b24784f2a3e8bcedd09d61ebb53dcec50aaaa79134` | Genuine native fixture binding-only async composition, repeated full host tests, full runtime regression, rebuilt static images/private-root contract tests, source and image digests. |
| `native-snapshots15-17-cpu-logs.tar.gz` | `35512fe9600cce905d5886a7729aa94958ef0c43abe5efbe0e987f79e1fd169f` | Shared admission-core extraction, its test-only unsafe-lint fix, lexical DAG compilation and ownership doctests, including the rejected sibling-test visibility build. |
| `native-snapshots18-19-cpu-logs.tar.gz` | `d554bad5025c0dc06c4b129a777effe13ff7f7e84618fe14ffdf5b4c0e323316` | Fresh main-based source inventory, cancellation compilation, graph and ownership controls, policy-exporter checks, and retained visibility/stdio/test-fixture failures with the exact bounded-copy diagnostic. |
| `native-snapshot20-cpu-logs.tar.gz` | `619f7fa8ad4e46924fdaa1efcd900a5779466fb1a22bd29af46bb8762386860c` | Full runtime/KFD/model/host/custody regressions, final cancellation controls, fresh static-image contracts and digests, plus rejected Cargo test compilation, dependency-policy and audit-invocation checks. |
| `native-snapshot21-cpu-logs.tar.gz` | `76bb2181cf95a42e0e349afb1a0d498383b34bc3d2097f5dbf5f6f1e741a3322` | Passing exporter and selected foundation checks, complete Cargo/verifier and service-manifest failures, original-account/host doctests, exact scripts and source inventory. |
| `native-snapshot22-cpu-logs.tar.gz` | `3125b256713239789db77f367311b1efed11583d8dfca45f5656cb1245c6ed4e` | Corrected Cargo/verifier and custody suites, static deployment replay, plus retained macro-fixture and build-script audit failures. |
| `native-snapshot23-cpu-logs.tar.gz` | `ac4f3b695573acb40cab6ae566a410974da05c27b3ebdb63e5d00dafcbb51a6a` | Joined compiler/application phase tests, doctests, binding-only compilation and static deployment replay; stale macro executable and overbroad Clippy invocation remained failures. |
| `native-snapshots25-26-cpu-logs.tar.gz` | `25a342d7b6e8d9c6cbd4916ae0ad8078ed49631a671d0c475a879edab934e3b2` | Post-modularization build repairs, full runtime/model/Cargo/host/broker/verifier suites, and retained KFD source-scan and phase-fixture failures. |
| `native-snapshot27-cpu-logs.tar.gz` | `1004acb0295b4e1f182ebae1454b69b9b7414ac4aa96446d084084641f41afe4` | Corrected phase/ownership controls, full coordinator/spawn tests and doctests; scoped Clippy findings and packaged-manager pre-Rust refusal remain failures. |
| `native-snapshots28-29-cpu-logs.tar.gz` | `af84f5b8fc841e6c57359e973a40b5e7fa6399c3399feacb21c786985e05fde2` | Complete signed-source KFD tests, fresh macro/binding and dependency audits; cooperative-driver/coordinator tests and corrected static contracts; both obsolete duplicate-child panic expectations remain failed full-runtime runs. |
| `native-snapshots30-31-cpu-logs.tar.gz` | `36094d1082fb4238acf00268164ab7048f5ad41bfa5868f3b702a4161886d7b0` | Bounded collision controls and two complete 2448-test runtime replays; retained seven-finding Clippy failure, then clean scoped Clippy and 17 scoped ownership doctests. |

Snapshot 11 passed the following full library suites:

| Package | Passed | Failed | Ignored |
| --- | ---: | ---: | ---: |
| `fe2o3-kernel-ir` | 1436 | 0 | 0 |
| `fe2o3-protected-service-spawn` | 341 | 0 | 32 |
| `fe2o3-runtime` with `scale-qualification,cpu-runtime-fixtures` | 2416 | 0 | 34 |
| `fe2o3-kfd` with `cpu-runtime-fixtures` | 2052 | 0 | 3 |
| `fe2o3-runtime-model` | 1138 | 0 | 19 |

The same batch passed twelve original-account doctests and the actual
`cargo-fe2o3 check` binding-only fixture path. The coordinator test target did
not compile in that batch because its isolated test body lacked the existing
test-only unsafe allowance. Therefore the batch itself correctly exited 1, not
success. Moving that allowance to the isolated body preserved production policy;
the subsequent coordinator run passed 356 tests with 20 explicitly ignored.

Snapshot 12 passed host all-target checking and all 99 host doctests (17 ordinary
and 82 compile-fail). Its full host run correctly failed three inherited-pipe EOF
tests while 366 passed and three were ignored; the new epoch controls passed.
Snapshot 13 passed runtime all-target checking, all 25 scoped ownership/async
tests, the Context Send assertion, and 15 scoped compile-fail cases. This includes
the public lending future awaited inside an already-running Tokio current-thread
task, LocalPool composition, and two 4096-operation rounds per executor.

Snapshot 14 passed both repeated full host runs (369 passed, zero failed, three
ignored each), and the full runtime suite with the new persistent scope gate
(2424 passed, zero failed, 34 ignored). The actual `cargo-fe2o3 check` path also
compiled the native single-device fixture's complete original-account -> host
proof -> Context async chain. Only `main` enters `block_on`; no nested owner
callback does. This is binding-only compilation, not a protected launch.

Snapshot 16 passed all-target runtime checking, all 59 existing/shared-core graph
tests, and all 25 scoped tests. Snapshot 15 had correctly rejected two new test
bodies lacking the crate's test-only unsafe allowance. Snapshot 17 compiled the
production lexical DAG and actual native binding-only fixture, and passed all
17 scoped compile-fail checks. Its unit targets failed because sibling tests
could not call a private helper; the following source makes that helper visible
only within its existing scope module, without expanding the public API.

The lexical DAG reuses the original admission/completion authority and data-version
ledger. Native completion and original result decoding precede graph success;
ordinary copy progress is cooperative and bounded per leaf. Its scope is
same-device control dependencies and existing copy-version lineage. It does not
provide producer-fed generated kernel arguments, cross-device DAGs, native
collectives, or evidence of physical compute/copy overlap.

Snapshot 20 passed 2442 runtime tests (34 ignored), 2052 KFD tests (3 ignored),
1138 model tests (19 ignored), 369 host tests (3 ignored), all 38 scoped tests,
17 scoped compile-fail checks and 90 custodian tests (18 ignored). Seven custody
libraries also passed their full suites. This batch still exited 1: new Cargo
test code needed an `OsStr` dereference, the inert codec fixture needed an explicit
dev-only workspace dependency exception, and the standalone audit command omitted
its required root package arguments. These failures were not counted as passes.

The corrected workspace policy passes 24 tests and the actual locked metadata
audit (143 members, 587 internal declarations). Its exception admits only the
custodian test fixture's `lower-mir-kernel` dependency; normal/build edges remain
forbidden. A workspace formatting replay passes after import/module-order fixes.
The runtime test module-order change also required fresh proof source pins and
restarting both active proof campaigns. Their interrupted prefixes remain
incomplete evidence, not adopted successes.

Snapshot 21 passed the eight policy-input exporter controls and seven native
fixture oracles. The complete Cargo binary suite passed 568 tests but failed two
workspace-local macro-tree pin tests (seven ignored); review of the owned-argument
adapter and all changed macro-tree files preceded the local-only pin refresh.
The external-source macro pin and admission rules were not changed. The verifier
suite passed 1470 tests but failed fourteen (45 ignored). The first new test
incorrectly expected cleanup to erase retained terminal records; its panic
poisoned the process-wide gate and caused thirteen follow-on refusals. The
corrected test requires the original root's exact consumed terminal, no unresolved
creation/trace obligations and original spawn-lease release. Production cleanup
and quarantine behavior were not changed by that correction.

The same snapshot's service-host suite passed 42 tests and rejected its stale
lower-layer queue-manifest reference. The service remains a single fixed-batch
owner; updating that reference does not admit the lower layer's additional
multi-epoch functionality through the service API. Its existing hash-binding
test remains mandatory. Other complete CPU suites in that batch passed: kernel
IR 1436 tests and artifact transactions 374 tests (one ignored), plus 99 host
and twelve selected original-account doctests.

Snapshot 22 passed full Cargo (570 tests), verifier (1493), runtime (2442),
KFD (2052), host (369), and all selected custody-library suites. The pure-Rust
audit rejected two unreviewed build scripts. Exact archive, build-script and
helper review preceded two narrowly pinned policy exceptions; the auditor's
native-library restrictions are unchanged. Its 30 regression tests pass, while
the final locked metadata replay remains pending.

Snapshot 23 passed coordinator tests (360), coordinator doctests (8 ordinary
and 74 compile-fail), deferred-phase tests (35), Cargo tests (570), service-host
tests (43), the selected custody suites, binding-only compilation and both
private-root deployment tests. Macro library and renamed-dependency tests
passed, but two typed-launch tests used a cached executable embedding snapshot
22's old fixture directory. They remain failed results until a clean package
rebuild and replay. The Clippy command also failed because it included
dependency lints; a correctly scoped `--no-deps` replay remains pending.

The subsequent modularization preserves the original production source growth
limits. Final source-policy, full CPU and maintained proof replays are separate
gates; passing source-scanner calibration alone is not a solver campaign.

Snapshot 26 passed full runtime (2442), model (1138), Cargo (570), host (369),
broker (450), and verifier (1493) suites. Four KFD source-scan tests failed
against relocated declarations; 2050 passed and three were ignored. The
relocated-source fixes preserve their original assertions and await replay.
One phase fixture compared a borrowed ledger identity across an owning move.
Its correction compares identity while the original pool remains stationary,
checks retained accounting after final shutdown, and adds a foreign-pool refusal.

Snapshot 27 passed all selected all-target checks, coordinator (365), spawn
(347), coordinator doctests (8 ordinary, 77 compile-fail), and the three exact
allocation-collision/credit-retention controls. The static replay built the real
images and passed 14 phase/child/creator controls, but its packaged manager
exited 126 before Rust rather than producing the required nonroot refusal.
The harness supplied environment entries that secure startup forbids. The
corrected harness uses an empty environment; it still requires exact exit 98,
message bytes, complete bounded capture and original-child reaping. Six Python
observer tests pass, including a real retained-FD empty-environment control;
the subsequent packaged replay is recorded below. Thirteen scoped coordinator
Clippy findings are addressed without introducing unaccounted heap custody.

Snapshot 28 represents the complete signed `64ba4c359` checkpoint. It passes all
2054 KFD tests (three ignored), all 30 pure-Rust audit tests and the actual locked
metadata audit over eight production roots and 82 packages. Cleaning only the
two affected packages in the owned target directory eliminates stale embedded
fixture paths: all macro tests pass (90 library, four renamed-dependency and ten
typed-launch tests), followed by the fresh actual Cargo binding-only fixture.

Snapshot 29 combines the coordinator/harness fixes with the cooperative-driver
candidate. It passes all 38 focused scope tests, including exact poll/wake counts
and 4096-owner sibling-fairness controls on Tokio and LocalPool. These are CPU
lifecycle owners, not 4096 concurrently GPU-published dispatches. Coordinator
Clippy passes with `--no-deps -D clippy::all`; 365 coordinator tests, eight ordinary
and 77 compile-fail coordinator doctests, 17 scoped runtime compile-fail tests,
and 40 native-harness Python tests also pass. The static replay rebuilds all five
images, passes the exact empty-environment nonroot refusal and both private-root
deployment contracts. It explicitly reports `joinedProtectedPhaseQualified=false`
and `rootServiceCleanupQualified=false`: no service or GPU run occurred.

Both snapshots' full runtime processes abort in an inherited duplicate-child
insertion test. That test still expected unwinding after allocation collision,
where the production owner must abort without dropping possibly live storage.
These runs remain failures with no inferred full-suite count. The test-only
successor uses the existing bounded exact-child harness: SIGABRT, an explicit
pre-fault custody marker, no panic/unwind, no core dump, a 20-second deadline,
16 KiB stderr cap and kill/reap on timeout. The independent post-owner recoverable
panic test and production collision behavior are unchanged.

Snapshot 30 passes all seven focused allocation controls and the complete runtime
suite: 2448 passed, zero failed, 34 explicitly ignored, no filtering, in 347.13
seconds. Its scoped runtime Clippy run correctly fails seven findings. Four
source files then add owning result/function aliases, preserve inline owner
returns without boxing, and simplify conditions with the same evaluation and
error order. Independent review found no ownership or behavior change.
Snapshot 31 passes scoped runtime Clippy with `--no-deps -D clippy::all`, another
complete 2448-test run (zero failed, 34 ignored, 348.49 seconds), and all 17 scoped
compile-fail ownership doctests. Neither run executes the ignored GPU tests.

The resulting 507-file runtime and 512-file runtime/schema captures pass all 15
producer source/diagnostic/construction commands with identical before/after
byte-and-mode manifests. Earlier stale-count and Context-file pin refusals remain
retained. Exact proof closures and all 119 live/planner mutant texts are unchanged;
that comparison is not successor solver qualification. Retained routing and
credit controls separately pass with unchanged narrow correspondence pins.

The complete signed local checkpoint `64ba4c359baa9e62d98132dcc7fa60d7ab332666`
passes workspace formatting and the actual commit-to-commit hygiene CLI against
`17078d5bff96de343c4d248c8e1883c4792d9bf0`. All 24 hygiene-policy regression
tests pass. This checkpoint is an input to further qualification, not milestone
closure or evidence of a successful production workflow.

Earlier failures also exposed test-process interference: unrelated libtest forks
could inherit a pipe writer or shared lock descriptor. Selected tests now create
those descriptors only after exact-test re-exec, with a watchdog, kill/reap guard,
and completion marker that rejects a misspelled zero-test filter. EOF assertions
remain single-attempt; production completion/cleanup behavior was not relaxed.
The spawn test's expected cleanup cost was corrected to include all original
retirement cells; no accounting limit was increased to hide the discrepancy.

## Signed Candidate CPU Replay

The signed candidate `2653a4ead0ac1c659b82c5d49b56230a1c096b1f` is published
to `runtime/a123-qualification-2653a4e` in both repositories. This is a
qualification branch, not a completed milestone or a `main` landing.

An independent comparison against all 15,302 signed Git blobs and their modes
finds that snapshot 42 differs only in five evidence/architecture documentation
files. All captured crates, tests, scripts and configuration match the signed
candidate. Snapshot 41 additionally differs in two KFD test observers, the
compiler-client test fixture and three proof-controller source-tree pins. These
comparisons establish source correspondence, not test execution or native
authority.

Fresh snapshot 41 rebuilds the affected KFD, runtime, host and Cargo packages.
Its full runtime suite passes 2,465 tests with 34 ignored, and its full host suite
passes 370 with three ignored. The 24 production-build configuration tests and
six production-plan tests pass. Runtime/KFD Clippy passes with
`--no-deps -D clippy::all`. Five new cohort-construction tests fail because the
shared test observer still expects a singleton read/write binding instead of
each original cohort member's write-only binding. The full KFD result remains
2,063 passed, five failed and three ignored; this failed attempt is retained.

Snapshot 42 corrects that test oracle without changing the constructor. It
independently specifies all three original write-only ranges and retains the
existing backing/content/ownership checks. After cleaning KFD and the compiler
client, all seven focused cohort-construction tests and the full KFD suite pass:
2,068 passed, zero failed, three ignored, 645.64 seconds. Strict KFD Clippy also
passes. The client passes both new short/overlong temporary-path fixtures,
all-target checking, and its full parallel suite: 111 passed and five ignored.
Strict client Clippy fails on pre-existing duplicate-module/non-Drop lifetime
markers and two newly added needless borrows. The two new test-only borrows are
corrected in snapshot 45; its focused new-lint replay and full client tests pass.
No full client Clippy pass is inferred.

The exact source deltas, cleanup/build logs, successful and failed commands,
closing source inventories and observed remaining executable digests are in
`native-snapshots41-42-candidate-cpu-evidence.tar.gz`, SHA-256
`f72ec4f60a77f428fef21f85496d3eb99c5964f965882dfde1b33ed41a5c7333`.
Its 53 archive members match the retained remote files byte-for-byte. Historical
KFD binaries removed by the next clean are not reconstructed from later ones.

All 22 snapshot-42 source/diagnostic/construction controls pass in 614.945
seconds, with identical before/after source byte-and-mode manifests and all 22
recorded process groups absent. The archive
`a2-source42-constructor-controls-20261007.tar.gz` has SHA-256
`7119517d189a1359a652dafb4a68700ef2f40993008a780b87f08c66fda6d0b2`.
This is source-control qualification, not a new solver campaign.

## Ledger And Cohort Foundation Replay

Snapshot 43's first clean compile refused a stale test-only raw-contract
expectation after introducing the closed singleton/cohort contract enum. Snapshot
44 added the shared ledger bodies but was never run because it inherited that
error. Snapshot 45 corrects the exact singleton expectation and the two new client
test borrows. After cleaning the runtime and client packages, it passes:

- All 2,473 runtime tests, with 34 explicitly ignored, in 345.08 seconds.
- Five cohort-foundation and five ledger tests, including the 2,304-case
  predecessor-equivalence test and explicit alias/empty-use cases.
- Runtime/host all-target checking, six graph-module tests, 53 scope tests and
  strict runtime Clippy.
- All 111 client tests with 16 test threads and five explicitly ignored, plus
  the focused newly introduced client-lint check.

The 45-member `native-snapshots43-45-ledger-foundation-cpu-evidence.tar.gz` has
SHA-256 `cc56872c927f3f72a06919df9412cd01a8750425562401ba2e92d90bda07ff7d`.
Closing inventories match all 15,305 files for snapshot 43 and all 15,307 for
snapshots 44 and 45, including modes and absence of extra files. Failed discovery
and the unrun snapshot are retained explicitly. These results predate the actual
three-original native receipt bridge and peer-gather graph integration; they do
not qualify those later changes or any GPU execution.

## Signed Candidate Component Replay

The complete maintained component pipeline finished on signed candidate
`2653a4ead0ac1c659b82c5d49b56230a1c096b1f`, fetched independently from both
qualification refs. The root supervisor closed all original groups and observed
its retained private mount namespace empty twice. An independent readback then
reclassified original raw diagnostics with their maintained family/selector/span
policies and recomputed the signed source and Git inventories:

- Generic gate: 64 direct positives, 694 expected-negative files, and seven
  executable-mutation families with 14 positive brackets and 117 mutations.
- Producer/live: six whole-root positives and 108 logical negative controls.
- Planner: two whole-root positives and 11 controls. Its maintained
  `signed_qualification: false` field is preserved, not promoted by the wrapper.
- Seven production families: 21 positive brackets and 138 logical negatives.
- All 15,302 source blobs and modes, nine Git metadata files and 6,861 reopened
  diagnostic/source/classifier inputs agree with the original archived bytes.
- Root closure covers 460 original groups and 2,496 recorded process occurrences;
  inaccessible observations were not counted as absent.

The primary private archive `candidate-2653a4ead-01-evidence.tar.gz` is
136,509,897 bytes with SHA-256
`6579fd14e5502b1946208a82c6e56479695dfbb1cf1ebfcfee2bc1a7298a5eae`.
The independent readback archive has SHA-256
`52eb3e959ace28fb622bead61b1999f76e1a60f1b45c5890329fe73b4afd0c94`.
The sole member exceeding the ordinary 16 MiB hygiene limit is the exact public
Git pack, independently checked for signature/tree/index agreement and complete
reachability from this shallow signed commit. It is a candidate-specific audited
exception, not a general relaxation. Failed audit-adapter attempts are retained.

The raw archive hygiene scan also reports eleven findings: seven committed
HSACO fixtures and four committed test-only private-key fixtures. Independent
byte comparison against this signed commit identifies every finding as the
existing public fixture. The failed raw scan and this separate findings review
are both retained; the scan itself is not described as passing. No user signing
key was copied into the evidence.

This is source-bound component qualification, not a whole-runtime theorem,
hosted production-workflow success, protected GPU execution or A1/A2/A3 closure.
In particular, it predates the new graph-ledger executable campaign and the
actual generated cohort/peer-gather changes. Those require successor qualification.

## Registry And Placement CPU Qualification

The later CPU snapshots retain both failures and successful replays. Their
results are not qualification of a subsequent signed commit or native hardware:

- `native-snapshots47-49-combined-cpu-evidence.tar.gz`, SHA-256
  `4387cefbea73b8f4064106015522ba87b499387bae3fb2989cc44777007dcf65`,
  records the three-original native receipt bridge and explicit peer gather.
  Snapshot 47 was captured only; 48 failed a private test-fixture access.
  Corrected 49 passed all-target checking, all 2,491 runtime tests (34 ignored),
  fourteen cohort tests and runtime doctests. Its combined lint command failed,
  and incorrectly specified focused invocations were not run successfully.
- `native-snapshots50-52-ci-fixture-cpu-evidence.tar.gz`, SHA-256
  `fc3c84d0badcb0d8828278c5f66b6d916cb63d1ef7392616b6d9c6caa1701514`,
  retains those corrections and subsequent CI fixture discovery. Snapshot 51
  passed strict runtime Clippy, full host tests (370 passed, three ignored),
  host doctests and the focused ownership/graph/verifier controls, but had
  broker fixture compilation and Cargo target-selection failures. Snapshot 52
  passed all 450 broker tests (35 ignored), while four Cargo tests failed from
  contention on the real bounded global reaper. Neither batch was accepted as
  a whole success.
- `native-snapshots53-55-registry-topology-cpu-evidence.tar.gz`, SHA-256
  `e1f9db59a285d69f6a1ce50a171d2d4aaa75e199ffcc15de1577e49fb752f9cb`,
  retains 56 members with exact closing source inventories. Snapshot 53 adds a
  test-only mutex around fifteen real global-reaper fixture callers, preserving
  the production eight-slot capacity and deadlines. Three full Cargo runs each
  passed 570 tests (seven ignored), and three full broker runs each passed 450
  tests (35 ignored), all with sixteen test threads. Expected failing nested
  children remain in the raw logs and are not additional aggregate failures.

Snapshot 54 adds the lower four-original native recipe registry; it was
materialized but not executed independently. Snapshot 55 adds bounded
topology-aware peer placement and explicitly cleans KFD, runtime and host
artifacts before compilation. It passed all-target checking, eleven registry
tests, six actual four-recipe adapter tests, thirty-six peer-gather tests,
115 compute/XGMI tests, sixty graph tests and fourteen cohort tests. The full
runtime passed 2,498 tests (34 ignored); KFD and runtime doctests passed.
The full KFD run had 2,078 passes, one stale source-structure assertion failure
and three ignored tests. Strict Clippy separately rejected the new registry
admission's large original-owner error return. Those failures keep the batch
failed; its passing prefixes do not close KFD qualification.

The source-structure correction checks the ordinary delegation and both
selector branches' ownership/poisoning order. A function-local lint allowance
preserves allocation-free return of all four original owners on refusal.
Snapshots 56 and 57 capture these narrow corrections. The fresh snapshot-57
replay passed all 2,079 KFD tests (three ignored) in 632.86 seconds, the focused
selector guard, four ordinary and sixty compile-fail doctests, and strict
all-target runtime/KFD Clippy. Source 56 was materialized but not separately run.
The twenty-member `native-snapshots56-57-kfd-selector-cpu-evidence.tar.gz`, SHA-256
`d4a475e93454f7b6540449fcee9aedc98ceaa3f0292e3da6e334f48b817a741d`,
retains exact closing inventories for both 15,329-file snapshots.

The broker fixture helper was extracted in source 61 to preserve the maintained
module-size limit. Two full broker repetitions passed, but the third exposed a
test startup race: the original process descriptor arrived before exec, and the
strict procfs observer could see the transient empty environment. That failed
batch remains recorded in `native-snapshot61-broker-helper-cpu-evidence.tar.gz`,
SHA-256 `a23ab6b8ba174f758d86447467805a7723e7cf887ba520f3c7fc5f7803149d4e`.
Source 62 adds a distinct post-exec readiness byte and establishes fixture
ownership immediately after spawn. It does not retry or relax production
observation. The focused run passed 21 tests (five ignored), followed by three
full sixteen-thread runs of 450 tests each (35 ignored), in 13.50, 13.47 and
13.60 seconds. `native-snapshot62-broker-readiness-cpu-evidence.tar.gz`, SHA-256
`67f77c580f76fed3b3cc51168f6dc72174ce7293329c1096a7e13ff96c3d5cb4`,
retains the exact closing source inventory and all command results.

The isolated future-feature snapshots 58 through 60 are not integrated into
this candidate. Their discovery archive
`native-snapshots58-60-independent-discovery-cpu-evidence.tar.gz`, SHA-256
`a900cd1f1f7ccdb131757f6af903c30a5a5621bc5cf1cc801edab3b17acb5580`,
retains a Registry4 runtime bridge compilation failure and a replica checkpoint
with fifteen passing focused tests but three strict Clippy failures. Neither
failed experiment is successor acceptance or native qualification.

The separately retained source-only archive
`a2-integration-source-controls-20261007.tar.gz`, SHA-256
`14f02f80654c5afb63043c2737f2f1494e0e3824fb1e760e2088b5a7c1fca338`,
contains 23 passing controls and exact before/after source inventories. It
predates Registry4 and topology placement. It is not solver execution and does
not qualify their later source-inventory changes.

The replacement source-57 controls passed all 23 maintained source-only stages
in 494.668 seconds, with all recorded groups absent and all 3,344 source/mode
records unchanged. The archive `a2-source57-controls-20261007.tar.gz`, SHA-256
`945f78dbff00ed495fded11f7708fc083fe4d8210e207131ed5cc1281ded4283`,
records the reviewed Registry4/topology inventory refresh and explicit theorem
exclusions. These controls do not execute the signed successor proof campaign.

The generic finalizer job's previous 3,000-second step expired while tests were
still passing. Its replacement partitions the complete default Cargo/libtest
inventory across four jobs and requires their exact disjoint union, original
ignored semantics, and doctests once. Stable per-shard artifacts within the same
workflow run preserve successful receipts during partial reruns; successful
replacement uploads overwrite only their own shard. Eleven inert controller
tests and all 23 generic-CPU wiring controls pass. The archive
`finalizer-ci-shards-20261007.tar.gz`, SHA-256
`5ab9ef5faacf3865fb7299d462ced7252b7e02c737e25f581ed927cd7c4c98bb`,
preserves the initial control timeout and pre/post rerun-fix sources. Real Cargo
discovery, four-shard execution and aggregate coverage remain separate gates.

## Static Native Deployment

Snapshot 8 built and checked all five musl images: proof manager, native
application manager, both application proof controllers, and administrative
proof-custodian provisioner. The verifier's `close_range` call now uses the Linux
syscall directly, retaining the same single-attempt fail-closed behavior without
requiring a libc wrapper absent from the static toolchain.

The maintained `scripts/qualify-native-static-cpu.sh` then selected the actual
libtest executable from Cargo JSON and passed both exact ignored private-root
deployment tests. Tests used original-root credentials in disposable private
mount namespaces, with temporary configuration roots. Nothing was installed in
the host's service namespace. The proof installation case passed one test, and
the manager installation case passed one test; their exact-test roster checks
refuse a zero-test success.

These tests cover installed-image/profile/account and replacement contracts.
The manager case uses an inert installed image and refuses live-manager admission.
They do not show a successful genuine manager/application exchange or protected
GPU execution. The hosted `native-static-cpu` job passes on signed candidate
`2653a4ead` in [GitHub CI run 37580588847](https://github.com/harsh-nod/fe2o3/actions/runs/37580588847/job/112659080753).
This qualification-branch result is not a successful `main` workflow. The generic
validation aggregate now requires this job, including rejecting a failed,
cancelled, skipped or absent result. Its wiring controls cover all three required
jobs; source integration alone is not branch protection or a successful run.

The reviewed-host proof job also runs the five signed-source dispatch/producer
campaigns, retained-routing/credit campaigns and the new shared graph-ledger
campaign. Every added evidence
directory participates in the existing always-upload and owned cleanup steps.
Local parsed-workflow tests exercise the exact command arguments and fail-fast
shell sequencing; five controls pass, including the commit-specific runner label
and unchanged repository, main-ref and non-pull-request conditions. The label is
routing, not authorization. These stubbed invocations do not run a solver. This job still
needs a properly isolated reviewed runner, with the pinned Git/SSH tools and
unchanged proof resource limits. No shared host was registered as a runner.

The complete static CPU script passed again on snapshot 14 after the original
account and host async changes. Its archive includes fresh five-image digests,
the release ELF contract, and both private-root tests (one test each). The same
restrictions above still apply; neither test starts a genuine native application.

## Registry And Retirement Successor

The current source adds distinct four-operation, two-cycle four-operation and
sixteen-operation generated registries. Original source carriers and resource
credits remain owned until the common native owner is closed and every required
result is decoded. These profiles do not establish independent GPU ordering,
rolling admission or thousands of published dispatches. The later ordered
1,024-operation arena is recorded below; the production-runner controller remains
separately staged and unqualified.

Snapshot 74 passed 2,545 runtime tests (34 ignored), 2,082 KFD tests (three
ignored), and 378 host tests (three ignored). Its strict lint failures remain in
the raw archive. Snapshot 75 passed 2,086 KFD tests but failed one new test whose
zero-record account was invalid before the intended admission check. The test
now uses a valid account and requires the precise resource-phase refusal. The
copy fixture's plain Cargo invocation also failed its intentional typed-driver
guard; the new qualification invokes it through `cargo-fe2o3`, without weakening
the guard. Snapshot 77 then exposed a missing host dependency in that fixture
and hard-coded child-test paths when the listener module is included by the
compiler client. The host dependency and target-specific lockfile are corrected;
the test child now derives its exact selector from the actual module path and
still requires its unique completion marker. Snapshot 77's full suites passed:
2,552 runtime tests (34 ignored), 2,087 KFD tests (three ignored), 379 host tests
(three ignored), and all 91 runtime and 82 host doctests. Its four unsuccessful
stages remain recorded, including the new inline-enum lint findings and existing
host `drop_non_drop` findings. No failed batch is promoted to a pass.

Snapshot 78's fresh-target frontend replay passed all six stages: dependency
lock generation, all-target frontend checks, two typed-copy tests invoked through
the actual driver, two cross-included listener tests, eleven direct listener
tests, and 29 authority-release tests. Archive SHA-256:
`e3cb9c1cd2abd9a45f26f29d6eb51d4a248892d42fe2168b5ae56e29c746de47`.
The bounded inline roster keeps its existing allocation-free ownership contract;
snapshot 79 adds narrow lint annotations and explicit size/Copy regression
checks. Its nine-stage batch passed strict runtime/KFD Clippy, all-target checks,
all 2,554 runtime tests (34 ignored), 91 doctests, and the retirement/pipeline
source controls after explicit runtime package cleanup. Both new size tests ran.
Archive SHA-256:
`7da4104ca1ade5ed91ce955b83c1c8eacf9f5d4ea26e5ba907236b0a8ed428fc`.
The source/mode inventory remained identical at closing. This does not change
the separately recorded older host lint failures into successes.
The refreshed source-binding chain passed 20 affected source/control entrypoints;
this is not a solver replay or a native launch result.

The production proof sequence now requires a ninth family: shared graph
reservation retirement. Its scope is the post-access reservation-retirement
body, assuming the original graph-access check and native-disposal premise. It
does not prove whole Context execution or GPU settlement. The maintained
qualifier requires three whole-root positive proofs and 23 calibrated logical
negatives, with original source/signature/tool checks. Development results and
source controls are retained. The signed `9113dc812` replay below subsequently
passed this ninth family. The earlier eight-family replay on `727f32888` does
not qualify these edits, and neither replay qualifies later source changes.

## Signed Nine-Family Replay And Ordered Arena

`candidate-nine79-01-evidence.tar.gz` contains the completed replay of signed
commit `9113dc8124f9e0c55c1d5a8af9b31ce917a804e6` (142,516,895 bytes,
SHA-256 `6447a6a7ff8995eb9b946ef47b84d24080fee2507d791668272ae170f36b417e`).
The generic gate passed 64 direct positives, 694 fixed negatives, and all seven
executable mutation families. Live validation passed six positives and 108
negatives; planner composition passed two positives and eleven negatives.
The seven production component families each passed three positive brackets;
their negative counts were 34/22/12/12/22/11/25 in prepare/preflight/bind/roster/
producer-preflight/routing/credit order. The graph-version ledger passed three
13-obligation positives and eighteen mutations. Reservation retirement passed
three 13-obligation positives and 23 mutations.

Independent archive readback reopened 7,313 raw files and verified all 15,423
signed source files, the commit signature, and the reachable Git objects. Root
closure recorded 517 absent original process groups and 2,275 process
occurrences. The initial raw audit's incidental-tool-directory refusal and its
narrowly scoped successor are retained; incidental measurements are not solver
diagnostics. Archive readback is not a second solver run. These results remain
component-scoped, with the existing identity, standard-library, native-disposal
and OS/hardware assumptions; they do not prove complete runtime execution.

The ordered Arena source now connects one common native CODE/kernarg/DATA owner
to 1,024 distinct, single-use scoped member receipts and copied-result observers.
The common DATA debit and original source loans remain retained until actual
common destruction and Context settlement. A copied member result is not scalar
completion or a release of its native partition. The pre-root metadata aggregate
also disposes result cells before releasing their shared payload debit on
admission refusal. This profile retains `WaitForPrior`; independent-order and
rolling-admission successors are not qualified by these results.

`arena86-88-cpu-evidence.tar.gz` (4,784,442 bytes, SHA-256
`9e6c5d65b9dbc810cfa47813ae9e532e92e3e77889681306c1a1ffd83539394a`)
retains three failed development stages and all fourteen passing snapshot-88
stages. Snapshot 88 passed fresh all-target checks, maintained strict runtime/KFD
Clippy, 2,579 runtime tests (34 ignored), 2,101 KFD tests (three ignored), 379 host
tests (three ignored), and all three packages' doctests. The focused checks
include both pre-root ownership controls and the detached-owner controls. All
seventeen original stage groups are reaped and absent, and source inventories
match at closing. Its source inventory SHA-256 is
`3e0345ad76631967ede75ed7b861bb8682c4ff2d54e557ea27fe3a2af1c31a8a`.
The independent local archive readback is retained separately as
`arena88-local-readback.tar.gz` (SHA-256
`40833175c6c7d7318fa6f86146ee24890c528c9a1cae41712f44f1c2d4375694`).

The detached-owner component first passed snapshot 82's full runtime suite and
docs, retaining the actual returned lower DATA owner before validation or
disposal. It does not admit a successor generated consumer. The lower cold-device
component separately passed snapshot 83's 2,094 KFD tests and fifteen focused
controls: only an original no-VM device with a definite reset observation can
produce its opaque reset owner. Ambiguous failure retains the refusal owner.
The runtime recovery join is a later change that requires its own qualification,
not a result of the lower-only tests. Diagnostic copy classification does not grant
machine acceptance. These newer components require an integrated replay; the
signed nine-family source does not qualify them by inheritance.

## Independent Arena And Retained Producer Checkpoint

Snapshot 95 adds an independent-publication Arena profile alongside the ordered
profile. Its fifteen-stage CPU qualification passed, including 2,582 runtime
tests and 2,103 KFD tests. The retained failed development snapshots remain
failures in `independent92-95-cpu-evidence.tar.gz` (SHA-256
`8e7ac79d71c68778ab032d80ae990c7310c6bc11e8b5cb0873c3ad810a34dbe0`).
This is still a finite, single-use 1,024-member Arena, not rolling admission or
evidence of 1,024 simultaneous GPU dispatches.

Snapshot 98 connects that profile to an actual compiler-bound application caller.
Its observations describe original publication, Pending and completion receipts;
an earlier published member must actually remain Pending after a later member's
observed completion to produce an out-of-order witness. No witness remains an
unqualified result. Observation counters do not measure kernel duration or
physical concurrency and cannot release native ownership. The twelve-stage CPU
batch passed the genuine `cargo-fe2o3 check --bins` fixture binding, 28 Arena
controls, fourteen caller controls, ninety scope tests, 2,591 runtime tests,
379 host tests and all runtime/host doctests. All twelve original process groups
closed, and the final initial-root census found no target references.
`independent98-cpu-evidence.tar.gz` is 1,833,380 bytes, SHA-256
`3f37f9b5078561b44103610a5b711c25425649b5b9be7d618948f4831641eee3`.
Its independent archive readback is `source98-local-readback.tar.gz`, SHA-256
`b2fa59f35ee615aa9f8a1369aa28895643fbfa4e0b77bd58cbf81cb9a31482eb`.

Snapshot 100 retains the actual completed producer DATA and its lineage across
two scoped completion steps. The first step validates and detaches within the
original source bracket; the second rechecks currentness before actual disposal,
credit release and decoding. The existing direct completion path is unchanged.
All eleven CPU stages passed, including five retained-producer controls, eighteen
detached-owner controls, 2,592 runtime tests and runtime doctests.
`cpu100-retained-producer.tar.gz` is 941,616 bytes, SHA-256
`38d2f3b034ffb667fff84e63e45d4a08dfdd753e76d58dc171853c451b72a689`.
This is not yet producer DATA to successor kernel/copy input binding.

Snapshot 96 connects the lower definite cold-device reset owner to Context and
scoped failure settlement. Only a never-activated device with the original
no-VM/reset evidence can take this path. Source and hold disposal precede its
device-local failure result, allowing an unrelated branch to continue. Warm,
published or ambiguous failures still retain ownership and fail closed. The
inline cold/ready owner union preserves the existing size bound. Sixteen CPU
stages passed, including 2,569 runtime tests, 2,094 KFD tests and both packages'
doctests. `native-snapshot96-cold-runtime-cpu-evidence.tar.gz` is 1,171,363 bytes,
SHA-256 `d62678132dad0b00cd31594fc13488cc8f456f72e0435c5d736e0324fb38f441`.

Snapshot 102 independently replayed all 23 CPU stages against signed combined
commit `2eaf58d5774b5113577cb90d94bc0a86ebc2bde8`: 2,610 runtime tests, 2,110
KFD tests, 379 host tests, all three packages' doctests, all-target checks and
strict runtime/KFD Clippy passed. Its SSH connection failed after the original
supervisor had completed; the raw results, source/tool inventories and original
process-group closure were recovered and independently audited, not inferred
from that transport exit. `cpu102-combined.tar.gz` is 1,362,698 bytes, SHA-256
`39aa7b3c65642f006b2177fec881ba6780b5dee7a1fa6ee3e1ca7aab6d9017ca`.

Snapshot 102 does not cover the subsequent caller-98 integration. That merged
source and its signed-source proof replay remain separate gates. None of
these results is protected GPU execution, a reset-isolation theorem, a passing
production runner job, or completion of A1/A2/A3.

## Open Exit Gates

- A1 still needs genuine protected mixed-duration/high-depth GPU qualification
  of the final async path, not only CPU ownership and executor tests. The native
  registry fixtures include sixteen fill launches and the ordered Arena's CPU
  controls and independent caller cover 1,024 slots; neither measures thousands of GPU operations,
  wakeup overhead or aggregate queue/signal/kernarg retention on hardware.
  The owner-array ceiling is not a whole-process memory bound. The generated
  registry and Arena profiles have finite single-use slots. Thousands of
  host-retained requests do not establish thousands of GPU-published dispatches;
  variable fixture extents also do not establish measured mixed-duration overlap.
- A2's shared borrowed-carrier DAG is implemented, but its generated arguments
  are frozen before admission and excluded from the ordinary allocation-version
  ledger. Producer-output to successor generated-input binding remains missing.
  General repeated multistage dataflow, measured compute/copy overlap and
  complete executable DAG/settlement refinement are not established by the
  scoped producer/live-validation and planner-input proof campaigns.
- A3 now has checked versioned replica/group bookkeeping and a qualified CPU
  component for definite cold pre-activation device-local failure. It still
  needs complete native cross-device DAG integration and hardware-qualified
  partial-run reporting. Warm, published and ambiguous terminal failures retain
  global fail-stop; the cold path does not certify general reset independence. Protected
  all-admitted-GPU execution with measured overlap remains unqualified. Ordered
  compute-then-serial-copy roster checks do not qualify overlap or unselected GPUs.
- Both MI300X aliases reject noninteractive sudo and the ordinary configured root
  SSH identity. The read-only audits are retained under `infrastructure/`.
  Root access on the gfx950 MI350 host does not admit it to the closed gfx942
  production proof profile.
- Reviewed runtime-proof runner capacity, genuine native service qualification,
  final source/proof inventory replay, and both-main landing remain separate
  requirements. No open milestone is closed by this evidence directory.
- The compiler-to-application service transition now has an implemented owned
  phase checkpoint. It retires the original compiler issuer/anchor before
  application admission while retaining the original cleanup account and armed
  creator. A failed compiler permanently drains that account; a successful
  compiler publishes completion only after application readiness/currentness.
  Source and CPU controls do not qualify the genuine protected service exchange.
  The packaged nonroot refusal and private-root deployment contracts are also
  distinct from a successful protected compiler/application/GPU execution.

These functional and executable-refinement gaps remain independently of host
access or CI availability. A passing final-source replay does not expand its
theorem boundary or turn a source-integrated workflow into a successful CI run.
