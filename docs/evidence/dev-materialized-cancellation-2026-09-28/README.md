# Ordinary Materialized Retry Cancellation

Development implementation and CPU qualification only. Accepted milestones,
A1/A2, #182, protected Worker application, formal correspondence and HIP/HSA
behavioral/performance parity remain unclosed.

## Source

- Production and tests: `d512b0310f1407d3afc9328864e9cf6908df6f0e`.
- Final test-only lint correction: `2816b6caf62047015a80cfcc4575988050f0f448`.
- Final source tree: `75bafe7b72ecaac24eff4a028c7be3f632037363`.
- Both commits have verified SSH signatures and sign-offs.

The second commit removes one unnecessary explicit `drop` in a lower test.
Runtime sources are identical to the revision used by the full runtime and
serial backend runs; the archive records that continuity. Final lower tests
and strict static checks use the second revision. The unrelated untracked
`dev-owner-inspection-2026-09-23` directory was preserved.

## Behavior

Ordinary `MaterializedPrepared` cancellation previously returned `TooLate`.
It now carries explicit preparation provenance instead of inferring history
from performance classification:

- Newly bound recipes use cancelled-only abort, including recipes rebound from
  previously detached DATA.
- Reused attached recipes authenticate their captured prior recycled generation
  and detach that exact recipe. That generation is not completion of the
  cancelled launch.

The selected primary/AUX Active owner remains indexed through retirement.
Cancellation preflights the exact recipe, ordered descriptor projection,
allocation/module retains, result reservation and lane lease. Returned DATA is
rooted before the outer lane loan closes, then validated for initializedness,
kind, size and canonical native alignment. Host-visible native alignment is
4096 even when the logical requested alignment is smaller.

Only successful retirement releases this submission's custody and records
unpublished cancellation. Returned native buffers may remain in the resident
cache with the current descriptors. No writebacks, dirty extents or dispatch
publication/completion events are applied. Queued successors retain their own
ownership; ordered-only successors continue, while explicit success dependents
fail before publication. `Cancelled` is the cancellation result; later
`Failed(-2)` alone is not initialization or NoEffect authority. Nonempty pipeline
state prevents retirement, and published submissions remain too late.

## Qualification

Commands use `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2`. Completed receipts include
exact argv, stdout/stderr, UTC start/finish timestamps and exit status.

| Check | Result |
| --- | --- |
| Runtime materialized cancellation groups, serial | 8 passed |
| Full all-feature runtime library | 1766 passed, 3 failed, 28 ignored |
| All-feature runtime backend, serial | 869 passed, 28 ignored |
| Final all-feature KFD recycled-detach selection, serial | 20 passed |
| Final all-feature KFD rebind selection, serial | 64 passed |
| Runtime + KFD all-feature/all-target strict Clippy | Passed |
| Runtime no-default-features check | Passed |
| Workspace format check | Passed |
| Source signatures and continuity | Passed |

Selections overlap; counts are not distinct-test totals. The full KFD suite,
runtime-model suite and doctests were not rerun in this packet.

The three full-runtime failures remain unwaived:
`authorized_execution::tests::cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`. They fail at
`authorized_execution.rs:1317` with `InspectSocket(PermissionDenied)`.
No admission guard was bypassed and no failing test was skipped.

The archive retains initial fixture compilation, profile lifecycle, dependency
contract and ring-configuration failures, plus the corrected reruns and Clippy
correction. One final lower run lost orchestration cell 128 without a terminal
receipt; it is explicitly incomplete, not a pass. Its separately labelled
replacement completed with 20 passes on the same final source.

## Evidence Limits

Runtime tests use public submit/flush/cancel/event APIs with scripted publication
and retirement. They exercise both origin classifications, primary/AUX lanes,
writeback preservation, queued successors, corrupt custody, inert terminal
reentry and 12 actual unrepaired-Drop subprocesses. They retain DataSpec bytes,
not fabricated native buffer or completion authorities. Scripted generation 7
does not prove a preceding native recycle. The nonempty-pipeline test is a
defensive malformed-state control, not a reachable Prepared/published-successor
sequence. Queue/allocation profile setup events are fixture-owned.

Separate lower fixtures execute genuine recycled owner, reserve/cancel, detach
and rebind transitions with injected native memory operations. They check exact
prior-generation return on each lane, and detach/rebind followed by another
reserve/cancel/abort with preserved DATA identities, layouts, initialization and
continuation queue/counter. These do not establish the composed runtime-to-native
path, native cache reuse, protected Worker execution, formal correspondence or
matched HIP/HSA performance. No GPU execution or performance claim is added.

MI300X access failed resolving `sharkmi300x-1`; no remote files or jobs were
created. Both source push attempts failed resolving `github.com`, for `origin`
(`harsh-nod/fe2o3`) and `upstream` (`powderluv/fe2o3`).

## Next Work

Ordinary initial binding/reuse/first-submit still precedes Active installation.
Introduce a distinct indexed binding/publication phase before its first mutating
native action, preserve current recycled metadata and prepared descriptors on
failure/unwind, and retain the one-time Pending-to-Active handoff. A never-
reserved recipe must not be labelled as this confirmed-retry Prepared state.
Native cancellation-to-cache/rebind qualification, allocator-failure injection,
Context composition, formal refinement and matched hardware performance remain
separate open gates.

`receipts.tar.xz` contains the frozen campaign and read-only swarm review notes.
Verify it with `sha256sum -c receipts.tar.xz.sha256` in this directory.
