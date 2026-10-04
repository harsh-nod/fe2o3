# fe2o3-source-isa-observation

This crate owns the frozen `F2SISUM1` frame and `F2SICOL1` collection wire
formats, the shared authority-free query projection, and the V1 agent JSONL
protocol. It does not compile, publish, load, launch, or execute a kernel and
does not grant compiler, proof, artifact, runtime, or hardware-observation
authority.

Producer attempts are stored only as inert fixed-width generation, session,
and invocation coordinates. The crate has no dependency on compiler build
transactions; `cargo-fe2o3` performs the one-way conversion after authority
release.

`wire_v1` strictly admits canonical binary observations. `agent_v1` uses that
same decoder for both human and typed inspection. It exposes:

- request schema `fe2o3-agent-source-isa-request-v1`;
- response schema `fe2o3-agent-source-isa-response-v1`;
- `discover_capabilities`;
- `inspect_source_isa_collection`, with a maximum page of 64 globally ordered
  observed or missing units;
- `run_agent_source_isa_jsonl_v1` for a bounded line-at-a-time service; and
- symmetric typed request/response serde values plus bounded response-line
  decoding for agent clients.

Requests are limited to 2 MiB, responses including their newline are limited
to 2 MiB, binary collections to 696,432 bytes, lowercase hexadecimal
collections to 1,392,864 bytes, and collections to 1,024 units. Request IDs
must be nonzero and unique within a service. Responses carry a monotonic
revision. An unambiguous nonzero ID recovered from an otherwise invalid
request is reserved before the error response; duplicated `request_id` or
`operation` keys suppress all correlation hints. Structured cursors carry a
positive position and a lowercase query
binding over the verified collection digest, canonical byte length, operation,
schema, filter, unit count, position, and preceding unit. Malformed,
cross-collection, terminal, and out-of-range cursors are rejected. The binding
is a public domain-separated digest, not an authentication claim.

Collection `completeness` describes observation transport. `page_exhausted`
only describes pagination. Neither establishes complete machine-code coverage,
semantic refinement, GPU execution, or protected qualification.

`characteristic_v1` is an additive, authority-free projection of an exactly
admitted production catalog. It does not change the frozen `F2SISUM1`,
`F2SICOL1`, or agent V1 protocols. A canonical `F2SICH1` collection binds the
exact target profile and KIR version; structural identity and counts; content
identities and byte lengths for source-map V2, neutral KIR, target KIR,
artifact, catalog, and structural bridge; and the catalog correlation and
semantic-map identities. The collection is bounded to 128 MiB, which covers
the approximately 118 MiB fixed-width worst case implied by the producer's
independent limits: 528,384 catalog records, 65,536 classified targets,
262,144 retained target correlations, 4,096 correlations per target, 65,536
pre-KIR facts, and 1,016,800 sparse anchors.

Classification is exact at target KIR. Global stores, workgroup loads, and
workgroup stores retain `plain`, `guarded`, or `matrix_tile` memory form;
workgroup barriers and the BF16/F32 M16N16K16 wave64 matrix multiply-accumulate
have their own tags. A classified target may have zero catalog correlations and
is still retained as a structural target. Target correlations preserve their
catalog ordinal and every producer axis: source/span, MIR, neutral KIR, target
KIR, semantic operation, compiler-handoff LLVM ordinal, transformation, and
sparse final-HSACO four-byte anchors. `no_source_provenance` and backend
`eliminated` records remain explicit, exact duplicate records and anchors keep
their occurrence multiplicity, and category-free records eliminated before KIR
are retained separately. Empty source spans are permitted only for those
pre-KIR facts. A complete scan may legitimately contain no characteristics or
omit ordinary non-characteristic catalog records.

Queries use three independent planes. Target pages return one occurrence per
classified target and support identity, exact kind, family category, and target
KIR selectors. Fact pages return catalog correlations plus pre-KIR facts and
support record kind, source/span, MIR, neutral KIR, target KIR, semantic
operation, compiler-handoff LLVM, transformation, exact PC, and pre-KIR-only
selectors. Interval pages are bound to one fact occurrence and never inline an
unbounded anchor vector in a fact result. Every page has at most 64 items and a
lowercase cursor bound to the collection, query or fact, position, and preceding
occurrence; terminal and cross-plane cursors are rejected.

`characteristic_agent_v1` exposes this contract through a separate canonical
JSONL protocol. A service owns one already decoded and exactly admitted binary
collection. Requests carry only its identity, never collection bytes or hex,
and provide `discover_capabilities`, `query_targets`, `query_facts`, and
`query_intervals`. Requests and newline-inclusive responses are bounded to 2
MiB, a service accepts at most 4,096 requests, revisions are monotonic, and
nonzero request IDs cannot be reused, including IDs recovered unambiguously
from malformed requests. Agent responses expose typed complete, missing,
unavailable, and error scan states and explicitly report false compiler, proof,
publication, runtime, hardware-observation, final-LLVM, decoded-ISA,
final-opcode, schedule, semantic-refinement, and complete-machine-coverage
claims.

Canonical collection and JSON digests are public structural bindings, not
signatures or transport authentication. Plain binary decode is inert until an
independent exact-projection admission compares it with trusted producer
output. Standalone JSON response decode checks canonical structure, bounds,
nonclaims, occurrence shape, and cursor coherence; clients still bind responses
to the expected local service and collection identity.
