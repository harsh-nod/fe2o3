# R103 Replacement Input Custody: Local Evidence

Source parent: signed R102 `50c4eb075013fde0a984a003c0b5b90eae562847`.

This record locally accepts **NATIVE-2C replacement-input custody**, using the
shared production construction sequence and CPU/scripted-platform fixtures.
It does not complete NATIVE-2C, A1/A2, issue #182, formal adapter correspondence,
live KFD qualification or HIP/HSA performance parity.

## Source Scope

Three production files change: `queue_live.rs`, `construction_primary.rs` and
`queue_dispatch_binding.rs`. The replacement entry now roots the original
memory session, receipt observation, predecessor generation, program vector,
packets and data before ring validation or geometry planning. Generation-aware
preparation runs in place under the existing primary construction owner. Its
completed dispatch transfers into that owner before the existing queue sequence.
No public signature, memory/platform trait, queue engine or backend-selection
API changes. Three additional files contain fixtures and test helpers.

The original generation `Result` enters preparation custody without an early
`?`: a rejected generation must record the failed preparation stage as well as
retain inputs. Replacement zero is stale, not pristine. Success advances `7`
to `8` and `u64::MAX - 2` to the last issuable `u64::MAX - 1`.

Eight dynamic tests exercise **254 shared-sequence runs: 252 failures and two
successes**. One additional source-wiring guard gives nine new test functions.

| Matrix | Runs |
| --- | ---: |
| Invalid ring and planning error/panic | 7 |
| Invalid/exhausted predecessor generation | 3 |
| Invalid packet program index | 1 |
| 31 preparation stages, error and panic | 62 |
| Native allocation/seal/map/write/projection failures | 126 |
| Late construction and currentness error/panic | 48 |
| CREATE uncertainty | 5 |
| Exact successor and last-issuable success | 2 |

Snapshots preserve the exact root, original vector storage/order/capacity,
program identities and selected descriptor observations, complete packet/data
observations, and the actual memory/account/resource-owner partition. Early
rejections do not arm the USERPTR gate. Native preparation failure quarantines
the session; later failure retains completed dispatch, original inputs and any
completed queue/doorbell owner. Late cases must reach their requested trace
occurrence, and closing-currentness cases check exact completed ownership.
Envelopes still borrow caller module bytes; retaining their vector does not own
those bytes.

The fixture receipt is metadata, not an executed predecessor destruction.
Late returned errors are type-checked, not compared against every nested error
variant. CREATE cases check custody/publication, not the full phase/history
oracle supplied by earlier primary tests. Recipe occurrence is checked nonzero,
not traced unchanged across every transfer. These limits are not native
incarnation, callback-internal custody or disposal qualifications.

## Accepted Gates

The authoritative fresh source campaign is **`r103-accepted`**, followed by all
ten auxiliary checks. The collector requires their complete records and exact
frozen input/output hashes; it does not combine an earlier GNU pass with a later
musl pass. [test-summary.json](test-summary.json) contains parsed results.

| Check | Result |
| --- | --- |
| GNU / musl runtime all-target suites | Each 2,555 passed, 5 ignored, 48 harnesses |
| GNU runtime/host documentation | 108 passed |
| musl runtime / host documentation | 91 / 16 passed |
| GNU host / musl host | 258 passed, 4 ignored / 141 passed |
| Generated macro fixtures | 7 passed |
| Frozen / restored construction | 69 / 69 passed, identical rosters |
| Registration / ordinary / Linux helpers / initialization | 2 / 1 / 20 / 4 passed |
| Transition / preparation / bind regressions | 20 / 23 / 4 passed |
| Source gates / auxiliary checks | 17 / 10 passed |
| Compiled behavioral negatives | All 6 fail their intended assertion |
| Non-documentation source identities | 5,659 unchanged and exactly restored |

