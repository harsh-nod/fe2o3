# Early Async Operation Events

This extends the existing owner-engine operation lifecycle. It does not add a
second graph executor, change device identities, or grant generated-kernel
authority. Native, executable-refinement and performance qualification remain
separate from CPU integration tests.

The [CPU qualification packet](evidence/dev-async-operation-events-cpu-2026-09-24/README.md)
passes 1,358 runtime tests on each of GNU and musl, with twenty existing
hardware-only ignores, including seventeen new regressions. All 46 doctests,
formatting, strict Clippy and unchanged-input checks pass. This does not extend
the earlier native diamond's captured source or evidence scope.

## API And Ordering

`enqueue_launch_with_event` consumes the existing frozen, payload-budgeted typed
launch request. `copy_async_with_event` and `directed_peer_copy_with_event` use
the same Context admission as their tracked counterparts. Each returns
`RuntimeAsyncEventOperationV1`, containing two independently awaited values:

- `event`: an exact Context dependency event after successful event recording.
- `operation`: the existing tracked final observation, submission handle and
  rejection diagnostics, with pre-submission cancellation and timeout control.

The event is not completion or proof of data availability. Directed consumers
may use it while the producer is still pending; Context and backend admission
continue to validate the exact route, producer and input version. By the time a
caller observes the receipt, independent progress may already have completed
the producer. No promise is made that it remains pending.

Submission, event recording and completion observation occupy separate owner
advances. The shared driver is rooted before each action and retains its
submission across event recording. Directed operations add no automatic stream
flush or event observer. Ordinary launch/copy operations retain their existing
independently budgeted stream flushes. Other engine work keeps its own budgets;
this is not a global one-action-per-tick guarantee.

## Event Lifetime

Await each consumer's successful event receipt before releasing its input
events. Enqueue acknowledgment is insufficient: the command queue materializes
a driver before that driver enters Context admission. For fan-out, await every
child's receipt. Releasing an input event earlier can make the queued consumer
reject without a native effect.

The Context owns recorded events until explicit `release_event` or cleanup.
Dropping either observer cancels nothing and releases no event or submission.
The completion result also does not release them. Existing Context owner
commands provide explicit retirement; owned shutdown provides final cleanup.

Drain closes admission. Enqueue the entire intended dependency workload before
beginning drain; a receipt arriving afterward does not reopen admission or add
unsubmitted descendants to the drain scope. Drain quiescence is not cleanup.

## Failures And Bounds

If submission returns no usable handle, the event result is
`SubmissionUnavailable` and the final observation owns the original error.
This does not imply definite non-publication or permission to retry.

Event recording is attempted once. `RecordingFailed` preserves its exact error
separately from the accepted operation, which continues progressing while the
Context remains live. Event capacity can therefore fail after successful
submission. No event slot is pre-reserved and no automatic recording retry is
introduced. Terminal errors, invalid backend handles and panics retain ordinary
Context custody; they do not convert uncertainty into success or cancellation.
Stop resolves unresolved observers but cannot revoke an already recorded event.

Both reply-cell credits and the ordinary request snapshot charge precede command
enqueue and all native effects. Failure to acquire the second reply credit
refunds the first; command refusal refunds both and disposes the snapshot. One
operation occupies one existing registry slot. Each observer's cell remains
charged while retained, including after resolution. These are existing count and
payload bounds, not a total native/process memory budget.

## Remaining Integration

This enables host-queued directed chains and diamonds without constructing
their submissions inside owner Context callbacks. It does not turn the
single-device completion graph into a multi-device graph. That requires explicit
group identity, submission readiness, resource reservations and retirement
contracts. General generated DATA rebinding, aggregate accounting, broader
native/fault campaigns, executable refinement and matched HIP/HSA measurements
remain open; A1/A2 and issue #182 are not complete.
