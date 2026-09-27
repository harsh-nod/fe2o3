# Native Compute Quiescence Control

Development qualification, not #182 closure, A1/A2 acceptance, full HIP/HSA
parity, complete runtime refinement, GPU qualification, or performance acceptance.

Implementation: `1cc1bcbd70179f32cd04a658fb22e38e0b1e1c5f`, SSH-signed and verified.
The primary agent implemented and tested; two read-only agents reviewed the
production integration and proof-campaign boundaries.

## Implementation

The production observer and Verus root include the same scalar transition body.
It classifies invalid/completed cursors, advances only an exact-quiescent entry,
and carries the resulting cursor and per-invocation poll budget. Production uses
both returned fields. Four publication/readiness guards use the shared exact
`cursor == roster.len()` predicate. The stronger ordered-successor fast path
still requires an empty roster, not merely a completed cursor.

Valid-state behavior is unchanged. A cursor beyond its retained roster now
terminalizes instead of looking like exhausted iteration. The regression checks
the direct quiescence adapter: no owner callback, unchanged FIFO/retains/credits
and allocation custody, and reinsertion of the exact pending launch. It does not
claim malformed metadata is detected before every public poll/flush action.
Injected corruption and terminal state are reset only for fixture teardown.

## Proof Qualification

[Verus root](../../../crates/fe2o3-runtime-model/verus/compute_quiescence_v1.rs)
and [campaign runner](../../../crates/fe2o3-runtime-model/verus/check-compute-quiescence.py).

| Lane | Result |
| --- | --- |
| Signed source/blob binding and per-phase source continuity | Passed |
| Pinned verifier closure before and after | Passed; 190 files, 129019839 bytes |
| Positive before, exact relocated positive, positive after | Each 12 verified, 0 errors |
| Production-body-only logical-negative mutations | 17 of 17 rejected |
| Inherited classifier and local campaign calibration | Passed; 9 and 4 groups |

The pinned release is `0.2026.08.09.92f466f`; invocations use `--no-cheating`.
The verified count is the verifier's root count, including generated obligations,
not twelve independently established runtime guarantees. Contracts cover exact
action classification, monotonic/overflow-safe cursor advancement, poll-budget
state, and exact scalar readiness. Witnesses cover empty, final, invalid and
maximum cursors, exhausted budget, and poll-then-pending/quiescent transitions.
Mutants alter cursor movement, completion, invalid-state handling, budget use and
readiness without changing proof contracts. Resource/timeout failures, malformed
output and foreign diagnostics do not qualify as logical-negative evidence.

This does **not** prove the production adapter loop, physical quiescence, the
authority of its exact-owner observation, vector/id correspondence, graph
capture, retain-map mutations/refunds, callback budgets across public operations,
error/unwind custody, complete publication gating, or CPU/GPU memory ordering.
The adapter remains covered by scripted tests and review, not formal refinement.

## CPU Qualification

| Lane | Result |
| --- | --- |
| Focused quiescence runtime tests | 39 passed |
| All-feature runtime library | 1664 passed, 3 existing failures, 28 ignored |
| Runtime doctests | 52 passed |
| Strict all-feature/all-target runtime Clippy | Passed |
| No-default-feature runtime check | Passed |
| Runtime formatting and whitespace | Passed |

The library suite is not green. The unchanged `authorized_execution::tests`
failures are `cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`, all failing
`InspectSocket(PermissionDenied)` at `authorized_execution.rs:1317`.

## Receipts

Frozen raw directory:
`/home/harsh/.codex-tmp/fe2o3-quiescence-control-20260927-QfMcCw1G`.
The [receipts archive](receipts.tar.xz) contains commands/results, development and
final CPU logs, compiler and test-binary identity, signed source patch/signature,
full proof-campaign receipts and inputs, and per-file SHA-256 checksums.
Archive SHA-256:
`3a4469375ff079e85d8f600b06f22abf2235b8401071f100567c52d64f6cc85a`.
Archive comparison and all receipt checksum checks passed.

## Remaining Work

This supplies the scalar-control portion of the shared-body gap recorded in the
[preceding SDMA checkpoint](../dev-native-sdma-quiescence-2026-09-27/README.md).
Concrete retained-graph/ownership refinement and production-adapter proofs remain
open. Deferred output/WAW needs dependency-bound successor-writer reservations in
Context as well as native admission; the current pending-writer rejection remains
in place. Broader mixed DAG/fanout and multi-window qualification, native XGMI
composition, and matched first-conversion/steady-replay KFD/HIP/HSA benchmarks
remain open, alongside the Worker, device-language, atomics/collectives,
multi-device/distributed, profiling and release gates of the full objective.

MI300X DNS failed before connection. No remote jobs or files were created.
No new native KFD, ELF, GPU, distributed-host or performance campaign ran.
