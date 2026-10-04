# Runtime causality query V1

`fe2o3-profiler-service runtime-causality-jsonl` is a bounded, deterministic,
read-only view over one exact KFD Runtime Profile V1 and, optionally, one exact
Semantic Profiler Bundle V4. It does not start a runtime, collector, debugger,
or decoder and cannot attach, schedule, submit, or change device work.

The KFD profile supplies observed capture-local event sequence, queue and stream
membership, allocation bindings, host-staging reads and writes, and dispatch
publication, completion, and release. The query derives only the lifecycle
relations required by the validated KFD profile schema. Every derived edge is
`inferred`, names the fixed V1 inference-rule identity, cites both exact event
identities and sequences, and stays in the
`kfd_runtime_profile_v1_event_sequence` producer-order domain. That sequence is
not a device clock or a global clock.

Host staging is not relabeled as a device copy. Runtime Profile V1 contains no
device copy-engine record and no inter-dispatch dependency edge, so the
`device_copies` and `dependencies` pages return typed `unavailable` status with
zero items. This is not evidence that no copy or dependency occurred. The KFD
dropped-event count and complete/incomplete runtime-history flag accompany every
page and dispatch completion/release stays unavailable when the evidence ends
before it is observed.

An optional Bundle V4 is admitted canonically and content-bound into the session
identity. It is a juxtaposed profiler input, not a join. Bundle V4 does not
retain a common direct-KFD dispatch identity, runtime API/copy records, or a
clock correlation. Dispatch bundles expose their own capture-local opaque
collector ticks without a frequency; ATT-reference bundles do not expose those
ticks. The query preserves Bundle V4 loss status and reports both per-dispatch
and cross-clock correlation as unavailable. It never matches by kernel name,
target string, ordinal, duration, or event proximity.

The JSONL protocol uses schema
`fe2o3-agent-runtime-causality-request-v1`. An `open` request carries canonical
lowercase hex for the exact runtime profile and optional profiler bundle.
Subsequent `binding`, `page`, `inspect_dispatch`, and
`profiler_juxtaposition` requests use unique nonzero request IDs, checked
revisions, and content-bound cursors. Malformed records consume the 64-attempt
budget. Oversized or unterminated input terminates with a typed error, and an
oversized result is replaced by a small terminal `response_too_large` record
before partial output is written.

This closes the local evidence-query portion of issue #215 runtime causality. A
producer for device-copy and dependency observations, an authenticated common
KFD/rocprof dispatch identity, and a clock-correlation interval with uncertainty
remain required. Distributed operation and global-time identities remain owned
by issue #182.
