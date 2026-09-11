# Engineering Ordered Batch V1

`DispatchOrderedBatch` is an explicit, unauthenticated gfx950 engineering-worker
operation. It does not change `Dispatch` or serial `DispatchSequence`, expose a
peer-group API, or grant protected execution authority.

The command contains 1 through 16 `OrderedBatchDispatchV1` records and one
`timeout_ms` in 1 through 600000. Records retain the ordinary kernel token,
kernarg payload size, workgroup/grid geometry and pointer fixups, but have no
individual timeout. Concatenated payload framing is exact and bounded. All kernel
resource, argument, mutable-alias and ownership checks run before packet exposure.
Cross-command producer/consumer reuse is allowed because the commands execute in
order; each individual command must still obey its declared accesses.

## Storage And Publication

The first ordered batch lazily adds a retained 1 MiB allocation: 16 aligned
64-KiB kernarg slots. It initializes 16 distinct 64-byte completion signal objects
inside the existing 4-KiB signal page, while the queue has a completed frontier.
The extra allocation remains in the normal private-resource accounting and is
released only by confirmed idle queue rollover or close. Legacy-only workers
never allocate it. No arena or signal is overwritten while its batch is pending.

Every packet uses `WaitForPrior` (`0x1502`), including system-scoped acquire and
release fences. The existing inert AQL batch API writes every INVALID packet body
before publishing any release header. One write-counter reservation advances by
the exact packet count, then one final doorbell names the last packet ID. Ring
capacity, wrap, monotonic read observations and conservative rollover limits stay
unchanged. Read-pointer advancement is not substituted for kernel completion.

Preparation failure exposes no packet. Publication is not transactional: once
the write counter is reserved or any header is exposed, a failure is terminal,
even if the final doorbell was not rung. No retry, guessed retirement, or cleanup
of possibly live allocations is attempted.

## Completion And Failure

One host wait phase observes the final ordered signal. Success additionally
requires acquiring every retained signal as completed, unchanged device/queue
epoch/frontier identities, valid counters, zero exception payload, and full exit
currentness/idle checks. Operational currentness checks occur before publication
and periodically during the wait; full checks bracket the operation. No host
read/write, free, load or unrelated command interleaves with this worker call.

The aggregate deadline starts after staging and before publication, and covers
publication, waiting and the final validation/fence. It is intentionally not the
serial sequence's per-kernel deadline contract. Success returns only
`DispatchOrderedBatchCompleted { completed_dispatches, elapsed_ns }`.
`elapsed_ns` is aggregate host wall time, including final validation; it is not
GPU time and must not be replicated or divided into claimed per-kernel timings.
Profiling retains packet count N but records one aggregate publish/wait phase.

Every failure terminally poisons the ordered context and returns the existing
fatal worker error. The outer worker retains uncertain resources until process
exit. No success, partial completion count or performance sample is returned.

## Qualification

Host tests inject every preparation, reset, body/header publication, final
checkpoint, doorbell, signal validation and exit-fence failure for 1 and 16
packets. They cover bounded framing/deadlines, distinct aligned argument/signal
addresses, all-body-before-any-header ordering, ring wrap/exhaustion, stale
identities, exception/counter faults and signal/tail initialization.

Bounded native checks passed on gfx950 on 2026-09-11, separately with full and
operational currentness. Each run executed 184 packets across 26 serial/ordered
dyadic producer/consumer chains: counts 1, 2 and 16 at one and three active rows,
repeated storage reuse, and one actual queue rollover with live user buffers.
Every output array, immutable input and surrounding guard matched; serial and
ordered paths agreed before and after rollover. Both workers closed/reaped and
all eight physical GPUs returned to the checked idle roster.

The checked source was `f64c86e0c` on upstream `c94e2101a`. Worker SHA-256:
`761027c596b896a58da822cf919adf4d480e9b4b71d8c79e93265e3d4002c5b6`.
Full-currentness result SHA-256:
`376d2fb2adc88af1481fe3c89c8e768dd01f68fa50065cd95a6615230a770a4e`.
Operational-currentness result SHA-256:
`0cf0c74446e9d1945076aec74d33bb9dad0dfee849ee1a5ce5c5d3e58fc272fc`.
The image-specific external fixture and its raw protocol records remain outside
the generic runtime implementation. These checks establish neither arbitrary
kernel correctness nor model/serving performance. Deliberate GPU faults were not
injected on shared hardware; failure-path coverage remains host-injected.
