# R96 Shared-Engine Auxiliary Construction: Local Evidence

Implementation parent: signed R95
`da90a0038c6ec4c697faf0fbe93d597e9fa36e1a`. Final validation ran above signed
planning checkpoint `10902ca32a853448f79b59cfcf22072e3cdd9325`.

This record accepts **NATIVE-2B.5A**, the production-used auxiliary phase
composition, under the [construction contract](../../runtime-auxiliary-queue-construction-custody-v1.md).
It does not close the full .5B matrix, A1/A2, issue #182, formal adapter
refinement, live KFD qualification or matched HIP/HSA performance.

## Accepted Source Scope

- Private generic preparation and CREATE/install phases use the existing
  primary environment and memory interfaces. The public Linux session remains
  concrete; no new public capability or allocator is introduced.
- The Linux wrapper invokes preparation inside the existing model-loan
  envelope, settles retake before CREATE and forwards its original engine,
  primary observation, auxiliary slots and both SDMA rosters.
- The borrowed target preserves the retained-ID scan, both actual
  `prepare_operation` calls and vacancy -> gate finish -> infallible install.
- Whole-parent terminal custody lives in the adjacent outer scope. Its
  original ownership order and R95 settlement behavior are preserved.

Two new integration functions exercise one success and eight late-failure
cells after successful primary construction: duplicate primary queue ID,
output-recovery error, ID-recovery error, doorbell error/panic,
doorbell-observation panic and gate-finalization error/panic. A third function
checks the concrete production wrapper's phase ordering and original bindings.

The paired fixture calls the shared production phases with the original
engine, foundation, memory session, accounts and platform owners. Actual model
loan/reclaim methods are used; no second engine or dependency owner is created.
It checks exact owner/native-record partitions, Device authorities, unchanged
original bytes/descriptors/premises, nonempty primary completion/dependency
snapshots, stable owner addresses and exact per-record charge bindings. Those
ledger snapshots are not an active GPU dispatch workload.

Added charges are exactly 8,192 N2 bytes/two records and 532,480 N1 bytes/three
records. The fixture's 2-MiB Host budget is configured before primary allocation;
it is not raised during auxiliary construction. Primary and auxiliary CREATE
outputs and shadows are joined to their exact keys/context-save tokens. On
failure there is no installation, owner drop or published-shadow cleanup.

The duplicate-primary-ID case is lower CREATE `Ambiguous` rejection: no ID or
outputs are committed, and the diagnostic is `CREATE_QUEUE result` with
`queue syscall result indeterminate`. It does not reach the later retained
auxiliary/SDMA roster check. Scripted platform leaves and a fixture outer scope
do not constitute full concrete Linux outer settlement.

## Gates

Authoritative records are `raw/r96-final-source-gate.json`,
`raw/r96-auxiliary-results.json`, `raw/r96-frozen.json` and the final `v2-`
mutation records. `test-summary.json` contains parsed totals.

| Check | Result |
| --- | --- |
| GNU runtime all-target suites | 2,519 passed, 5 ignored, 48 harnesses |
| musl runtime all-target suites | 2,519 passed, 5 ignored, 48 harnesses |
| KFD library harness within each full suite | 939 passed |
| GNU runtime/host documentation | 108 passed |
| musl runtime / host documentation | 91 / 16 passed |
| GNU host / musl host | 258 passed, 4 ignored / 141 passed |
| Generated macro fixtures | 7 passed |
| Frozen focused construction suite | 41 passed |
| Focused auxiliary / primary construction | 8 / 27 passed |
| Python runner/checker tests | 151 passed |
| Final source gates / auxiliary checks | 17 / 12 passed |
| Compiled negative mutations | 4 exact tests rejected after compilation |
| Non-documentation source identities | 5,648 unchanged and exactly restored |

Filtered suites overlap full suites and are not additional unique tests. Gates
include both Clippy profiles with warnings denied, format/whitespace, dependency
policy/tests, CI test-gate checks and standalone lockfiles. Auxiliary checks
also cover existing ordinary/lower construction, Linux helpers, initialization,
preparation, transitions and bind regressions, production metadata and the
pure-Rust closure audit.

The 686-file expected-negative inventory check is not a Verus solver run.
Runtime-model, resource-accounting, completion and Cargo inputs are unchanged
from R95. No new theorem, executable adapter proof or hardware result is added.

## Mutations And Earlier Attempts

The final `raw/r96-mutations.json` requires exactly four named mutations and
the pinned nightly, locked/offline KFD test command with `--exact`:

| Mutation | Required rejection |
| --- | --- |
| `v2-completion-owner` | Missing completed-lane completion ledger |
| `v2-closing-currentness` | Missing actual closing observation in the exact event sequence |
| `v2-installed-generation` | Generation 2 instead of the exact initial generation 1 |
| `v2-original-sdma-roster` | Missing original SDMA binding in the production wrapper |

The first three are runtime fixture checks. The fourth is a production-wiring
source guard, not executed SDMA collision evidence. Every mutation compiles,
fails exactly its named test with exit 101 and zero passed/one failed, changes
only its intended source file and is restored before the positive final gates.
The retainer reconstructs each mutation from its manifest and checks hashes.

The first full-source attempt, `r96-accepted`, failed the GNU gate at the stale
`every_live_foundation_mutation_and_moved_owner_uses_the_unwind_envelope`
source guard: KFD passed 938 tests and failed that one test. The guard was
updated to follow the shared helper while preserving capture-before-custody
ordering. A focused rerun passed; `r96-format-preflight` then failed one wrapping
check. That formatting was corrected before the authoritative `r96-frozen`
suite, all four `v2-` mutations and `r96-final` gates.

Earlier focused/Clippy runs, the repaired-guard-only run and the original
four-mutation campaign remain preliminary records in `raw/`; none substitutes
for final-source acceptance. Interactive fixture development also corrected
the initial Host budget and collision expectations. An initial mutation-helper
reverse match was ambiguous and stopped before testing; exact restoration and
a contextual replacement resolved it. Those interactive diagnostics are not
independent accepted run records.

Evidence tooling was strengthened before final acceptance: complete source
inventories bind the focused, format, full and auxiliary runs; exact mutation
rosters/commands and expected test totals reject empty or filtered campaigns;
named new tests must pass in both full targets. Source and retained-file hashes
are recorded separately. Three read-only reviewers checked production
equivalence, fixture sensitivity, accounting and evidence boundaries.

Final evidence-helper editing was interrupted by shared-filesystem exhaustion
after all tests had finished. Only stale incremental caches in this task's
workspace were removed. The helper was restored, and the complete source file
set and all final hashes were rechecked; runtime source and test records did
not change. No other worktree or active build was cleaned or stopped.

## Remaining Acceptance

NATIVE-2B.5B still requires original-parent production outer settlement through
opening, preparation/control-prefix and real loan/reclaim failures; composition
with the retained primary runtime lease and local Linux gate/shadow helpers;
full CREATE uncertainty/malformed/panic, both late currentness failures,
occupied/reused slots, retained auxiliary/SDMA rosters, cleanup failure and
first-panic transport. These are bounded in the
[current work orders](../../runtime-a1-a2-swarm-current.md#auxiliary-acceptance-packets).

Replacement/insertion, native generated adoption, callback-internal unreturned
owners, concurrent bootstrap, aggregate bounds, protected compiler integration,
new formal refinement, live KFD and performance remain separate work. No SSH
sessions, MI300X processes or remote staging were created by this campaign.
