# R55 gfx942 striped-SDMA wait CPU-cost diagnostics

R55 extends the explicitly profiled R53 striped-tail wait with a best-effort
calling-thread CPU-cost observation. The ordinary
`wait_gfx942_striped_sdma_copy_batch_for_v1` path continues to instantiate the
shared implementation with `PROFILE=false`; that instantiation does not call
the CPU clock or rusage observers and does not update CPU-cost fields.

## Measurement boundary

The profiled path snapshots Linux `CLOCK_THREAD_CPUTIME_ID` and
`getrusage(RUSAGE_THREAD)` immediately around the tail-scan/wait loop. A valid
sample records:

- calling-thread CPU nanoseconds;
- the `ru_nvcsw` voluntary context-switch delta; and
- the `ru_nivcsw` involuntary context-switch delta.

`Gfx942SdmaStripedWaitCpuMeasurementStatusV1` explicitly labels the sample
`Available`, `Unavailable`, or `Invalid`. A syscall failure yields
`Unavailable`. A negative, malformed, overflowing, or regressing clock or
counter yields `Invalid`. Both non-available states clear all three optional
values rather than substituting zero or returning an operational error.

The two APIs do not describe identical intervals internally: the CPU clock is
read before the rusage snapshot at each boundary. Both observations include
some measurement overhead. Context-switch deltas classify scheduler events but
do not measure scheduler delay or establish why the thread was descheduled.

## Authority boundary

The observations do not participate in tail readiness, the final full ordered
audit, currentness checks, all-ready authorization, retirement, timeout
recovery, panic conversion, poisoning, or custody. Measurement failure and
numeric invalidity cannot manufacture completion, reject completion, change a
pending submission into terminal custody, or become a queue-session error.

The existing lifetime-bound all-ready witness and native gfx942 fence-ordering
premise remain the completion basis. Thread CPU time and context-switch counts
are measurements only. They are not a proof or refinement of that basis.

## Benchmark output and claim limits

The `aggregate-profiled` benchmark row advances to
`fe2o3.kfd-striped-wait-diagnostics.v2`. Every numeric wait diagnostic has
per-sample, p50, and p95 fields. The CPU-cost fields additionally retain each
sample's availability status; if any sample is unavailable or invalid, their
aggregate percentile fields carry that non-numeric status instead of silently
computing a partial distribution.

This change adds no hardware result, parity result, scheduler-causality result,
or speedup claim. The profiled path is perturbed by its own observations and
must not be compared with the ordinary path as though their host overhead were
identical.
