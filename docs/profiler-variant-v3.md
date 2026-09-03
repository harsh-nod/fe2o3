# Profiler Variant V3

Profiler Variant V3 is an additive library and fresh-process contract for
resolving the KIR identity boundary retained by Variant V2. It does not change
Variant V1, Variant V2, or their JSONL wires.

`build_profiler_variant_request_v3` and `compare_profiler_variants_v3` first
recompute the complete Variant V2 comparison. A treatment may additionally
provide `ProfilerVariantProductionKirEvidenceV3`, containing references to an
already-admitted `ProductionKirV7StructuralBridgeV1`, its exact production
Source/ISA catalog, and the production Characteristic projection derived from
that pair.

Canonical bridge or catalog bytes are not accepted as proof. Their decoded
owners are intentionally inert until the production finalizer replays the
inputs that created them. `ProductionProfilerKirArchiveV1` provides the
separate self-contained replay boundary: its inert decoder verifies the exact
archive identity and framing, then admission reruns the complete Worker V3
finalizer derivation and reconstructs the catalog, bridge, and Characteristic
owners. The archive authenticates their internal derivation from its complete
content; it does not authenticate the external provenance of that content.

## Fresh-process JSONL route

Run the read-only service with:

```text
fe2o3-profiler-service variant-v3-jsonl
```

The service accepts newline-delimited requests under
`fe2o3-agent-profiler-variant-request-v3`:

1. `discover_capabilities` returns the exact service-contract identity, bounds,
   unavailable semantics, and nonauthority statement.
2. `open_structural_archive` accepts canonical lowercase hex and a caller-pinned
   `ContentIdentityRecordV1`. It verifies the archive checksum and identity,
   reruns finalizer replay, and retains at most two distinct fully admitted
   owners.
3. `compare_variants` embeds the unchanged V2 treatment wire and optionally
   cites one previously opened archive identity for each side.
4. `compare_complete_structural_catalogs` accepts the same inputs, recomputes
   Variant V3, and considers an add/remove claim only from two complete
   admitted producer catalogs in one exact comparison domain.

An exact replay that encounters absent compiler instrumentation or another
supported production projection gap returns `structural_archive_unavailable`
with a stable class and reason code. It is a successful typed-unavailable
result, and no query owner is retained. Malformed framing, identity
substitution, duplicate archives, unknown archive references, noncanonical
hex, stale revisions, duplicate request IDs, and exceeded bounds fail closed.
Every response has a domain-separated content identity that a fresh client can
verify independently.

The process has no file or network operation and accepts the complete archive
only through the JSONL request. It grants no execution, replay of a GPU
schedule, attach, collection, decoder, publication, load, launch, dispatch, or
runtime authority. "Replay" here means deterministic compiler-finalizer
evidence validation, not kernel execution or live capture.

## Exact join

For each upgraded treatment, V3 requires all Bundle V4 dispatches to carry the
same canonical V7 digest and byte length as the admitted bridge. It then checks
the bridge, catalog, source-map V2, artifact, structural, correlation,
semantic-map, neutral V8, and target V8 identities.

Each positive PC or decoded-ATT Characteristic occurrence is resolved by:

1. querying the bridge at its exact V7 function, block, and operation;
2. requiring exact coordinate identity at neutral and target V8;
3. querying the bridge-bound catalog at that target operation; and
4. requiring exactly one catalog record to match its source/MIR identities,
   KIR coordinates, semantic-operation identity, compiler-handoff LLVM
   coordinate, ISA interval, record kind, and transformation.

Zero matches are a substitution error. Multiple exact matches are an ambiguity
error; V3 never chooses one by order or name. The returned record carries the
catalog ordinal and every immutable bridge/catalog identity used by the join.

A complete Characteristic archive is re-admitted against the supplied
production projection. A partial archive may contribute a positive structural
occurrence only when its binding matches the admitted production evidence and
the individual occurrence uniquely matches the catalog. Its partial status is
retained as typed unavailable, so unobserved records have no absence meaning.

## Interpretation

`structural_changes` contain only Variant V2 changed axes for positive pairs
whose two occurrences independently passed the exact production structural
join. Their basis is
`exact_structural_positive_co_observation`. This is stronger than comparing a
declared Bundle V4 V7 digest directly with a V8 Characteristic coordinate, but
it is not:

- proof that the caller-declared schedule executed;
- proof that a structural or resource change caused a timing/counter delta;
- proof of addition or removal from sampled, selected-CU, lossy, or partial
  evidence; or
- execution, attach, collection, decoder, load, dispatch, or publication
  authority.

Those analytical limits remain typed unavailable in every V3 result. The
complete nested Variant V2 result is retained. When both sides contain positive
structurally resolved occurrences, V3 separately identifies
`profiler_kir_to_characteristic_kir_bridge_unavailable` as the V2 fact its new
evidence supersedes; it does not rewrite the older result.

## Complete structural catalog comparison

`ProfilerCompleteStructuralComparisonV1` is a bounded additive contract. It
does not reinterpret a missing PC sample or ATT record. Instead, it requires
both self-contained archives to replay to internally consistent complete
catalog projections and complete supported-Characteristic scans. Its exact
comparison domain is the existing content-bound semantic workload plus the
archive-derived complete set of exact source-node, source-span, MIR-node, and
MIR-coordinate sites. The per-side Source Map V2 identities remain visible but
are not required to match because each also binds its side's canonical KIR.
Different stable source/MIR universes are
`cross_domain_source_mir_universe_identity`; V1 has no source-lineage authority
and returns no delta.

Within that domain the contract keys each classified target-KIR witness by its
structural kind and complete deduplicated source-plus-MIR site set. It compares
key multiplicities over all functions, blocks, and operations, without names
or fixed kernel shapes. Added and removed records include every exact
archive-local occurrence identity on both sides. This preserves duplicated
lowering occurrences and reports the exact count difference without claiming
which indistinguishable duplicate continued across treatments.

The contract returns no partial add/remove set when either owner or complete
projection is missing, any classified occurrence lacks a stable source/MIR
identity, or the bounded result would overflow. These conditions use stable
reason codes. `schedule_execution_unavailable` and
`causal_attribution_unavailable` are always retained. Catalog completeness is
the finalizer-admitted structural projection scope; it is not complete ISA,
live execution, external provenance, or performance coverage.

## Bounds and hostile behavior

V3 retains the Variant V2 occurrence ceiling of 512 records per treatment,
permits at most two treatment bindings, and caps encoded results at 8 MiB. The
complete structural comparison retains at most 4,096 changed keys and 4,096
exact side occurrences across those keys, and caps its encoded result at 16
MiB. It never truncates to those bounds: overflow is typed unavailable before
any add/remove set is returned. The service retains at most two archive owners
and 64 requests. Archive and request bounds are derived from the existing
bounded semantic handoff, external provider, compact transcript, and HSACO
maxima; capability discovery reports
their exact numeric values.

Request identities bind both admitted structural evidence identities. Tests
cover missing/substituted catalog matches, ambiguous matches, side and catalog
ordinal substitution, partial-scan reporting, result bounds, archive
truncation/trailing data, checksum-valid section substitution,
duplicate/reordered sections, uppercase hex, response substitution, and the
real executable route. The end-to-end fully available structural-owner test is
environment-gated on the production Worker; the ordinary synthetic fixture
proves exact replay and preserves `compiler_instrumentation_absent` rather than
inventing a bridge.
