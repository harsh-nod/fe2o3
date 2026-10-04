# Direct-KFD runtime profiler V1

`fe2o3-profiler-protocol` is the authority-free observation path for direct-KFD
executions that ROCprofiler SDK does not observe. It is additive to Bundle V4:
it does not decode or replace rocprofv3, ATT, or rocprof-compute-viewer output.

## Producer contract

Call `KfdRuntimeBackendV1::enable_profiler_v1` before constructing a
`RuntimeContextV1`. The caller supplies a nonzero, per-execution 256-bit capture
scope and a bounded event limit. After logical context cleanup and
`shutdown_native_v1`, `finish_profiler_v1` returns one canonical capture.

The current production emitter records successful direct-KFD transitions:

- logical stream, allocation, module, and kernel lifecycle;
- host staging writes and reads with allocation-relative ranges and an explicit
  range-only or content-identity policy;
- creation and teardown of the backend's native queue;
- the exact artifact, kernel name/signature, dispatch shape, launch geometry,
  typed binding regions, and the point after AQL publication succeeds;
- KFD completion observation, submission release, and host-monotonic phase
  durations.

Resource identities are domain-separated hashes scoped to the capture. The
wire contains no raw device address, descriptor, queue ID, packet ID, or
runtime handle. Recording is a bounded prefix. Once capacity or encoding loses
an event, later events are counted but not retained, and completeness is false.
Profiling evidence never participates in launch authority.

Typed semantic profiling is a separate opt-in extension. Call
`enable_profiler_with_semantic_profile_v1` before resource creation, then use
`finish_profiler_with_semantic_profile_v1` for the frozen V1 capture and its
separately versioned KFD Runtime Semantic Profile V1 sidecar, or
`finish_profiler_with_dispatch_timestamps_v2` for runtime-owned timestamp and
semantic custody. The original enable, Runtime Profile V1 finish, and timestamp
V1 finish paths retain their existing allocation and validation surface.

The sidecar has exactly one ordered record for every retained publication.
Each record binds the Runtime Profile V1 content identity, event identity and
sequence, dispatch shape, launch geometry, and an explicit ordinary or typed
atomic/collective classification. Atomic contracts include success and
compare-exchange failure ordering plus weak mode; collective contracts include
the exact workgroup participant count. Structural sidecar validation grants no
authority. Only the distinct non-constructible V2 runtime custody type
authenticates a sidecar for the additive semantic-query V2 report, which is
still read-only and does not prove machine semantics.

`KfdRuntimeProfilerConfigV1::new` selects range-only host content records so
large staging buffers do not require an extra digest on every operation.
`with_host_content_identities` explicitly selects content identities. Complete
full-buffer writes reuse the runtime's existing SHA-256 when available; other
captured host content is hashed synchronously. Both modes retain the same
lifecycle and range checks, and the selected policy is part of the capture.

The target-profile field is bounded exact text plus wave width, so the protocol
does not require a wire redesign for a newly admitted KFD target. The production
runtime currently emits only its admitted `gfx942:xnack-` Wave64 profile.

## Truth boundary

The following remain typed unavailable in every V1 runtime capture:

- rocprofv3 dispatch correlation;
- device-clock timestamps and copy-engine events;
- counters and PC samples;
- decoded ATT events;
- authenticated source/IR/ISA correlation; and
- semantic execution history below the dispatch envelope.

`host_write` and `host_read` are observations of the runtime's host staging
path. They are not observations of a GPU DMA engine. `publish_to_completion_ns`
uses the host monotonic clock around completion polling; it is not a GPU device
duration. Host timings are measured while the synchronous observer is enabled
and can include observer work, including publication-record encoding before
completion is observed. A capture does not establish that an unavailable
rocprof or ATT event did not occur.

## Agent query

`fe2o3-kfd-profiler-query` uses Linux `openat2` with `NO_SYMLINKS` and
`NO_MAGICLINKS` across every path component, admits one private single-link
regular file, checks stable same-descriptor metadata and bytes twice, and
rejects path substitution before admitting canonical V1 bytes. It accepts
versioned JSONL requests on stdin:

```json
{"schema":"fe2o3-agent-kfd-profiler-request-v1","request_id":1,"operation":"discover_capabilities"}
{"schema":"fe2o3-agent-kfd-profiler-request-v1","request_id":2,"operation":"inspect_capture"}
{"schema":"fe2o3-agent-kfd-profiler-request-v1","request_id":3,"operation":"list_events","limit":64}
```

