# fe2o3 semantic query

`fe2o3-semantic-query` is a bounded, deterministic, read-only debugger query
surface over canonical Semantic Trace V1 files. It works with every conforming
trace producer; it does not compile, load, dispatch, stop, or mutate a kernel.

The typed library and `fe2o3-trace-query` JSON CLI expose:

- capability discovery and a dispatch/capture summary;
- paginated workgroup, wave, lane, KIR-site, operation-occurrence, memory-access,
  allocation-region, diagnostic/fault, and provenance/evidence observations;
- exact event sequence, scope, unresolved site claim, provenance, evidence,
  capture completeness, and boundary information; and
- a bounded `plan-next-capture` operation for the closed goals `memory_fault`,
  `barrier_divergence`, `performance_hotspot`, and `correctness_mismatch`;
- a compact `diagnosis-status` observation report containing only observed
  faults and missing facts, never hypotheses; and
- explicit unavailable capabilities for state that Trace V1 does not carry.

Native pointers, GPU virtual addresses, queue identifiers, KFD handles, register
contents, and source-variable values are neither accepted nor returned. Opaque
dispatch and evidence identities are inert byte arrays. Allocation identity is
the trace-local `(ordinal, generation)` pair.

## CLI

```text
fe2o3-trace-query capabilities < trace.fe2o3tr1
fe2o3-trace-query summary < trace.fe2o3tr1
fe2o3-trace-query lanes --limit 32 --workgroup 0,0,0 --wave 0 < trace.fe2o3tr1
fe2o3-trace-query memory-accesses --allocation 2,0 --sequence-start 10 < trace.fe2o3tr1
fe2o3-trace-query faults --limit 64 < trace.fe2o3tr1
fe2o3-trace-query plan-next-capture --goal memory_fault < trace.fe2o3tr1
fe2o3-trace-query diagnosis-status --goal correctness_mismatch < trace.fe2o3tr1
```

Other page commands are `workgroups`, `waves`, `sites`, `occurrences`,
`memory-regions`, and `evidence`. The CLI is deliberately stdin-only and never
opens a path. `--cursor` carries both fields of the query-bound cursor returned
by the previous page as `QUERY_BINDING_HEX:EVENT_POSITION`; identities serialize
as fixed lowercase hex. Filters are conjunctive. JSON output has an exact byte
ceiling configured by `QueryLimitsV1`; input, page size, arguments, and resident
response construction are independently bounded.

## Capture planning

Capture plans are deterministic for fixed canonical trace bytes and query-tool
version. Each ordered step contains only facts not already established by the
trace's observed events and validated completeness/boundary state. It names the
tool family, reproduction or selected semantic scope, qualitative runtime and
storage overhead, privilege and attach requirements, separate-capture
exclusions, and why the fact would discriminate the selected goal. Existing
evidence is referenced by the trace binding, header claims, event sequence, and
inert evidence identity. Header identities and producer names remain untrusted
claims.

The current supported paths are simulator Trace V1, rocprofv3 dispatch JSON,
Counter Capture V2, PC Sample Capture V3, the rocprofv3 ATT manifest, and
normalized ROCgdb imports. Counter values and PC samples remain unsupported
Trace V1 facts even though their separate capture formats have read-only query
surfaces. Decoded ATT wave timelines, register/source values, and output
comparisons remain unsupported. ATT is
selected-wave evidence and never establishes full-grid coverage. Trace V1 has
no compute-unit selector, so hardware steps report it as unspecified rather
than inventing one.

The direct-KFD observation boundary currently reports only redacted queue
lifecycle facts. Plans label direct-KFD dispatch capture as future/unavailable;
they do not claim that an actual KFD dispatch, completion, timing interval, or
semantic execution trace exists. Plans and diagnosis status never claim a
successful diagnosis or performance prediction.

Site ordinals and Kernel-IR content identities remain producer claims. The query
surface does not resolve them to names or source locations without a future
authenticated catalog adapter.

## Profiler capture queries

The same crate provides the read-only `CaptureQuerySessionV1` protocol and
`fe2o3-capture-query` CLI for canonical Semantic Capture V1 documents:

```text
fe2o3-capture-query capabilities < run.fe2o3cap1
fe2o3-capture-query open < run.fe2o3cap1
fe2o3-capture-query list-runs < run.fe2o3cap1
fe2o3-capture-query list-devices --limit 32 < run.fe2o3cap1
fe2o3-capture-query list-dispatches --limit 128 < run.fe2o3cap1
fe2o3-capture-query inspect-dispatch --dispatch DISPATCH_HEX < run.fe2o3cap1
fe2o3-capture-query hotspots --limit 32 < run.fe2o3cap1
```

