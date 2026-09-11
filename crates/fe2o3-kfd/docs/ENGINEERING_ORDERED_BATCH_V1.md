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
epoch/frontier identities, valid counters, zero exception payload, and exit
currentness/idle checks. Dispatch boundaries, publication and periodic wait
checks honor the existing explicit operational-currentness option; default-mode
checks remain full. Allocation (including the lazy ordered arena), queue rollover
and teardown retain full lifecycle checks. No host
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

Those frozen native receipts predate the dispatch-boundary policy change:
their operational runs still forced full checks around every ordered call.
The current candidate removes those extra full scans only for the already
opted-in operational policy. It requires separate host/native/model validation;
the earlier receipts are not evidence of a performance improvement or of this
new boundary selection.

## Dispatch-Policy Checkpoint

The boundary selection at `c110ac55c655579e0969b310402800b0c2666694` passes
483 engineering library tests, 20 integration tests, 420 default library tests,
31 doctests, strict all-target release Clippy, formatting and the release worker
build. Worker SHA-256:
`b91ddef78135829f607d1b83f5cf898d745b36013327f0d2d12771aba1b5150b`.

Separate native counter probes compare the old worker above and this candidate
under full and operational policies. Each fresh worker executes a cold one-packet
chain, a warm one-packet chain and a warm sixteen-packet chain. Fixture allocation
precedes snapshot A; snapshot B precedes full-array/guard readback. Every delta
contains two commands (including the earlier snapshot command), the exact packet
count, and no reads, writes or kernel admissions. Full-currentness check counts:

| Worker / Policy | Cold 1 | Warm 1 | Warm 16 |
| --- | ---: | ---: | ---: |
| Old / full | 13 | 11 | 26 |
| Candidate / full | 13 | 11 | 26 |
| Old / operational | 4 | 2 | 2 |
| Candidate / operational | 2 | 0 | 0 |

Periodic polling can add checks; these are observed counts, not universal exact
full-mode counts. The candidate's cold operational call retains two full checks
for lazy allocation. All four probes pass exact dependency arrays, immutable
inputs and guards, free their 39 fixture allocations, close/reap normally and
leave all eight physical GPUs idle. These overlapping host counters do not
establish exclusive CPU time or GPU/model performance.

Candidate full/operational counter-result SHA-256 values:
`6d2e1d69a6e9aaec88b10c8329c2f53b0fd77a5e8ef8cac51d60b55f6650dfeb`
and `9fd7dee0650ba82f2ae374886a37794fb89117a8bc0fa4637d5a8de060948fb1`.

The frozen 184-packet/26-chain lifecycle fixture also passes again on this
candidate in both modes, including storage reuse and a real rollover at 152
retired packets followed by 32 more packets with live buffers. Full arrays,
guards, close/reap, unforced process cleanup and all-eight idle checks pass.
Full/operational lifecycle-result SHA-256 values:
`e5b4001c4c2497ce9f719d846c8631b6cef55f3d2262ef7688ec6e5f539845dd`
and `94886ad068f34d1db036c66a9d7631ae63269ba8b4c06255f6f3cd4090de3e3c`.
Image-specific fixtures remain external. Deliberate GPU faults remain excluded
on the shared host; host-injected failure coverage is unchanged.
