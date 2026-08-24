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
```

Other page commands are `workgroups`, `waves`, `sites`, `occurrences`,
`memory-regions`, and `evidence`. The CLI is deliberately stdin-only and never
opens a path. `--cursor` carries both fields of the query-bound cursor returned
by the previous page as `QUERY_BINDING_HEX:EVENT_POSITION`; identities serialize
as fixed lowercase hex. Filters are conjunctive. JSON output has an exact byte
ceiling configured by `QueryLimitsV1`; input, page size, arguments, and resident
response construction are independently bounded.

Site ordinals and Kernel-IR content identities remain producer claims. The query
surface does not resolve them to names or source locations without a future
authenticated catalog adapter.
