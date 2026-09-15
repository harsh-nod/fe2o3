# Live Retained Persistent-Control Release

## Status

R123 is locally accepted above published R122
(`d1c87064bd10e513f5aad328e0b3445bd78c296a`), with
[336 retained raw artifacts](evidence/local-r123-live-retained-control-release-2026-09-15/README.md)
and two passing independent archive reviews. This accepts the retained-control
integration at the CPU/test boundary; it does not advance A1/A2 or issue #182 to
completion or establish native, formal, total-memory or performance parity.

## Ownership Boundary

`queue_live/retained_control_release.rs` composes the existing detached-control
cleanup root with the live model-loan driver. It does not create another native
cleanup implementation or extract returned data.

The installed dispatch remains available through preflight and full currentness
validation. An outer slot owns it before opening the model loan. The callback
moves it into `ReturningControlCleanupCustodyV1`, also owned outside the loan.
Cleanup completion is necessary but insufficient for success: the closing model
retake must succeed before the root can be dropped.

The sequencer captures lower cleanup panics outside the model driver. This keeps
the original lower payload available even if retake or the driver's poison hook
panics. It does not claim to recover an original retake panic replaced inside the
existing driver by a subsequent poison-hook panic.

| Outcome | Required Result And Custody |
| --- | --- |
| Existing poison, unpublished work or persistent attachment | Preserve preflight error; no new parent transfer |
| Absent or non-detached dispatch | `Ok(false)` without cleanup |
| Detached-generation or currentness failure | Retain original controls with terminal parent |
| Opening error before callback | Restore exact original dispatch; no cleanup or retake |
| Successful envelope without callback | Contract error; restore and retain original owner |
| Lower cleanup error | Retain cleanup root; closing error takes precedence |
| Lower cleanup panic | Retain cleanup root; preserve original lower panic |
| Completed cleanup followed by failed retake | Retain completed root; never reconstruct disposed controls |
| Successful cleanup and retake | `Ok(true)`; no installed control owner remains |

Direct calls transfer the terminal parent before propagating their settled result.
Lane facades record sticky terminal transport before propagation; their enclosing
custody operation must restore the selected lane before transferring the parent.
Control release never commits or changes the detached-data ledger.

## Validation

The constructed cohort uses genuine prepared single- and three-binding dispatch
owners, actual logical reserve/publish/complete/recycle/detach transitions, and
the existing primary/auxiliary model-loan fixture. Detached typed data and the
displaced ordinary dispatch remain separately rooted. Completion occurrences and
native backend calls are test fixtures, not hardware observations. The later-slot
case relocates a constructed auxiliary lane; it is not another independent engine.

Acceptance covers exact restoration on opening rejection, destructive
prefix and untouched-suffix checks, post-retake native/model equality, retained
completion receipts, error and panic precedence, retry rejection, and concrete
public facade restoration. Development failures remain distinct from subsequent
passing runs. GNU and musl full regressions, all 25 source/auxiliary gates and
ten compiled mutations and both restored regression cohorts have passed their
declared checks. The complete CPU/test evidence is archived and independently
reviewed.

## Development History

The first compile rejected two test assertions that referred to a nonexistent
observation field; they now check the actual cleanup stage. The second cohort
passed 17 tests and failed one test expectation: the fake currentness backend
increments its dedicated counter without appending an operation-log entry.

The third cohort passed 24 tests and failed two newly strengthened snapshot
checks. Both failures came from deliberate certificate-revision regression:
the test setter recomputes the revision seal as well as changing the revision.
The raw snapshots differ only in that derived seal after revision normalization.
The corrected oracle accepts this change only for the exact requested regressed
revision and only after the existing active-foundation authenticator has checked
the seal. Certificate identity, generation, model, native records, mapped bytes,
charges, counters and unrelated storage still compare exactly. A separate small
calibration checks identity corruption and seal corruption independently.

These failed development cohorts are retained under distinct source maps; none
is an accepted full regression. The expensive native-prefix matrix passed in
both executing cohorts, but those results do not qualify later source changes.

