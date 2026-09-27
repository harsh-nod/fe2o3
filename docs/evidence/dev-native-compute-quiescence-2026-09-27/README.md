# Native Compute Quiescence Across Dependency Chains

Development checkpoint, not #182 closure, A1/A2 acceptance, full HIP/HSA parity,
concrete formal refinement, hardware qualification, or performance acceptance.

Implementation: `f1ec522a4ed217d0a293d5dcb501a41c72e8000a`, SSH-signed and verified.
The primary agent implemented and tested; two read-only agents reviewed
ownership, mixed FIFO/copy behavior, integrity, capacity, fixtures, and proof scope.

## Implementation

- Pending compute independently retains the actual transitive Compute owners of
  its bindings. Owners must authenticate against real pending/active records and
  be reachable through immutable retained native dependency/FIFO/quiescence
  edges. Unrelated owners reject; broken retained edges terminalize before commit.
- The new roster requires exact quiescence, not success. It does not increase
  explicit dependency depth or add failure propagation. Observation advances at
  most one live roster ancestor per helper call; pending/error/unwind paths keep
  consumer custody. Publication and cancellation/settlement release its independent
  retains exactly once. Public poll may also observe occupied native lanes.
- Normal, deadline, and ordered-successor publication cannot bypass an incomplete
  roster. Direct flush can observe a pending ancestor without misclassifying it
  as dependency-ready/no-lane Busy. The router's existing action bound is preserved.
- Same-stream active persistent inputs can be ordered through an intermediate.
  Authenticated uninterrupted pending-compute FIFO suffixes avoid copying older
  pending-owner history into every roster. An SDMA, active, settled, or noncontiguous
  boundary stops that optimization. In particular, cancelling an interior Ready
  SDMA copy cannot discard the consumer's independent pending-Compute ancestor.
- Bounds follow the admitted capacity profile, including its existing 1024-owner
  qualification profile. The new CPU regression admits 258 pending host-visible
  launches, cancels/releases them in reverse order, and refunds the ledger. It
  uses synthetic dirty-shadow state to prevent eager publication; it is not
  native scale qualification or evidence of actual GPU backing.
- A monotone child peer-history bit avoids inherited-peer DAG traversal before
  the first admitted gate when router peer indices are empty. A real pending
  prefix test asserts zero allocations and an allocating slow-path control.
  After peer history exists, admission and routed poll/wait/flush/drain detect
  missing ledgers behind ungated intermediates with pending gated ancestors.
  Retired peer history does not change native-only drain/progress policy.

Native traversal is iterative and bounded; it follows only retained predecessors,
not global submission scans. Plain compute FIFO roster storage stays linear in
the tested chain. Admission can still walk a growing FIFO suffix or dependency
prefix. The sticky peer-history bit conservatively preserves that walk cost for
the child's remaining lifetime. Neither observation establishes a latency win.

## Qualification

| Lane | Result |
| --- | --- |
| Final all-feature runtime library | 1656 passed, 3 existing failures, 28 ignored |
| Thirteen added tests | All passed in the final full run |
| Runtime doctests | 52 passed |
| Strict all-feature/all-target runtime Clippy | Passed |
| No-default-feature runtime check | Passed |
| Formatting, whitespace, source continuity, signature | Passed |

The full library suite is not green. The three failures remain the
`authorized_execution::tests` cases
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`, with
`InspectSocket(PermissionDenied)` at unchanged `authorized_execution.rs:1317`.
No test was waived or changed to conceal these failures.

Coverage includes explicit versus FIFO failure semantics, same/cross-stream
native intermediates, live and pending ancestors, cancellation/refund, exact
root-release Busy, deferred publication, cancelled SDMA intermediates, stale
same-stream owners, missing prefix retains, error/unwind restoration, scaled
admission, and missing peer ledgers through five public ingress paths.
Scripted owners do not establish GPU kernel output or concrete machine behavior.
The injected observer callback never enters native ownership. Deliberately
corrupted CPU metadata is restored only for teardown, not native terminal recovery.

## Receipts

Frozen raw directory:
`/home/harsh/.codex-tmp/fe2o3-native-quiescence-20260927-is56MLA0`.
[Receipts archive](receipts.tar.xz), SHA-256:
`3312146027bcaf2cdc28634d92ddb8b969a4c7fd727631bddc40c1a159da703e`.
The archive includes command notes, all retained development failures, final
logs, source and binary hashes, the signed source commit object, and its verified
signature. Archive comparison and receipt checksum verification passed. Final
publication attempts are recorded separately from qualification.

## Open Work

Transitive native SDMA endpoint ownership, deferred active output/WAW admission,
broader mixed DAG/fanout and late-history qualification, native XGMI composition,
and shared-body Verus refinement of the new roster/cursor/retain transitions
remain open. Existing peer-gate proofs do not cover this new state. Hardware
completion, storage restoration, and publication still require concrete adapter
and device evidence; CPU tests do not promote them to proved conclusions.

No new Verus campaign, native KFD suite, ELF audit, GPU run, distributed-host
campaign, or matched HIP/HSA benchmark ran. Matched first-conversion versus
steady-replay copy/compute measurements and the broader Worker, device-language,
atomic/collective, multi-device/distributed, profiling, and release gates remain
part of the full objective.

MI300X failed DNS resolution before connection. No remote jobs or files were
created, so no shared-machine cleanup was needed for this checkpoint.
