# Debugger and profiler architecture V1

Status: active closure contract for GitHub issue #215.

This document fixes the ownership, evidence, identity, resource, and
qualification rules for the fe2o3 debugger and profiler. A feature is not
complete merely because a schema can represent it. Completion requires a
producer, strict admission, a query path, hostile tests, and an acceptance test
at the evidence origin claimed by the response.

## Decisions

1. ROCgdb, rocprofv3, ROCprofiler SDK, ATT decoders, and
   rocprof-compute-viewer remain collection substrates. fe2o3 owns semantic
   identity, admission, correlation, bounded queries, capture comparison, and
   explanations.
2. The CPU simulator is a deterministic execution oracle. It is not a GPU
   timing or performance predictor and does not establish hardware execution.
3. Direct runtime inspection and control use KFD. Debugger or profiler tools
   may be launched as separately identified collectors, but HIP and HSA runtime
   APIs are not runtime dependencies or authority sources.
4. A service or agent client is read-only by default. Build, launch, attach,
   pause, instrumentation, and recapture require separate explicit
   authorization. Telemetry never grants compiler, artifact, load, dispatch,
   proof, or runtime authority.
5. MCP is an optional adapter. The canonical model is the versioned Rust
   schemas and deterministic non-agent CLI/service protocol.
6. Native hardware state is reported only after the backend authenticates its
   device, process, queue, dispatch, workgroup, wave, and lane identities.
   ROCgdb `target-id` text and KFD queue headers alone do not establish that
   chain.
7. Distributed analysis consumes the operation and clock-correlation
   identities owned by #182. #215 must not invent a parallel distributed
   identity system.

## Truth and availability

Every material field has one origin:

- `declared`: supplied by an admitted compiler, artifact, launch, or contract
  record;
- `proved`: supplied by an exact admitted proof or certificate;
- `observed`: supplied by the named runtime, debugger, profiler, or simulator
  execution;
- `inferred`: derived by a versioned rule whose input evidence is retained;
- `unavailable`: absent with an enumerated reason such as unsupported,
  optimized out, not captured, incomplete coverage, or truncated.

No query may convert absence into a negative observation unless the admitted
capture completeness record covers that event class and scope. A simulator
rerun and an instrumented rerun are different executions from the original GPU
dispatch. A proof about an abstract program is not an observation of machine
execution.

## Identity graph

Durable joins use content or protocol identities, never names, paths, raw
addresses, descriptors, queue IDs, process IDs, or collector-local code-object
IDs by themselves. The complete graph is:

```text
environment/tool/configuration
  -> device content identity / KFD node occurrence
  -> artifact / code object / source map
  -> run / process occurrence / queue occurrence / dispatch
  -> kernel / source site / MIR / KIR / schedule / LLVM / ISA interval
  -> workgroup / wave / lane / logical work item
  -> allocation / argument region / byte interval / property / event
```

Each edge states whether it is declared, proved, observed, inferred, or
unavailable. Optimized mappings can be one-to-one, one-to-many, many-to-one,
moved, duplicated, fused, outlined, inlined, or eliminated. They cannot be
collapsed to a preferred source line.

## Pinned baseline

The qualification host was measured on 2026-08-29. These pins describe the
tested baseline, not a claim about every installation.

| Component | Tested pin | fe2o3 use | Current authenticated boundary |
| --- | --- | --- | --- |
| KFD | Linux 6.8.0-124, MI300X/gfx942 | runtime observation and queue control | device, queue occurrence, suspend/resume, sequential XCC queue headers |
| ROCgdb | `rocm-rel-7.2-93`, GDB 16.3 | bounded MI launch/attach and process control | generic MI threads; no authenticated GPU wave identity |
| rocprofv3 | 1.1.0, git `97f5574f`, ROCm 7.2.4 | dispatch/counter/PC/ATT collection substrate | strict structured import and artifact references; missing output is unavailable |
| ROCm | 7.2.4 | installed collector/toolchain baseline | not runtime authority |
| rocprof-compute-viewer | not installed on the qualification host | external raw ATT drill-down | no fe2o3 decoded ATT acceptance yet |
| Mojo | not installed on the qualification host | comparison workflow only | no local parity measurement |

