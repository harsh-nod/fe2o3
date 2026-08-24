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
the rocprofv3 ATT manifest, and normalized ROCgdb imports. Counter values, PC
samples, decoded ATT wave timelines, register/source values, and output
comparisons are explicitly capture-only or unsupported Trace V1 facts. ATT is
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
