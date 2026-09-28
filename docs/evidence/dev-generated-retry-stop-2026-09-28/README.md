# Generated Retry/Stop Retirement

Development implementation and CPU qualification only. This does not close
A1/A2, #182, protected Worker application authority, formal correspondence,
HIP/HSA parity or matched performance gates. Accepted milestones are unchanged.

## Source

- Base: `453a5dfaee96ca31673e59244aa30dc3171ae0de`.
- Production fix: `3f69dcf058e49a8fd8210001a0ebdf493e2a12f3`.
- Qualified source: `e77f8524379bf8a2e06bdcbfdfced303687908b2`.
- Qualified tree: `bce8976f9936441ea956a828b16a61727cbc0693`.
- Both source commits have verified SSH signatures and sign-offs.

The second commit corrects the positive metadata fixture's host-control state;
it does not change production behavior. Exact-source continuity is recorded.
The unrelated untracked `dev-owner-inspection-2026-09-23` directory was preserved.

## Fix

Generated ISSUE previously restored `Ready` after a classified native retry.
The cancelled epoch retained reservation history, so Stop selected pristine
abort, which correctly rejected that owner. Stop could not dispose this
definitely unpublished recipe.

`RetryReady` now selects a separate cancelled-only abort. Native admission checks
the exact queue, ordinary control, releasable completion state, vacant epochs,
nonzero reservation history and absence of recycled/detached-predecessor history.
Pristine admission is unchanged. Both paths share rooted control cleanup,
closing retake, DATA return and terminal parent transport.

The unpublished continuation preserves the queue, exact next generation,
capacity and account identity. Detach refunds the epoch table; rebind reacquires
its debit, validates historical queue identity and mints a fresh occurrence.
Counter exhaustion permits disposal but rejects rebind before consuming custody.
No recycled generation or completion receipt is fabricated. Generic observation
never reissues retry-ready work; unknown handoffs cannot retire immediately.

## Qualification

Commands use `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2`. The archive contains exact
argv, stdout/stderr, UTC start/finish times and exit status for each completed run.

| Final Check | Result |
| --- | --- |
| KFD all-feature library `pristine`, serial | 84 passed |
| KFD all-feature library `rebind`, serial | 63 passed |
| Runtime all-feature library, complete run | 1758 passed, 3 failed, 28 ignored |
| Runtime all-feature library `kfd_backend`, serial | 861 passed, 28 ignored |
| KFD isolated credential-bound telemetry test | 1 failed: `SocketAdmission` |
| Runtime + KFD all-feature/all-target strict Clippy | Passed |
| Runtime no-default-features check | Passed |
| Workspace format check | Passed |
| Signatures and source continuity | Passed |

Selections overlap; their counts are not distinct-test totals. The full KFD
suite, runtime-model suite and doctests were not rerun in this packet. Prior
packet results are not substituted for current-scope execution.

The three runtime failures are in `authorized_execution::tests`:
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`. Each fails with
`InspectSocket(PermissionDenied)` at socket admission. The separate KFD failure
is `target_debug_telemetry_v2::tests::credential_bound_channel_accepts_typed_failure_before_publication`.
These remain unwaived; no admission checks or tests were weakened or skipped.

The archive preserves an initial test assertion compile error, the incorrect
metadata fixture's failed run and their corrections. An earlier serial runtime
run lost its execution handle without a terminal receipt; it is explicitly
incomplete, not a pass. The final complete run above supersedes it for scope.

## Evidence Limits

Lower CPU fixtures execute actual reserve/cancel state transitions, cleanup,
ledger settlement and primary/AUX terminal transport with injected native
operations. Runtime receipt tests use drop-counted tokens; backend positive
release tests use metadata-only retirement. Foreign-key rebind mutation is a
defensive private-state corruption test, not a supported public queue migration.

These tests do not execute the protected generated Worker-to-native path or
prove a native retry flowing through the runtime routing and Stop on hardware.
The direct three-way generated lane routing is reviewed source, not GPU evidence.
No formal proof or matched HIP/HSA benchmark was added.

MI300X access failed resolving `sharkmi300x-1`; no remote jobs or files were
created, so there was no shared-machine cleanup to perform.

`receipts.tar.xz` contains the frozen raw campaign and read-only review notes;
verify it with `sha256sum -c receipts.tar.xz.sha256` in this directory.

## Remaining Work

Ordinary `MaterializedPrepared` cancellation still returns `TooLate`. Its owner
may have newly bound cancelled-only history or reuse of a previously recycled
attached recipe. Both need correctly distinguished retirement and qualified
allocation/module, reservation, dependency and lane-lease settlement. They must
not both be routed through the new strict cancelled-only primitive. The raw
review notes map the production and test boundaries for that next step.
