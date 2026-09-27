# Shared Disposal Group Checkpoint

CPU developer evidence, not overlapping queued admission, formal proof or parity.

Source: `aa8b5070e76cf662dfc94a7c042b4285ddc792a8`.
Parent: `d291588cd99f75409ee9f0b82222436fdb9d14a2`.
The source commit has a verified SSH signature for `harmenon@amd.com`.

## Implemented

Context now discovers closed Ordinary writer groups and freezes complete writer
rosters plus a unique allocation-receipt union before native release. An exact
allocation-to-group/ordinal index resumes partial disposal without rediscovering
removed public handles. Pending co-owners refuse recoverably; corrupt custody
quarantines. Unique receipts and original Context roots retain all credits until
the whole native union and model group commit.

Flat model evidence uses checked packed headers and borrowed member/union slices.
Nested and flat forms share the same validation and commit bodies. Finalization
does not allocate evidence storage after native effects. Group commit/refund
progress remains visible through faults, and retained-writer diagnostics include
Context roots whose model slots disappeared after a committed prefix. Generated
and synchronous disposal remain on their separately authenticated scalar paths.

Frozen roots and quiescence markers are immutable during the group lifetime;
public submission metadata is independent. Retries check exact receipts and the
leader binding. Finalization audits every root, avoiding a full roster scan on
each allocation retry. These are source/test properties, not timing claims.

## Validation

- Model suite: 1,070 passed, 19 existing ignored; 29 doctests passed.
- Flat/nested rejection equivalence and full-state comparisons for success and
  every injected error/panic boundary; malformed packed ranges, capacity, empty
  segments, terminal precedence and fixed storage checks.
- Context disposal suite: 20 passed, including seven new multi-root tests.
- All 24 four-allocation release orders with/without submission metadata, native
  retry, pending refusal, index corruption, reused backend handles, credit-refund
  prefixes and missing-model-root custody diagnostics are covered.
- Full serial runtime suite: 1,685 passed, 3 failed, 28 existing ignored.
- Runtime doctests: 52 passed.
- Strict all-feature/all-target Clippy, minimal model check, formatting and
  whitespace checks: passed.

The three runtime failures remain the recorded authorized-execution tests
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`, at
`authorized_execution.rs:1317` with `InspectSocket(PermissionDenied)`.
They are not skipped, waived or counted as passing.

## Remaining Boundary

Runtime multi-root tests use genuine disjoint Unknown submissions with private
multi-seed freeze. They do not qualify overlapping queued Context components.
Public queued-write admission remains closed. Queue capacity, availability guards,
event authentication and queued-writer reconciliation are next; successor input,
partial-write and ReadWrite support, shared-body proofs and native qualification
also remain open. See the [integration plan](../../runtime-successor-writer-plan-v1.md).

No Verus, mutation, relocation or source-bound proof campaign ran. No GPU or
matched HIP/HSA comparison ran. Both GitHub and MI300X probes failed DNS; no remote
jobs or files were created. The full parity goal remains open.

`receipts.tar.xz` retains commands/exits, final/development logs, source digests,
toolchain identification and source signature verification. Archive SHA-256:
`df7bacc8ca6c3340343f414fd5a95539ff042090c7e9c6e2eca9a633f5b7add9`.
