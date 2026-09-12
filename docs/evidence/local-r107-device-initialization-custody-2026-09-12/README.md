# R107 Device Initialization Custody: Local Evidence

Locally accepted CPU/shared-sequence checkpoint above documentation-only parent
`5ba89a0e642cf02effc8e462e33be93b19aec1c2` and accepted R106 runtime
`ad6f71304ddb0f41cc102f9e6ecabd7ed7840efe`. This accepts N3-D's named initializer
and stack-regression checks, not N3-L, A1/A2, issue #182 or HIP/HSA parity.

See the [summary](test-summary.json),
[fresh source gates](raw/r107-accepted-source-gate.json),
[auxiliary checks](raw/r107-accepted-auxiliary-results.json),
[environment](raw/r107-accepted-environment.json),
[mutation recipes](raw/r107-accepted-mutations.json),
[collector](raw/r107-accepted-retain-local.js),
[closed collector validation](raw/r107-accepted-collector-validation.json) and
[ownership contract](../../runtime-device-initialization-custody-v1.md).

## Production Change

One private root owns the original arbitrary byte box or repeated-byte recipe
and the actual returned device lease through allocation, CPU initialization and
final GPU mapping. Source validation borrows the original box; CPU and final
GPU-map helpers borrow the lease. Admitted errors/panics retain that root in the
original engine without replacing the first error or panic. No unreturned lease
is fabricated. Host-only rejection and the existing first-currentness policy
remain distinct from admitted failure.

The standalone entry uses the same complete-entry helper as the fixture. Its
in-place core leaves ownership with an outer caller for eventual N3-L retake
integration, which is not implemented by this checkpoint. Public signatures,
PUBLIC allocation profile, one preflight hash, complete arbitrary-source copy
and exact readback, and repeated-byte no-second-readback behavior are preserved.

The original inline-root candidate reproducibly overflowed an existing runtime
test's stack. A private one-slot vector now reserves fallibly during engine
acquisition, before native effects. Retention checks vacancy/capacity and cannot
grow or overwrite the slot. This adds one pre-effect allocation per engine,
not per initializer or failure. Total retained-memory accounting remains open.

## Accepted Campaign

| Check | Result |
| --- | --- |
| Full GNU / musl | Each: 2,594 passed, five ignored, 48 completed harnesses |
| Source gates | 17 passed, including docs, host/macro, Clippy, Python, dependency and lockfile checks |
| Auxiliary checks | 10 passed, including focused regressions, proof inventory and production dependency audit |
| Frozen / restored initializer | 11 / 11 passed |
| Frozen / restored borrowed initialization | 8 / 8 passed |
| Frozen / restored transitions | 30 / 30 passed |
| Named runtime stack regression | Same exact command passed on the corrected source |
| Compiled behavioral negatives | Nine compiled and failed at their intended named assertion |
| Source identity | All 5,667 non-documentation hashes unchanged/restored |

The eleven new functions exercise both accounting modes and source kinds,
boundary lengths, six currentness boundaries, native errors/panics, GPU-map
prefixes, malformed allocation outputs, real lower-entry leases, foreign
coordinates/accounts, external ownership, pre-effect retry and slot refusal.
They compare original source identity/content/metadata, lease/native coordinates,
map arguments, exact account domains/charges and unrelated original owners.
Slot pointer/capacity observations cover the matrix; the separate slot test
requires the exact occupied-slot panic and unchanged retained ownership.

Each negative changes exactly its declared source file, runs one exact test,
then restores the whole source snapshot before the next mutation. The collector
reconstructs nine distinct mutated hashes, checks exact failing test/line/markers,
matches passing rosters against R106 plus the eleven new names, and verifies
mutation/restoration chronology. Independent read-only review also checked the
nine raw outcomes, source maps, restorations and focused rosters.

## Preserved Failures

The original [GNU attempt](raw/r107-final-source-gate.json) attempted only its
first gate and returned 101 after a runtime harness stack abort. The remaining
sixteen gates did not start. Its 47 completed harness summaries do not establish
full-suite acceptance; there is no fabricated source-after or completion record.
The [isolated reproduction](raw/r107-stack-repro.json) also returned 101/SIGABRT
at `caller_authority_and_missing_history_cannot_enter_copy_only_observation`.
These are abort diagnostics, not successful compiled assertion negatives.

The fresh `r107-accepted-*` cohort has its own freeze, runners, environment,
focused/full/auxiliary results, mutation tools and restoration records. The
collector pins the exact 51-name preexisting artifact roster and every byte hash.
Early source maps with 5,666 identities, the first matrix compile error, warning
bearing preliminary passes and both unused older mutation catalogs are retained.
The earliest formatter's stdout was not retained; its surrounding preliminary
test is not used as acceptance. The later slot-format attempt records its
expected source change and is also separate from the accepted campaign.

The corrected [stack check](raw/r107-accepted-stack.json) uses the same command
and runner-controlled environment as the failing reproduction. No runner stack
override or deadline change was introduced. Original ambient stack limits were
not captured. The [later observation](raw/r107-accepted-stack-environment-observation.json)
records an unset `RUST_MIN_STACK` during the fresh GNU gate; it is not a
retrospective observation of the earlier failed process.

## Limits and Next Work

These fixtures use scripted native leaves, not successful Linux/KFD execution
or complete original-engine composition. The configured capacity case saturates
bytes and records together, not independently. Slot allocation failure and its
pre-native constructor order are source-reviewed, not dynamically fault-injected.
Snapshots cover declared ownership/progress fields, not every private bit.
No CPU mapping-address substitution qualification or global allocator-count
measurement is added.

Proof inventory is not a solver run. No new formal adapter, live GPU,
performance, aggregate-memory or protected Worker V3 qualification is claimed.
No SSH or MI300X job ran for R107. Next are N3-L initialized-device insertion,
independent C1 identity coverage and V1 journal issuance; N4 cleanup and the
separate native/formal/performance gates remain open.
