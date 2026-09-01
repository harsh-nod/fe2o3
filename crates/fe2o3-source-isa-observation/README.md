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
