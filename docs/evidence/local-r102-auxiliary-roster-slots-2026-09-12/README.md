# R102 Auxiliary Rosters And Slots: Local Evidence

Source parent: signed R101 `77ce1196f2867e79eb450b5a9ba5924ed13152fa`.

This record accepts **NATIVE-2B.5B-3C**, the named CPU/local-helper retained-roster
and destination-slot matrix. Together with the earlier packets it completes the
planned local .5B matrix, not complete NATIVE-2B/2C, A1/A2, issue #182, formal
adapter correspondence, live KFD qualification or HIP/HSA performance parity.

## Source Scope

Four files contain test-fixture changes: new `integration_roster_slot_tests.rs`,
the shared auxiliary integration and prefix fixtures, and one `#[cfg(test)]`
ID-only helper in `sdma.rs`. Production mechanisms and Cargo/proof inputs do
not change. Existing first-slot oracles remain wrappers with the same selection;
the new candidate-aware oracle checks the actual constructed queue separately
from injected roster metadata.

Two test functions exercise **16 auxiliary constructions: 14 failures and two
successes**, plus **four borrowed preflight rejections**. Every case runs
through both original-runtime routes. The driver, preparation, shared engine,
foundation, accounts, and local Linux registration/protection/cleanup helpers
are reused; CREATE remains scripted.

| Case | Required result |
| --- | --- |
| Retained auxiliary ID collision | Inject inert metadata after selecting a valid destination; reject before event observation, owner assembly or doorbell mapping. |
| Directional / striped SDMA collision | Match the second ID-only roster entry; retain the complete ordered observation roster and reject at queue-ID admission. |
| Vacant generation 4 reuse | Noncolliding directional/striped observations; install the real candidate at generation 5 without vector growth, reject generation 4 and accept 5. |
| Changed prepared index / generation / kind | Retain the completed candidate and mapped doorbell; do not finalize the gate or install. |
| Unreserved append | Same late retention, without growing or changing the original destination vector. |
| Exhausted generation / occupied real slot | Direct borrowed preflight before/after each reuse success; no opening, loan, CREATE or state change. |

The three collision cases give six failed constructions; four late-slot cases
give eight. The two reuse successes contain the four separate preflight checks.
Those checks are not four additional constructions.

The exact original model records/history and primary memory/account/platform
owners survive. Each constructed candidate appends only the three expected
admission/CREATE history events and remains Active in the model; these
rejections do not fabricate currentness loss or engine poison. Failure separately
retains the terminal parent, poisoned outer ledgers/local gate and unfinished
creation arm. Confirmed outputs and engine authority remain rooted.

Collision failures retain unassembled dispatch/completion/runtime/event/shadow
owners; late-slot failures retain the completed bundle. Successful installation
moves the real candidate exactly once. Assertions compare exact error stages,
callback counts/cutoffs, roster coordinates/storage, generation and owner
partitions. They require the actual new allocation debits: N2 adds 8,192 backing
bytes and two records; N1 adds 8,192 bytes plus the completion arena and three
records. Late rejection is not reported as unchanged total usage.

The injected auxiliary is deliberately inconsistent metadata, not a second
constructed auxiliary. SDMA inputs have no initialized ring, control, completion
or doorbell resources. The profile still permits only primary plus one auxiliary.
Neither these observations nor the generation-4-to-5 check qualify native SDMA
bootstrap, destroy/recreate, incarnation succession or preceding disposal.
Local resource counters describe reservations, payloads and events; only
deliberate fixture disposal returns them to zero. That is not native teardown.

## Gates

Authoritative records are the final source/auxiliary result and completion
records, focused/frozen/restored records and all six `r102-mut-*` records.
[test-summary.json](test-summary.json) contains parsed totals.

| Check | Result |
| --- | --- |
| GNU / musl runtime all-target suites | Each 2,546 passed, 5 ignored, 48 harnesses |
| GNU runtime/host documentation | 108 passed |
| musl runtime / host documentation | 91 / 16 passed |
| GNU host / musl host | 258 passed, 4 ignored / 141 passed |
| Generated macro fixtures | 7 passed |
| Roster/slot matrix | 2 test functions; 16 constructions and 4 separate borrowed preflight checks |
| Frozen / restored construction | 60 / 60 passed, identical rosters |
| Registration / ordinary / Linux helper / initialization | 2 / 1 / 20 / 4 passed |
| Transition / preparation / bind regressions | 20 / 23 / 4 passed |
| Final source gates / auxiliary checks | 17 / 10 passed |
| Compiled behavioral mutations | All 6 fail their intended dynamic assertion |
| Non-documentation source identities | 5,658 unchanged and exactly restored |

The final collector checks exact commands, source hashes, the complete prior
58-test construction roster plus the new two, and unaffected totals against
committed R101. Filtered tests overlap full suites and are not additional unique
tests. Source gates include both Clippy profiles with warnings denied, Python
harness tests, formatting/whitespace, dependency policy/tests, CI test-gate and
standalone lockfiles. Auxiliary checks include focused lifecycle regressions,
proof inventory and production dependency closure. Inventory checking is not a
solver run.

`raw/r102-environment.json` and the resumed environment record identify WSL2,
pinned nightly-2026-04-03 and exact cargo/rustc binaries. Environment observations
match apart from their timestamps. Runs use locked/offline dependencies, four
build jobs, disabled incremental compilation and no `XDG_RUNTIME_DIR`.
Wall times describe the validation campaign, not runtime performance.

The focused run passed. Two broader construction attempts were interrupted
without terminal test results: one exec reported 143 after the code-mode host
closed stdout; the subsequent attempt's handle was missing after restart and no
test process survived. Their logs, source inventories and explicit unaccepted
records remain retained. Only the later completed frozen/restored runs qualify.

## Behavioral Negatives

| Mutation | Decisive dynamic failure |
| --- | --- |
| `auxiliary-collision` | Bypassing the roster check reaches the later USERPTR destination error instead of the required queue-ID admission stage. |
| `directional-collision` | Ignoring the directional roster incorrectly returns construction success. |
| `striped-collision` | Ignoring the striped roster incorrectly returns construction success. |
| `prepared-generation` | Ignoring prepared-generation drift incorrectly installs the altered destination. |
| `append-reservation` | Removing reserved-capacity admission incorrectly returns success. |
| `reuse-generation` | Committing the predecessor generation observes 4 where the oracle requires 5. |

All six compile, run their exact named test and exit 101 with zero passed/one
failed. The collector reconstructs each tested mutation, verifies the sole
changed source path and dynamic assertion markers, and requires complete
restoration. These are behavioral negatives, not Verus negative proofs.

## Swarm Handoff

The [current board](../../runtime-a1-a2-swarm-current.md#remaining-work-at-a-glance)
assigns Native replacement input, then live rebind/insertion/release custody and
DATA-ADOPT; Admission takes CO-2A identity tests, then descriptor/private-reply
composition; Resources takes issuance/membership/settlement proofs before the
production Context journal. Native teardown explicitly includes auxiliary
removal, and replacement assertions must cover complete original packet/program
inputs. These reviewed work orders are not implemented follow-ons.

Read-only workers reviewed source, actual mutation/restoration evidence and
remaining handoffs. Primary owns edits, serialized builds, proof pins, hardware
scheduling and signed dual publication. Queued work is not running unattended.
No SSH session, MI300X process or remote staging was created for R102, so no
shared-GPU cleanup was required. No solver, live KFD or performance acceptance
is added.
