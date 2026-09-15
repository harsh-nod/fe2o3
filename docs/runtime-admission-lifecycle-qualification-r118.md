# R118B Admission And Lifecycle Qualification

Status: R118B locally accepted above R117
`a07ec44309e214f2a8ef0e687e610c8e60a36224`, with
[retained evidence](evidence/local-r118b-admission-lifecycle-2026-09-15/README.md)
and two passing independent archive reviews.
The original compiled campaign stopped after 72 qualified negatives and one
rejected attempt; that stopped R118 history remains unaccepted. The corrected
R118B source passes a fresh complete campaign, not a continuation that reuses
the earlier negative results.
C1, C2 and C3 retain separate contracts and behavioral oracles within one
integrated source campaign. No production API, semantics, feature or dependency
change is introduced.

## Accepted Checkpoint

GNU and musl each pass 2,780 tests with five ignored across 48 libtest harnesses
and the separately accounted CSV target. All seventeen source gates, ten
auxiliary checks and six frozen/restored suites (9/4/5/1/15/14) pass. The 78
compiled negative executions cover 74 distinct source maps: 75 production
executions, one combined-defense execution and two helper-calibration executions.

Every negative reaches its exact named behavioral oracle and restores all 5,692
source identities. The closed collector and both independent reviews verify
[1,432 raw artifacts](evidence/local-r118b-admission-lifecycle-2026-09-15/review.md).
This accepts local host/test coverage only, not native ISSUE/COMPLETE, production
journal integration, Worker/compiler authority, formal correspondence, total
memory bounds or HIP/HSA performance parity.

## R118B Correction

Only `async_engine/tests/owned_tests/preparation_tests/completion_tests.rs`
changes above the original reviewed map. Observation vectors and booleans are
captured under short locks; assertions run after releasing those locks. The
trace lookup similarly returns an owned optional thread identity before
unwrapping. Expected values, assertion order, callbacks and payload destructors
remain unchanged. Independent source and oracle reviews pass.

The corrected source map is
`3aa1ab32e9e6e6ffd0474ec9055d3c29b02bf53e45129406617e50f0b3c15126`
with the same 5,692 identities and eighteen added test names. The corrected
test file hashes to
`e729e1c0cfa7458fb8504f9bc1d18519ce59a5abe00be94a7c5aba22762fe7f9`.
Formatting, all five C3 tests, all 733 runtime-library tests and strict
all-feature/all-target Clippy pass with unchanged source endpoints and closed
owned process groups. These are preliminary results, not full acceptance.

A single preliminary repetition of `c3-continue-retirement-after-failure`
now reaches the unchanged owner-prefix assertion at line 338 and produces the
normal named FAILED row and one-test summary, without SIGABRT or destructor
double-panic. Its frozen inputs retain 760 pins. Exact restoration recovers all
5,692 corrected source identities, then the five restored C3 tests pass again.
The original aborted transcript still fails the old oracle parser and is not
retroactively accepted. The targeted run is not the new 78-case campaign.

- Targeted run: `r118b-retirement-regression.json`, SHA-256
  `14205e2e096a37e43e62b9c5f1c96bd19b30cd210023efc623f810ae006b3ff8`.
- Restoration: `r118b-retirement-regression-restoration.json`, SHA-256
  `45dd10efd0a9a87ddaa69ed9498a510d820898c79f1ad90a1462f57e34a77c91`.
- Restored C3: `r118b-restored-completion.json`, SHA-256
  `dc56441a3f27e5296c1520d469477c5ea1d2f17abf804d309c87be85855f58b1`.

The new namespace preserves all 735 artifacts from the stopped history and an
explicit byte-for-byte prior test-file snapshot. The original test-file hash is
`477292b9e97382a4087af9cb259e0ff08241cf9882095bb82a8df9fba16a5f85`.
The origin-v2 record hashes to
`197028ede4e23ac3dfad31c65f2e2f2c621c80e12d65c6aec51953a4a93da094`.
Formatting is an independent initial record and precedes origin capture.
The first origin helper incorrectly counted 652 prefix-matching artifacts
against the expected 735 and stopped before writing outputs; its unchanged
helper is retained, but no raw execution record was captured. Origin-v2 adds
the 83 original C1/C2/C3 names pinned by the old manifest. It does not fabricate
an execution record or restoration predecessor for either earlier failure.

All nineteen C3 oracle locations are remapped explicitly; only the old-waker
assertion's printed expression changes. Mutation patches, names, categories and
test identities stay unchanged. The corrected mutation core passes 48 contracts,
including derivation of all 78 executions and 74 distinct source maps. Fresh
GNU and musl each pass 2,780 tests with five ignored across the exact 48 libtest harnesses
and one retained harnessless CSV target. Independent roster and source-endpoint
reviews pass.

