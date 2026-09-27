# Bounded Queued Writer Model Checkpoint

Developer evidence, not runtime parity or GPU/performance qualification.

Source commit: `9ff331dace8b4a47cc156aa92930fbf85d405626`.
Parent: `a78072fbe2a83fc71f992efd4237f146b77f81c5`.
The source commit has a verified SSH signature for `harmenon@amd.com`.

## Implemented

`ContextQueuedWriterJournalV1` adds a bounded exclusive destination owner over
the unchanged single-pending-writer journal. It reserves complete canonical
rosters, tracks exact predecessor identities, reserves epoch headroom, and
activates only from actual reconciled success. Queued cancellation unlinks only
its own members; descendants cannot inherit a cancelled writer's predecessor.
Unknown is fail-stop. Availability APIs enforce outer custody without exposing
an inner dereference. Shared borrowed preflights preserve inner error precedence.

Fixed-capacity arenas require no allocation after construction. Admission,
activation and settlement make O(k) indexed roster passes, not queue-depth scans.
Tests audit both indexes, arena conservation and stable storage addresses/capacity.
This is a source/property claim, not a HIP/HSA timing comparison.

## Validation

- Model suite: 1,054 passed, 19 existing ignored; 29 doctests passed.
- Twenty new queue test groups, including 384 four-writer outcome/order cases.
- Runtime all-feature library: 1,673 passed, 3 failures, 28 existing ignored.
- Runtime all-feature doctests: 52 passed.
- Strict all-feature/all-target Clippy for model and runtime: passed.
- Minimal model check, formatting and whitespace checks: passed.
- Existing mixed-acquisition Verus root: 738 verified, 0 errors after shared
  preflight extraction. This does not prove the new queued owner. No new mutation,
  relocation or authenticated proof campaign was run; historical pins remain unchanged.

The three runtime failures remain the previously recorded authorized-execution
tests `cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`. They fail at
`authorized_execution.rs:1317` with `InspectSocket(PermissionDenied)`.
They are not skipped, waived or counted as passing.

## Remaining Boundary

Context still rejects pending-output successors. Before enabling them, it needs
exact event/dependency authentication, retained queued rosters, all availability
guards, and safe Unknown-group disposal. Per-writer disposal is insufficient
when Active-Unknown and queued-Unknown writers share storage. Queued-output
reads, partial writes, ReadWrite, full outer-owner proofs and matched native
hardware qualification remain open. See the
[successor-writer plan](../../runtime-successor-writer-plan-v1.md).

GitHub and MI300X probes failed DNS resolution. No remote files/jobs were created.
No GPU run, copy-performance measurement or new HIP/HSA parity claim is made.

`receipts.tar.xz` retains raw final/development logs, exact commands, source and
verifier digests, toolchain identification and signature verification. The source
commit identifies the full code tree; these receipts are developer regression
evidence, not a replacement for the project's source-bound proof campaigns.

Archive SHA-256:
`92eff6d23d78e08bd45666afccbeb1654ebfe9caec5cfa0fdac3d5fc766b69ca`.
