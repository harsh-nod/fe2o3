# Completion Observation Contract V1

CO-1 separates completion observations from reply settlement and native resource
disposition. The first production consumer is ordinary
`async_engine/operation.rs::Operation::advance`. Generated preparation still has
no production adoption hook; this packet does not enable generated ISSUE,
readback or typed decoding.

R98 [local acceptance](evidence/local-r98-completion-contract-2026-09-11/README.md)
records six frozen/restored tests, four compiled behavioral negatives, seventeen
source gates and eight auxiliary checks. No new formal, live KFD or performance
result follows from this CPU/source packet.

## Observation Classes

Private `async_engine/generated_operation/completion_contract.rs` borrows
`Result<RuntimeCompletionStatusV1, RuntimeErrorV1<E>>`. It needs no `Clone`,
`Debug`, `Send` or other bound on `E`. The result contains no owner, receipt,
publication permit, decoder or reply cell.

| Input | Class | Meaning |
| --- | --- | --- |
| `Pending` | `Pending` | Keep observing the same operation. |
| `BackendRejected(error)` | `Rejected` | The observation was rejected, not the already-issued operation. No reissue or inferred nonpublication. |
| `Succeeded` | `SuccessCandidate` | A candidate observation, not generated decode/readiness/disposal authority. |
| `Failed(reason)` | `Failed(reason)` | Preserve the exact failure, including the distinction between a backend code and cancellation. |
| `QuiescentWithoutResult` | `QuiescentWithoutResult` | No successful execution result. |
| `BackendQuiescent(error)` | `QuiescentError` | Preserve the original error even if Context separately records quiescence. |
| Backend terminal/protocol error or `ContextTerminal` validation | `Terminal` | A terminal observation does not dispose possibly live resources. |
| Other validation errors | `ObservationError` | Preserve the original validation failure; no new authority is created. |

Already-settled state belongs to the existing reply gate, not a new classifier
state machine. Repeated `Reply::complete` cannot replace the original reply or
create another reserved credit. Classification neither reads nor changes that
gate.

## Production Ordering

The ordinary driver's submission, control and registry paths are unchanged.
After submission, one advance makes at most one Context poll. Only successful
polling proceeds to the existing Context-local query. A polling error is passed
unchanged to classification and, except for a rejected observation, the raw reply.

This ordering matters for `BackendQuiescent`: Context can record
`QuiescentWithoutResult` before returning the backend error. Querying after that
error would incorrectly replace it with a successful query result. Similarly,
KFD polling can progress or publish pending work; it must not be duplicated as
part of classification.

Pending leaves the reply pending. Rejected increments the existing saturating
counter, retains the exact last error and continues observing the same
submission. Every other class calls the existing `finish` with the original
observation. The exhaustive class match prevents a future class silently
inheriting the finish branch.

Ordinary `finish` reports a host observation. Its driver-return value is still
subject to the registry's Context-terminal gate, and possibly reachable native
resources remain in Context custody. Reply completion does not itself prove
native retirement. CO-1 changes neither that gate nor Context's completion,
cleanup, callback or protocol transitions.

## Validation Boundary

Six focused CPU tests live in
`async_engine/tests/owned_tests/control_tests/completion_tests.rs`:

- Thirteen prebuilt cases cover all current observation families. Additional
  backend-code extremes preserve exact codes, including values numerically equal
  to legacy cancellation/quiescence codes.
- A borrowed non-Clone error probe survives repeated classification with no
  early drop or allocation. Allocation counting uses the existing test allocator.
- The existing reply gate retains one result and one credit until its actual
  owners are dropped.
- Repeated rejection and Pending retain one issue, exact poll counts, the last
  rejection, control phase and existing reply timing.
- Failed and quiescent-error observations preserve their distinct raw replies
  while Context records the corresponding conclusive status.
- Terminal error reporting retains registry and Context records and performs no
  backend release during terminal cleanup. This fixture is not native
  process-lifetime quarantine qualification.

Zero-allocation coverage applies only to classification over prebuilt inputs,
not Context polling, user arguments, backend execution or the outer flush loop.
The ordinary owned-loop, cancellation, repeated-rejection and drain tests remain
regression gates. Compiled negative mutations must fail behavioral tests for
premature rejected/Pending completion, overwritten quiescent errors and suppressed
terminal replies. Source must be exactly restored before positive final gates.

## Remaining Integration

CO-2 supplies exact identity validation and CO-3 composes existing reply/custody
with the data-only completion adapter. Native DATA-ADOPT and ISSUE must bind
actual resources and publication occurrences. CO-4/COMPLETE then checks exact
completion, the complete existing readback roster, closing currentness and native
disposition before invoking the existing R85 decoder. Do not reserve the R80
reply/readback storage again.

## Formal Correspondence Work

Existing runtime-model `r61_owner_async_custody`, `r62_operation_control` and
`r64_payload_budget` sources have corresponding `_v1.rs` Verus models. Their
reply-once, host-control transition, capacity and credit guards are narrower
than the concrete runtime. They do not establish exact enum projection, mutex
and CAS behavior, unique Rust permits, waker/result destruction order or native
cleanup truth. CO-1 does not rerun or extend those proofs.

The next correspondence packet must put normalized observation/continuation
policy in production-consumed shared model code, with authenticated proofs of
those same definitions rather than a parallel handwritten classifier. Keep the
runtime projection borrowed and preserve the original raw error. The registry
decision must give terminal Context or panic precedence over removal/parking,
even if a driver returns `true`. Continue using the existing reply and control
guards; do not introduce another settlement state machine or credit owner.

Required checks include exact failure/cancellation projection, no finish on
Pending/rejection, unchanged issue identity, forwarding the original finishing
result, and exhaustive returned/panicked, terminal/live, retired/retained and
prepared/unprepared registry decisions. Reuse the behavioral mutations and add
terminal-precedence and duplicate-resolution negatives. Proved normalized
guards alone would still leave the Rust projection, ownership transitions and
native adapter correspondence open; acceptance must name those separately.

These CPU checks do not establish formal adapter correspondence, live KFD
behavior, native completion throughput or HIP/HSA performance parity. The
version-journal [contract](runtime-context-version-journal-v1.md) separately owns
writer/lineage settlement; a completion class is not a NoEffect receipt.