The stopped-history validator passes 45 contracts and a separate closed
validation run. Its 735 historical artifacts plus ten explicit origin inputs
retain the 72 qualified pairs and rejected attempt 73, without fabricating that
attempt's qualified restoration. Direct early-mutation and final-restoration
corruption cases ensure pair validation cannot be skipped. It preserves the
snapshot supplement's actual predecessor branch and checks captured input bytes
again at the end. Legacy validation calls still read original files directly;
this establishes endpoint consistency, not continuous source immutability or an
entirely snapshot-only validation path.

The corrected-history validator passes all 27 contracts and a separate closed
validation run. It requires fresh GNU/musl executable rosters, the corrected
regression's ordinary named failure and exact restoration, and the original
stopped history as a separate unaccepted result. It captures 801 input names
and rejects late byte or membership substitutions. All nine runner contracts
also pass, preserving five real fixture records and four scripted cases;
independent review confirms the raw outcomes and process-group cleanup.

The new injected-I/O freeze boundary passes 57 contracts, including a real
history/support validation with captured writes and explicit source, artifact,
clock, toolchain, output-occupancy and fixture-record rejection cases. Both
the source snapshot and the directory listing are captured before validation;
closing observations reject byte and membership drift, including old-prefix
Markdown inputs. All 22 mutation-lifecycle contracts pass on separately pinned
helpers. Independent record reviews pass. The direct freeze then captures all
5,692 source identities and 862 validation inputs, with unchanged toolchain
binaries and the stopped R118 campaign still explicitly unaccepted.

All fifteen remaining source gates and ten auxiliary checks pass on the corrected
source. Together with the two fresh full runs this accounts for seventeen source
gates. The six frozen suites pass 9/4/5/1/15/14 tests, with exact GNU-derived
rosters, source endpoints, predecessor records and closed owned process groups.

The collector requires all 78 negative executions, all restorations and a
matching closed collector transcript. Its main contract suite now includes the
prior supplemental final-snapshot assertion; all 167 cases pass, with exact
helper pins and independent record review. The guarded preparation then pins
935 historical artifacts, 23 helpers and the launcher into 163 planned entries.
The guarded launcher completes all 78 compiled negatives, all restorations,
six restored suites and the closed collector. Both independent reviews pass
for the complete archived campaign. None of the 78 executions is counted as
passing from the old campaign or targeted regression. The final full source map
is restored; all checked owned process groups are closed.

- Corrected mutation core: `r118b-mutation-core-contracts-v1.json`, SHA-256
  `9dfcb102e8be51828422934d51f21f107aa797b264c04e89b4f9a7c8f340b76a`.
- Fresh GNU: `r118b-reviewed-gnu-all.json`, SHA-256
  `e1325bb38c5b33aa10f37bd68040abadd02a1e3d8b153c9cb3074c26a4361d17`.
- Stopped-history contracts: `r118b-prior-history-contracts-v1.json`, SHA-256
  `f4992655f5e13eb7ed5c6d267adacfcc139178acd2bdbf0b879cdde5058ee0ad`.
- Stopped-history validation: `r118b-prior-history-validation-v1.json`, SHA-256
  `c584370ad6609bff10fd9f03f3fc5c6e9dfda3062a29a972682886fac8698bc1`.
- Fresh musl: `r118b-reviewed-musl-all.json`, SHA-256
  `3ed1a3bda6d6abedf7f33eb30ac2455c79ddc59ba4119dc6af0f8546b63d8c36`.
- Corrected-history contracts: `r118b-current-history-contracts-v1.json`, SHA-256
  `b62e6a92784c7ca891080560af5a89bcf6bd8f3bb1a97a7968de4572d46ce5a8`.
- Corrected-history validation: `r118b-current-history-validation-v1.json`, SHA-256
  `10360b0b0744caaaae45842ab3db201128d62050fef64d8aa6a58729e5b28eb7`.
- Runner contracts: `r118b-runner-contracts-v1.json`, SHA-256
  `6e7c750f2f9fc1447189abe680ac02183718111a1a7fa9546b22344556495fa9`.
- Freeze contracts: `r118b-freeze-contracts-v1.json`, SHA-256
  `ab8a1a2237ade39b18940fbae9080a33525fc987707a902a369f649546e5358e`.
- Lifecycle contracts: `r118b-qualification-lifecycle-contracts-v1.json`, SHA-256
  `aa208ebb2ef1adae77b962797004b37227766674214cc5e4a8dfb80978e89c36`.
