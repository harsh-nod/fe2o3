# XGMI Host Attribution V1

Development-only host instrumentation behind `hardware-diagnostic`. This adds no
execution authority, completion certificate, formal refinement, parity status,
or performance acceptance. The uninstrumented runtime's full currentness checks,
ticket/mapping ownership, publication order, error classification, and wait policy
are unchanged. No caching or validation elision is introduced.

## Scope

The lower `submit_batch_diagnostic_v1` and `poll_diagnostic_v1` APIs share the
ordinary batch/poll implementation. A const-disabled timer is used by the ordinary
APIs, with no diagnostic clock reads or allocations. The explicit diagnostic APIs
return host durations only on success, including Pending polls. Errors retain the
same failure enums and native custody; unwinds propagate unchanged.

Submission measures opening currentness, packet preparation, publication, closing
currentness, and the whole lower call. Polling measures opening currentness,
completion observation, closing currentness, and the whole lower call. These are
host intervals, not device/engine durations or syscall counts. The diagnostic
timer adds ten monotonic clock reads per submit and eight per poll. Their
overhead and the delayed closing check are part of the instrumented path.

Each duration uses checked nanosecond conversion; invalid/unavailable values are
not replaced with zero. Repeated stage measurements invalidate that stage. The
runtime also requires every relevant stage and `sum(stages) <= total`.

## Runtime Capture

`enable_xgmi_copy_diagnostics_v1(expected_submissions, max_call_records)` must run
before resource/queue creation. It preallocates a roster of at most 40,000 calls
and requires room for one submit and one completed poll per expected submission.
This first profile supports only single-packet batches. A larger batch still
executes normally but invalidates the capture. Pending polls consume bounded
roster entries; exhaustion invalidates diagnostics, not execution.

Every entry carries the exact backend submission ID and directional device UIDs.
There can be one recorded in-flight submission per direction. The recorder is
armed before each lower call, so an unwind leaves an incomplete capture. Entries
are appended only after returned tickets are rooted, or after mappings and logical
completion are settled. Lower-call failures, incomplete settlement, wrong IDs,
invalid timing, unsupported shape, and roster overflow cannot produce a complete
capture. Observation append does not allocate or alter the workload result.

`finish_xgmi_copy_diagnostics_v1` is available only after successful context and
native shutdown, vacant creation roots, absent queues, and zero logical resources
and indexes. It requires exact expected submission/completion counts and no live
or incomplete call. Successful extraction is single-use. This is not the
authenticated profiler, and its observations never authorize resource release.

## Benchmark

Build `gfx942-runtime-xgmi-peer-benchmark` with `hardware-diagnostic` and append
`--diagnose-xgmi` to its ordinary six arguments. Diagnostic mode requires depth 1.
It captures remap warmups/samples, both prime calls, and persistent-hot
warmups/samples: `4 * (warmups + samples) + 2` submissions. Output appears only
after explicit teardown. Instrumented aggregate rows carry
`diagnostic=xgmi-host-stages-v1`; per-call rows use
`schema=fe2o3.xgmi-host-attribution.v1` and `authority=none`. Ordinary output is
unchanged. Unsupported flags/features/shapes fail rather than silently dropping
diagnostic mode.

The initial performance hypothesis comes from source inspection: a full peer
route check currently performs four topology discoveries, and one submit plus
one successful first poll has four such checks. This diagnostic measures the
whole validation stages; it does not directly count those discoveries or prove
that any specific syscall is the bottleneck.

## Qualification Limits

CPU timer/recorder tests check bounded storage, error/unwind transparency, stage
validity, identity/order, Pending handling, teardown gates, and single-use
extraction. They are not live KFD fault injection or Rust/native formal refinement.
The earlier settled MI300X packet predates this implementation and does not
qualify instrumented execution. A follow-up must compare diagnostic-off/on
processes at the same source and binary with rotated order, exact GPU/workload
identity, byte checks, and owned cleanup. Only then report host-stage attribution
and instrumentation overhead. Matched HIP/HSA performance acceptance, high depth,
concurrency, and the retained-custody retirement gap remain separate work.
