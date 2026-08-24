# fe2o3 semantic import

`fe2o3-semantic-import` converts bounded debugger and profiler evidence into
canonical Semantic Trace V1 without loading a GPU runtime or opening a device.
It is an inert adapter library plus the stdin-only `fe2o3-trace-import` CLI.

## Accepted evidence

- `rocprofv3-json` accepts the documented rocprofv3 JSON format and selects one
  `rocprofiler-sdk-tool[process].buffer_records.kernel_dispatch[dispatch]`
  record. Grid and workgroup dimensions plus start/end timestamps become two
  `Observed` dispatch events. The source JSON SHA-256 and optional artifact
  identity are evidence references. Process, thread, queue, agent, kernel, and
  dispatch IDs are never copied into the trace. Raw identifiers affect only the
  exact source identity and identities derived opaquely from that source.
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
- `rocgdb-s09` accepts canonical LF-delimited evidence produced by the existing
  S09 `normalize_rocgdb` policy. It rejects raw hexadecimal addresses, numeric
  process/lane/wave identifiers, duplicate marker intervals, and mismatched
  HSACO bindings. Since normalization deliberately removes the numeric scope
  needed by Trace V1, the result is an event-empty partial capture.

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

## CLI

The CLI accepts evidence only on stdin and writes only canonical binary Trace
V1 on stdout. It does not accept paths, directories, devices, FIFOs, raw ATT
files, or handles. Input is capped at 8 MiB before parsing and output at 64 KiB;
arguments, JSON recursion, process/record counts, trace events, evidence sets,
and all integer conversions are independently checked.

```text
fe2o3-trace-import rocprofv3-json \
  --kir-sha256 KIR_SHA256 --kir-len KIR_BYTES --wave-width 64 \
  --process-index 0 --dispatch-index 0 \
  < results.json > dispatch.fe2o3tr1

fe2o3-trace-import rocprofv3-att-manifest \
  --kir-sha256 KIR_SHA256 --kir-len KIR_BYTES --wave-width 64 \
  --grid 1024,1,1 --grid-workgroups 4,1,1 --workgroup 256,1,1 \
  < ui_output_agent_N_dispatch_N/filenames.json > att.fe2o3tr1

fe2o3-trace-import rocgdb-s09 \
  --kir-sha256 KIR_SHA256 --kir-len KIR_BYTES --wave-width 64 \
  --artifact-sha256 HSACO_SHA256 --artifact-len HSACO_BYTES --artifact-format 1 \
  --grid 1024,1,1 --grid-workgroups 4,1,1 --workgroup 256,1,1 \
  < normalized-rocgdb.txt > debug.fe2o3tr1
```

`--artifact-sha256`, `--artifact-len`, and `--artifact-format` must appear
together. They are optional for profiler inputs and required for ROCgdb S09 so
the normalized `hsaco_sha256` fact is checked. Duplicate flags are rejected. Use `fe2o3-trace-query`
against the output for capability discovery, dispatch summaries, and bounded
agent-facing queries.

Raw `.att`/`.out`, compute-viewer wave JSON, PCs, register values, source-line
text, and inferred performance are outside this adapter. They need a future
authenticated artifact-to-KIR correlation format before they can establish
semantic site events without inventing facts.