- Frozen environment: `r118b-environment.json`, SHA-256
  `7b591b09ac92fa0bfc7ac539aef8e9b4f0abfcfe7292a6dc50454a67a48c26fd`.
- Remaining source gates: `r118b-source-campaign.json`, SHA-256
  `711d440699bd7d8b5b061df865ba9e417fb048024ac8b378e5cf551499589606`.
- Auxiliary checks: `r118b-auxiliary-campaign.json`, SHA-256
  `fda9b216b5a1754b923a35e32ecead7e8b69f40817baca76ced2744d13d02d7a`.
- Qualification contracts: `r118b-qualification-contract-tests.json`, SHA-256
  `4adb009db085b656240e4ad4eac5f0771205b065385b09e78a9125c9f2660f32`.
- Campaign manifest: `r118b-qualification-manifest.json`, SHA-256
  `09001546014f174774a90fe8fb2ab57de91a59c33abfc341ac6cccd07a23186d`.

These records form core -> GNU -> stopped-history contracts -> stopped-history
validation -> musl -> corrected-history contracts -> corrected-history validation
-> runner contracts -> freeze contracts -> lifecycle contracts, followed by the
direct freeze observation. All completed
records preserve the corrected source map and closed owned process groups.
Source and auxiliary campaigns precede the six focused suites, then the 167
qualification contracts and manifest observation. Manifest preparation is not
compiled-mutation or archive acceptance.
The complete mutation campaign, restored suites, closed collector and archive
reviews subsequently pass. The original full-test and mutation results below
belong to the old source cohort, not prerequisites for accepting the corrected
source.

## Source And Checks

Eight modified files and three new test modules add eighteen behavioral tests:

| Contract | New Tests | Test Module |
| --- | --- | --- |
| C1 submission identity | 9 | `context/tests/submission_identity_tests.rs` |
| C2 generated descriptor identity | 4 | `authorized_execution/tests/generated_identity.rs` |
| C3 reply and retained-owner lifecycle | 5 | `async_engine/tests/owned_tests/preparation_tests/completion_tests.rs` |

Paths are relative to `crates/fe2o3-runtime/src`. Before promotion, every
modified file's isolated-parent blob matched committed R117. All eleven files
initially matched their isolated candidates byte-for-byte. On that initial map,
`526da8f7036ffed98099eb2ff10c07b3f516e094dd7184c57bb75f07b0ba6929`,
formatting, all 733 runtime-library tests and strict Clippy passed. Independent
review verified the exact retained 715 tests plus eighteen additions, source
endpoints, runner pins and process/clock closure.

Review then added fixed test-only observations for actual Wait, Drain, Cancel
and Event arguments in the C1 mock and snapshots. Valid controls now check the
exact backend target, both Event coordinates, and the explicit Drain entrypoint.
These additions close wrong-neighbor routing blind spots without adding new
test functions or production state. Poll already checks per-submission counter
changes; callback placement and released-owner/backend-ID removal have separate
oracles. The original candidate sources remain untouched.

The reviewed source map is
`df2e27085ee920fdd07d2a6955503b1ecdcadc2d7c10c6fd4cec6768cecdadbb`
across 5,692 identities. Formatting, all 733 runtime-library tests and strict
all-feature/all-target Clippy pass on this map. All three children are closed,
their owned process groups are empty and source endpoints match. These remain
preliminary checks. Initial and intermediate three-observation results retain
their separate source cohorts, not reviewed-source prerequisite status.

The reviewed full GNU and musl dependency-closure runs each pass 2,780 tests
with five ignored across 48 libtest harnesses and one unchanged harnessless
CSV benchmark case roster.
Comparison with committed R117 verifies the exact eighteen runtime additions,
715 to 733, with every other target's passing and ignored rosters unchanged.
Both children and owned process groups are closed, all 5,692 source identities
match before/after and the predecessor/clock checks pass. Independent reviews
confirm both prerequisites. Retained record SHA-256 values are:

- `r118-reviewed-gnu-all.json`:
`bed9f11ea709df10eef07c640679405ccff9ac3261ee0e9698436704b55c1d79`.
- `r118-reviewed-musl-all.json`:
`91ff92dbc70b970a6438789c7faa076149794cb133543751b93595226149040c`.

These are completed full-test prerequisites, not full R118 acceptance.

## Original History

All nineteen original candidate runs are retained with their original HEADs,
worktrees, runner hashes, logs, endpoint maps and clocks:

- C1: eight runs above R112, including initial E0308 compilation failure and
  later 9 focused/724 runtime passes, Clippy and formatting. Its old-boot map
  contains 5,299 materialized SHA objects and 381 explicitly unmaterialized Git
  index descriptors. Those descriptors are not materialized source hashes and
  this chain must not be joined to a different boot.
- C2: five runs above R115; 4 focused/719 runtime passes, Clippy and formatting.
  The first formatting run changed source.
- C3: six runs above R116; 5 focused/720 runtime passes, Clippy and formatting.
  Both initial formatting runs changed source, with intervening manual review
  fixes. The `runtime-all` record ran runtime `--lib`, not dependency closure.

Original isolated history is not an integrated prerequisite. Preserve every
failure and formatter change. The integrated runner has its own R118 clock
contract and source chain.

## Historical R118 Qualification Prerequisites

This section records the stopped original R118 cohort, not current R118B status.

All seventeen source gates now pass: the two full-test prerequisites above and
the subsequent fifteen-gate campaign. All ten auxiliary checks and the six
frozen focused suites pass, with exact rosters of 9/4/5/1/15/14. The retained
one-test reply suite completes with distinct results and establishes
first-result-wins. Source endpoints remain the reviewed 5,692-identity map.

Campaign-launch and collector contracts now pass 166 checks, plus one
supplemental final snapshot-comparison check. Compiled negatives with exact
restoration, restored focused suites, the closed collector and independent
archive reviews remain required.

Freeze separate C1/C2/C3 mutation targets and assertion markers. C2's nineteen
prospective negatives include a paired artifact-length weakening where digest
validation masks a simple deletion. C3 distinguishes production mutations from
helper calibration; individual terminal/Retiring guard deletions can mask each
other. C1 must similarly distinguish full-ID lookup defenses from a redundant
explicit context-generation check. Compile failures, timeouts, zero selected
tests and unrelated assertions are not qualifying behavioral negatives.

## Historical R118 Mutation Preparation

The reviewed definitions specify 78 executions and 74 distinct complete source
maps: 75 production executions, one combined-defense execution changing two
files, and two separately labeled helper calibrations. Four C1 source mutations
are each checked at two different behavioral oracles. In-memory derivation
checks exact method scopes, unique anchors, every declared assertion location,
all eighteen selected test identities and the complete repeated-source groups.
The real Rust mutation campaign stopped after 72 qualified executions. Its
73rd attempt is rejected, not a qualifying negative; five executions remain
unrun. The complete campaign is not accepted.

The new multi-file helper passes 48 recorded contract tests, including partial
application writes, unknown external edits, missing process closure, malformed
scope rejection and recovery after partial restoration. Physical restoration
is distinct from qualification: an I/O error after writing all original bytes
still prevents acceptance. The integration runner must preserve application
attempt history, authenticate terminal records and require exact full-map
restoration before advancing its evidence chain.

Seven executed helper inputs are pinned by
`r118-mutation-core-inputs-v1.json`, SHA-256
`9afd082517e9ffd1669d78d61ab660fdf320554fb1b3b39377d53f0ab60af600`.
The closed, independently reviewed `r118-mutation-core-contracts-v1.json`
record follows musl and hashes to
`6af8e092050a6f4ba31a0f05d8bbc2ba3728ca6bc412653f77d649cfd56d725e`.
Its source endpoints match the same reviewed map. These helper results do not
qualify campaign launch or any Rust negative.

Nine recorded runner contracts also pass. Independent review found two gaps
in the draft history validator: coercible sparse values and incomplete required
artifact membership. Both are corrected, with thirteen schema/membership
regression tests passing in `r118-history-schema-contracts-v2.json` on the
unchanged reviewed source map. This bounded suite uses injected dependencies;
it does not execute full historical-evidence validation or the campaign.
Its original parse failure, original helper inputs and corrected version are
preserved separately. The full history preflight subsequently passes all
thirty original records, twelve source maps and eight transitions. The real
freeze validates that history and the later runner/schema/preflight chain,
retaining 213 validation-input pins. Original-boot C1 evidence remains
independent rather than becoming a current-boot causal prerequisite.

The corrected freeze harness passes fifty contracts, including late source and
artifact substitutions, contradictory positive-test logs, exact sparse types
and the independent old-boot history. Its first harness failure is preserved;
the correction fixes a shadowed test callback, not runtime code. Independent
review verifies both attempts and the passing run.

