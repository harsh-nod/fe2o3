# Closed Unknown Group Disposal Checkpoint

CPU developer evidence, not formal refinement, GPU qualification or parity.

Source commit: `da58d3ed57dff47f24ebbdd134f7c9a4cb46efd6`.
Parent: `d0bee64ebda0a0e51c33e028b15602c2fcac01a3`.
The source commit has a verified SSH signature for `harmenon@amd.com`.

## Implemented

The queued-writer model can destroy a closed group of Unknown writers sharing
allocations. It checks exact canonical rosters, both directions of closure,
queue identities, destination readers and selected writers' input leases outside
the group. Neighbor inspection supports component discovery without allocating.
Queued-only destinations and zero-destination roots are covered.

Group commitment retains outer custody through every inner destruction step.
Rejected preflight does not mutate state. An error or unwind after commitment
starts leaves a terminal owner, including if earlier inner steps succeeded.
Destruction never propagates predecessor success. Validation has bounded scans
and binary searches; no heap allocation occurs after owner construction. These
are source/test properties, not measured performance improvements.

Context separately retains an Ordinary writer's conclusive quiescence after
submission metadata is released. Initial and final disposal require that marker.
Quarantine cannot create it; a later quarantine preserves an already-earned
marker without allowing cleanup. Generated writers keep their separate receipt
validation. Context does not yet use the queued owner or its group disposer.

## Validation

- Model suite: 1,067 passed, 19 existing ignored; 29 doctests passed.
- Thirteen new group test functions cover shared and branching ownership,
  omission/corruption, input readers, recycled identities, exact inner commit
  prefixes under error/panic, terminal API refusal and neighbor discovery.
- Runtime parallel library run: 1,675 passed, 4 failed, 28 existing ignored.
- Runtime serial library rerun: 1,676 passed, 3 failed, 28 existing ignored.
- Runtime doctests: 52 passed.
- Strict all-feature/all-target Clippy for both crates: passed.
- Minimal model check, formatting and whitespace checks: passed.

Both runtime runs failed the previously recorded authorized-execution tests
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned` at
`authorized_execution.rs:1317` with `InspectSocket(PermissionDenied)`.
They are not skipped, waived or counted as passing.

The parallel run also failed
`worker::tests::v4_flush_deadline_failure_reaps_and_seals_the_worker` on its
75 ms scheduler-tolerance assertion. The full serial rerun after concurrent
Clippy compilation finished passed it. Both logs are retained; its cause is not
proven and the first failure is not erased. No timeout or Worker code was changed.

## Remaining Boundary

Context shared-group discovery, unique native allocation receipts and deferred
credit refunds remain required before opening queued-write admission. Public
successor writes, queued-output reads, partial writes, ReadWrite, the new owner's
shared-body proofs and matched native qualification remain open. See the
[successor-writer plan](../../runtime-successor-writer-plan-v1.md).

No Verus, mutation, relocation or authenticated proof campaign ran for this
checkpoint. Existing inner proofs do not prove the new outer group transition.
GitHub and MI300X probes failed DNS resolution; no remote jobs or files were
created. No new GPU, copy-performance or HIP/HSA parity claim is made.

`receipts.tar.xz` retains final and development logs, exact final commands and
exit statuses, source digests, toolchain identification and signature verification.
This is developer regression evidence, not a replacement for source-bound proofs.

Archive SHA-256:
`d4ed75d5b1a7f89abfc86963f03264142cd120e8406213eb47e4e53b0680e221`.
