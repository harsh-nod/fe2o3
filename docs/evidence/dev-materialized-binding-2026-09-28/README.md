# Ordinary Initial Binding Custody

Development implementation and CPU qualification only. Accepted checkpoints,
A1/A2, protected Worker application, formal correspondence and HIP/HSA
behavioral/performance parity are unchanged.

## Source

- Production and new tests: `e19b9fcf93c781dabe097caf34ee2fec40f5ef65`.
- Existing detach-test corrections: `4ce9a18dc4a361380fa1f7470f844c3663fe7017`.
- Final test-only lint correction: `4e7c98c065eb3ea2714bc0cdc79067673607b260`.
- Final source tree: `b131d76d8fd2747883ce89c0b04e0052dc0bfb59`.

All three commits have verified SSH signatures and sign-offs. The final commit
only replaces a test's odd-case predicate with `!case.is_multiple_of(2)`; the
archive records the complete delta from the full-suite revision. Production
sources are unchanged by either test correction. The unrelated untracked
`dev-owner-inspection-2026-09-23` directory was preserved.

## Behavior

Ordinary publication previously performed binding/reuse and first submit before
installing Active. It now installs a distinct `MaterializedBinding` phase before
detach, overwrite, materialization or first submission. Pure host program and
capacity preflight still precede that transfer.

The indexed root retains the accepted recipe, allocation/module custody,
writebacks, current descriptors and publication profile. First submission marks
the native-owned attempt and stores a successful batch or retry classification
inside the native callback, before its outer lane loan closes. Only confirmed
retry becomes `MaterializedPrepared`; incomplete binding cannot resume or
cancel. Success installs the published owner before profiling. Error/unwind
retains Active and retires only the one-time Pending FIFO/dependency handoff.

Attached overwrite preserves the prior recycled metadata through the complete
outer operation and records the authenticated prior generation before writing.
Detach likewise preserves metadata on rejection and roots returned DATA inside
the callback. An existing resident roster prevents conflicting detach. Native
binding timing still includes program/preallocation work; this refactor does
not remove preparation costs from reported durations.

## Qualification

Commands use `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2`. Receipts contain exact
argv, output, UTC start/finish timestamps and exit status.

| Check | Result |
| --- | --- |
| Final all-feature materialized-path selection, serial | 15 passed |
| All-feature runtime backend, serial | 873 passed, 28 ignored |
| Full all-feature runtime library | 1770 passed, 3 failed, 28 ignored |
| Final runtime + KFD all-feature/all-target strict Clippy | Passed |
| Final runtime no-default-features check | Passed |
| Final workspace format check | Passed |
| Source signatures and lint-only delta | Passed |

The backend/full suites ran at `4ce9a18dc`; the focused/static checks ran again
at `4e7c98c0`. Selections overlap and must not be added as distinct test totals.
The full KFD suite, runtime-model suite and doctests were not rerun.

Three full-runtime failures remain unwaived at `authorized_execution.rs:1317`,
with `InspectSocket(PermissionDenied)` / `Operation not permitted`:
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`.
No admission guard was bypassed and no failing test was skipped.

The archive retains the initial profiler-fixture failure, two old detach-oracle
failures, strict Clippy failure and corrected reruns. The detach fixture now
requires repeated rejection to preserve metadata rather than treating its
premature removal as successful cleanup. Its explicit cleanup removes only
malformed metadata, not a native owner. Final qualification was serialized on
unchanging sources.

## Evidence Limits

New public-admission scripts cover 44 lane/origin/fault combinations, four
healthy first-submit controls and 22 actual unrepaired-Drop subprocesses.
They check accepted recipe identity, descriptors/writebacks, allocation/module
custody, exact successor/event retains, result reservation, lease, content,
publication counts and terminal reentry. The Drop cases alternate origins;
they are not the full 44-case matrix. Successful lifecycle and retry/cancel
coverage also remains in the existing materialized cancellation suite.

These use real logical admission but scripted submission outcomes and DataSpec
bytes, not native batches. Scripted generation 7 is provenance-storage coverage,
not proof of a preceding native recycle. The shared metadata helper adds 16
one/three-descriptor sequences checking exact prior buffer identity and content
across modeled overwrite-prefix and outer-close errors/unwinds, and success-only
retirement. The detach wiring test is source-order evidence. No test here proves
that every injected outer-close outcome is currently reachable in native
execution, nor qualifies the composed runtime-to-native path on a GPU.

MI300X access failed resolving `sharkmi300x-1`; no remote files or jobs were
created. Both source pushes failed resolving `github.com`, for `origin`
(`harsh-nod/fe2o3`) and `upstream` (`powderluv/fe2o3`). There is no native execution,
formal-refinement or performance claim in this packet.

## Next Work

Ordinary Prepared retry still temporarily removes Active and restores Prepared
after an indeterminate attempted submission. Terminal gating prevents current
reuse, but explicit continuously indexed attempt custody remains unfinished.
Reuse a submit-only Binding transition without repeating preparation, binding,
materialization or Pending settlement. The scripted retry path should use the
same transition, reset publication timing and terminalize profiling unwinds.

Coupled native binding/cancellation/cache-rebind qualification, allocator-failure
injection, Context composition, formal native refinement, protected Worker and
matched hardware performance remain separate open gates.

`receipts.tar.xz` contains the frozen campaign and read-only swarm review notes.
Verify it with `sha256sum -c receipts.tar.xz.sha256` in this directory.