The all-feature fourth cohort, `r123-development-focused-04`, passed 27 tests
with no failures or ignored tests on source map
`696df28f9eca3272d63499c5834c5747fe1dc03b813c3472a16454ea26f6aa52`.
It explicitly excluded the expensive native-prefix matrix. The strengthened
public-route checks cover restored-slot ownership, attachment preservation,
both process-poison probes and the populated SDMA pool snapshot. Independent
read-only review confirmed those earlier oracle gaps are resolved.

Adding the explicit retake-count assertion and source-order guards changed the
source map. Strict Clippy then rejected two test-helper signatures for type
complexity in `r123-development-clippy-01`, on map
`f38846bf0064ce47667f705bb4c885d366a05f063f2ea1775720402b2c6ee351`.
A shared test-only `ControlReleasePrefixV1` alias fixes both signatures without
changing production behavior. `r123-development-clippy-02` passes with
`--all-features --all-targets -p fe2o3-kfd -p fe2o3-runtime -- -D warnings` on map
`8380746325d6717df78a608eb5be8f3ff3c48e42f32eec19aba2ab0677fdcb08`.
Both Clippy runs closed normally with unchanged endpoint source maps and no
remaining live members of their owned process groups. These are development
records, not an accepted full-source qualification or a native GPU run.

On that same source map, `r123-development-focused-05` passed all 128 selected
tests with no failures or ignored tests (1,081 filtered out). It includes the
complete 144-case native-failure fixture matrix, retained-control public routes,
the currentness/source-order guard, preparation, lower control release, and
pristine-abort cleanup with its typed-data children. Test execution took 323.59
seconds; the recorded command took 363.581 seconds including compilation. It
closed normally with unchanged endpoint source maps and no live members in its
owned process group.

`r123-development-r122-regression-01` then passed all 17 selected R122 tests
with no failures or ignored tests (1,192 filtered out): four concrete
data-release routes and thirteen constructed live-release cases. This exercises
the changed shared parent snapshot, poison hook and preparation fixture through
the existing data-release path. Test execution took 364.90 seconds; the recorded
command took 365.239 seconds. The same source map remained unchanged and the
owned process group was absent after normal closure. The two test rosters are
disjoint, giving 145 passing focused/regression tests, not a full workspace run.

`r123-development-gnu-all-v1` passed 2,865 tests with no failures and five
ignored tests across the five-package all-feature, all-target GNU suite. Its
48 libtest harnesses and one CSV benchmark match the committed R122 executable
roster, with exactly 19 added KFD tests and no runtime additions. KFD passed
1,209 tests and runtime passed 743. The recorded command took 1,492.781 seconds,
closed normally, preserved the same source map and left no live members in its
owned process group. `r123-development-gnu-check-v1` checked the exact passing
and ignored names, executable identities, all summary fields, source endpoints,
runner identity and closure records. This is not a hardware benchmark result.

`r123-development-musl-all-v1` also passed 2,865 tests with no failures and five
ignored tests across the corresponding musl suite. The 48 libtest harnesses,
one CSV benchmark, 1,209 KFD tests and 743 runtime tests match the exact declared
rosters. The recorded command took 2,077.730 seconds, closed normally, preserved
the same source map and left no live owned process-group members.
`r123-development-musl-gate-check-v2` checked both full-suite records and rosters.

The recorded parser calibrations then passed 32 full-record/roster cases, 30
source-gate transcript cases and 64 mutation-oracle cases. These 126 cases test
the evidence helpers with actual baseline inputs and synthetic counterexamples;
they do not execute the ten planned compiled runtime mutations. The mutation
checker requires the intended assertion, including both the primary caught
assertion and following unwrap for sticky transport. It rejects malformed or
out-of-body panic headers, compilation failures and subprocess aborts.

Read-only leaf-gate review subsequently found four additional byte-identical
doctest fences shifted by one line in `queue_dispatch_binding.rs`. The original
gate checker accounted only for three shifts in `queue_live.rs`. The successor
gate checker retains the original full plan and adds the four hash-pinned
relocations, covering all seven moved fences. Its calibration adds eight stale
or missing-row rejection cases, for 38 cases. This successor passed static
review and all 38 cases in `r123-development-gate-parser-calibration-02` with
normal closure and unchanged source. The earlier 30-case gate calibration
remains development history, not qualification of the successor. Current helper
calibration is 32 full-record, 38 gate and 64 mutation cases, totaling 134.

