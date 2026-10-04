# Semantic debug transformation map V2

Status: additive exact-cardinality schema and production MIR-to-KIR finalizer projection are
implemented. Authenticated semantic duplication, fusion, outlining, inlining, and movement
producers are not implemented.

Semantic Debug Transformation Map V2 is an authority-free sidecar over one exact canonical
Semantic Debug Map V1. It changes no V1 bytes. Its binding retains the V1 map content identity and
the complete V1 source-map, semantic-MIR, canonical-KIR, schedule, LLVM-module, and finalized-
artifact identities.

## Cardinality is not classification

Every relation records stable input and output node identities, adjacent layers, exact producer-
evidence identity, and one of these cardinalities:

- one-to-one;
- one-to-many;
- many-to-one;
- many-to-many; or
- eliminated, with no output node.

Classification is separate. It is either `Preserved`, an observed `Duplicated`, `Fused`,
`Outlined`, `Inlined`, `Moved`, or `Eliminated` transformation, or a typed unavailable reason.
One-to-many cardinality alone does not establish duplication, outlining, or inlining. Many-to-one
cardinality alone does not establish fusion. Nodes may participate in multiple V2 relations; the
format does not impose V1's unique mapping-owner restriction on a transformation graph.

An observed transformation is admitted only when the document's exact capability record identifies
an authenticated producer for that class. Canonical decoding reconstructs descriptive evidence and
grants no compiler, artifact, publication, load, launch, runtime, debugger-control, or hardware
authority.

## Production projection

The Worker V3 finalizer constructs V2 only after it has:

1. admitted all 13 semantic-to-LLVM association axes;
2. replayed the whole-module production KIR-to-LLVM evidence;
3. decoded the exact V4 semantic-MIR-to-KIR correspondence;
4. checked every Source/MIR/KIR node and relation against Source Map V2, semantic MIR, canonical
   KIR V8 and its identical V7 projection; and
5. rebound Semantic Debug Map V1 to the independently inspected final HSACO.

The V4 correspondence authenticates an exact contiguous operation range for each semantic MIR
statement, including zero operations. It does not carry an optimization classification. The current
projection therefore reports:

- one-to-one Source-to-MIR and MIR-to-KIR relations as `Preserved`;
- zero-output MIR-to-KIR relations as observed `Eliminated`; and
- multi-operation MIR-to-KIR relations with their exact one-to-many cardinality and
  `ProducerDidNotClassify`.

In particular, ordinary lowering expansion is not relabeled as semantic duplication. The legacy V1
projection can be imported into V2 for exact endpoint compatibility, but every legacy transformation
label remains `LegacyClaimNotAuthenticated` unless independently joined to producer evidence.
Artifact-only V1 admission has no V4 correspondence and exposes no production V2 projection.

## Current typed boundaries

The schema represents duplicated, fused, outlined, inlined, moved, and eliminated observations.
The current V4 production projection authenticates only elimination among those six classes.
Duplicated, fused, outlined, inlined, and moved are
`UnavailableNoAuthenticatedProducer`. Backend pseudo-probe `Coalesced` and
`DuplicatedAndCoalesced` observations in Source/ISA Characteristic V1 are not aliases for semantic
fusion, movement, outlining, or inlining.

Scheduled-lowering, optimized/final LLVM origin, and complete ISA transformation producers remain
future work. Until those producers retain exact input/output identities and authenticated
transformation classifications, V2 preserves their absence rather than inferring by symbol or
function name.