The multi-file mutation runner passes twenty-two integration tests using
in-memory source writes and scripted child outcomes. These cover partial
application, missing or foreign terminal records, manifest-bound runner
identity, clock failure during recovery and diagnostic-write failure. They
exercise `qualifyMutation`, not actual child spawning or the main launch loop.
The separately pinned launch preflight and collector suite now passes 166
contracts. It covers zero-effect admission rejection, pre-spawn guards,
validated output retention, exact two-file restoration, full scripted cohort
collection, and captured archive bytes with readback. The closed record
`r118-qualification-contract-tests.json` hashes to
`7982b236fd01913006dd5c4b9f9a233b58d8c986389afd748bb813189acd1514`.
Its 19 helper inputs are retained in `r118-qualification-contract-inputs-v1.json`.

One collector corruption test is rejected by a predecessor hash before reaching
the final captured-byte check. A separate, pinned supplemental test changes
only source-map whitespace, preserving semantic equality, and verifies rejection
at that final check. Its closed record
`r118-qualification-snapshot-contract-tests-v1.json` hashes to
`0518a8790bbaaf11222ceea6edb7b98384478fc8777f53b04e78fe05f9fea792`.
Both suites retain the same reviewed source endpoints and empty owned process
groups. Collection still uses endpoint comparisons, not continuous-immutability
or snapshot-only semantic validation.

The manifest freezes 163 entries: 78 mutation runs, 78 restoration records,
six restored focused suites and one closed-collector run. The launcher checks
qualified input pins, current source and vacant outputs before each mutation
and immediately before spawning, then pins validated outputs for later launches.

The closed source and auxiliary campaign records are respectively
`760d65e124d02ba1335fc53a839e121d7a1c6a25a7575ade73bf26224738f33d`
and `3467ef2fe3028c9f741bd8258293c959007e5dd86ffa3ec67fd27cfe73101f15`.
The final frozen reservation record is
`e94d685694dfca77f4ba529cab8c31b75eae13aec5d23b905d3f3985f588be54`.

## Stopped Compiled Campaign

The manifest SHA-256 is
`62e084d3ca50068e1a350bb414edd882072dad37c6b48da4080668bd4ff4898a`.
The first 72 executions reach their named behavioral oracles and restore the
complete reviewed source map. Attempt 73,
`c3-continue-retirement-after-failure`, compiles and reaches the intended
retirement-attempt assertion in `completion_tests.rs:317`: the mutant visits
the third owner instead of stopping after the second owner's failure.

That assertion holds the mock-state mutex while panicking. During unwinding,
retained test payload destruction calls `lock().unwrap()` on the now-poisoned
mutex in `adoption_tests.rs:18`, causing a second panic and test-binary SIGABRT.
Cargo exits 101, but no named libtest FAILED row or final summary is produced.
The qualifier correctly rejects this result rather than accepting an abort.
The terminal record SHA-256 is
`1d0df4e943002665f1c2c6e30f72b54e1318303cfc751380341017037c2b6d69`.
The original log, endpoint maps and qualification-failure diagnostic remain
unchanged in the retained R118 working evidence.
The diagnostic SHA-256 is
`187cfe5dbf0692cc004045684527dcf175a841859d8896596ca76e91067a84db`.

The failed attempt's owned process group is empty, the launcher has exited,
and independent current-source comparison confirms all 5,692 identities match
the original reviewed map. Physical restoration does not qualify the negative.
No restored focused suites, closed collector or accepted archive were produced.

The R118B correction above snapshots the test's attempt/disposal observations
and issue/flush flags under the mutex, releases the guard, then performs the
unchanged assertions.
Independent review also identifies mock-state assertions at lines 122, 211,
275, 395 and 431-433, the trace lookup around line 386, and waker-observation
assertions as requiring the same guard-lifetime review. Use separate snapshot
statements: an inline clone inside an assertion can still retain a temporary
guard through panic. Keep the fixture destructor and behavioral oracle intact.
Preserve the failed
cohort and frozen helpers; any corrected source and qualification inputs need
new versioned evidence. Do not resume or rewrite the old immutable campaign,
weaken the expected owner prefix, or treat its old full-test prerequisites as
qualification of changed source.

## Limits

C1 observes aggregate credit usage, not private credit-account identity or
storage. C2 observes exposed descriptor metadata, not private packet integrity
or native authority. C3 uses actual host lifecycle drivers with scripted
adoption/retirement callbacks, not GPU completion. Existing distinct-result CO1
tests, not repeated identical Stop outcomes, establish first-result-wins.

This candidate does not implement native DATA-ADOPT, generated ISSUE/COMPLETE,
Worker/compiler authority, production journal integration, formal refinement,
aggregate memory bounds or HIP/HSA performance parity. Those remain explicit
downstream work in the [current roadmap](runtime-swarm-next-packets.md).
