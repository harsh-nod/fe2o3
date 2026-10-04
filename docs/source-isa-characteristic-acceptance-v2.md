# Source/ISA characteristic acceptance V2

Status: lossless producer/observer release, exact readmission, and the protected
ordinary-Rust build/capture adapter are implemented; the protected 3x2 matrix
has not run because the qualified authority-service environment is unavailable.

This is the next protected T1 acceptance for GitHub issue #215. It does not
replace the V1 ordinary-unit observation matrix and does not turn an inert
observation into compiler, artifact, publication, debugger, profiler, runtime,
or hardware-observation authority.

## Matrix

The production adapter builds these unmodified ordinary-source units on
both `gfx942` and `gfx950` through the sealed production V2 authority boundary:

| Family | Source unit | Exact target-KIR classification |
| --- | --- | --- |
| elementwise fill | `fe2o3_production_extraction_fixture` default `fill` kernel | global store, including its exact target-KIR memory form |
| neutral workgroup reduction | `fe2o3_workgroup_sync_v1` with `lds-kernel` | LDS write, LDS read, workgroup barrier, output global store |
| tiled BF16 GEMM | `fe2o3_collected_tiled_gemm_v1_fixture` | exact `Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate` target-KIR profile, output global store |

Every target characteristic retains every exact catalog correlation. A
`SourceAnchored` correlation carries its source span, MIR coordinate, neutral
KIR coordinate, target KIR coordinate, semantic-operation identity,
compiler-handoff LLVM ordinal coordinate, transformation, and complete sparse
final-HSACO interval list. A `NoSourceProvenance` correlation carries no source,
MIR, or neutral-KIR fields. Backend-eliminated correlations remain attached to
their target operation, retain their target-KIR and compiler-handoff LLVM
coordinates, use transformation `Eliminated`, and have an empty ISA list.

Records eliminated before KIR are a separate source/MIR-only population. Their
source span may be empty, matching the source-map contract for an eliminated
call site. They have no neutral or target KIR, semantic-operation, LLVM, ISA, or
transformation field. Exact duplicate catalog facts and exact duplicate ISA
intervals retain distinct occurrence ordinals; set projection must not erase
their multiplicity.

The Source/ISA catalog producer can represent backend `Duplicated` and `Eliminated` records.
`Fused`, `Inlined`, and `Moved` are typed
`UnavailableNoProductionEmitter`; `Outlined` is typed
`UnavailableNoSchemaRepresentation`. These capabilities describe producer
expressiveness, not an assertion that a particular compilation observed a
shape. This does not authenticate semantic duplication in MIR-to-KIR correspondence. The existing
coalesced and duplicated-and-coalesced records remain exact
backend observations rather than aliases for those unavailable classes.
Independent producer and parser tests preserve duplicate and
duplicated-and-coalesced cardinality. No current ordinary-source or canonical
semantic-MIR real-Worker fixture has produced an observed duplicated round
trip, so representability does not close that T1 acceptance gap.

The compiler-handoff LLVM coordinate is the producer's function, block, and
instruction ordinal tuple. V2 does not invent a function identity or LLVM
module identity. The BF16 claim terminates at the structurally admitted
target-KIR operation. Coordinate association does not establish optimized or
final LLVM custody, LLVM instruction semantics, decoded ISA opcode semantics,
a machine schedule, live program-counter ownership, GPU execution, or
performance.

## Complete scans

An admitted body carries the exact producer catalog-record count plus counts
for target operations scanned, catalog records scanned, classified target
operations, retained target correlations, and retained pre-KIR eliminations.
Records scanned must equal the catalog-record count, target operations scanned
must equal the structural operation count, and every retained fact preserves
its original catalog-record ordinal.

A complete scan with zero target characteristics is a successful empty
observation: `All` returns zero matches, the first page is exhausted, and no
cursor is issued. It is not encoded as a fabricated per-category `Missing`
record. Resource exhaustion or unavailable producer evidence is an outer typed
admission result and cannot be represented as a complete collection.

A classified target operation may also be structurally present with zero
catalog correlations (`StructuralOnly { record_count: 0 }`). It remains a real
target-characteristic occurrence in the complete scan and structural query
surface. It does not acquire a synthetic catalog occurrence, source, LLVM
coordinate, transformation, or ISA interval.

## Binding

The collection body binds only evidence exposed by the exact producer:

1. target profile and KIR version;
2. neutral KIR, target KIR, Source Map V2, and final artifact content identities,
   each as SHA-256 plus byte length;
3. target structural counts; and
4. structural-bridge and catalog content identities as SHA-256 plus byte length;
   and
5. structural-binding, correlation, and semantic-map identities.

The outer Broker V3 envelope separately binds the production configuration,
ordinary-source unit, and target. Configuration and unit identities are not
copied into the producer body. The body has no generic source identity,
observation-collection identity, LLVM module identity, or LLVM function
identity because the characteristic producer does not supply those facts.

Across the two targets for one family, the Broker configuration and unit plus
the producer neutral KIR and target-neutral Source Map V2 must be equal. Target
KIR, artifact, structural binding, structural bridge, catalog, correlation,
and semantic-map identities must be distinct.

Canonical decoding establishes bounded structure and a content association
only. A canonically resealed semantic substitution may remain structurally
valid. Rejection of a valid reseal occurs when an adapter compares the complete
decoded projection with independently admitted producer evidence.

The protected adapter must reseal and reject each independent substitution:

1. target-KIR characteristic kind or memory form;
2. catalog record kind;
3. source coordinate;
4. MIR node or coordinate;
5. neutral-KIR node or coordinate;
6. target-KIR coordinate;
7. semantic-operation identity;
8. compiler-handoff LLVM ordinal coordinate;
9. transformation;
10. ISA interval;
11. ISA interval multiplicity;
12. pre-KIR elimination;
13. complete-scan count;
14. structural-bridge identity;
15. catalog identity;
16. artifact identity;
17. producer target;
18. Broker configuration identity; and
19. Broker unit identity.

Changing only an outer digest is not a substitute for these canonical reseals.

## Queries

Structural queries return target-characteristic occurrences bound to collection
and target-characteristic ordinal, including structural-only operations. They
are selectable by characteristic kind and target-KIR coordinate. Fact queries
separately return occurrence identities bound to collection, the original
catalog-record ordinal, target-characteristic ordinal, and correlation ordinal,
or to collection, original catalog-record ordinal, and pre-KIR elimination
ordinal. Required fact selectors cover all catalog facts,
target-characteristic kind, catalog record kind, source node/span, MIR
node/coordinate, neutral-KIR node/coordinate, target-KIR coordinate,
semantic-operation identity, compiler-handoff LLVM ordinal coordinate,
transformation, exact PC, and pre-KIR-only facts.

Target-KIR, LLVM, and transformation queries include backend-eliminated target
facts. Exact-PC queries exclude them because their ISA list is empty. Source
queries include source-anchored target facts and pre-KIR eliminations;
`NoSourceProvenance` facts do not acquire a synthetic source.
Exact-PC lookup accepts only the aligned instruction start recorded by an ISA
interval; an interior byte is not a second program-counter occurrence.

Fact pages return a bounded fact core and interval count. Complete ISA interval
lists are paged separately with a fact-bound cursor and at most 64 intervals per
page, so one large producer record cannot exceed the response bound. Every
cursor binds the collection, operation/query or fact, next ordinal, and
preceding occurrence. Cross-collection, cross-query, stale, terminal,
out-of-range, and predecessor-substituted cursors fail closed.

## Broker V3

Broker V3 remains a read-only, single-response transport:

```text
magic
|| transport_config_identity
|| production_config_identity
|| ordinary_source_unit_identity
|| target_code_u16_le
|| body_length_u64_le
|| body
|| EOF
```

Target codes are exactly `1` for `gfx942` and `2` for `gfx950`. The exact magic
is `FE2O3/SOURCE-ISA-CHARACTERISTIC-BROKER/V3\0`. The repaired, unshipped V1
body is nonempty and at most 128 MiB, enough for the bounded production catalog plus target
characteristic grouping metadata. The reader rejects an oversized length
before allocation, reads the declared length exactly, and requires immediate
EOF.

The transport configuration identity is SHA-256 over these length-delimited
fields:

1. `FE2O3/SOURCE-ISA-CHARACTERISTIC-BROKER-CONFIG/V3\0`;
2. the exact magic;
3. `fe2o3-source-isa-characteristic-collection-v1`;
4. the 128 MiB maximum as `u64` little endian;
5. `u64-le-length-prefix`;
6. `config-unit-target-cell-binding`; and
7. `exact-eof-required`.

The resulting frozen identity is
`d8cda5df0538ddd552b4b93bff3d8f1b9fefc379a0e941e271f0ca508e51ae74`.
It identifies framing configuration only and does not authenticate the body or
grant authority.

## Production adapter gate

The executable contract is
`production_source_isa_characteristic_matrix_v2.rs`. Supported-target tests
exercise the complete lossless reference model and Broker V3 hostile framing.
The authority-free finalizer-to-observer release, independent exact
readmission, protected ordinary-source build, and Broker V3 capture adapter are
implemented. The adapter maps the complete producer projection, compares exact
forward and reverse query occurrence sets and interval multiplicity, and
performs canonical hostile reseals. The manual `source-isa-unit-matrix.yml`
workflow keeps contract validation separate from the protected matrix. The
matrix runs only with both explicit dispatch confirmations, the protected
runner label, and all eleven sealed authority/build environment bindings. A
green contract validation job is not a protected 3x2 result.

On that provisioned runner the exact lane is:

```console
FE2O3_RUN_SOURCE_ISA_CHARACTERISTIC_MATRIX_V2=1 \
  scripts/ci-local.sh source-isa-characteristic-matrix-v2
```

The protected adapter:

1. invokes all six sealed production builds without modifying source;
2. obtains one config/unit/target-bound Broker V3 body per cell and
   canonical-decodes the repaired, unshipped characteristic V1 collection;
3. invokes `admit_production_source_isa_characteristics_v1` with canonical
   target KIR, `ProductionSourceIsaCatalogV1`, and
   `ProductionKirV7StructuralBridgeV1`;
4. independently exact-projects every producer binding, structural
   classification, source-anchored fact, no-source fact, backend elimination,
   pre-KIR elimination, duplicate occurrence, and complete-scan count;
5. compares complete forward/reverse occurrence sets and separately paged ISA
   interval lists; and
6. canonically reseals all nineteen hostile substitutions and rejects each at
   independent exact projection.

Until the protected authority service supplies its exact production
configuration, measured driver, compiler/backend/trampoline bindings, and
Worker custody, the six-cell test remains unrun. The reference contract and
canonical semantic-MIR finalizer matrix are not substitutes for this ordinary
attributed-Rust protected acceptance evidence. T1 also remains open until a
production fixture produces an observed duplicated round trip; the exact
producer capability and independent cardinality tests do not relabel that
missing observation as accepted evidence.
