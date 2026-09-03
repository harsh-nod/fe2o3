# Profiler Variant V3

Profiler Variant V3 is an additive library contract for resolving the KIR
identity boundary retained by Variant V2. It does not change Variant V1,
Variant V2, or either JSONL service.

`build_profiler_variant_request_v3` and `compare_profiler_variants_v3` first
recompute the complete Variant V2 comparison. A treatment may additionally
provide `ProfilerVariantProductionKirEvidenceV3`, containing references to an
already-admitted `ProductionKirV7StructuralBridgeV1`, its exact production
Source/ISA catalog, and the production Characteristic projection derived from
that pair.

Canonical bridge or catalog bytes are not accepted as proof. Their decoded
owners are intentionally inert until the production finalizer replays the
inputs that created them. The V3 API therefore belongs at the in-process
producer/admission boundary. Exposing it through an agent JSONL service
requires a separately authenticated archive that can re-establish those owners
without weakening that boundary.

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

## Bounds and hostile behavior

V3 retains the Variant V2 occurrence ceiling of 512 records per treatment,
permits at most two treatment bindings, and caps encoded results at 8 MiB.
Request identities bind both admitted structural evidence identities. Tests
cover missing/substituted catalog matches, ambiguous matches, side and catalog
ordinal substitution, partial-scan reporting, and result bounds.
