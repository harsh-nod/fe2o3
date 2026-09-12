# R105 Pristine Rebind Custody: Local Evidence

Locally accepted CPU/shared-sequence checkpoint above signed R104
`b69a6f21c2beb0aad870d3f4b8cdc2eb183f456f`, with planning-only publication
parent `c70773b9c9d9383f79a1c2fe829f1e1163596750`. The seventeen source gates
completed before that three-document commit; all source hashes are unchanged.
The repeated restored focused suites and complete evidence collection pass.
This is not completion of NATIVE-2C, A1/A2, issue #182 or HIP/HSA parity.

See the [machine-readable summary](test-summary.json),
[source gates](raw/r105-final-source-gate.json),
[auxiliary checks](raw/r105-auxiliary-results.json),
[captured environment](raw/r105-environment.json),
[mutation definitions](raw/r105-mutations.json),
[collection checks](raw/r105-retain-local.js) and
[production contract](../../runtime-pristine-rebind-custody-v1.md).

## Production Change

Pristine rebind now uses the same owning input root, preparation, model loan,
retake, borrowed validation, checked extraction and commit as ordinary rebind.
The root takes the continuation only after common preflight succeeds and before
opening the loan. Opening failure retains it unconsumed; preparation entry
consumes it exactly once. Resume errors pass directly into failed-Generation
preparation rather than returning before custody records the failure.

The continuation preserves its exact next generation and mints a fresh recipe
occurrence, without ordinary/recycled increment or retry authority. Completed
preparation remains rooted until closing and validation succeed. The prior
consuming pristine bind/finish implementation is removed; abort/control-disposal
behavior and public APIs are unchanged.

Entered pristine returned errors retain their previous process-terminal policy,
including opening errors. Ordinary returned errors and pristine preflight keep
their separate classifications. Operation panic still wins over closing
failure; otherwise closing failure precedes an operation error. R104's
monotonic facade transport and restore-before-retain ordering are reused.

## CPU Matrix

Eight new dynamic functions plus two migrated existing-name tests exercise 119
N2 scenarios. The other eighteen pristine regressions remain separate; initial
setup preparation/abort calls are not additional scenarios.

| Matrix | Scenarios |
| --- | ---: |
| Real continuation generations 1/7/8/last | 4 |
| Preparation outcomes crossed with six retake outcomes | 18 |
| Opening error, panic and loan-generation exhaustion | 3 |
| Suppressed Complete failure with normal/bypassed validation | 2 |
| Internally corrupted continuation generation | 1 |
| 31 preparation stages, error and panic | 62 |
| Migrated settlement and constructor-rejection tests | 6 |
| Preflight identity/state/ring/completion rejection | 8 |
| Three lane ordinals crossed with five callback outcomes | 15 |

The preparation helper executes 96 scenarios: six successful installations,
46 returned errors and 44 caught panics. The eight preflight scenarios reject.
All fifteen facade scenarios transport a terminal parent; a swallowed error or
callback sentinel is not successful binding. R104's nine dynamic ordinary test
functions still execute their 112 scenarios. Its tenth, textual routing guard
now also follows the pristine production branch.

The new setup uses one actual preparation fixture for original memory, accounts,
foundation, preparation, lower pristine abort and subsequent rebind. It disposes
three code allocations and one kernarg before the rebind baseline. An exact
disposed-identity roster verifies every released tombstone and excludes only
those identities from live ownership. Rebind must perform no further unmap,
FREE or VA release; counters are not reset to hide setup disposal.

Oracles check exact packet/data descriptors, backing storage, ordered identity
ledgers, program identities, account charges, unchanged data bytes, retained
owner partitions, generation/occurrence and phase-dependent loan/retake counts.
The integrated roster has four storage variants; R82's separate tests preserve
five-variant coverage. Synthetic `MAX` corruption is a defensive private test,
not a legal public continuation. It rejects with `GenerationExhausted`, records
failed Generation and makes no allocation/map calls.

## Gate Campaign

The final source campaign is `r105-final`, followed by ten auxiliary checks.
Focused frozen/restored filters are pristine, ordinary rebind and construction.
Acceptance uses the `r105-repeat-restored-*` runs, not the original restored
passes associated with the rejected chronology campaign below.
The collector compares complete passing-name multisets with R104 plus exactly
eight additions; the two migrated old names remain, not silently removed tests.
Focused filters overlap the full suites and are not additional unique tests.

| Check | Result |
| --- | --- |
| Source gates / auxiliary checks | 17 / 10 pass |
| GNU / musl runtime all-target suites | Each 2,573 pass, five ignored, 48 harnesses |
| Frozen / repeated restored pristine | 28 / 28 pass |
| Frozen / repeated restored ordinary rebind | 10 / 10 pass |
| Frozen / repeated restored construction | 69 / 69 pass |
| Nine compiled behavioral negatives | All nine compile and fail their intended oracle |
| Frozen source identities | 5,664 |

Tool/binary identities and runner hashes were captured before the frozen/full/
auxiliary/mutation campaigns, after preliminary focused attempts. The campaign
uses pinned `nightly-2026-04-03`, locked/offline resolution, four Cargo build jobs,
four test-harness threads, disabled incremental compilation and unset
`XDG_RUNTIME_DIR`. No deadline is lengthened. Metadata stdout and stderr are
separately retained. Proof-inventory checks are not solver execution.