Feature comparison must be task based. fe2o3's intended differentiator is an
exact semantic/evidence graph and deterministic agent queries, not a claim that
it currently exposes more native registers, richer ATT decoding, or lower
overhead than the substrate tools.

The pinned task-level comparison is recorded in
[`debugger-profiler-task-matrix-v1.md`](debugger-profiler-task-matrix-v1.md).

The comparison baseline is the public
[ROCgdb documentation](https://rocm.docs.amd.com/projects/ROCgdb/en/latest/),
[ROCprofiler SDK documentation](https://rocm.docs.amd.com/projects/rocprofiler-sdk/en/latest/),
[ROCprof Compute Viewer overview](https://rocm.docs.amd.com/projects/rocprof-compute-viewer/en/amd-mainline/quick-reference/rcv-at-a-glance.html),
and [Mojo GPU debugging](https://docs.modular.com/mojo/tools/gpu-debugging/).
The Mojo page currently describes a CUDA-GDB/NVIDIA debugging path; that does
not establish an AMD debugger comparison on this qualification host.

## Hard bounds

The implementation rejects input above its compiled bounds before allocating
unbounded state. Important current ceilings include:

| Surface | Hard ceiling |
| --- | ---: |
| debugger request line | 1 MiB (generic), 64 KiB (live GPU V3) |
| debugger response line | 16 MiB maximum, 2 MiB default/live GPU V3 |
| query page | 4,096 items (generic), 256 items (live GPU V3) |
| relative memory read | 1 MiB |
| debugger breakpoints/watchpoints | 4,096 each |
| profiler command arguments | 256 arguments, 4,096 bytes each |
| profiler stdout/stderr | 16 MiB each |
| profiler capture storage | 4 GiB |
| profiler collection time | 900 seconds |
| profiler artifacts | 4,096 entries, depth 8 |

These are safety ceilings, not recommended defaults or performance budgets.
Every capture records its requested limits, truncation/loss state, and actual
artifact sizes. T6 requires measured overhead budgets for no-capture,
counters, PC sampling, ATT, debugger-stop, and instrumented modes before a mode
can be called production-qualified.

## Threat model

All external bytes, paths, collector output, target declarations, cursors, and
client requests are hostile. Admission must reject:

- symlink, rename, mutation, truncation, or path substitution after inspection;
- stale source maps, artifacts, topology, devices, queues, dispatches, and page
  cursors;
- queue-ID reuse or a process/queue occurrence different from the authorized
  occurrence;
- collector-local identifiers presented as durable cross-capture identity;
- noncanonical JSON/CSV numbers, duplicate fields, unknown schema fields,
  excessive nesting, excessive item counts, and trailing data;
- raw descriptors, addresses, tokens, or execution capabilities in responses;
- incomplete, sampled, or truncated evidence presented as complete; and
- tool output that reports success without producing the requested artifacts.

Cleanup owns and terminates only processes and queue suspensions acquired by
the session. A failed identity revalidation poisons the stateful operation and
cannot be cleared by retrying with a new caller assertion.

### Variant regression evidence

`fe2o3-semantic-query` keeps Bundle V4's same-artifact comparator unchanged.
The separate Profiler Variant V1 path instead anchors two canonical treatment
manifests to one caller-declared semantic-workload identity and the exact
supplied rocprofv3 source bytes. Semantic Import re-imports those bounded bytes
and requires every source-derived normalized Bundle V4 fact to match the
supplied bundle. Schema-valid trailing catalog entries that no dispatch uses do
not participate in comparability. This is a content-bound supplied-source
relation, not a signature or external provenance claim. Admission requires
exact environment, collector
tool/configuration, stable per-dispatch device, selector, workload, and launch
axes while permitting KIR, schedule, artifact, optional ISA projection, and
observed HSACO resources to differ.

Counter V2 and PC V3 wires remain unchanged. A separate in-process relation
sidecar maps process-local raw `dispatch_id` values to exact Bundle dispatches;
collection order, equal timing, or equal launch envelopes never establish that
relation. Each side capture is also re-imported from the exact supplied bytes
and its normalized process, device, source ordinal where represented, launch,
timing, KIR, artifact, and source-map axes are checked against the mapped
Bundle record. Missing, duplicate, unknown, or mismatched dispatch IDs leave
the side evidence typed unavailable. Independently collected V2/V3 files have
no common exact supplied-source relation and remain unavailable. Because
Bundle V4 has no content-bound kernel-symbol field, HSACO resource admission
currently requires a single-kernel artifact instead of trusting a
caller-selected metadata ordinal.

The only seeded explanation is a deterministic co-observation: captured
duration increased while declared schedule identity and observed static HSACO
resources changed. It cites the exact manifests, bundles, dispatch records,
schedule identities, and resource identities and explicitly does not claim
causation or superiority. Duration, counter, and resource deltas are all empty
unless every narrow comparison axis is exact; device comparison uses the exact
stable device assigned to each dispatch and ignores unused catalog entries.
Decoded ATT, runtime/copy events, PC-to-semantic/ISA
correlation, semantic-to-schedule-to-ISA localization, counter completeness,
dispatch argument/input equivalence, loss-free complete-workload coverage,
clock normalization, and causal attribution remain typed unavailable. Exact
comparison axes therefore do not imply a complete workload comparison. This
foundation does not satisfy the T3 exit by itself.

## Closure matrix

The issue can close only when every row is accepted on the current production
tree. A dependency may remain in another issue only when its versioned input
contract and an unavailable response are both tested here.

| Track | Exit evidence | Current closure gap |
| --- | --- | --- |
| T0 baseline/ADR | archived capture queries without a source checkout; pinned feature/task matrix; ownership, threat, completeness, and budget policy | measured overhead budgets and installed compute-viewer/Mojo task comparison are absent |
| T1 semantic map | elementwise, collective, and tiled kernels round-trip source to ISA and ISA to source; optimization shapes and hostile substitution tested | general schedule/LLVM/ISA intervals and optimization-shape round trips are incomplete |
| T2 debugger | seeded OOB and barrier divergence identify dispatch, site, workgroup, wave/lane scope, region/phase contract, and origin without raw-log parsing | V4 exact KFD publication-to-structured-MI correlation is implemented and the installed ROCgdb path reports typed stopped-state unavailability; authenticated native registers/source/memory and general cooperative live-kernel acceptance remain incomplete |
| T3 profiler | seeded schedule/resource regression is attributed to semantic/IR/ISA change with exact comparable evidence | strict variant admission and schedule/resource co-observation exist; authenticated semantic/IR/ISA localization, decoded ATT/runtime/copy attribution, and causal end-to-end explanation are incomplete |
| T4 agent protocol | fresh non-privileged client completes T2/T3 diagnoses, cites evidence, and plans the minimum next capture | canonical read-only service and complete operation acceptance are incomplete |
| T5 distributed | eight-GPU overlap regression localizes to an admitted graph edge or interval while preserving clock/loss uncertainty | awaits versioned #182 operation, transfer, collective, and clock-correlation inputs |
| T6 qualification | overhead, target, pagination, loss, reset, optimized-out, partial-capture, and comparative usability gates pass | broad target/overhead/reset/usability evidence is incomplete |

## Required acceptance records

Every committed acceptance fixture contains:

1. exact compiler, map, artifact, device, run, dispatch, and configuration
   identities that are available;
2. a capability record enumerating unsupported and uncaptured state;
3. evidence IDs for every material diagnosis field;
4. bounded raw inputs or content-addressed references plus loss/truncation
   status;
5. deterministic query requests and canonical responses;
6. a hostile counterpart that changes one identity, scope, or completeness
   field and fails closed; and
7. for measured modes, warm-up policy, repetitions, clock domain, duration
   statistic, storage, and observed perturbation relative to the admitted
   no-capture comparator.

Closing #215 means these records pass in production CI and on the declared
hardware targets. Documentation, a UI projection, or a schema without its
producer and acceptance evidence is insufficient.
