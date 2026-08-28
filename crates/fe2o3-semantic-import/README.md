# fe2o3 semantic import

`fe2o3-semantic-import` converts bounded profiler evidence into
canonical Semantic Trace V1 without loading a GPU runtime or opening a device.
It is an inert adapter library plus the stdin-only `fe2o3-trace-import` CLI.
It also owns Semantic Capture V1 and Semantic Counter Capture V2, strict canonical JSON containers for a
bounded set of profiler dispatch envelopes. The container complements rather
than replaces per-dispatch Trace V1.

## Accepted evidence

- `rocprofv3-json` accepts the documented rocprofv3 JSON format and selects one
  `rocprofiler-sdk-tool[process].buffer_records.kernel_dispatch[dispatch]`
  record. Grid and workgroup dimensions plus start/end timestamps become two
  `Observed` dispatch events. The source JSON SHA-256 and optional artifact
  identity are evidence references. Process, thread, queue, agent, kernel, and
  dispatch IDs are never copied into the trace. Raw identifiers affect only the
  exact source identity and identities derived opaquely from that source.
- `rocprofv3-capture` parses that same structured JSON once and imports every
  bounded kernel-dispatch record. It assigns a source-derived run identity,
  domain-separated redacted device identities from recorded agent handles, and
  source-and-ordinal-derived dispatch identities. Raw process, device, queue,
  kernel, and dispatch handles never enter the capture.
- `rocprofv3-counter-capture` accepts the rocprofiler-sdk 1.1 dispatch-counter
  JSON shape. This contract was checked against ROCm 7.2.4 SDK revision
  `97f5574fe2fdc7bef44fb01545347912ee9f1779`, its installed
  `source/docs/rocprofv3-schema.json`, and live `rocprofv3 --pmc ... -f json`
  output. It joins process-local `counters` definitions to
  `callback_records.counter_collection` records by exact agent/counter handles,
  then redacts those handles into source-bound identities.
- `rocprofv3-att-manifest` accepts the `filenames.json` file emitted for the
  rocprofv3 compute-viewer UI format. It recognizes the installed
  rocprofiler-sdk 1.1 shape (`wave_filenames`, `se_filenames`,
  `global_begin_time`, and `gfxv`) and the current versioned shape
  (`thread_trace: true`, `version`, and `wave_filenames`). Required containers
  must be nonempty and strings are bounded. A standalone manifest does not
  authenticate launch events or an ISA-to-KIR mapping, so the result is an
  event-empty partial capture. Referenced wave files, decoded
  instructions, raw `.att`/`.out`, `code.json`, and snapshots are not opened or
  parsed by this manifest adapter.

Every typed-library result includes the exact, domain-separated raw-source
content identity plus imported and unavailable fact lists in `ImportedTraceV1`.
The CLI publishes only the canonical Trace V1 byte stream. Opaque dispatch and clock identities are domain-separated
hashes of that identity and the selected record ordinal. KIR, artifact, and
source identities remain untrusted content claims; importing does not resolve
or authenticate them.

The report explicitly lists unavailable dispatch timing, invocation,
workgroup, wave, lane, KIR-site, memory, and register/value facts as applicable.
Sparse inputs are `Truncated(CollectorLoss)` with
`DispatchAlreadyActive`/`DispatchContinuesAfterCapture`. rocprof dispatch
records use full dispatch boundaries but remain truncated because they do not
contain the semantic execution history. `DispatchOutcomeV1::Completed` has the
narrow lifecycle meaning that rocprof recorded an `end_timestamp`; it does not
claim correct kernel output, successful harness completion, or absence of a
diagnostic/fault. Diagnostic and fault history is explicitly unavailable.

## Semantic Capture V1

`encode_capture_v1` produces one compact canonical JSON representation;
`decode_capture_v1` rejects whitespace variants, unknown or duplicate fields,
unknown schema versions, stale run/dispatch/reference identities, noncanonical
ordering, invalid timing, and documents over 8 MiB. The capture content address
is SHA-256 over a versioned domain and the complete canonical bytes. A capture
contains ordered run, device, and dispatch catalogs and is independent of
kernel shape.

