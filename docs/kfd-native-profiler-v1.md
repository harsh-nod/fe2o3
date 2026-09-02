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
- content-bound host staging writes and reads, with allocation-relative ranges;
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
duration. A capture does not establish that an unavailable rocprof or ATT event
did not occur.

## Agent query

`fe2o3-kfd-profiler-query` opens one single-link regular file with
`O_NOFOLLOW`, checks bounded same-descriptor metadata, reads it twice, and
admits only canonical V1 bytes. It accepts versioned JSONL requests on stdin:

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

## MI300X qualification command

The existing exact-artifact qualification benchmark can publish a capture when
given both an explicit scope and a new output path:

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

## ROCprof boundary

`cargo fe2o3 profile` still owns strict rocprofv3 JSON/CSV and ATT-reference
admission. A successful rocprofv3 process with no schema-valid artifact remains
typed unavailable; it is never replaced with inferred dispatch records. The
direct-KFD capture provides separately identified runtime evidence that can be
queried today. Joining it to rocprof counters, PC samples, or decoded ATT will
require an independently observed common identity and a new additive schema.
