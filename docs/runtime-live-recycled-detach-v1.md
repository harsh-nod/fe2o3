# Live Ordinary Recycled Detach

## Status

R124 is locally accepted at the CPU/test boundary above published R123
(`948d1f2637ef08061aabf42485929869e3ed5dbc`). It implements ordinary live
`detach_recycled_fixed_dispatch` settlement and its selected-lane facade.
Two independent archive reviews support this ownership-boundary qualification.
A1/A2 and issue #182 remain incomplete.

## Ownership Boundary

The existing lower `ReturningControlCleanupCustodyV1` performs control cleanup.
The live sequencer does not introduce another native cleanup implementation.
An outer owner retains the original dispatch before opening the model loan;
the lower root and returned data stay outside that loan until retake succeeds.
The callback returns only a unit result. Completed cleanup alone cannot expose
data to the caller or commit the detached-data ledger.

The sequencer borrows the recycled generation and authority/premise cardinality
before taking the owner. It reserves both public output and storage-identity
vectors before full currentness validation or control disposal. The lower root
separately reserves its returned-authority vector before its first disposal.
After successful retake, authority conversion only unwraps existing typed
tokens into preallocated vectors. Initialized-content descriptors are not
reconstructed after dispatch; initialization is recovered from retained
premises, matching the preceding conversion behavior.

Generation, count, storage identities and insertion position commit together
after conversion. No fallible callback, allocation or validation follows the
first ledger write. Direct calls retain the terminal parent before propagating
the settled result. Facades record sticky transport before propagation; the
enclosing lane-custody operation restores the lane before retaining its parent.

## Failure Policy

| Boundary | Result And Custody |
| --- | --- |
| Existing poison, persistent attachment, unpublished work | Preserve preflight error and owners; no new terminal transfer |
| Any nonempty detached ledger field | Terminal contract failure; retain original owner and unchanged ledger |
| Nonreleasable completion owner or absent dispatch | Nonterminal preflight rejection |
| Invalid recycled generation/cardinality | Terminal rejection with original owner; no disposal |
| Outer output/identity reserve failure | Nonterminal rejection with original owner; no currentness or disposal |
| Currentness error or panic | Retain original owner with terminal parent |
| Opening error before callback | Restore exact original owner without new poison or retake |
| Missing callback in successful envelope | Restore original owner and retain terminal parent |
| Lower cleanup error | Retain the exact cleanup prefix; closing error takes precedence |
| Lower cleanup panic | Retain the exact cleanup prefix and preserve original lower panic |
| Completed cleanup followed by failed retake | Retain completed cleanup and all returned authorities; ledger unchanged |
| Failure after conversion but before ledger access completes | Retain all converted data; ledger unchanged |

Borrowed shape checks intentionally precede model opening/currentness. Therefore
their errors now win when the dispatch shape and the engine are both invalid.
Opening errors also intentionally improve upon the preceding unconditional
poisoning policy. A recycled attached persistent control remains admissible when
no public persistent attachment blocks it, preserving prior AfterRecycle
behavior. A detached persistent control is rejected by the detached ledger or
authority/premise cardinality, not a newly invented Ordinary-only restriction.

Terminal cleanup retains the existing explicit no-drop policy. This does not
prove an aggregate retained-memory bound. Captured lower panics survive secondary
retake/poison panics; the sequencer does not recover an original retake panic
already replaced inside the model driver.

## Development Evidence

The new fixtures use ordinary production preparation and actual logical
reserve/publish/complete/recycle transitions. Both single- and three-binding
cases retain all five input data representations. Completion occurrences,
completed typing and native memory calls are fixtures, not GPU observations.

Independent static review found no blocking production issue. Test review found
missing ledger/preflight oracles and an incorrectly placed facade setup; the
candidate now includes explicit checks for those boundaries. Static review is
not test execution or qualification.

`r124-development-focused-01` failed compilation on a test-only moved value in a
pattern guard. No tests ran. The runner closed normally with unchanged endpoint
source maps and no live owned process-group members. This launch began before
formatter completion was observed and cannot qualify a frozen-source campaign.
The failed record and raw log remain retained separately.