Every recorded fact carries an origin from the closed vocabulary `declared`,
`proved`, `observed`, `inferred`, or `unavailable`. Current rocprof dispatch
captures use `observed` only for the structured device reference, launch, and
timestamp envelope. KIR, artifact, and source-map identities are caller
declarations, not authenticated correlations. Missing artifact or source-map
identities are explicit `unavailable` facts.

Kernel-dispatch buffer records are not sampled, but the format does not prove
that the collector lost no records. Capture V1 records `not_sampled` and,
separately, loss as `unknown`/`unavailable`. Completeness is
`partial_semantic_execution_history` even when every structured dispatch record
in the input was imported. Counter records, PC samples, ATT wave events, KIR
site history, and register/value state remain unavailable.

## Semantic Counter Capture V2

Counter Capture V2 is a separate versioned format, so closed Capture V1 bytes
and decoding behavior do not change. It preserves each finite binary64 counter
value as exact bits with `observed` origin, the dispatch launch/timing envelope,
the counter catalog, and source-global record ordinals. KIR, artifact, and
source-map identities remain caller declarations. Loss is explicitly unknown.

The rocprofv3 1.1 raw counter record has a counter ID but no hardware-instance
ID even though the catalog lists counter instances. The importer therefore
marks instance/dimension correlation unavailable and does not guess a mapping.
PC samples, ATT events, source/ISA correlation, semantic execution history, and
execution control also remain typed unavailable. Query-side sums group raw
records by dispatch and counter identity and are labeled `inferred`; raw values
remain `observed`.

## CLI

The CLI accepts evidence only on stdin and writes canonical binary Trace V1 or
canonical JSON Capture V1 on stdout. It does not accept paths, directories, devices, FIFOs, raw ATT
files, or handles. Input is capped at 8 MiB before parsing and output at 64 KiB;
arguments, JSON recursion, process/record counts, trace events, evidence sets,
and all integer conversions are independently checked.

```text
fe2o3-trace-import rocprofv3-json \
  --kir-sha256 KIR_SHA256 --kir-len KIR_BYTES --wave-width 64 \
  --process-index 0 --dispatch-index 0 \
  < results.json > dispatch.fe2o3tr1

fe2o3-trace-import rocprofv3-capture \
  --kir-sha256 KIR_SHA256 --kir-len KIR_BYTES --wave-width 64 \
  --artifact-sha256 ARTIFACT_SHA256 --artifact-len ARTIFACT_BYTES \
  --artifact-format 1 \
  --source-map-sha256 MAP_SHA256 --source-map-len MAP_BYTES \
  --source-map-format 1 \
  < results.json > run.fe2o3cap1

fe2o3-trace-import rocprofv3-counter-capture \
  --kir-sha256 KIR_SHA256 --kir-len KIR_BYTES --wave-width 64 \
  < counters_results.json > run.fe2o3ccap2

fe2o3-trace-import rocprofv3-att-manifest \
  --kir-sha256 KIR_SHA256 --kir-len KIR_BYTES --wave-width 64 \
  --grid 1024,1,1 --grid-workgroups 4,1,1 --workgroup 256,1,1 \
  < ui_output_agent_N_dispatch_N/filenames.json > att.fe2o3tr1
```

`--artifact-sha256`, `--artifact-len`, and `--artifact-format` must appear
together. They are optional metadata for profiler inputs. Duplicate flags are
rejected. The three `--source-map-*` flags are likewise atomic and apply only
to `rocprofv3-capture`. Use `fe2o3-trace-query` for Trace V1 and
`fe2o3-capture-query` for Capture V1, and `CounterQuerySessionV2` for bounded
Counter Capture V2 operations.

Raw `.att`/`.out`, compute-viewer wave JSON, PCs, register values, source-line
text, and inferred performance are outside this adapter. They need a future
authenticated artifact-to-KIR correlation format before they can establish
semantic site events without inventing facts.