The response schema is `fe2o3-agent-kfd-profiler-response-v1`. Event pages are
bound to the exact canonical capture identity, and `inspect_dispatch` returns
the content-bound publication and optional completion records for one opaque
dispatch identity. Request, response, page, capture, event, and binding counts
all have hard limits. The query process has no build, load, dispatch, attach,
pause, or recapture operation.

Valid requests that cannot be satisfied return a typed `error` response and do
not end the JSONL session. A response decoder enforces the version, bounds, and
canonical encoding. Malformed requests receive a typed error with request ID
zero unless a bounded unsigned request ID can be recovered.

## MI300X qualification command

The existing exact-artifact qualification benchmark can publish a range-only
capture when given both an explicit scope and a new output path:

```bash
cargo run -p fe2o3-runtime --features hardware-qualification \
  --example gfx942-runtime-vecadd-benchmark -- \
  UNIQUE_ID 1 1 1 \
  1111111111111111111111111111111111111111111111111111111111111111 \
  /tmp/fe2o3-kfd-profile-v1.json

printf '%s\n' \
  '{"schema":"fe2o3-agent-kfd-profiler-request-v1","request_id":1,"operation":"inspect_capture"}' \
  '{"schema":"fe2o3-agent-kfd-profiler-request-v1","request_id":2,"operation":"list_events","limit":64}' \
  | cargo run -p fe2o3-profiler-protocol --bin fe2o3-kfd-profiler-query -- \
      /tmp/fe2o3-kfd-profile-v1.json
```

This is an exact qualification fixture, not a general application authority.
The profiler API itself is independent of kernel name and applies to every
module, ABI, binding roster, and launch geometry admitted by the runtime.

The opt-in `scripts/ci-local.sh hardware-smoke` lane runs a separate short
producer/query acceptance with a fresh scope. It is deliberately distinct from
the 5-warmup/30-repetition profiler-overhead protocol.

## Rocprof wrapper host-wall comparison

`cargo fe2o3 profile --measure-direct-kfd-wrapper-overhead` runs exact paired
invocations of one caller-declared direct-KFD target without a collector and
through the sealed rocprofv3 `--kernel-trace` wrapper. It is an explicitly
authorized process-wall comparison, not a GPU capture-overhead measurement.
The dry run binds the sealed harness, target, collector and native SDK closure,
target and collector argument vectors, allowlisted environment, working
directory identity, direct-KFD topology, limits, order, repetitions, and caller
candidate budget. Collection additionally requires the plan-derived
`--acknowledge-repeated-target-execution` digest because repeated targets may
have external side effects.

The required qualification shape is five warmup pairs followed by thirty
measured pairs. Even pairs run raw then wrapped; odd pairs reverse the order.
Every process uses the existing profile process-group supervisor, timeout, and
bounded stdout/stderr capture. Timing uses `CLOCK_MONOTONIC_RAW` immediately
before spawn through supervision and bounded pipe drain. No observations are
discarded as outliers. The transaction has a one-hour harness limit and rejects
a requested repetition/timeout product above that bound.

```bash
TARGET=/absolute/path/to/direct-kfd-target
OUTPUT=/absolute/new/output-directory

cargo fe2o3 profile --kind dispatch-json \
  --tool /opt/rocm-7.2.4/bin/rocprofv3 \
  --python /usr/bin/python3.12 \
  --output-dir "$OUTPUT" --timeout-ms 30000 \
  --measure-direct-kfd-wrapper-overhead \
  --overhead-warmup-pairs 5 --overhead-measured-pairs 30 \
  --overhead-candidate-budget-bps 1000 -- \
  "$TARGET" TARGET_ARGS...
```

Use the printed `collection-authorization` and
`repeated-target-execution-acknowledgement` in a second invocation with
`--collect`. The durable
`fe2o3-rocprof-wrapper-host-wall-comparison-v1.json` records every leg's exact
outcome, duration, stream identities/truncation, and complete bounded wrapper
output inventory. A failed or truncated warmup or measured leg suppresses the
summary. The median signed per-pair delta and candidate comparison never grant
production qualification.

An empty rocprof output inventory means only that this exact wrapper execution
created no admitted artifact. It is not proof that no GPU work occurred or
that rocprof observed nothing internally. Kernel-trace capture overhead and
loss/completeness remain typed unavailable without an admitted capture;
counter, PC, ATT, and debugger overhead remain typed unmeasured. The harness
records the target as caller-declared direct KFD and does not elevate that
declaration into runtime evidence. See the checked-in
[MI300X host-wall record](evidence/mi300x-rocprof-wrapper-host-wall-2026-09-03.md).