`r123-development-source-snapshot-01` captured 19 changed/new Rust files,
including three additions and no removals, against committed R122. The snapshot
contains the exact source bytes and prior hashes; it was created exclusively
and read back unchanged. Its SHA-256 is
`e670a525c7cc0bc94f73d4e7dcf592be21d3714a367d5963e6d64d62c5967879`.
The checker, calibration and snapshot commands all closed normally on the same
5,703-identity source map with no remaining owned process-group members. This
snapshot was not itself the accepted qualification archive.

All fifteen source gates and ten auxiliary gates in the frozen full plan then
passed, including the GNU/musl doctest and host rosters, seven macro fixtures,
both seven-package Clippy configurations, 151 Python tests, formatting,
dependency policy, eight dependency-policy tests, CI command contracts and
all 32 standalone lockfiles. The auxiliary checks cover registration, ordinary
construction, Linux helpers, initialization, transitions, preparation, bind,
proof inventory and production metadata/audit. Proof inventory is not a solver
run or formal implementation correspondence.

`r123-development-gate-check-all-v2` validated both full suites and all 25 gate
records against their exact commands, rosters and transcript policies. All runs
closed normally with the same source map and no live owned process-group
members. The final checker took 2.658 seconds. This completes the declared
source/auxiliary gates, not the compiled-negative or restored-source campaign.

All ten declared compiled mutations subsequently reached their intended
behavioral assertions with normal Cargo 101 exits: opening terminal status,
retake-error precedence, original lower panic, panic-poison reporting, final
poison precedence, original-owner restoration, direct parent transfer, sticky
facade transfer, generation validation and currentness validation. No mutation
qualified through a compilation failure, unrelated assertion or process abort.
The sticky-transfer case includes both the caught primary assertion at
`retained_control_release.rs:155:25` and its following outer unwrap at
`199:35`; the unwrap alone is not sufficient evidence.

Each mutation has an exact one-file source map, a closed behavioral checker and
a closed restoration record. All ten restore the full 5,703-identity baseline.
Independent read-only review passed both five-case groups, including all thirty
records, their clocks, predecessor identities and absent owned process groups.
This mutation review does not replace the restored regression suites or final
archive review.

`r123-development-final-restored-focused-v1` passed all 128 selected tests
(1,081 filtered) after the final mutation was restored. Its command took
325.790 seconds, including compilation. The subsequent
`r123-development-final-restored-r122-v1` passed all 17 selected tests
(1,192 filtered) in 327.108 seconds. Neither run failed or ignored a test.
Their exact rosters are disjoint, giving 145 restored tests on the unchanged
5,703-identity baseline; both commands and their owned process groups closed.

## Accepted Evidence

The closed collector validates 69 qualifying runs and retains six historical
runs separately. Its complete replay is byte-identical. The final archive
contains 336 hash-checked raw artifacts totaling 141,014,885 bytes, including
the collector's four closed outputs. All 76 recorded owned process groups are
absent. Two independent reviews bind prepared summary SHA-256
`7a1ecb1cb51a5d68d837895db8744aa5d0feffb7341d64e0ab9a51eefc46e955`.

The accepted source map is
`8380746325d6717df78a608eb5be8f3ff3c48e42f32eec19aba2ab0677fdcb08`.
Its 5,703 identities and exact nineteen-file delta, including three additions,
match the retained source bundle. Both full suites pass 2,865 tests with five
ignored; all 25 additional gates, ten compiled negatives across ten maps,
134 checker calibrations and disjoint 128/17 restored suites pass.

## Remaining Qualification

Ordinary recycled detach, other applicable live/control routes, queue teardown
and N5 generated data adoption remain separate implementation work.

This checkpoint cannot by itself establish live KFD execution, authenticated
formal implementation correspondence, bounded aggregate terminal memory,
multi-device admission or HIP/HSA performance parity. Reusing the forward cleanup
iterator introduces no per-control result vector, but does not prove a whole-
operation complexity improvement over underlying ledger and currentness work.
