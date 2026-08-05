# fe2o3 artifact manifests

`fe2o3-artifacts` owns target-neutral artifact identity and launch-contract
records, bounded artifact containers, and source-level proof evidence. It does
not load payloads, launch kernels, select a device, run Verus, or verify
compiled machine code.

## Validation boundary

Model fields are private and direct construction uses fallible constructors.
Values returned by those constructors are structurally and semantically
validated according to the invariants below. Validation does not establish
authenticity and does not bind an identity to executable payload bytes. That
binding is provided separately by `ArtifactContainerV1`; producer
authentication remains outside this crate.

`DigestBytes` contains opaque identity bytes supplied by build tooling. A bare
manifest does not assign those bytes a digest algorithm or bind them to a
payload. `PayloadDigest` makes an algorithm explicit and can calculate or
verify a digest over bytes. That primitive proves only a byte-level match under
the selected algorithm; it does not establish who produced those bytes.

## Identity and launch invariants

- Names and compiler, tool, and target identity text use bounded, explicit
  ASCII grammars.
- Target capabilities are unique and stored in canonical enum order.
- Code-object identities record an opaque digest, format, and nonzero payload
  length without claiming the referenced payload exists or is authentic.
- Launch rank is one through three. Inactive dimensions are one, dimensions
  are nonzero, dimension products do not overflow, and static plus dynamic
  shared-memory requirements fit in `u32`.

## ABI invariants

- ABI alignments are bounded powers of two and total argument storage is
  bounded to 1 MiB before device-specific limits are applied. Fields are
  aligned, ordered, non-overlapping, in bounds, and compatible with the
  selected pointer width.
- Scalar, pointer, and slice records have consistent size, mutability, access,
  and address-space semantics. Immutable and constant references cannot grant
  write access. Standalone reference fields must match one supported pointer
  width; an `AbiLayout` binds them to the target's exact width.

`AbiKind::Slice` is one logical field in the ordered ABI. Its physical launch
representation is exactly pointer followed by length, with total size twice
the target pointer width and target-pointer alignment. Typed launch code must
expand that field once while preserving its single logical argument position.

## Manifest invariants

`ManifestV1` is the structurally and semantically validated aggregate. It is
not evidence that referenced code is authentic or even present.

- At least one code object and one kernel are present, with fixed upper bounds.
- Code-object IDs, kernel IDs, logical kernel names, and exported symbols are
  unique. Set-like records are stored in canonical order.
- Every kernel references a listed code object, uses the target pointer width,
  and requires only capabilities supplied by the selected target.

Host launch integration and runtime loading policy remain outside this model
layer.

## Container model

`CodeObjectPayload` verifies nonempty, bounded bytes against an explicitly
identified cryptographic digest. `ArtifactContainerV1` requires one payload for
every code object in its manifest and no others. It rejects digest-algorithm or
declared-length mismatches, duplicate, missing, and extra payloads, and payload
sets over fixed per-object and total byte limits. Payloads are stored in digest
order so the validated closure has one canonical model representation.

A valid container establishes payload-byte binding under its selected digest
algorithm. It does not establish signer authenticity, the identity of the
producer, compiler correctness, proof validity, or runtime safety.

## Proof records and matching

`ProofRecordV1` records algorithm-tagged identities for a concrete kernel
instance, source tree, crate graph, proof-erased executable, build environment,
artifact selection, artifact contract, source memory semantics, effects, type
layout, capability semantics, and the functional specification. It also records
the build configuration, verification model and axioms, measured verifier,
solver, and evidence-recorder identities, invocation, outcome, proved
properties, and named trusted contracts. A measured tool identity includes its
name, version, executable or immutable-distribution digest, and complete
result-affecting configuration digest. Variable collections are bounded, unique,
and canonical.

`ManifestV1::proof_target` reconstructs the artifact-owned environment,
selection, and contract identities under versioned domains. Because manifest v1
stores opaque digest bytes, callers must supply explicit algorithm-tagged
kernel, source, executable, and code-object identities. Reconstruction rejects
any byte mismatch instead of assigning an algorithm to manifest bytes. Artifact
selection hashes both the code-object digest algorithm and bytes. It also
requires measured compiler and artifact-producer identities whose names and
versions match the manifest; their executable and configuration measurements
are included in the environment identity. Instance, crate-graph, and
source-contract identities remain explicit compiler inputs.
`ProofMatchPolicy` returns `MatchedProofEvidenceV1` only when every expected
identity matches, the proof completed, all V1 properties are present, and the
trusted-item list exactly matches policy. Failure and timeout are rejected.

Matching establishes equality with caller-supplied evidence. It does not
authenticate a Verus invocation and cannot create a `Verified` assurance.
Invocation authentication alone is insufficient, and
`MatchedProofEvidenceV1` is never sufficient for assurance promotion. Only a
future audited driver that authoritatively derives the complete property set and
trusted escape inventory from source and verifier output, and authenticates the
whole canonical record, may create a stronger private type. Even that
authenticated source-level evidence would still trust Verus, the solver, model
axioms, proof erasure, semantic hashing, the compiler stack, and the runtime; it
would not by itself prove that emitted AMDGPU machine code refines the source.

## V1 wire format

`ManifestV1::to_bytes` emits a canonical little-endian binary representation.
It starts with the eight bytes `FE2O3AM\0`, a `u16` version (`1`), and a reserved
zero `u16` flags field. Strings use a `u16` byte length, lists use explicit
`u16` or `u32` counts, and enums use fixed-width numeric tags.

Records occur in model order: compiler, producer, target, canonically sorted
code objects, then canonically sorted kernels with launch and ordered ABI
fields. Encoding does not authenticate identities or payloads.

`ManifestV1::from_bytes` treats every byte as untrusted and returns a manifest
only after wire and model validation. It rejects inputs larger than 4 MiB,
truncation, trailing bytes, noncanonical order, unsupported versions or flags,
unknown tags, and invalid zero or oversized counts before allocation. A
successfully decoded manifest is structurally and semantically validated; it
is not authenticated and is not bound to executable payload bytes.

## Container v1 wire format

`ArtifactContainerV1` is a separate envelope, so manifest v1 bytes remain
unchanged. Its canonical little-endian header contains `FE2O3AC\0`, container
version `1`, zero flags, an explicit digest-algorithm tag, a zero reserved
field, the manifest byte length, and the payload count. The canonical manifest
follows, then a digest-sorted table of payload digest and `u64` byte length,
then the payload bytes in that same order.

Decoding rejects an envelope over its fixed maximum before parsing. It bounds
the manifest, payload count, every payload, and aggregate payload bytes before
allocating payload buffers. It rejects unknown versions, flags, algorithms,
and reserved values, malformed manifests, duplicate or noncanonical payloads,
missing or extra payloads, digest or length mismatches, truncation, and trailing
bytes. Successful decoding establishes that the embedded bytes match the
manifest's complete code-object closure under the selected digest algorithm.
It does not authenticate the envelope or make claims about its producer,
compiler, proofs, or runtime behavior.

## Proof record v1 wire format

`ProofRecordV1` uses a separate canonical little-endian envelope beginning
with `FE2O3PR\0`, version `1`, and zero flags. Decoding is capped at 1 MiB and
rejects truncation, trailing data, unknown tags, unsupported versions or flags,
out-of-range counts, noncanonical ordering, duplicate logical keys, and model
validation failures. Its SHA-256 digest can be embedded by later bundle
finalization without changing manifest v1 bytes.