These operations expose exact capture/run/device/dispatch/source/KIR/artifact/
source-map identities together with each fact's origin, sampling status, loss
status, and completeness scope. Pagination cursors bind both the complete
capture content address and operation; they cannot be replayed against another
capture or list. Hotspots sort observed dispatch-envelope duration in ticks,
with opaque dispatch identity as the deterministic tie-breaker. They do not
normalize clocks, infer kernel causality, predict performance, or claim that
unrecorded dispatches did not exist.

Input, page item count, conservative page construction, encoded response bytes,
CLI arguments, and cursor positions have independent hard bounds. Capability
discovery reports counter records, PC samples, ATT wave events, semantic
execution history, and execution control as typed unavailable capabilities.
The query surface never invokes rocprofv3, opens a device, reads a path, or
grants execution authority.

`CounterQuerySessionV2` extends the same content-bound, read-only protocol to
Semantic Counter Capture V2. It lists counter definitions, dispatch envelopes,
and exact raw values; filters values by dispatch/counter identity; inspects a
dispatch; and ranks sums by `(dispatch, counter)`. Raw values are `observed`.
Sums are `inferred` and identify the aggregation rule and input count. Cursors
bind the capture, operation, and both filters. Capabilities explicitly report
hardware-instance, source, ISA, PC-sample, ATT, execution-history, and
execution-control data as unavailable. `query_json` enforces the configured
response ceiling and emits deterministic JSON for agent callers.

`PcSampleQuerySessionV3` and the stdin-only `fe2o3-pc-sample-query` CLI extend
the protocol to stochastic PC records:

```text
fe2o3-pc-sample-query capabilities < run.fe2o3pcap3
fe2o3-pc-sample-query open < run.fe2o3pcap3
fe2o3-pc-sample-query list-dispatches --limit 64 < run.fe2o3pcap3
fe2o3-pc-sample-query list-samples --limit 128 < run.fe2o3pcap3
fe2o3-pc-sample-query pc-hotspots --limit 32 < run.fe2o3pcap3
```

Raw sample pages retain `observed` origin, code-object-relative PC, opaque
collector timestamp, logical execution mask, and sampled wave location.
Hotspots count records by `(dispatch, code object, relative PC, instruction
type)` and are labeled `inferred`; they are not instruction counts, time
attribution, or complete execution coverage. Pages stream at most
`page_limit + 1` ordinary records. Hotspot aggregation has an independent
65,536-group ceiling, and encoded JSON has a 1 MiB hard ceiling.

Capabilities explicitly mark clock conversion, source/ISA correlation,
complete instruction history, cross-capture comparison, execution control, and
decoded ATT wave timelines unavailable. ATT decoder availability is a
collection-host/toolchain property; PC Capture V3 neither invokes nor replaces
the SDK decoder.

## Capture comparison

`compare_dispatch_captures_v1` and `compare_counter_captures_v2` perform a
strict compatibility audit without changing capture bytes. Current device and
counter identities are deliberately bound to one complete profiler source, and
captures contain no authenticated stable environment identity. Consequently,
two distinct captures return `unavailable_source_bound_identity`, no deltas,
the supporting or comparison-blocking capture content identities, and a minimal next
capture requirement using the existing capture-plan tool/fact vocabulary.
Identical canonical bytes report only exact byte equality; they do not establish
a cross-run regression and still request stable authenticated identities for a
future comparable capture. Equality of KIR, artifact, and source-map fields is
reported as equality of `declared` claims, not as authenticated correlation. No
raw clock ticks or counter names are compared.

The stdin-only `fe2o3-capture-compare {dispatch-v1|counter-v2}` CLI accepts one
bounded binary frame: an eight-byte little-endian baseline length, the baseline
capture, then the candidate capture. It emits bounded deterministic JSON and
never opens paths or invokes a runtime. A future useful cross-run comparison
requires an authenticated stable environment/device/counter identity evidence
contract; caller declarations are intentionally not accepted as a substitute.
PC Capture V3 is intentionally absent from comparison because its run, device,
code-object, and dispatch identities are bound to the complete source bytes.

## Semantic Profiler Bundle V4

