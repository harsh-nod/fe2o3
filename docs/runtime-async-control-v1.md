# Local Async Operation Control

R62 advances A1 of [#182](https://github.com/harsh-nod/fe2o3/issues/182).
It does not complete the asynchronous distributed execution plane.

## API and Ownership

`RuntimeAsyncProgressHandleV1::{launch_tracked, copy_async_tracked,
peer_copy_tracked}` use the existing context admission, owner registry, and
backend paths. They return `RuntimeAsyncTrackedOperationV1`, a standard future
with the same output as the existing untracked operation future. The original
APIs remain unchanged and allocate no control record.

`operation.control()` clones one opaque process-local identity. Compare controls
with `same_operation`; neither an address nor a serializable distributed ID is
exposed. A control holds no backend and cannot release a submission, allocation,
module, or stream. Dropping the observer or control does not cancel progress.

`cancel_before_submission()` atomically competes with the owner's transition
from `Queued` to `SubmissionStarted`, immediately before calling the existing
context submission path. A cancellation winner makes that operation's submit
closure unreachable. Repeating it returns `AlreadyCancelled`. It cannot cancel
dependencies, unrelated work, or anything whose context submission has begun.
This boundary is deliberately earlier than native publication: even a later
definite context rejection does not reopen this control for submission.

The cancelled future resolves with `CancelledBeforeSubmission` when the owner
processes or discards its record. Cancellation remains visible through channel
shutdown, registry rejection, and observer abandonment. A blocked external
adapter can delay future resolution; the atomic decision itself does not wait.

## Host Phases

| Phase | Meaning |
| --- | --- |
| `Queued` | Context submission has not started; cancellation can still compete. |
| `CancelledBeforeSubmission` | Cancellation won; this operation will not enter context submission. |
| `SubmissionStarted` | The owner won; native publication has not been established by this observation. |
| `Observing` | The exact context submission is retained and polled. |
| `ObservationFinished` | The owner decided a host observation, including possible error, before waking its observer. Not proof of GPU success or result availability after observer Drop. |
| `StoppedBeforeSubmission` | The record was rejected/discarded before context submission. |
| `StoppedAfterSubmission` | Observation stopped after context submission began. Publication and quiescence cannot be inferred. |

No phase returns to `Queued`. Terminal phases absorb later control actions.
Only the ordinary context result establishes completion status. Rejected polls
retain and retry observation of the same submission; they never redispatch it.

## Recoverable Timeout

`operation.observe_with_timeout(timer)` takes an executor-supplied
`Future<Output = ()>`, including a pinned timer. It returns either
`RuntimeAsyncTimeoutResultV1::Completed(result)` or `TimedOut { operation }`.
The latter is the original, still-awaitable operation, with the same reply and
control identity. No timeout creates a second result consumer.

Each poll checks the operation first, then the timer. A result already observed
ready wins; a result arriving between those checks may be recovered from
`TimedOut.operation`. Timeout clears only the old reply waker. It never consumes
a concurrently arriving result, changes a GPU status, retries, cancels, or
releases custody. Dropping the timed wrapper abandons observation only.

The supplied executor owns clock/deadline/wakeup semantics. This API does not
claim a built-in wall-clock timer service or hard deadline, and creates no timer
thread. One boxed timer belongs to the caller's observation wrapper, not the
owner registry. Arbitrary timer/capture bytes are not end-to-end byte bounded.

## Verification Boundary

Eight authenticated abstract Verus properties cover cancellation/start
exclusion, irreversible cancellation, at-most-once start, no reopening, stop
disposition, terminal absorption, started-path observation, and timeout/Drop
identity/custody preservation. Eight expected-negative mutations must fail.

The shared pure Rust table is used by production and exhaustively tested against
an independent numeric oracle, including all action traces through depth six.
Its correspondence to the Verus model is reviewed, not automatically proved.
Rust CAS linearization/order is **Contracted**; owner-thread/future/registry and
waker behavior are **Validated** by scripted tests. These are not a proof of
executors, OS scheduling, Rust atomics, adapters, GPU completion, or the whole
engine. No execution authority or existing proof-audit restriction is weakened.

The R62 hardware profile uses the R61 static-musl guard, authenticates both
runner sources, and requires two exact passes on the selected idle GPU. A
bounded test-only owner gate forces cancellation and timeout before submission.
The cancelled full-buffer copy must not alter padding; the timed-out body copy
must still complete after observer Drop. Full input/output/padding and explicit
native cleanup are checked. This is neither kernel nor performance evidence.

## Remaining Issue Gates

Production generated compute still needs the semantic-to-machine refinement
producer in [#214](https://github.com/harsh-nod/fe2o3/issues/214). The compiler
does not yet expose the authenticated DAG/effect/version handoff owned by
[#134](https://github.com/harsh-nod/fe2o3/issues/134). Runtime work must not
replace either with fixture authority or a competing compiler IR.

A1 still needs end-to-end byte budgets, named-executor integration, mixed-duration
high-depth production hardware execution, and full shutdown/drain qualification.
A2-A7 still require graph reservations/versions/residency, integrated group
placement, authenticated two-host execution and collectives, fault campaigns,
and precommitted scaling/performance gates. The complete acceptance list remains
in [the execution-plane status](runtime-async-execution-plane-v1.md).
