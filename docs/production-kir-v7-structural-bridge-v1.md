# Production KIR V7 structural bridge V1

`ProductionKirV7StructuralBridgeV1` is the bounded, name-independent coordinate bridge between
the canonical KIR V7 interpreted by the CPU simulator and an admitted production Source/ISA
catalog. It is reconstructed from all of these exact inputs:

- verified canonical KIR V7 bytes;
- verified canonical production KIR V8 bytes;
- canonical Source Map V2 bytes whose subject is that exact V7 identity;
- exact finalized artifact bytes; and
- an admitted `ProductionSourceIsaCatalogV1` carrying the identical Source Map, artifact,
  correlation, semantic-map, target profile, neutral KIR, target KIR, and structural identities.

The current producer makes a canonical V7 debug projection from the exact production V8 module.
Finalizer admission already requires the independently decoded V7 and V8 modules to be equal. The
bridge projects that admitted fact into explicit block-entry, operation, and terminator records.
Each record maps the same coordinate in V7, neutral V8, and target-bound V8; the existing target
structural binding separately proves that target binding retained the coordinate shape.

This is sufficient to carry a Diagnosis V2 operation site, including an out-of-bounds statement,
to the production catalog. `query_target_catalog` performs the exact bound-catalog handoff for an
operation coordinate. A `WorkgroupBarrier` operation can therefore preserve the catalog's
`NoSourceProvenance` result. Block entries and terminators, including the following `Return`, have no
catalog operation coordinate and return distinct typed unavailability; the bridge does not invent a
source span by borrowing a neighboring operation.

## Transformation boundary

V1 admits only an exact coordinate-identity KIR-version projection. The current producer has no
real one-to-many, many-to-one, eliminated, or unrepresented V7-to-V8 migration. Those states do
exist on other admitted axes: Source/MIR-to-KIR duplication or elimination and KIR-to-ISA
duplication, coalescing, or elimination remain records in `ProductionSourceIsaCatalogV1`. The
bridge binds that catalog but does not relabel its transformations as a KIR-version migration.

If verified V7 and production modules differ, admission returns
`NonIdentityStructuralProjectionUnavailable`; it does not compare names, source text, or ordinals
by assumption. V1 accepts only an already-admitted V8 Source/ISA catalog. The upstream finalizer and
catalog admission retain the current typed V9 source-projection gap; the bridge does not fabricate a
V9 catalog or re-label that unreachable state. A future non-identity migration needs new producer
evidence and a new bridge version.

## Wire and queries

The V1 binary wire is capped at 64 MiB and 1,000,000 records, with the byte limit imposing the
tighter effective record bound. Records have one canonical order and no duplicates. The identity
is SHA-256 over the complete canonical preimage under the
`FE2O3/PRODUCTION-KIR-V7-STRUCTURAL-BRIDGE/V1\0` domain.

Decoding creates `InertProductionKirV7StructuralBridgeV1`. It exposes claimed bridge and catalog
identities but no records or queries. `admit_exact_projection_v1` independently reconstructs the
complete bridge from the exact inputs and requires equality before returning the admitted type.
The admitted type supports exact V7, neutral-production, and target-production site queries.

Malformed headers, identities, records, counts, ordering, or duplicates fail closed. Stale or
substituted V7, production KIR, Source Map, artifact, catalog, correlation, semantic-map, target,
or structural identities cannot acquire query access.

## Nonclaims

The bridge proves structural coordinate identity only. It does not prove semantic refinement,
source attribution for every site, an instruction schedule, complete ISA coverage, optimized or
final LLVM custody, a live program counter, GPU observation, or simulator facts about GPU
execution. It owns no compiler, artifact, debugger, profiler, publication, load, launch, attach,
dispatch, or runtime authority.
