# R101 Auxiliary Recovery And Currentness: Local Evidence

Source parent: signed R100 `4424f4607d8a64677556b32713a74b1ca5c6557a`.

This record accepts **NATIVE-2B.5B-3B**, the named CPU/local-helper recovery and
currentness matrix. It does not close 3C roster/slot composition, NATIVE-2B/2C,
A1/A2, issue #182, formal adapter correspondence, live KFD qualification or
HIP/HSA performance parity.

## Source Scope

Only two test-fixture files change: the new `integration_recovery_tests.rs` and
its three-line module declaration in `integration_prefix_tests.rs`. Two test
functions reuse the existing shared auxiliary driver and exact primary/pair
custody assertions. Production queue/runtime/model/accounting/completion code
and Cargo inputs remain unchanged. The original primary engine, foundation,
memory, accounts, data, ledgers and platform owners remain in the fixture.
R99's local registration, gate, mapping, shadow protection and cleanup helpers
execute; CREATE remains scripted.

The matrix has **32 auxiliary constructions: 30 failures and two successes**.
Four currentness boundaries crossed with Err/panic and both original-runtime
routes give sixteen failures. Runtime-created, output recovery and ID recovery
Err/panic plus event-ID panic across both routes give fourteen more. Event-ID
has no error-return channel. Each route also supplies a successful trace
baseline. Auxiliary-only trace windows select exact currentness occurrences;
every currentness failure compares its complete trace prefix to prevent shifted
injection. Callback failures check occurrence/counts, terminal tail, history and
the exact retained-owner partition.

| Failure boundary | Required retained state |
| --- | --- |
| Before CREATE | Cleaned unpublished shadow state; no second publish/CREATE, ID, outputs or completed auxiliary lane. |
| After CREATE | Engine-confirmed ID/outputs; no recovered construction outputs or auxiliary queue-live registration. |
| Before doorbell | Completed lane and queue-live registration; no doorbell or doorbell observation. |
| After doorbell | Completed lane, mapped doorbell and exact observed coordinates. |
| Runtime-created | Engine outputs remain; construction outputs absent and local runtime not queue-live. |
| Output recovery | Engine outputs and queue-live registration remain; construction outputs absent. |
| ID recovery / event-ID | Recovered construction outputs and unassembled dispatch/completion/runtime/event/shadow owners remain. |

Returned currentness errors append exact `CurrentnessLost` history for primary
then auxiliary, move both model phases to Ambiguous and poison engine authority.
Currentness panics do not fabricate that transition: pre-CREATE stays Planned,
later auxiliary phases stay Active, and the primary remains Active. Other
callback failures also leave Active model state without fabricated quarantine.
Every failure separately retains the terminal parent, poisoned outer ledgers
and local gate, admitted engine authority and unfinished creation arm. No
destination installation or gate finalization occurs.

Assertions preserve the exact original primary record except its required
quarantine phase, the complete history prefix and every appended coordinate.
Original error stages, wrapped sources, panic payloads and callback cutoffs are
checked. The recovered-output and recovered-ID fixture callbacks convert their
injected errors into missing-output/ID contracts; no raw callback-error
propagation is claimed. The original CREATE and successful-pair oracles are
unchanged. The resource-prefix container remains present but empty after its
authority transfers to the engine.

Local resource counters describe reservations, payloads and events, not native
allocation counts. Only deliberate fixture disposal makes those counters zero;
this is local cleanup, not confirmed native queue teardown.

## Gates

Authoritative records are `raw/r101-final-source-gate.json`, its completion
marker, `raw/r101-auxiliary-results.json`, its completion marker, the focused/
frozen/restored records and all four `r101-mut-*` records.
[test-summary.json](test-summary.json) contains parsed totals.

| Check | Result |
| --- | --- |
| GNU / musl runtime all-target suites | Each 2,544 passed, 5 ignored, 48 harnesses |
| GNU runtime/host documentation | 108 passed |
| musl runtime / host documentation | 91 / 16 passed |
| GNU host / musl host | 258 passed, 4 ignored / 141 passed |
| Generated macro fixtures | 7 passed |
| New recovery/currentness matrix | 2 test functions, 30 failures and 2 trace baselines |
| Frozen / restored construction | 58 / 58 passed, identical rosters |
| Registration / ordinary / Linux helper / initialization | 2 / 1 / 20 / 4 passed |
| Transition / preparation / bind regressions | 20 / 23 / 4 passed |
| Final source gates / auxiliary checks | 17 / 10 passed |
| Compiled behavioral mutations | All 4 fail their intended dynamic assertion |
| Non-documentation source identities | 5,657 unchanged and exactly restored |

Filtered tests overlap full suites; counts are not additional unique tests.
The collector requires R100's 56 construction tests plus both new tests in
frozen/restored runs and both full targets. Gate commands and unaffected totals
match committed R100, allowing only the two new full-target tests. Source gates
include both Clippy profiles with warnings denied, Python harness tests,
format/whitespace, dependency policy/tests, CI test-gate and standalone
lockfiles. Auxiliary checks include proof inventory and production dependency
closure. Inventory checking is not a solver run.

`raw/r101-environment.json` records WSL2, pinned nightly-2026-04-03 and cargo/rustc
binary hashes. Runs use locked/offline dependencies, four build jobs, disabled
incremental compilation and no `XDG_RUNTIME_DIR`. Wall times are test campaign
metadata, not CPU/GPU runtime performance measurements. The first focused run
passed; there is no preliminary failed positive attempt in this packet.

## Behavioral Negatives

| Mutation | Decisive behavioral failure |
| --- | --- |
| `pre-doorbell-error` | Ignoring the pre-doorbell currentness error maps a doorbell when the oracle requires its absence. |
| `post-doorbell-error` | Ignoring the post-doorbell currentness error returns success/installs instead of retaining failure. |
| `primary-quarantine` | Skipping the primary during quarantine leaves its exact record Active instead of Ambiguous. |
| `recovered-output-placement` | Delaying construction output storage until after ID recovery loses that root slot when ID recovery fails. Engine outputs still remain; this does not demonstrate loss of all output authority. |

Each mutant compiles, runs its exact named test and exits 101 with zero passed/
one failed. The collector reconstructs the tested bytes, validates the sole
changed source path and dynamic failure markers, then requires full source
restoration. Restored construction and final GNU/musl runs pass. These are
behavioral negatives, not Verus negative proofs.

## Swarm Handoff

The [current board](../../runtime-a1-a2-swarm-current.md#remaining-work-at-a-glance)
assigns Native 3C then replacement/rebind/insertion/release custody, Admission
CO-2A then descriptor/private-reply coverage, and Resources issuance/membership/
settlement proofs before a production Context journal. The board records the
live-lane restoration dependency, callback-internal custody limits, meaningful
identity mutations and the distinction between Reserved eligibility and full
destination Begin. These are reviewed work orders, not implemented follow-ons.

Read-only workers reviewed the source, evidence boundaries and next packets.
Primary owns edits, serialized gates/mutations, proof pins, hardware scheduling
and signed publication. Queued packets are not unattended jobs. No SSH session,
MI300X process or remote staging was created, so no shared-GPU cleanup was
required. No new solver, live KFD or performance result is claimed.
