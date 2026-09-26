# Async Bounded Quiescent Settlement

Date: 2026-09-26. Development evidence only; accepted milestones, A1/A2,
formal correspondence, native qualification and HIP/HSA parity are unchanged.

## Source And Behavior

Signed implementation commit: `32d92e297bb3e73e20948827bac8e2a3f1b2e438`.
Parent: `f262e6209b34527ec674a2113892a59fbe4208fd`.

The ordinary async operation driver previously treated any `BackendQuiescent`
observation as permission to retire. Context may instead have observed a
producer: its bounded completion planner preserves that producer diagnostic
even when propagation to the requested submission is still Pending. Retiring
the driver withdrew its promised continued progress. Context retained resource
custody; this was not a demonstrated unsafe release or permanent drain deadlock.

The driver now retains the first such diagnostic when Context is live and the
exact requested submission is Pending. It continues normal bounded advances
and restores the diagnostic on requested `QuiescentWithoutResult`. Current
errors and conclusive non-quiescent observations are not replaced. A quiescent
error that terminalizes Context in the same call is still delivered immediately.
Stopping observation returns `EngineStopped`, not the stored diagnostic.
Unrelated Context quarantine prevents further registry progress until shutdown;
it does not synthesize successful completion.

This adds one optional backend error to each ordinary driver, with no error
cloning, new allocation, recursion or unbounded local progress loop. Generated
completion classification, graph execution, drain and the shared planner are
unchanged. Graph execution already rechecks requested status after quiescence;
drain already checks the global pending count before declaring quiescence.

## CPU Qualification

One parameterized regression covers 16 combinations: journal/legacy Context,
ordinary/directed observation policy, and retained/dropped/stopped/quarantined
observation. It publicly constructs 255 producers and one async root, retains
native successes through top-down observations, then discards the leaf result.
It checks 131 settled callbacks and 125 outstanding chain nodes at the bounded
yield, exact callback ordering, retained progress/control/reply credit, zero
automatic flushes or releases, and final local settlement without another
backend call. The terminal variant preserves the pending prefix and custody.

The directed-policy branch uses the public async directed API. The ordinary
policy branch uses the internal enqueue helper with a public Context directed
submission; it is not an independent public typed-launch qualification.

| Check | Result |
| --- | --- |
| Initial regression against unchanged parent runtime | Intended failure: registry length 0, expected 1 |
| Final focused regression, including Stop/quarantine extensions | 1 passed, all 16 combinations |
| Unfiltered all-feature runtime library | 1,526 passed, 3 failed, 28 ignored |
| All-feature/all-target runtime Clippy with `-D warnings` | Passed |
| Runtime `--no-default-features` check | Passed |
| Workspace formatting check | Passed |

Build, test and lint commands used offline mode, `CARGO_INCREMENTAL=0` and
`CARGO_BUILD_JOBS=1`. The three full-suite failures remain the
`authorized_execution` telemetry tests at `authorized_execution.rs:1317`, each
reporting `InspectSocket(PermissionDenied)`. They are not skipped or counted as
passes. Focused and broad counts overlap. The initial failing regression was
run before the Stop/quarantine matrix extensions were added.

Two independent read-only reviews found no correctness blocker. A specifically
injected post-deferral validation/protocol error remains a test gap; the new
Context-quarantine case instead verifies registry Stop precedence. No Verus
campaign was rerun or extended for this driver change. The existing finite-graph
planner proof does not prove the new ordinary driver or full Context composition.
There is no new native or performance result. MI300X access failed DNS before
any remote process or artifact was created.

`receipts.tar.xz` contains the failing baseline, focused and full test output,
Clippy/minimal/format logs, Rust toolchain identity and signed source summary.
Its SHA-256 is
`fd5da18506fb7bbd25eca58380fff2ef454423d9f5f01dbac17c90f395dccd48`.
An archive comparison against all eight original logs passed.
