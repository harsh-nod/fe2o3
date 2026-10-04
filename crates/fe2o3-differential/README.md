# fe2o3-differential

`fe2o3-differential` is standalone infrastructure for reproducible differential
testing. It generates a bounded scalar kernel-expression subset from an exact
seed, provides wrapping `i32` CPU reference semantics, encodes cases in a
canonical V1 format, reports output mismatches, and reduces a case while a
caller-provided mismatch predicate remains true.

The expression subset includes constants, the one-dimensional global ID,
same-index input loads, unary negation and bitwise not, wrapping arithmetic,
bitwise operations, comparisons, and lazy `select` control flow. Cases are
limited to 4 inputs, 256 work-items, 127 expression nodes, 12 levels of depth,
and 16 KiB of canonical bytes.

This crate does not execute a GPU, invoke a compiler, authenticate artifacts,
or establish CUDA Oxide parity. A passing generated corpus is not correctness
or safety evidence. Integration must bind canonical cases to exact compiler,
runtime, device, and artifact identities and must preserve both successful and
failing execution evidence.

The reducer is deterministic for a deterministic predicate. It reaches a local
minimum over its documented expression, launch-shape, input-buffer, and scalar
value transformations; it does not claim a globally minimal reproducer.

## Semantic conformance corpus

The V1 semantic corpus is a second, independent model for high-risk Rust GPU
contracts:

- pointer distance, including allocation provenance, alignment, signed distance,
  and unsigned underflow;
- volatile loads and stores with bounds, alignment, and access permissions;
- `copy_nonoverlapping` element ranges, bounds, and non-overlap;
- aggregate and tagged-enum layout, with niche layout rejected until supported;
- integer switch selection, default handling, and duplicate-value rejection;
- workgroup/device atomic operations, ordering validity, and unsupported scopes;
- bounds and cross-lane non-atomic race obligations.

Generation is deterministic from a seed, feature, and ordinal. The default
corpus contains both supported cases and specific expected compile rejections
for every feature. Cases and reduced reproducers have bounded canonical V1
encodings and deterministic mutation-detection identities. These identities are
replay metadata, not cryptographic authentication or execution authority.

The CPU oracle returns either an exact semantic observation or a specific
compile-rejection reason. A backend report is classified into exactly one of:

- `SupportedPass` when an executed result exactly matches the CPU oracle;
- `ExpectedCompileRejection` when the rejection class exactly matches;
- `SemanticMismatch` for wrong output, wrong rejection, unexpected execution,
  or unexpected rejection;
- `HardwareUnavailable` for an explicit device, driver, target, or permission
  failure.

There is no skipped-success state. Hardware unavailability remains visible and
cannot increase a pass count. The semantic reducer visits bounded candidates in
a stable order while a caller-provided exact mismatch predicate remains true.

## GPU runner integration

An external runner still needs to lower each `SemanticCase` into Rust source,
invoke the authenticated compiler and verifier path, execute supported cases on
the selected GPU, and return a typed `BackendOutcome`. A durable result must bind
the canonical case identity to the compiler, proof policy, finalized artifact,
runtime, target, and physical device identities. Expected rejection cases should
stop before artifact loading; executable cases should compare copied-back values
against `evaluate_semantic_case`. Only that end-to-end evidence can update parity.
