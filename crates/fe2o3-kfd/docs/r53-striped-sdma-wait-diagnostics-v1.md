# R53 gfx942 striped-SDMA wait diagnostics

R53 adds an explicitly profiled variant of the R47 blocking striped-tail wait:
`ComputeAqlQueueSessionV1::wait_gfx942_striped_sdma_copy_batch_profiled_for_v1`.
The ordinary wait and the profiled wait instantiate one shared custody and
terminal-state implementation with profiling respectively disabled and enabled
at compile time.

## Observations

One successful profiled wait returns the ordinary completed owner plus
`Gfx942SdmaStripedWaitDiagnosticsV1`. The record contains:

- active queue and request counts;
- tail scan rounds and individual tail observations;
- spin, yield, and sleep pause counts;
- the sum of requested sleep durations;
- host-monotonic offsets after the first scan round containing any ready tail
  and after the first all-ready round;
- host-monotonic durations for tail binding, opening currentness, tail scanning,
  the final ordered audit, closing currentness, and retirement.

All counters and nanosecond conversions saturate rather than wrap. The
requested sleep total does not measure scheduler sleep time. The stage
durations are separate intervals and do not purport to sum to the caller's
complete wait interval.

## Custody

Profiling does not introduce a second completion or cleanup state machine. Both
public methods delegate to the same generic implementation, which owns the sole
submission, timeout recovery, panic conversion, terminal poison, exact pending
return, all-ready authorization, and completed return. The lower wait retains
the existing one-tail-per-active-shard scan and one final full ordered audit.

The diagnostics are returned only with successful completion. Timeout and
terminal failures retain the existing typed custody and do not trade that
authority for a partial diagnostic record.

## Claim limits

The record is an observational performance diagnostic. It carries no device
timestamps, physical-engine counters, in-phase clocks, scheduler state, or
admission authority. Timestamp reads and counter updates perturb the profiled
path, so its host overhead is not interchangeable with the ordinary path.
This tranche adds no wait-policy change, event-driven completion, hardware
qualification, parity result, or performance claim.
