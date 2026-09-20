# Runtime XGMI Currentness Capture

The opt-in `fe2o3-runtime/hardware-diagnostic` feature joins the lower-level
[full-currentness intervals](runtime-xgmi-currentness-diagnostics-v1.md) to
successful depth-one aggregate runtime calls. This successor adds the runtime
integration and benchmark flag that the lower-level document left pending.
It does not relax validation or establish native performance improvement.

## API And Lifetime

`enable_xgmi_aggregate_currentness_diagnostics_v1(expected_calls)` selects a
bounded recorder before resource creation. The existing single-packet,
aggregate-only, and new currentness captures are mutually exclusive. The
selected storage reserves space for 1 through 40,000 observations up front;
successful calls append without growing it. Existing V1 observation types,
aggregate-only methods, and output schemas are unchanged.

Each call must be unpublished, depth one, and use a strictly increasing
backend submission ID. The recorder binds the original source/destination
identities and outer aggregate host intervals to both opening and closing
pair observations. Missing intervals, checked-sum overflow, invalid nested
containment, mismatched identities, unsupported shapes, retries, pending
results, or errors invalidate capture without changing the workload result.
Nested pair totals must fit their corresponding aggregate opening/closing
spans; nested durations are never added again to the outer sum.

`finish_xgmi_aggregate_currentness_diagnostics_v1()` returns
`Vec<KfdRuntimeXgmiAggregateCurrentnessObservationV1>` only after successful
logical and native teardown and an exact complete capture. Extraction is
single-use. Calling an extractor for the other mode does not consume or
invalidate the capture. Error precedence is terminal, busy, absent/wrong
mode, then incomplete/invalid.

The private close adapter delegates submission, waiting, timeout
classification, original custody types, and the absolute deadline. It calls
the matching normal or terminal diagnostic close and stores detail only
after successful close. There is no additional cleanup authority or `Drop`
implementation. Native execution stays inside the existing panic-abort
boundary. Bad diagnostic data cannot abandon a batch or release its owners.

## Benchmark

The `gfx942-runtime-xgmi-peer-benchmark` example accepts the explicit flag
`--aggregate-peer-batch-hot-currentness-diagnose`, only with the diagnostic
feature and depth one. It uses the existing persistent-hot workflow and
checked `2 * (warmups + samples + 1)` observation limit before native open.

After explicit teardown, it emits one
`fe2o3.xgmi-aggregate-currentness-attribution.v1` row per successful backend
call, including primes and warmups. The row preserves identities and all
eight aggregate intervals, then reports 12 opening and 12 closing pair and
topology fields. Ordinal/submission identity and workload phase must be
checked before calculating measured statistics. Summary rows retain the
existing hot-only schema with `diagnostic=aggregate-currentness-attribution`.
Formatting and printing occur outside timed work.

These are forgeable owner-free host observations, not device or copy-engine
timings, completion authority, authenticated profiler records, or formal
refinement. No performance ratio should be inferred from unmatched campaigns.
Native qualification of this new mode remains pending.

## CPU Qualification

The [CPU packet](evidence/dev-xgmi-currentness-capture-cpu-2026-09-20/PROTOCOL.md)
checks GNU/musl diagnostic-enabled execution and GNU feature-off regression.
Tests cover both recorder modes, fixed capacity, admission/order, malformed
finish without state advancement, every missing nested field, overflow,
containment boundaries, wrong-mode extraction, teardown precedence, adapter
outcome/close-error/custody equivalence, retry deadlines, boxed panic identity,
and unavailable detail preserving operation success. CLI tests bind every
formatted field, feature rejection, exclusivity, and unchanged old schemas.

This evidence is scoped CPU regression coverage, not full HIP/HSA parity,
machine-code refinement, a Linux atomic-snapshot guarantee, or a speedup.
