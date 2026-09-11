# Local R90 Persistent Bind Settlement Evidence

Implementation baseline: signed planning commit
`287f2b77be5eb8de333f64e3f73b1f892970ad1e`, above signed R89
`7867f1d5fa8ac3e20e33226c8dcab54d7df31908`, on both topic remotes.
This packet implements the
[CONTROL-3 bind-settlement contract](../../runtime-persistent-bind-settlement-v1.md).

## Status

All seventeen frozen-source gates and auxiliary checks pass, with 5,629
non-documentation source identities unchanged. Three compiled mutations fail
at their intended assertions; restored source passes and matches the accepted
inventory. Production, fixture and contract reviews found no blocking issue.
No SSH, GPU workload, remote staging or Verus solver was started. This is local
CONTROL-3 acceptance; native constructor custody remains NATIVE-2.

## Final Acceptance

| Gate | Accepted result |
| --- | --- |
| GNU and musl runtime crates, all features/targets | 2,468 passed and five ignored per target, across 48 harnesses each. |
| GNU all-feature host / musl default host | 258 passed, four ignored / 141 passed. |
| GNU runtime and host doctests | 108 passed. |
| Musl runtime / default host doctests | 91 / 16 passed. |
| Generated macro fixtures | Seven passed. |
| Focused loan / bind / settlement / replay | Four / four / five / 21 passed. These overlapping selections include existing regressions. |
| Existing preparation / transitions / allocation / shared memory | Nineteen / twenty / fourteen / 198 passed; transitions and allocation are subsets of shared memory. |
| Existing retention / readback / charged / shell / storage / adoption / async | Thirteen / ten / fifty / nineteen / nine / fifteen / 233 passed. These are overlapping selections, not additional unique tests. |
| Lint, formatting and policy | Both Clippy configurations pass with warnings denied; formatting, whitespace, dependency policy/tests, local CI gate and 32 standalone lockfiles pass. |
| Runner/checker tests and production dependency audit | 151 Python tests pass; 43 packages and eight build scripts pass the production metadata audit. |
| Proof inventory | 686 negative files inventoried; no Verus solver rerun. |

The accepted run is `r90-accepted`, after the stopped `r90-final` attempt below.
The [summary](test-summary.json), [changed-source hashes](source-files.sha256),
[retained-file hashes](retained-files.sha256),
[gate record](raw/r90-accepted-source-gate.json) and
[complete source inventory](raw/r90-accepted-source-inputs.json) preserve exact
commands, results and source identities. Focused mutation/restoration receipts
snapshot every non-documentation source before and after execution. Raw logs,
helpers and mutation patches are retained without reformatting. Captured helpers
also preserve their original trailing whitespace and terminal blank lines.
Final source and documentation whitespace checks exclude the `raw/` archive;
those archival whitespace diagnostics are not runtime-source diagnostics.

## Scope And Oracles

Fourteen new KFD test functions supplement existing source/behavior guards:
four loan-settlement tests, three real-owner bind/replay matrices, three borrowed
replay-retention tests, three early/terminal bind tests and one replay phase-panic
test. The occupied-roster regression and preparation placement guard are
strengthened existing tests, not additional new functions.

The loan helper crosses operation success/error/panic with retake success/error/
panic, checks opening rejection/panic and exactly one retake, and preserves the
original operation payload even when a secondary payload's destructor panics.
An external owning sentinel survives closing error/panic.

Initial single/three-binding matrices use actual preparation/loader/retention
helpers with real tokens over fake-native records. They compare exact original
data and control identities, descriptors, roles, generation and N1/N2 charges.
Operation faults cross retake outcomes; final validation fails or panics while
completed preparation remains rooted. These retake and final-validation closures
are scripted adapter-boundary outcomes, not Linux foundation round trips.

Replay fixtures retain actual original code/kernarg controls and data through
borrowed retention and retake failures. They compare capacity, original content
descriptors, storage identity, predecessor/occurrence and unchanged native
observations, without rebuilding control. Completion/recycling is model-only;
unchanged fixture bytes are not a GPU execution result. Separate phase-sequencer
tests inject every phase error/panic and check custody outside the loan.

Early/cancellation fixtures use the real persistent-use ledger with synthetic
native leases, not charged native fixture records. They verify each prepared
entry and terminal-dominant retry after successful cancellation. Public ingress
tests preserve exact incoming/existing owners and the single-binding foreign-first
compatibility policy. Source guards establish placement, one replay loan, unchanged
currentness policy and nonfallible final installation under exclusive empty-slot
admission. They do not simulate arbitrary mutation of an occupied slot mid-commit.

## Intermediate Attempts

Initial library attempts found stale source-location guards after factoring the
production helpers. An intermediate fixture compile failed because a test module
shadowed the production retention module; an explicit import alias corrected it.
The 888-test pre-gate library run passed. Those preliminary outputs are retained
only in the execution transcript, not substituted for frozen acceptance logs.

The first frozen attempt, `r90-final`, passed both runtime targets, host suites,
doctests and macro fixtures, then stopped at three Clippy `err_expect` diagnostics.
The replay outcome mapping now uses `expect_err`. This stopped attempt and its
source inventory remain distinct from the later acceptance run.

## Mutation Oracles

Each isolated mutation compiles and fails exactly one selected test with Cargo
exit 101. Source hashes are recorded before and after each test; restoration
must match the complete accepted non-documentation inventory.

| Mutation | Intended rejection |
| --- | --- |
| Prefer the closing panic to the original operation panic | The operation/retake matrix observes `retake` instead of `operation`. |
| Ignore queue health after successful local cancellation | The actual prepared-use cancellation test observes retryable `true` for a terminal queue. |
| Move replay data out before its borrowed validation callback | The injected validation panic resumes, but the exact original token is missing from its external owner. |

No mutation remains. These compiled adapter regressions are separate from the
historical Verus negative inventory, and do not constitute new formal proofs.

## Open Boundaries

CONTROL-3's composed CPU/source acceptance does not qualify a Linux queue round
trip, GPU completion or new executable refinement. The generic loan helper still
does not root arbitrary owning return values through retake panic; these bind
callers keep native owners outside it and return only status.

Primary/auxiliary/replacement constructor custody, DATA-ADOPT, ISSUE, runtime
completion, generated API/graph/drain, aggregate/control-memory budgets, unreturned
backend mappings and teardown/unmap/release custody remain open. Process-aborting
allocation failure and arbitrary callbacks hiding moved native ownership are not
repaired here. No usable retry or native disposal authority is reconstructed.

Model, resource-accounting, completion and manifest/lockfile inputs remain
unchanged from the R73 proof checkpoint. Its 62 positive sources and 1,374
obligations are historical evidence, not proof of this new adapter. No Linux
execution, new theorem, performance gain, A1/A2 completion or HIP/HSA parity is claimed.