`ProfilerQuerySessionV4` and `fe2o3-profiler-query` provide a unified,
content-bound surface for Bundle V4:

```text
fe2o3-profiler-query capabilities < run.fe2o3prof4
fe2o3-profiler-query list-dispatches --limit 64 < run.fe2o3prof4
fe2o3-profiler-query hotspots < run.fe2o3prof4
fe2o3-profiler-query list-att-references < att.fe2o3prof4
fe2o3-profiler-query waits < att.fe2o3prof4
fe2o3-profiler-query plan-waits < att.fe2o3prof4
```

Runs, stable devices, dispatch envelopes, duration hotspots, and ATT artifact
references are paginated with bundle-and-operation-bound cursors. Every result
links its bundle and record evidence and retains `declared`, `observed`,
`inferred`, or `unavailable` origin. A wait query over an ATT reference
manifest returns a typed unavailable item; an empty decoded event set is never
invented. Next-capture planning is bounded and requests the supported rocprof
Compute Viewer decoder plus a future strict decoded-event importer. It states
that selected-wave ATT evidence does not prove full-grid coverage.

`fe2o3-profiler-compare bundle-v4` accepts the same two-capture length-prefixed
frame as the older comparator. It checks exact environment, collector tool,
collector configuration, stable-device, dispatch sequence/device/launch, KIR,
and artifact content claims and emits an evidence-linked numeric
dispatch-duration delta only when those claims are present and match. An
artifact identity missing from both runs is `unavailable`, never an exact
match. Argument and input content identities are not represented, so this is
not a regression diagnosis.
Content equality is explicitly equality of caller-declared identities, not
runtime authentication. `counter-delta-v2` emits deterministic
binary64 sums for counter dimensions with observed records in both captures;
`pc-delta-v3` currently returns typed unavailable because V3 code-object
identities are capture-local and cannot safely join relative PCs across runs.
Missing dimensions are unavailable, not zero. V2/V3 stable environment
identity, stable cross-run PC identity, decoded ATT/waits, clock conversion,
causal diagnosis, and performance prediction remain unavailable.

## Read-only agent profiler service

`AgentProfilerServiceV1` and `fe2o3-profiler-service jsonl` expose a persistent,
versioned local protocol over already collected Profiler Bundle V4 evidence.
This is a library and stdin/stdout JSONL boundary for local agents, not an MCP
server or a profiler collector. It never invokes rocprofv3, launches or attaches
to a process, opens a path, or grants execution authority. An `open_capture`
request carries exact canonical Bundle V4 bytes as bounded lowercase hex; the
service returns their full content identity and retains at most four captures
by default.

The V1 operation inventory includes capability discovery, capture open, bounded
pages for runs/devices/dispatches/ATT references/duration hotspots/waits,
dispatch inspection, dispatch-scoped kernel identity inspection, Bundle V4
comparison, and bounded next-capture planning. The agent Plan V1 request names
one typed ambiguity, the exact missing-evidence set, kernel/dispatch evidence
selectors, and caller-declared overhead, storage, and record ceilings. The
service checks
that set against the opened capture and emits at most one minimum
discriminating recipe. Capability discovery returns both nested Plan V1 schema
names and all plan-specific limits. A recipe carries logical counters and data
classes, required-but-unverified rocprofv3/Compute Viewer capabilities, the
explicitly unavailable logical-counter mapper or strict decoded-event importer
when it is needed, mutual exclusions, sampling/completeness limits, profiler
privilege, typed-unavailable overhead when no measurement for the planned action
was admitted, and a bounded storage estimate derived from the existing bundle
size. Storage is capped at the 4 GiB architecture ceiling; an estimate above the
caller's smaller ceiling is reported as a constraint violation without clamping.
Because overhead is unavailable, the planner does not claim that a proposed
capture satisfies the caller's overhead ceiling.
The service can only plan: every stateful collection
requires separate explicit authorization, and the service has no launch or
attach authority. Dispatch and kernel selectors identify evidence in the open
capture. A new-capture recipe derives a reusable launch shape only after Bundle
V4 admits the exact logical-grid/workgroup relation and the planner reconstructs
it defensively. The observed dispatch device must join exactly one source-bound
device carrying a declared stable identity; the selector also binds that stable
identity and the declared environment, collector tool, and collector
configuration. The capture-local observed source device remains a separate plan
provenance entry rather than becoming an actionable future selector; the observed
launch origin is bound explicitly. The recipe does not reuse the old dispatch
occurrence. Missing, duplicate, or inconsistent device/launch relations are
rejected during admission and cannot become future selectors. CU selectors are
rejected because Bundle V4 has neither authenticated
CU topology nor an authenticated collector capability for targeting it.