`r124-development-focused-02` compiled and ran sixteen tests: thirteen passed,
including the complete 144-case native-fault fixture matrix, and three public
route tests failed. The public parent fixture started with an already-detached
generation and insertion position. Those tests consequently encountered the
ledger guard before their intended currentness/completion paths. The subsequent
fixture correction clears only the selected lane's ledger during setup.
The run took 387.431 seconds including compilation, closed normally, preserved
source map `8763218d4c6cba8669edec9079ac645bfc99a93dd264c4a308205e0576de6b8b`,
and left no live owned process-group members. Its partial successes do not
qualify the corrected source.

`r124-development-focused-03` passes all sixteen selected tests with no failures
or ignored tests (1,210 filtered out), including the new 36-case post-disposal
projection-failure matrix and corrected direct/facade cases. It explicitly
excludes the expensive 144-case native-fault matrix from the preceding run.
The corrected source map is
`cf8be06a556dd9e17ef148423194e0558b923889793dc0bcafa595d6e9fdccbd`.
The command took 164.934 seconds including compilation (127.73 seconds of test
execution), closed normally, preserved its source endpoints and left no live
owned process-group members. This focused result is not full-source acceptance.

`r124-development-clippy-01` passes with `--all-features --all-targets`
for KFD and runtime under `-D warnings`, on the same corrected source map.
It took 27.904 seconds, closed normally, preserved source endpoints and left
no live owned process-group members.

`r124-development-regression-01` passes all 158 selected tests with no failures
or ignored tests (1,068 filtered out) on that same source map. It includes all
seventeen R124 tests, both 144-case native-fault matrices for recycled detach
and retained control, the new 36-case projection matrix, lower control cleanup,
and shared preparation regressions. It took 430.232 seconds (429.94 seconds of
test execution), closed normally, preserved source endpoints and left no live
owned process-group members.

`r124-development-format-01` also passes `cargo fmt --all -- --check` on
the same source map, with normal closure and no live owned process-group members.
`git diff --check` passes. These remain development checks, not the frozen
full-source acceptance campaign.

The opening-restoration and malformed-shape tests now assert owner presence
before taking their snapshots. The strengthened source map is
`a633edf685c4cbe3a24b1500a6a086ba36d39bec9b042abb5f88c52e4024cc24`,
with 5,706 non-doc source identities. `r124-development-format-02` passes on
this map after 20.096 seconds. The fresh `r124-development-regression-02` passes
all 158 selected tests with no failures or ignored tests and 1,068 filtered out,
including both 144-case fault matrices and the 36-case projection matrix.
It takes 488.787 seconds including compilation (449.40 seconds of tests).
`r124-development-clippy-02` passes the same strict KFD/runtime command after
26.297 seconds. All three runs close normally, preserve their source endpoints
and leave no live owned process-group members.

The prospective compiled-mutation review identifies nineteen behavioral probes,
including independent bypasses of all four detached-ledger admission predicates.
Their scoped forward/inverse edits, nineteen distinct source maps and diagnostic
coordinates pass read-only derivation checks. At that stage, these were not
executed mutation results; the completed campaign is recorded below.
Lost restoration or borrowed admission must fail the explicit owner
assertion, not a later missing-owner unwrap. Outer completeness and returned
shape guards are also protected by unchanged lower guarantees; their isolated
deletion must not be counted as an independently killed behavioral mutation.

The external prospective full plan, `r124-development-full-plan-v1.json`, has
SHA-256 `7b56d255f63cc2c839cd2b16928d0ed4ba7db2b94f73d50baef9bb48697b24b3`.
It uses committed R123 as its baseline, declares seventeen new KFD tests and
expects 2,882 passing tests with five ignored on each GNU/musl full run.
`r124-development-gnu-all-v1` now passes the exact declared roster on that frozen
source: 2,882 passed, zero failed and five ignored across 48 libtest targets and
the unchanged harnessless CSV target. It takes 1,733.989 seconds, closes normally,
preserves source endpoints and leaves no live owned process-group members.
The new runner pins the source and full
commands with sixty-minute deadlines; the plan retains all twenty-five
source/auxiliary leaf checks and seven byte-identical doctest relocations.
Read-only review found no adaptation issue in the runner, builder or successor
record/transcript checker. `r124-development-anchor-check-01` subsequently passes
the successor checker's exact 158-test filtered roster and Clippy/format record
checks, including source endpoints, command identities, clocks and owned-group
closure. It takes 0.560 seconds, closes normally on the same source map and
leaves no live owned process-group members. This validates the development
anchors only.

