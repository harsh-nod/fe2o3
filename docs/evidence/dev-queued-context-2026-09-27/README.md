# Queued Context Launch Checkpoint

CPU developer evidence only, not formal proof, native qualification or parity.

Production source: `cb30cb0c817a461dbb01d117496038402c715620`.
Final source, including terminal-mock test retention:
`c09b111022ebf356303a000a23153371754b8bd4`.
Parent: `117791de9814319b80a550c022c128abfc65768f`.
Both source commits have verified SSH signatures for `harmenon@amd.com`.

## Implemented

Public producer-aware Context launches can now queue full-allocation Write
outputs behind the exact latest Ordinary writer named by an explicit dependency
event. Admission validates the complete alias/destination roster and preflights
bounded writer/member/epoch capacity before IDs or backend effects. An explicit
member-capacity constructor supports more queued members than allocations.

Context retains exact queued roots and reconciles producer success before
settling consumers through the existing bounded completion planner. Cancelled
predecessors leave descendants blocked; unknown effects retain original custody.
Whole-writer, multi-parent and mixed busy/idle output rosters are covered at the
Context/model boundary. The native backend's R/R/W profile remains narrower.

Availability checks exclude queued allocations from host access, ordinary
writers, unsupported input reads and early disposal. Closed Unknown groups use
the existing shared-disposal protocol. Expected generated-retirement refusal
stays recoverable before native effects, while corrupt shell custody still
quarantines. Full shell-roster validation does not stop at the first busy shell.

## Validation

- Fourteen new tests: twelve queued-launch scenarios, member-constructor bounds,
  and generated-retirement availability/corruption. One prior rejection test now
  verifies supported full-output queuing.
- Runtime unit tests: 1,699 passed, 3 failed, 28 ignored.
- Runtime integration tests: 10 passed, 1 failed, 3 ignored hardware tests.
- Runtime doctests: 52 passed.
- Unchanged model: 1,070 passed, 19 ignored; 29 doctests passed.
- Strict all-feature/all-target runtime/model Clippy, formatting and source
  signature checks passed on the final source.

The three unit failures are authorized-execution socket-inspection tests:
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`, each failing at
`authorized_execution.rs:1317` with `InspectSocket(PermissionDenied)`.

The R57 source gate also fails. Its pinned v1 policy requires rejection of A/B/C
before initial C H2D; earlier commit `04a3b5cc5` changed the example to reject a
mixed-memory A/B/HostVisible output after DeviceLocal initialization changed.
This checkpoint preserves the failing assertion and policy rather than silently
substituting the expected string or rewriting historical evidence. A versioned
qualification repair remains required. No failing gate is waived or skipped.

Development failures, including disk exhaustion, an intermediate terminal-mock
Drop abort and a misspelled package command, are retained in the archive. Only
this worktree's disposable incremental/example build output was cleaned.

## Remaining Boundary

Queued-output reads, partial/ReadWrite output semantics, broader native launch
profiles, shared-body proofs and MI300X integration remain open. The existing
inner-journal proof does not prove this outer queued Context composition.
No Verus, source-bound proof campaign, GPU test or matched HIP/HSA benchmark ran.
MI300X DNS failed; no remote jobs or files were created. The full parity goal
remains open. See the [integration plan](../../runtime-successor-writer-plan-v1.md).

`receipts.tar.xz` contains commands/exits, final and development logs, source
digests, toolchain identification and source signature verification. Publication
is attempted separately after freezing this evidence. The companion SHA-256
file identifies the archive.
