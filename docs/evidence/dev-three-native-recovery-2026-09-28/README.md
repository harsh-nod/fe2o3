# Exact-Three Native Receipt Recovery Controls

CPU developer evidence using actual lower-layer receipt types and registered
memory fixtures. Completion observations are injected. This is not GPU
execution, executable formal refinement, A1/A2 or #182 closure, or HIP/HSA
behavioral/performance parity.

Baseline: `37cdd7513fed18126c7f9dcf2ef82c2a86cc9dbc`.
Implementation: `71f939ebb2d8cd3197b2a5c016c982746a524408`.
Final source: `9f9e4de8196efaad3ce8a9b6f5701de92003efda`.
Source tree: `44779501984b8872a6151eb264d1022e250bcc19`.

[Frozen receipts](receipts.tar.xz), [SHA-256](receipts.tar.xz.sha256).

## Coverage

The registered-memory fixture now supports both Published and Recycled states.
No public receipt constructor, test-authority Cargo feature, or surrogate native
outcome tag was added.

- Foreign-session public poll returns the original published receipt. Both
  populated sessions retain their exact owner/lease, arena, control, attachment,
  initialization, memory and detached-ledger snapshots. Epoch and completion
  occurrence match before and after rightful-session Pending continuation.
- Foreign-session public detach returns the original recycled receipt without
  changing either session. Binding and recycle observation are checked, then
  both sessions finish through their original queues and release all resources.
  The recycle observation itself contains only packet count; exact owner/lease
  snapshots and successful original-session continuation provide the other checks.
- Genuine outstanding ComputeWrite reservations reject aggregate retirement at
  each of the three ordinals. All owner/frontier identities, ledgers, attachments,
  digests and initialization remain unchanged; no earlier member retires.
  Cancelling the actual reservation permits retry. Success clears all settled
  entries, preserves read digests and write initialization, and releases data
  and control. Reservation without the dependency frontier is a negative control.

Three new test functions join three existing restore tests, including the
lower-owned terminal-custody rejection cases. All six pass on the final source.
The new frontier identity observer is crate-private and test-only.

Runtime poll-failure, detach-failure and retirement handling now use concrete
private functions over the existing KFD types. Returned receipts are retained
before error formatting; absent recovery remains lower-owned; retirement
`Err(Self)` is reindexed as the exact Detached roster. This is a reviewed
production refactor, not an end-to-end test passing genuine lower failures
through runtime custody callbacks. Scripted runtime tests remain separate.

## Qualification

Final checks ran serially with `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2`, without
source edits. The archive includes command lines, full logs, statuses, source
signatures, post-test continuity, nonfinal attempts and two read-only reviews.

| Check | Result |
| --- | --- |
| Full KFD unit suite, all features, serial | 1,687 passed; 1 failed |
| Runtime backend selection, all features, serial | 858 passed; 28 ignored |
| Full runtime unit suite | 1,755 passed; 3 failed; 28 ignored |
| Runtime model unit suite | 1,080 passed; 19 ignored |
| Integration tests | 11 passed; 3 hardware tests ignored |
| Doctests | 52 runtime and 29 model passed |
| Strict KFD/runtime all-feature/all-target Clippy | Passed |
| No-default runtime, workspace formatting | Passed |
| Signed source and exact source continuity | Passed |

The complete suites are **not green**. KFD's
`credential_bound_channel_accepts_typed_failure_before_publication` fails at
`target_debug_telemetry_v2.rs:1173` with `SocketAdmission`. Its unchanged V2
adapter wraps the shared socket validator's error. Runtime retains the three
previously recorded `InspectSocket(PermissionDenied)` failures at
`authorized_execution.rs:1317`: `cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`.

A separate process successfully creates an AF_UNIX SEQPACKET pair, but this
environment denies `SO_DOMAIN`, `SO_TYPE`, `getpeername` and `SO_PEERCRED` with
`EPERM`. That identifies an environmental restriction; it does not waive the
four failed tests or establish telemetry qualification in another environment.
No admission checks were weakened and no permission escalation was requested.

## Remaining Boundaries

Genuine lower failures still need end-to-end runtime/native coupling. Lower
detach and retirement also have an unqualified internal-panic boundary while
their roster is local: normal tails are preflighted and callback/allocation-free,
but this packet does not establish retained ownership for an internal panic.
Review found no healthy returned-error owner loss.

Generated execution, Context correspondence, aggregate memory bounds, machine
refinement and matched HIP/HSA performance remain open. A separate source audit
identified a generated retry/Stop mismatch: retry restores Ready, but Ready
retirement calls pristine abort after native reservation history exists. The
next repair needs cancelled-only unpublished provenance and history-aware abort,
not weaker pristine validation or fabricated recycled authority. Details are
archived in `next-action-review.md`; this packet does not implement that repair.

MI300X hostname resolution failed twice. No remote resources were created;
cleanup removed only this task's obsolete local default-feature test executable.
Accepted lane checkpoints and A1/A2 status are unchanged.
