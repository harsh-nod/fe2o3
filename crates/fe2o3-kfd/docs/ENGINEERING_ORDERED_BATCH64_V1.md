# Engineering Ordered64 V1

`DispatchOrderedBatch64` is a separate, explicitly selected, unauthenticated
gfx950 engineering-worker operation. It is not a default, a capability, a proof
of arbitrary-kernel safety, or a production execution route. A parent must pin
the new worker identity and require `DispatchOrderedBatch64Completed`; old or
unsupported workers fail closed. The ready version/identity schema is unchanged.

The original `DispatchOrderedBatch`, completion response, sequence command,
performance configuration, peer routes, and their 16-item limits remain
unchanged. There is no automatic promotion or fallback between ordered modes.

## Fixed Limits

- 1..64 dependent packets, with one aggregate 1..600,000ms
  publication-to-observed-completion deadline, not 64 independent deadlines.
- Unchanged 65,536-byte JSON header, 4MiB cumulative payload, 65,536-byte per-item
  kernarg and 256 pointer-fixup limits. Pointer-rich 64-item commands may exceed
  the header limit and must be rejected before any payload is read or written.
  Callers must serialize/check their actual command groups; 64 is a maximum,
  not a promise that every possible group fits.
- A dedicated 4MiB fixed-stride kernarg arena and exactly 64 64-byte completion
  signals in the existing 4KiB page. No signal page, queue ring or unrelated
  sequence capacity is enlarged.
- The first ordered mode in a queue epoch fixes arena shape. A subsequent
  opposite-mode command is a terminal error before staging/publication, even
  when its item count would fit. To change mode, use a fresh process or a
  successful existing idle rollover. Standalone and sequence dispatch retain
  their existing completed-frontier/signal-zero behavior.

## Retained Lifecycle

Both modes share the unchanged entry fence, immutable preparation scope, fresh
preparation fence, storage retention before fallible initialization, all bodies
before release headers, one exact reservation, WaitForPrior on every packet,
publication currentness/counter/exception/deadline check, and one final doorbell.
Final-signal completion never replaces observing all retained signals. Pending
device/epoch/frontier identity, exit fence, aggregate deadline and terminal
poison after any uncertainty remain required. There is no retry, rollback or
early resource reuse after publication uncertainty.

The 131,072-packet conservative outstanding budget is unchanged; grouping does
not reduce its per-packet consumption. Idle rollover destroys the old queue and
all internal arena/signal resources before reinitialization, without dropping
user kernels/buffers. The new arena mode is selected anew only after that path.

## Qualification

CPU tests exercise separate framing, payload/header rejection, every slot and
cardinality boundary, 64-slot mapped signal reuse, ring wrap/overflow/frontier
checks, every fake preparation/staging/publication/signal/exit failure, all four
preparation-boundary fault kinds at every slot, and deadline poisoning. They do
not inject native reset/exception faults or establish hardware correctness.

Before integration, run unchanged ordered16/sequence/peer/rollover regressions,
strict crate Clippy, and a bounded native dependent chain with exact outputs,
immutable inputs and complete guards. Repeated batches and idle rollover must
retain correct output before any latency comparison. AQL's cited independent
publication theorem does not cover this WaitForPrior native route. Host elapsed
time and cumulative counters are not GPU timestamps or per-kernel timings.
