# Aggregate XGMI Host Attribution V1

Development-only instrumentation behind `hardware-diagnostic`. It does not cache
or omit currentness checks, change native ownership, certify GPU completion, or
establish formal refinement or HIP/HSA parity. The existing aggregate execution
path is shared with the diagnostic path; const-disabled timers do not read the
diagnostic clock or allocate observation storage.

## Capture Contract

Call `KfdNativeXgmiRuntimeBackendV1::enable_xgmi_aggregate_diagnostics_v1` before
creating resources. Its expected successful-call count is bounded to 1..=40,000,
and all record storage is fallibly reserved before execution. It cannot be armed
alongside `enable_xgmi_copy_diagnostics_v1`.

This first diagnostic profile accepts only unpublished depth-one aggregate calls
executed in strictly increasing backend submission-ID order. The general
aggregate execution API remains broader. Unsupported shape/order, ordinary native
publication/polling, Pending, retries, errors, interrupted calls, missing timing,
and capacity exhaustion invalidate the capture without changing workload results.
Only exact `RuntimePeerCopyBatchPollV1::Succeeded` results can append observations.

Records are appended after native mapping custody and logical status settlement.
Each includes the submission ID and both directional device IDs. Successful
single-use extraction through `finish_xgmi_aggregate_diagnostics_v1` requires
successful context and native shutdown, absent queues, vacant creation roots,
logical quiescence, no terminal retirement, and exactly the expected record count.

## Measured Phases

| Field | Host Interval |
| --- | --- |
| `admission_validation_ns` | Liveness, exact-roster/custody validation, scratch reservation |
| `preparation_ns` | Queue preparation, mapping custody transfer, copy requests |
| `opening_currentness_ns` | Full native pair currentness at batch opening |
| `submission_ns` | Native batch submission |
| `wait_ns` | Native fence waiting and associated boundary checks |
| `closing_currentness_ns` | Full native batch closing currentness |
| `settlement_ns` | Mapping restoration and backend status settlement |

`total_ns` surrounds backend aggregate progress. It excludes facade enqueue and
facade settlement/release outside that backend call, as well as diagnostic
candidate lookup/arming and observation append. Unmeasured gaps and clock
overhead remain in the total. All phases and the total use checked nanosecond
conversion; missing/repeated phases or an overflowing phase sum invalidate the
capture. The phase sum must not exceed the total. These are host intervals, not
device timings, syscall counts, authenticated profiler records, or cleanup rights.
Instrumentation consumes real time but does not extend the caller's deadline.

## Benchmark and Qualification

Build `gfx942-runtime-xgmi-peer-benchmark` with `hardware-diagnostic`, then append
`--aggregate-peer-batch-hot-diagnose` to the ordinary six arguments. It requires
depth one and captures `2 * (1 + warmups + samples)` successful calls. Prime,
warmup, and measured calls are all retained and must be analyzed separately.
Output appears only after explicit teardown, with per-call schema
`fe2o3.xgmi-aggregate-host-attribution.v1` and `authority=none`. The existing
persistent-hot summary gains `diagnostic=aggregate-host-attribution` only in this
new mode. Use `--aggregate-peer-batch-hot-only` on the same executable for the
uninstrumented control.

The [CPU protocol](evidence/dev-xgmi-aggregate-attribution-cpu-2026-09-19/PROTOCOL.md)
checks recorder boundaries and enabled/disabled execution equivalence, including
custody order, failure classification, retries, deadlines, and panic payloads.
It is not native fault injection. Dedicated executable tests of dual-recorder
enable ordering and mixed ordinary/aggregate invalidation remain absent.
The [MI300X protocol](evidence/dev-xgmi-aggregate-attribution-mi300x-2026-09-19/README.md)
binds off/on/on/off trials to one signed source and ELF with fresh shared-host
admission and owned cleanup. The completed packet attributes 98.8085% of measured
hot backend time to full opening/closing
currentness. Its on/off facade medians remain near 14.3 ms; this small shared-host
sample does not establish causal instrumentation overhead or HIP/HSA parity.