## ROCprof boundary

`cargo fe2o3 profile` still owns strict rocprofv3 JSON/CSV and ATT-reference
admission. A successful rocprofv3 process with no schema-valid artifact remains
typed unavailable; it is never replaced with inferred dispatch records. The
direct-KFD capture provides separately identified runtime evidence that can be
queried today. The additive
[`runtime-causality-jsonl`](runtime-causality-v1.md) service pages exact runtime
events, queue/stream/allocation membership, host staging, and inferred
publication/completion/release lifecycle edges. It also content-binds an
optional Bundle V4 as a juxtaposed input. It does not join records across the
collectors: rocprof counters, PC samples, decoded ATT, and dispatch clocks still
require an independently observed common identity and clock relation.

The PC/Source-ISA V1 query now handles a narrower, independently admitted
rocprof PC-sample case: exact rocprof source, Capture V3, HSACO, code-object
relation, and Characteristic V1 bytes can be joined to sparse source/IR/ISA
coordinates. This does not join those PC samples to a direct-KFD profile;
direct-KFD still has no observed PC or cross-collector common identity.

### Live qualification

The opt-in `--direct-kfd-runtime-capture` path juxtaposes one exact rocprofv3
run with one canonical capture emitted by any target using the production KFD
runtime profiler. The absolute path must be new, outside collector output, use
a canonical parent, and occur exactly once in the target argv. For example:

```console
RUNTIME=/absolute/new/direct-kfd-runtime.json
OUTPUT=/absolute/new/rocprof-output
TARGET=/absolute/gfx942-runtime-vecadd-benchmark
SCOPE=1111111111111111111111111111111111111111111111111111111111111111

cargo fe2o3 profile --kind dispatch-json \
  --output-dir "$OUTPUT" \
  --direct-kfd-runtime-capture "$RUNTIME" -- \
  "$TARGET" unique-id 1 1 1 "$SCOPE" "$RUNTIME"

cargo fe2o3 profile --kind dispatch-json \
  --output-dir "$OUTPUT" \
  --direct-kfd-runtime-capture "$RUNTIME" \
  --collect --authorize-collection <plan-sha256> -- \
  "$TARGET" unique-id 1 1 1 "$SCOPE" "$RUNTIME"
```

The first command is inert and prints the path-bound authorization. The second
must use the same still-absent runtime path and output directory. A successful
transaction publishes these generated members before its manifest:

- `fe2o3-direct-kfd-runtime-profile-v1.json`, the exact canonical target
  capture copied from an already retained descriptor; and
- `fe2o3-direct-kfd-rocprof-qualification-v1.json`, which binds exact collector,
  configuration, argv, environment, target, stdout/stderr, complete collector
  inventory, exit, and runtime identities.

Admission rejects a stale file, symlink, parent or leaf substitution,
non-private or multiply linked file, overflow, noncanonical runtime capture,
reserved collector output, or durability/readback mismatch. Both generated
members count against the plan's storage limit and are published with
no-replace transaction semantics. The runtime copy is reread and decoded from
output custody before manifest publication.

The qualification outcome distinguishes a runtime capture with no dispatch,
a successful collector with no artifacts alongside runtime-observed
dispatches, and collector artifacts that remain unjoined. No outcome invents a
common dispatch, code-object, or clock identity. ATT and PC-sampling facts say
only whether those capabilities were requested or probed in this exact record.
The record grants no collection or dispatch authority and never proves
universal collector inability.

The checked-in [MI300X qualification evidence](evidence/mi300x-direct-kfd-rocprof-2026-09-03.md)
records one ROCprofiler SDK 1.1.0/ROCm 7.2.4 run in which the direct-KFD target
published, completed, and released three dispatches while the successfully
exited collector produced no artifacts. That is an exact observed outcome,
not a claim about every SDK version or direct-KFD workload. A follow-up raw
collector probe confirmed that kernel tracing was enabled, the collector client
and contexts started, the requested output directory was writable, and the
minimum output threshold was zero, while the collector still reported zero
services generating output. The installed CLI exposes no direct-KFD dispatch
registration mode. Consequently, adding an output flag or weakening artifact
admission would not repair this boundary; a future collector path needs an
actual common direct-KFD queue/dispatch observation contract.