The captured Python version is still 3.12.3 but its build metadata differs from
R104: Aug 31 2026, 10:18:26 rather than Jun 19 2026, 12:46:00. The initial
read-only collector-prefix check rejected the old blanket environment-equality
assertion. The collector now records and pins this exact difference and checks
all current version outputs against the R105 capture. Rust binary identities,
other version outputs and the frozen runner hashes remain pinned; the earlier
environment record has not been rewritten. This is not a claim of identical
Python builds across R104 and R105 or Python binary authentication.

Default-concurrency reliability is not established by the four-thread setting;
R103's prior musl watchdog failures remain historical and unresolved. Test wall
times and incidental all-target diagnostics are not performance measurements.

## Compiled Negatives

The declared mutations increment the continuation, return early on resume
failure, lose continuation custody, drop the input root, skip validation,
extract failed Complete, omit pristine process poisoning, overwrite transport,
and omit lane restoration. Each compiled, ran its exact dynamic test, failed
the intended oracle, and was followed by restoration of every frozen source
identity. No negative exited through compilation failure, timeout or signal.
There are nine accepted cases in the repeat campaign and nine historical
negative runs, eighteen compiled runs in total, not eighteen distinct cases.

Before execution, review narrowed lost-continuation mutation to the failed-root
retention boundary and missing lane restoration to terminal transport only.
Healthy setup still restores, and original continuation custody reaches opening
before the negative discards it. The root-loss marker pins the final retention
assertion rather than any earlier unwrap. These superseded designs were not run.

Incremented generation and omitted terminal restoration first trigger caught
assertions, then fail their terminal test oracle. The collector requires both
diagnostics rather than calling the caught assertion the uncaught failure.
The collector checks ordered frozen tests, mutations, individual restoration
timestamps and final restored tests for the accepted repeat campaign.
The original collector rejected the first campaign before archive creation:
`omit-pristine-process-poison` restoration is recorded at
`2026-09-12T13:46:05.206Z`, but `overwrite-transport` starts at
`2026-09-12T13:46:05.146Z`, a 60 ms inversion. Its cause is not established;
this record does not infer clock skew or overlapping source mutation.

All nine negatives were repeated using the same pinned execution runner,
source, commands, environment and deadlines, followed by fresh restored
28/10/69 filters. The collector validates the original behavioral results and
pins their exact single chronology violation, but does not accept that campaign
or substitute its original restored passes. It requires strict ordering for
the repeat campaign, the preserved
[rejecting collector's exact hash](raw/r105-retain-local-before-repeat.js) and
the [rejection record's contents](raw/r105-collector-rejection.json).
No original timestamps or logs were rewritten.
Restoration records establish source equality at observation points, not an
independently authenticated execution timeline or native authority.

## Preliminary Attempts

`r105-focused-01` failed compilation on private test-helper exports. It overlapped
the initial formatter and reports changed source, so it cannot supply acceptance.
`-02` compiled but passed 18 and failed nine: old fixture assumptions counted
released control tombstones as live owners and expected terminal preflight to
preserve the session continuation. The corrected oracles verify exact disposal
and existing poison semantics rather than dropping ownership assertions.

`-03` failed compilation on an ambiguous injected callback error conversion.
`-04` passed 28 before the facade VM alignment correction; `-05` passed 28 on
the subsequently frozen source. Preliminary ordinary rebind passes ten and
all-feature/all-target Clippy denies warnings successfully. Those preliminary
passes are retained separately, not substituted for the frozen/full/restored
acceptance campaign.
Later formatting/builds are serialized; the initial formatter output is not
archived. Raw diagnostics and input-source maps remain separate by attempt.

## Retained Bytes

Raw files are copied byte-for-byte, including failed attempts and their source
maps. `retained-files.sha256` covers the retained files; publication checks
compare raw files with their originals, staged bytes with working bytes, all
5,664 frozen source identities and local documentation paths.

Whole-staged `git diff --cached --check` reports 26 new-blank-line-at-EOF
warnings, all in raw logs. Their bytes are preserved. The non-raw staged
source/documentation check passes with no whitespace warnings.

## Scope And Handoff

Preparation/abort/rebind share original fixture memory and accounts. The facade
is separately constructed with `engine=None`, though queue/completion owners now
use that foundation's model VM. It injects preparation and propagates the
transport request explicitly. The actual public bind is then exercised in the
already-terminal state. This does not establish successful public native bind,
original queue-engine/platform custody or native incarnation succession.

The fifteen facade cases cover primary plus auxiliary vector indices zero and
one, not arbitrary native configurations. The 31-stage sweep is not a new
lower-level syscall/allocation/map fault campaign. Retained envelopes borrow
caller module bytes. Allocator abort and panic-abort remain outside unwind
recovery. No new formal source, solver result, live KFD execution or matched
HIP/HSA performance is claimed.

Three read-only workers reviewed ownership, test oracles, mutations and evidence;
Primary owns edits, serialized gates and publication. Next Native work is N3-C
coherent copy-stage transition custody with same-foundation composition tests;
CO-2A identity and VER-1A.2a issuance remain independent. Cleanup, generated
adoption/issue/completion, versions, aggregate
memory closure and later A3-A7 work stay open. No shared GPU machine was used.
