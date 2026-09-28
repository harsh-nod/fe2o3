# Indexed Three-Binding Completion Custody

CPU developer evidence, not GPU execution, executable formal refinement,
A1/A2 or #182 closure, HIP/HSA parity, or matched performance evidence.

Baseline: `69be2b64ed51a7317f98deda852b814139698972`.
Implementation: `b74c074cbb379c023c8e90e6dfdf1c38c5f4657f`.
Qualified source, including regression expectations:
`d4bfdea2a949ecf80351f8667360b6215a6a9a2f`.
Source tree: `ff3234814fc4824fe6eb2ffe7c916ac92c32f232`.

## Change

The old exact-three completion path removed Active before consuming native
poll/recycle and detach receipts, restoration, and detached-control release.
The pre-fix public scripted slot-mismatch regression returned a terminal error
with Active missing. This change preserves both logical and data custody.

Preparation reserves one boxed completion root before extracting inputs. Armed
publication carries the empty root, which publication and cancellation validate
against exact admissions and Reserved state. Scalar and three-binding completion
share a fully typed phase carrier, not an untyped native receipt abstraction.

Public poll and wait move only execution into the indexed root. Pending restores
the original published variant. Returned published, recycled, detached and retired
owners are reindexed before validation, diagnostics or reporting. Native optional
recovery distinguishes exact returned custody from lower-owned terminal custody;
retirement rejection retains the exact completed roster. Unwind preserves Active,
poisons the backend, and resumes the original panic.

Restoration checks the entire three-input roster before changing any slot.
Completed prefixes are record-owned; unfinished inputs and shells remain indexed.
The full logical commit is preflighted before detached-control release. A release
failure keeps restored data, Active and uncommitted accounting, without claiming
the lower control resources are unchanged or retryable. Success settles accounting
and records completion before reporting, with Active removed last. Post-commit
reporting unwind retains the exact committed prefix instead of undoing settlement.

## Coverage

Eight new test groups use public scripted admission and three explicit origins:
authenticated H2D, initialized storage, and replay from a prior completed launch.
Replay warmup uses a separate stream so copy-event dependencies and FIFO retains
are unchanged. Actual admitted origins are asserted, not inferred from setup.

- Successful poll/wait and queued-successor completion preserve all three owner
  IDs, byte addresses/digests, preallocated box identities and read metadata.
  Counted public completion and expired wait allocate zero. These fixtures do
  not simulate kernel arithmetic or establish GPU-result correctness.
- Pending poll and expired wait preserve the exact root, shells and accounting.
- Reservation refusals precede input extraction and preserve original storage;
  initialized-storage preparation can already have performed promotion. Healthy
  Prepared cancellation disposes the empty root and releases all fixture owners.
- 126 subprocess cases cover three observation-failure labels, retirement unwind,
  all three slot/shell/effect ordinals, each restoration prefix, commit-roster
  rejection, missing/error/unwinding control release, and post-commit report unwind,
  across three origins and both poll/wait entry points.
- 36 subprocess cases reject malformed Reserved roots before publication or
  cancellation, retaining the original prepared input, shell and logical ledger.

Every fault child requires an inspection marker and actual SIGABRT on unrepaired
backend Drop, with core dumps disabled. Successor recipes, FIFO, exact accounting
and live owner counts are inspected. Profile-unwind oracles expect the completed
commit prefix; they do not incorrectly assert that all accounting is unchanged.

## Qualification

The archived `qualified-*` commands ran sequentially on the signed source above
with `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2`, with no concurrent source edits.

| Check | Result |
| --- | --- |
| Runtime backend selection, serial | 492 passed |
| Full runtime unit tests | 1,755 passed, 3 failed, 28 ignored |
| Runtime model unit tests | 1,080 passed, 19 ignored |
| Integration tests | 11 passed, 3 hardware tests ignored |
| Doctests | 52 runtime and 29 runtime-model passed |
| Lower KFD persistent selection | 166 passed |
| Lower KFD retained-control selection | 27 passed |
| Strict all-feature/all-target Clippy | Passed |
| No-default runtime check, workspace formatting | Passed |
| Signed-source verification and source continuity | Passed |

The full command exited 101. Three existing authorization/telemetry tests fail
at `authorized_execution.rs:1317` with `InspectSocket(PermissionDenied)`:
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`.
These failures are not waived; the complete suite is not green.

The archive retains the pre-fix failure and nonfinal attempts. Those include
compiler/Clippy findings, an overstrict promotion-step oracle, and older producer
tests expecting terminal input storage or a restoration-time diagnostic. The
final tests instead assert exact retained Published owners before consumption.
Only the `qualified-*` campaign establishes final-source qualification.

## Open Boundaries

The scripted Poll/Detach/Retire labels share one owner-retaining branch. They do
not execute native receipt recovery, lower-call unwind, or real frontier rejection.
Lower KFD tests are separate evidence, not coupled runtime/GPU qualification.
Next are genuine returned-receipt poll/detach rejection controls, atomic exact-three
retirement rejection, and production-shared concrete outcome-reducer witnesses.

Native control/profiling faults, generated execution, Context formal composition,
aggregate memory, production refinement and matched HIP/HSA performance remain
open. Source review confirms initialized-storage retirement produces accepted
replay storage; that audit is not an executable proof. The root adds a pre-bind
allocation, so zero counted completion allocation is not a zero-allocation launch
or end-to-end speedup claim. Accepted checkpoints and A1/A2/parity are unchanged.

MI300X hostname resolution failed; no remote files or jobs were created. Cleanup
removed only named stale test executables from this task's local build tree.
