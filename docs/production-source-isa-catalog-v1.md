# Production Source/ISA Catalog V1

`ProductionSourceIsaCatalogV1` is the durable, name-independent observation form of an exact
production Source-to-sparse-ISA correlation. It is built only from
`AdmittedProductionSourceIsaCorrelationV1`; callers cannot seed a source site, KIR operation, LLVM
coordinate, program counter, transformation result, or artifact identity during production
admission.

The catalog retains every admitted projected record. Each record has an explicit kind and contains
the exact fields permitted by that kind:

- `EliminatedBeforeKir`: source-node identity and span plus semantic-MIR identity and coordinate.
- `SourceAnchored`: all source/MIR fields, neutral and target KIR coordinates, semantic-operation
  identity, compiler-handoff LLVM coordinate, backend transformation status, and zero or more exact
  four-byte final-HSACO intervals.
- `NoSourceProvenance`: target KIR coordinate, semantic-operation identity, compiler-handoff LLVM
  coordinate, transformation status, and zero or more exact final-HSACO intervals.

ISA intervals are relative to the kernel entry symbol selected in AMDHSA metadata declaration
order. V1 admits only kernel ordinal zero and aligned four-byte half-open intervals. For an anchored
record, the ISA list is empty exactly when its transformation is `Eliminated`; every other
transformation has at least one interval. Anchor elimination is not evidence that the KIR operation
did not execute. Duplicate intervals remain duplicate records; the catalog does not manufacture a
one-to-one relationship.

## Identities and joins

The header commits to the correlation identity, finalized semantic-map identity, raw Source Map V2
content identity, finalized artifact identity, and a structural-binding snapshot. The structural
snapshot includes its exact production identity, target profile, KIR version, neutral and
target-bound KIR content identities, and coordinate counts. These axes let a later diagnosis
protocol reject a catalog from another simulation bundle, compiler observation, target binding, or
HSACO instead of joining records by names or ordinal coincidence.

The catalog identity is SHA-256 over a domain-separated canonical binary preimage. Records and each
record's sparse intervals have a deterministic total order. Decode requires exact magic, version,
length, reserved fields, flags, kind-specific shapes, bounds, ordering, interval shape, and identity.
Per-record field bytes, ISA bytes, and remaining global ISA budget are checked before record
allocation.

Decode returns `InertProductionSourceIsaCatalogV1`, which deliberately exposes no records or query
surface. A claimed correlation identity cannot prove that wire records are complete. The inert
catalog becomes queryable only through `admit_exact_projection_v1`, which reconstructs the complete
catalog from an independently admitted `AdmittedProductionSourceIsaCorrelationV1` and requires
byte-for-byte canonical equality. Omitted, inserted, or substituted records are typed
`ExactProjectionMismatch`. Construction, re-admission, and index validation are `O(n log n)`; exact
queries use binary search and allocate nothing.

The format is bounded by `MAX_PRODUCTION_SOURCE_ISA_CATALOG_BYTES_V1`,
`MAX_PRODUCTION_SOURCE_ISA_CATALOG_RECORDS_V1`, and
`MAX_PRODUCTION_SOURCE_ISA_CATALOG_ISA_INTERVALS_V1`. Oversized declarations are rejected before
record allocation. Before reserving record storage, decode requires the payload to contain at least
`record_count * 88 + declared_isa_count * 24` bytes, using checked arithmetic. The 88-byte minimum
is derived from the exact smallest valid record layout: an 8-byte record header plus the mandatory
24-byte target-KIR coordinate, 32-byte semantic-operation identity, and 24-byte compiler-handoff
LLVM coordinate of `NoSourceProvenance`. Each exact ISA interval occupies another 24 bytes. Catalog
construction never selects or prioritizes a seeded diagnostic site.

## Queries

Exact many-to-many queries are available for:

- source-node identity;
- source span;
- semantic-MIR node identity;
- semantic-MIR coordinate;
- neutral-KIR node identity;
- neutral KIR coordinate;
- target KIR coordinate;
- semantic-operation identity;
- compiler-handoff LLVM coordinate; and
- aligned, symbol-relative ISA program counter.

Unknown coordinates, unaligned PCs, nonzero V1 kernel ordinals, and PCs outside admitted sparse
anchors remain typed unavailable results. Queries return the full matching record set so a diagnosis
can report ambiguity instead of hiding it.

## Authority boundary

The catalog is observation data. Inert decoding proves canonical structure and self-identity only.
Exact projection re-admission proves equality to the supplied admitted correlation, but does not
re-admit the underlying compiler transaction or authenticate a separately supplied Source Map,
semantic map, artifact, code-object load event, debugger stop, profiler sample, or live PC.

The API therefore makes hard false claims for complete instruction coverage, a production schedule,
live-PC ownership, semantic refinement, optimized/final LLVM custody, debugger authority, profiler
authority, publication authority, and runtime authority. V9 production admission and V9 wire claims
remain typed unavailable rather than inferred or admitted.
