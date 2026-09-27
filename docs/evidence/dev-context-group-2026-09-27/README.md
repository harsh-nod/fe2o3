# Context Ordinary Group Integration Checkpoint

CPU developer evidence, not shared queued admission, formal refinement or parity.

Source: `e5a30e9e531d677a4a56b069fa950eece4ab118b`.
Parent: `2ecfa57e31c55fa9cf9b8e9c23aa5e16511d2694`.
The source commit has a verified SSH signature for `harmenon@amd.com`.

## Implemented

Context now owns `ContextQueuedWriterJournalV1`. Queued admission remains closed;
one preallocated member per allocation preserves current disjoint active-roster
capacity. Test accessors expose the wrapper, not an inner dereference bypass.

Ordinary Unknown disposal now preflights and commits the model's singleton writer
group. Existing per-allocation native receipts retain the original complete roster
and credits until all native releases and the whole model group commit succeed.
All model errors after native disposal quarantine Context. Generated and
synchronous disposal retain their scalar, separately authenticated paths through
the wrapper, including removal of its outer member records.

Empty stable/producer read arenas now skip disposal scans using their retained
counts. Nonempty arenas still check selected consumers' outstanding inputs,
including inputs outside the destination union. This is valid-state algorithmic
reasoning, not a timing or HIP/HSA performance claim.

## Validation

- Disposal tests: 13 passed, including new orphaned outside-input rejection before
  native release and eight full member-capacity disposal/reuse cycles.
- Existing disposal permutations now assert outer-member retention until complete
  commitment and full member-capacity return afterward.
- Model suite: 1,067 passed, 19 existing ignored; 29 doctests passed.
- Full serial all-feature runtime suite: 1,678 passed, 3 failed, 28 existing ignored.
- Runtime doctests: 52 passed.
- Strict all-feature/all-target Clippy, minimal model check, formatting and
  whitespace checks: passed.

The three failures remain the previously recorded authorized-execution tests
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`, at
`authorized_execution.rs:1317` with `InspectSocket(PermissionDenied)`.
They are not skipped, waived or counted as passing. Worker deadline tests passed
in this serial run; the prior checkpoint retains its parallel timing failure.

## Remaining Boundary

Shared Context group discovery, frozen unique-allocation receipts and group credit
accounting must precede public queued-write admission. Finalization needs an
allocation-free retained-roster view, not self-referential borrowed evidence.
Successor support for queued-output reads, partial writes and ReadWrite, plus
exact event authentication, remains open. See the
[integration plan](../../runtime-successor-writer-plan-v1.md).

No Verus, mutation, relocation or source-bound proof campaign ran. Existing inner
proofs do not prove the wrapper or group composition. GitHub and MI300X probes
failed DNS resolution; no remote files or jobs were created. No hardware ordering,
copy-performance, or HIP/HSA comparison is claimed.

`receipts.tar.xz` retains exact commands/exits, final and development logs, source
digests, toolchain identification and source signature verification.

Archive SHA-256:
`c3480f9c5692aeff385762bb1bfef736cdeabb1ffe9f59209675151e600782bb`.
