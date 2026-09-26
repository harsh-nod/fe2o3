# Accounted Context Fail-Stop

Development work on signed parent `00831935f7cb36414d707f742c7d1640dc43964e`.
This closes a direct-Context custody gap found while reviewing the Worker
server-local owner. It does not establish native execution, formal refinement,
Worker V3 compiler authority, A1/A2 completion or HIP/HSA performance parity.

## Production Change

One constant-time predicate now enables unwind containment whenever the Context
has any configured request account, a version journal, or retained scalar-peer
or producer-launch custody. It does not depend on the selected device having an
account or on there being a currently retained allocation. Ordinary allocation,
release, metadata and backend guards use the same predicate. Nonjournal writes
and owned native shutdown use the guarded invocation too.

Terminal sealing quarantines every Context-local retained request before any
no-journal early return. It keeps logical/native handle records and original
panic payloads, does not call cleanup, and does not drain external Reserved or
Retained credits sharing an account. The Worker owner now reuses this central
sealing operation without a duplicate quarantine pass.
After that drain, sealing marks each retained writer Unknown without repeating
empty allocation-credit lookups over reader, producer and binding rosters.

Rejected and nonterminal Quiescent failures keep their prior semantics. An
allocation-specific settled-no-owner result still refunds only that attempt.
Confirmed disposal remains refunded even if later cleanup panics. The fully
unconfigured, unjournaled legacy fast path is unchanged. Constructor panic and
immutable backend observations remain separate boundaries.

## Tests and Diagnostics

Nine in-tree groups exercise ordinary and shared-domain request accounting:

- Nineteen public effect paths, including allocation/release/read/write on an
  unconfigured device while another device owns accounted allocations.
- Original boxed panic identity, all-local quarantine, exact byte/record charges,
  retained native/logical handles and no backend reentry after sealing.
- Empty configured accounts, after-last-disposal entry, and legacy compatibility.
- Terminal/protocol errors and nonterminal rejection/quiescence controls.
- Partial cleanup: one confirmed disposal followed by panic on the next owner.
- External same-account Reserved/Retained credits retaining their own phases.
- Owned shutdown panic/Terminal versus retryable failures after ordinary cleanup.
- Backend allocation effects before panic, retaining both the unknown attempt's
  debit and the previously known allocation records.

The initial four-group baseline compiles against unchanged production logic and
reports three intended assertion failures plus one passing retryability control.
An earlier harness compilation error passed a borrowed handle to the consuming
`release_submission` API; its diagnostic is retained in `base/initial-compile.log`.

The first full-suite rerun aborted. A serial `--nocapture` replay isolated a
generated fixture's stale post-terminal phase assertion: used bytes/records
were unchanged, but credits correctly moved from Retained to Quarantined. The
assertion unwound through its native-custody abort-on-Drop guard before reaching
the fixture's explicit retained-owner path. `initial-final/isolated-abort.log`
records this failure. Other generated/journal tests were updated to expect all
local credits quarantined on terminal sealing; exact charges, handles, journal
state, confirmed refunds and nonterminal expectations remain checked.

A second abort was isolated to the different-live-token generated fixture's
same stale phase expectation (six Retained versus six Quarantined, equal charge).
The diagnostic run excluding that one known abort reported 304 Context passes
and two further phase-assertion failures. `second-final/` retains both logs;
this exclusion was diagnostic only, not a substitute for the final unfiltered
library run. Those two assertions and the fixture were subsequently corrected.

The third unfiltered run completed with 1,515 passes, five failures and 28
ignores. In addition to the three known socket-environment failures, two SDMA
fixtures expected the old phase/timing: only the failed allocation's debit
quarantined, and read panic sealing deferred until retry. They now assert all
local credits quarantined and immediate sealing for configured Contexts. Their
original owners, backing bytes, charges, driver steps and no-disposal checks
remain intact. `third-final/` preserves that intermediate run.

The pre-simplification campaign reached 1,517 passes, only the three known
socket-environment failures and 28 ignores; 52 doctests and static checks passed.
It is preserved separately before removing the redundant post-drain traversals.

The baseline and first failed full campaign are preserved separately from final
results. These are scripted CPU regressions, not proof that a native backend
refines its contracts. No new Verus campaign or GPU/performance run is claimed;
historical proof receipts remain bound to their original source cohorts.

## Final Results

| Gate | Result |
| --- | --- |
| New fail-stop regression groups | 9 passed |
| Unfiltered all-feature runtime library | 1,517 passed, 3 failed, 28 ignored |
| Runtime doctests | 52 passed |
| All-feature/all-target Clippy, `-D warnings` | Passed |
| No-default-feature check | Passed |
| Formatting and whitespace | Passed |
| Source manifest check | Passed |

Counts overlap. The three failures are
`authorized_execution::tests::{cooperative_debug_telemetry_emits_only_bounded_logical_records,
failed_session_end_is_explicit_and_terminal, pre_native_telemetry_failure_is_returned_and_poisoned}`.
Each reports `InspectSocket` with `Operation not permitted`; they remain failures,
not passing qualification. The final library command has no exclusions.

`final/sources.sha256` pins the repository Cargo inputs, crates and examples.
The passing source-check log and intermediate source manifests are retained as
deterministic gzip archives. After all processes terminated, the owned target
directory (551,256 KiB) was removed. The before/absence logs record that cleanup;
no other job's files were removed and no remote artifacts were created.

## Reproduction

```sh
bash docs/evidence/dev-context-accounted-fail-stop-2026-09-26/validate.sh /tmp/fe2o3-owned-target final
```

Use an owned unused target path and remove only that path after all processes
terminate. `base` results describe the pre-fix production revision with the first
four regression groups installed, not a passing run of the final implementation.