The collector checks exact commands, the prior 60 construction tests plus the
new nine, and the six reconstructed mutations. Test totals match committed R102
except for the nine expected additions to each GNU/musl all-target suite.
Focused suites overlap full suites and are not additional unique tests.
Gates include both Clippy profiles with warnings denied, Python harness tests,
formatting/whitespace, dependency policy/tests, the CI test gate and standalone
lockfiles. Auxiliary checks include proof inventory and production dependency
closure. An inventory check is not a solver run.

The fresh source and auxiliary campaigns use **four Rust test-harness threads
and four Cargo build jobs**, with incremental compilation disabled,
locked/offline dependencies and `XDG_RUNTIME_DIR` unset. This limits inter-test
concurrency, not internal runtime threads. It is not identical-environment or
default-concurrency acceptance. The pinned nightly-2026-04-03 cargo/rustc binary
identities match R102; [the campaign environment](raw/r103-accepted-environment.json)
records the explicit concurrency change and load observed at its start.
Validation wall times and incidental all-target diagnostics are not matched
runtime performance measurements.

## Retained Failed Attempt

The original **`r103-final` campaign remains failed and unaccepted**. GNU passed
2,555 tests; musl passed 2,553 with two failures and five ignored. Both failures
were in the unchanged runtime unit harness:

- `r65_executor_drains_2048_accepted_operations_with_full_reply_budget`: the
  timer-first 10-second watchdog expired before quiescence assertions.
- `blocked_request_write_obeys_absolute_deadline_and_reaps_the_worker`: the
  expected request-write timeout was returned, but its deadline-plus-75-ms
  scheduler-tolerance assertion failed before subsequent reaping assertions.

Both package-filtered musl diagnostics subsequently passed unchanged, as did
both named tests in the complete fresh musl suite. No source, test deadline or
test filter was changed for the fresh full campaign. Scheduling contention is
consistent with observations, but the original failure cause is not proved.
Default-concurrency musl reliability remains a qualification follow-up.
Diagnostic passes do not substitute for full-suite acceptance.

Original logs and the two-row failed campaign record remain untouched. That
campaign has no source-after or completion record; none is fabricated here.
The earlier test-only compile failure, pre-freeze focused pass and preflight
Clippy run are retained separately and do not supply headline acceptance.

## Behavioral Negatives

| Mutation | Decisive dynamic failure |
| --- | --- |
| `zero-pristine` | A recycled zero predecessor is incorrectly accepted as pristine. |
| `reset-generation` | A fresh owner loses the exact successor generation. |
| `generation-custody` | Early propagation bypasses the failed Generation custody stage. |
| `completed-owner` | Discarding prepared dispatch loses the owner required during queue construction. |
| `original-programs` | Clearing the program vector violates original input storage/length custody. |
| `native-quarantine` | Omitting post-native error quarantine leaves the session Active. |

Each mutation compiles, runs its exact named dynamic test and exits 101 with
zero passed/one failed. Source identities match the reconstructed single-file
mutation, then the complete frozen source is restored. These are behavioral
negatives, not Verus negative proofs.

## Swarm Handoff

The [current work orders](../../runtime-a1-a2-swarm-current.md#remaining-work-at-a-glance)
assign Native live-lane restoration/rebind, then insertion, teardown and
DATA-ADOPT; Admission takes Context identity, descriptor identity and private
reply/custody composition; Resources takes issuance, membership, settlement and
proofs before the production journal. The
[journal policy freeze](../../runtime-context-version-journal-v1.md#r103-planning-freeze)
is planning only. These follow-ons are not implemented by R103.

Three read-only workers reviewed production ownership, test oracles, evidence
and the remaining handoffs. Primary owns edits, integration, serialized builds,
proof pins and publication. No MI300X/MI350 job, live KFD execution, solver rerun
or matched HIP/HSA benchmark was performed. Issue #182 remains open; its
[observed status](raw/r103-issue-status.json) does not grant implementation
acceptance. The retained-file manifest binds this record and all raw artifacts.