`r124-development-gnu-check-v1` validates the full GNU transcript, executable and
test identities, exact counts, source endpoints, command, clocks and owned-group
closure. The raw GNU log has SHA-256
`37d273d9bf911f7ca56a98794054e9868df6d24108e715d1e3997c21c84c77d9`.
The full, gate and mutation parser calibrations subsequently pass 32, 43 and 100
cases respectively. These 175 in-memory checker cases are not compiled runtime
mutations or executions of the then-pending source/auxiliary gates. All four checks
close normally with unchanged source endpoints and no live owned group members.
The exclusive source snapshot records twelve changed Rust files, including three
new files, with bundle SHA-256
`7ba51ddfaa96f0b1c8b7c765120aab4cd5482c6d29dfbc3aa101c15edfc1ca99`.
Its producer also closes normally on the unchanged source with no live owned
group members. Independent static review closes an omitted anchor-before-GNU
chronology guard in the then-unexecuted collector. Its subsequent execution is
recorded below.

The full musl regression was interrupted by a host reboot. Its partial log and
starting source map are retained, but there is no completion record or closing
source map. No exit status, elapsed duration, normal cleanup or passing full-suite
result is inferred from that partial log. The current source still matches all
5,706 frozen identities. The existing single-boot plan is not accepted or
silently weakened. The versioned successor full plan has SHA-256
`e0d5f50276944d64e626c8f7dac4a06477fd1a290c6a396ef2a011dbb568a63d`;
the qualification plan has SHA-256
`c826c5007f0311222781dee8314bb325a111745b2cf0ef710c954346911453fd`.
They declare two independent boot segments, preserving completed GNU as a
hash-pinned semantic prerequisite rather than a cross-boot clock predecessor.
`r124-development-readmission-v1` passes on unchanged source and pins 91
historical inputs. Fresh full/gate/mutation checker calibrations then pass
54/43/100 cases, separately from the retained 175 old-version cases. These
197 new cases include historical-record substitution, current live-group,
cross-boot predecessor, changed-source, interrupted-completion and exact new
musl command/deadline/runner rejection checks. Independent static reviews pass
for the resumption contract, checker calibrations and successor collector.
Historical process groups are identified by boot ID and PGID, using exact
pinned recorded closure without scanning their numeric PGIDs on the new boot.
The new `r124-development-musl-all-v2` run completes normally in 3,013.061 seconds,
inside its sixty-minute deadline: 2,882 passed, zero failed and five ignored,
with the exact 48 libtest targets and one CSV target. Its raw log has SHA-256
`48467c5f0b06b8e18ed3c111c30d36acba7cd6cd26b5413a2054a6e6f4f8741c`;
its completion record has SHA-256
`94164e4f165209cdf5556ab552dd9e6a248110260f04042238ce44187951835a`.
All 5,706 source identities are unchanged and the current-boot owned group is
absent. `r124-development-musl-gate-check-v2` validates both full transcripts and
their exact rosters/records in 0.822 seconds, also with normal closure and
unchanged source. All 25 source/auxiliary leaves and the exact all-transcript
checker now pass. Independent read-only review confirms their exact rosters,
52 source snapshots, predecessor chain and normal closure. The final gate record
has SHA-256 `ccb3b255c4431b5a9ada2b3d38da48b24295e18116d0dee6526701f84f72846e`.
All 19 compiled behavioral negatives reach their declared assertions with normal
Cargo 101 exits, followed by passing checker and restoration records. Each
negative stops at its first decisive assertion; this is not independent rejection
across every matrix combination. The 5,706-entry source map is restored. The final
regression passes all 158 tests, with zero failures/ignored and 1,068 filtered,
in 491.358 seconds; independent review confirms its exact roster and normal closure.
Record SHA-256: `07965a8657b270787a0032103e640af60e4b369ee52665a2778c15d882d27b6b`.
The collector passes in 16.754 seconds, with unchanged source and normal closure.
The [prepared archive](evidence/local-r124-live-recycled-detach-2026-09-15/README.md)
contains 476 raw artifacts and replays the collector exactly. Its prepared summary
has SHA-256 `3e2feb503647305f871dee64cbdae2ffaa435593599b7858fd9ecb8e03d20b5e`.
Both independent archive reviews pass. The summary records the reviewed prepared
hashes separately from metadata-only finalization; raw evidence remains unchanged.
Native execution, formal implementation correspondence, total-memory
bounds and matched HIP/HSA performance evidence remain separate open gates.
