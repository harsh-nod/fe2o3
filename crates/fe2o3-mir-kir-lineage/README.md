# fe2o3 MIR-to-KIR Lineage V4

This crate defines the dependency-light, canonical, bounded **inert** data model
for exact semantic-MIR-to-Kernel-IR lineage. It is standard-library-only and
does not depend on compiler owners, MIR, KIR, verification, artifact, runtime,
or authority crates.

The model retains:

- semantic MIR V2/V3 raw-canonical SHA-256 identities and exact lengths;
- Kernel IR V5/V6 domain-separated policy identities and exact lengths;
- a production-only semantic MIR V3 to Kernel IR V6 policy pair, plus an
  explicitly non-production legacy-inert V2 to V5 pair;
- exact gfx942 lowering policy, configuration, and resource limits;
- the normative V3-to-V6 checked Add/Sub/Mul refinement obligation from
  [CHECKED_ARITHMETIC_REFINEMENT.md](CHECKED_ARITHMETIC_REFINEMENT.md);
- aggregate function, kernel, block, statement, terminator, and operation counts;
- semantic-body, f32-intrinsic-declaration, and diagnostic-trap-declaration functions;
- exported kernel mappings; and
- semantic and closed-rule synthetic block classifications.

Statement ordinals and first-operation ordinals are derived from record order
and cumulative operation counts. The wire stores one operation count per
statement, including zero-operation statements, followed by the terminator
count. This makes gaps and overlaps unrepresentable and lets the configured
maximum 1,048,576 zero-op statements fit comfortably below the unchanged 4 MiB
lineage input cap.

`MAX_LINEAGE_BYTES_V4` is an exported hard 4 MiB bound. Caller input limits may
only tighten it. Both decoding and canonical construction enforce the hard cap
independently before growing their byte buffers. Decoding rejects oversized
input, resource or work exhaustion, arithmetic overflow, truncation, trailing
bytes, non-shortest or overflowing varints,
unknown schemes, versions, or tags, invalid production/legacy version pairs,
inconsistent totals, duplicate or out-of-range identities, incomplete or
excessive operation coverage, and noncanonical re-encoding. Referenced semantic
MIR is capped at 128 MiB and referenced canonical Kernel IR at 16 MiB. The exact
production KIR scheme is SHA-256 policy V1 over a framed canonical V6 preimage;
it is not a generic SHA-256 field. These identity-length claims are independent
of the decoder's 4 MiB lineage input cap.

Every caller-supplied semantic-function, KIR-function, kernel, block, statement,
and operation count limit is checked by both decoding and `from_model`. One
checked admission-work budget accounts for input bytes and records,
structural traversals and bitmap initialization, and canonical re-encoding.
The measured stage breakdown is retained for exact-boundary tests and auditing.

Decoding does **not** establish that either artifact exists, authenticate either
identity, prove correct lowering, satisfy the checked-arithmetic owner gate, or
grant authority. A downstream move-only validator must compare both embedded
artifact header versions and exact identity schemes before beginning exhaustive
typed MIR/KIR traversal. That traversal must
then compare operations, operands, results, types, metadata, block parameters,
terminators, CFG edges, function metadata, and kernel metadata. KIR checked
arithmetic is a native operation and needs no additional declaration class.

See [WIRE_FORMAT.md](WIRE_FORMAT.md) for the canonical V4 field order.
