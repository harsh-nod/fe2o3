# gfx950 Serial Completion Backoff

This is an engineering-runtime experiment, not protected runtime admission,
a production performance qualification, or a full-model megakernel. It changes
only the pause policy between fresh serial completion observations.

## Policy

The existing serial path slept for 50 microseconds after every incomplete poll.
The candidate instead admits at most 16 additional fresh polls within one
10-microsecond window, anchored immediately after the first incomplete poll.
Once either limit expires, every subsequent incomplete observation uses the
original 50-microsecond sleep. The budget never restarts for that dispatch.

```text
publish the prepared dispatch using the existing checked path
repeat:
    poll using the unchanged completion/currentness/error checks
    return immediately on completion or error
    on the first incomplete observation, anchor the spin window
    if fewer than 16 extra polls were admitted and elapsed time < 10 us:
        admit one additional fresh poll and issue a CPU spin hint
    otherwise:
        permanently select the existing 50 us sleep for this dispatch
```

The time limit bounds admission to additional polls. It does not promise that
a poll, an operating-system call, or a descheduled thread finishes within
10 microseconds. The count limit independently prevents unlimited spinning.
Every dispatch receives a fresh budget; completed or failed observations are
never reused as evidence for another dispatch.

## Unchanged Contracts

`publish_prepared_dispatch` and `poll_pending_dispatch` remain byte-for-byte
unchanged. The first due currentness fence is not deferred. Pending-dispatch
identity, acquire completion observation, queue counters, exception handling,
the original timeout, and completion-time idle/currentness checks remain in
the existing poll function. Failures propagate through the same ownership and
quarantine path without retrying publication or freeing uncertain work.

Serial IPC sequences use this same serial execution path. Peer rounds, ordered
AQL batches and device-currentness implementations are unchanged. The candidate
neither batches packets nor overlaps GPU kernels. It changes no model weights,
activation precision, arithmetic, launch geometry or GPU code object.

## Validation

Six focused tests cover the 16-poll cap, the strict 9,999/10,000-nanosecond
boundary, permanent fallback, a reversed injected clock, terminal first polls,
anchoring after a slow first observation, and later errors with no additional
poll or pause. Existing runtime checks remain separately covered by their tests.

On the frozen baseline plus this single-file patch, the full KFD library suite
passes 496 tests with one existing ignore; strict scoped Clippy and the release
worker build pass. A separate current-branch build passes the same checks and
produces a byte-identical worker. CPU qualification uses two pinned CPUs, two
test threads and build jobs, and nice level 10. Ignored hardware tests are not
included in these CPU counts.

The GPU comparison uses actual single-request, target-only Qwen3-8B BF16 on
MI350X/gfx950. Both workers use the same v3 native image with scalar projections, controller,
checkpoint, five-token prompt, 32-token independent reference and device-side
TP1 residual handling. All 32 choices and decoded bytes match; each request
performs 36 forwards and 22,176 dispatches. Retained input/plan hashes,
currentness receipts, worker closure, topology and idle checks pass independently.

| One unwarmed observation | Existing sleep policy | Bounded-poll candidate |
| --- | ---: | ---: |
| Mean post-first-token interval, seconds | 1.231650567 | 1.192911924 |
| Post-first tokens/second | 0.811918597 | 0.838284856 |
| Setup, seconds | 167.792913303 | 169.620889420 |
| Whole-command user CPU seconds | 151.65 | 153.98 |
| Whole-command system CPU seconds | 30.33 | 29.94 |

These are unqualified host observations, not an established speedup. Each has
31 post-first-token intervals, no warmup, and concurrent CPU work. Whole-command
CPU accounting includes setup, teardown and waited-for children; it is not
decode-only CPU use. The resource pair does not measure completion-poll counts.
The separate instrumented runs below measure those counts.

## Separate Counter Diagnostic

Both instrumented requests also pass all 32 independent reference choices,
input/currentness checks and worker closure. Each submits 22,176 packets and
records 22,393 worker commands, 89,137 operational-currentness checks, 36 reads
(144 bytes), and 180 writes (19,296 bytes). Cached workload execution performs
no additional kernel admissions. These are different captures from the resource
pair above, not extra counters attached to those timings.

| Instrumented host observation | Existing sleep policy | Bounded-poll candidate |
| --- | ---: | ---: |
| Completion polls | 337,974 | 574,169 |
| Polls per dispatch | 15.2405 | 25.8915 |
| Worker command wall time, seconds | 40.476575122 | 40.356761093 |
| Dispatch preparation, seconds | 1.729881243 | 1.952099832 |
| Dispatch publication, seconds | 1.718533726 | 1.935345915 |
| Completion wait, seconds | 35.265477265 | 34.493446267 |
| Currentness checks, seconds (overlapping) | 6.752211722 | 7.604924692 |

These are host-runtime counters, not GPU timestamps. Currentness checks overlap
other phases and must not be added to them or plotted as disjoint components.
The counter window includes the earlier snapshot command and later idle fence,
but excludes worker closure. Instrumentation, lack of warmup and concurrent CPU
activity prevent a stable throughput or CPU-cost conclusion. More completion
observations with nearly unchanged command wall time do not establish this
backoff as an effective solution to the end-to-end performance gap.

The baseline worker is SHA256
`b4cb30788d4a32d9cae240823c26a80e9103f5698f91d95b16d2bda7e78270f9`;
the candidate worker is
`737f151f798b16cf8393659950cddf1b62faa204e8c8dbbc5c2753b969391d20`.
The controller is
`a28848eadc9aa7a34f21e7909b881c1aa8cdbf7d9bc8f788645504ced77357c2`;
the unchanged HSACO is
`583a889a4ac5f03c7b004cdd52c1d062649853326e9f4b3d251e11de706573b5`.

See the [Ferric target-model ablations](https://github.com/harsh-nod/ferric/blob/feat/gfx950-megakernel-performance-42/docs/GFX950_TARGET8B_ABLATIONS_V2.md)
for the fixed workload and separate comparison families. No result here
establishes 700 tokens/s, GPU overlap, or parity with a tuned serving engine.