Pages reuse Bundle V4 content-bound cursors. Every successful value carries the
service-contract identity, exact capture identities, relevant record
identities, and a homogeneous, mixed, or empty aggregate of the item-level
truth origins. Plan results additionally bind the canonical planning request,
the full bundle/environment/tool/configuration and relevant joined-device content identities, any
available artifact content identity, the full KIR claim, the selected dispatch,
and any derived launch selector. Every handled request
attempt, including zero and duplicate IDs, consumes the request budget.
Responses have checked monotonic revisions. A private, service-instance response
binding rejects request ID, revision, audit, plan, or evidence substitution
before encoding; that binding is deliberately not serialized and makes no
client-verifiable service-state claim. Resident captures, configured input bytes, page
items, plan inputs, and encoded response bytes have independent hard ceilings.

The closed inventory also admits workgroup/wave/lane inspection, source/IR/ISA
correlation, fault, decoded wait/memory/barrier, property, causal regression
explanation, and reproducer export requests so callers do not need to infer
support from missing methods. Bundle V4 does not carry the evidence needed for
those operations; capability discovery and operation responses report typed
`unavailable` reasons. Ranked regression explanation specifically remains
unavailable because duration deltas do not establish causal counter or decoded
event attribution. Dispatch-duration ranking through Plan V1 also remains typed
unavailable: Bundle V4 does not establish a complete set of at least two
comparable dispatches under an authenticated environment join. `inspect_kernel`
is only the KIR/artifact/source-map
identity binding declared on an observed dispatch. It does not expose source
text, ISA, arguments, variables, or semantic execution history.

The V1 service mode is:

```text
fe2o3-profiler-service jsonl
```

The `jsonl_binary_keeps_state_across_requests_and_terminates_on_malformed_input`
integration test is the checked wire example: it performs capability discovery,
capture admission, dispatch and KIR selection, and Plan V1 construction, then
asserts the unavailable overhead, future launch shape, and rejected CU selector
representations in the serialized response.

Each request must be exactly one LF-terminated JSON object with schema
`fe2o3-agent-profiler-request-v1`. Each response is one deterministic bounded
object with schema `fe2o3-agent-profiler-response-v1`. Malformed or oversized
input emits a typed terminal error and closes the session. The existing
`generic-core` workspace gate builds and tests all targets in this crate, which
includes the service binary and its protocol tests.

## Distributed-overlap extension

`AgentProfilerDistributedOverlapServiceV1` and
`fe2o3-profiler-service distributed-overlap-jsonl` form a separate versioned
extension. They do not add an operation, capability field, schema, or result to
`AgentProfilerServiceV1`; the existing V1 JSONL wire remains frozen. The
extension has only `discover_capabilities` and `explain_distributed_overlap`.
Discovery returns the extension request, response, and result schemas together
with the exact version and full content identity of its dependency contract, so
a fresh client can construct the flat explain request without opening a capture.
Every serialized extension response, including the trailing LF and maximum
`u64` request/revision representation, is independently capped at 4,096 bytes.

The dependency identity canonically binds `harsh-nod/fe2o3` issue #182,
contract version 1, and the required producer axes. Those axes are the operation
identity; exact directed dependency-edge identity; distinct predecessor and
successor operation identities; node, device, and queue identities; compute,
copy, transfer, and collective intervals or phases; local clock domain; clock
correlation interval; distinct correlation uncertainty and precision; explicit
loss and completeness status; and evidence content identity and schema version.
The service accepts none of those facts yet. It returns bounded
consumer-requirements metadata with no capture or record evidence.

Measured intervals will require observed origin. Producer inferences will
require a rule identity and exact input evidence, and any eventual overlap value
will itself be inferred from admitted inputs. Loss must be reported with origin
and lost-record count or be explicitly unknown with origin and a reason. Missing
events cannot establish idle time, completion, or causality. Global-time
precision remains unavailable without an admitted correlation interval plus
uncertainty/precision; causal localization remains unavailable without complete
dependency and phase evidence. The extension cannot execute, attach, schedule,
collect, or grant authority. It implements no issue #182 identity producer or
distributed runtime and does not claim the #215 T5 exit.
