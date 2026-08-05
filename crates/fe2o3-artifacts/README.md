# fe2o3 artifact manifests

`fe2o3-artifacts` owns target-neutral artifact identity and launch-contract
records. It does not load payloads, launch kernels, select a device, or make
proof claims.

## Validation boundary

Model fields are private and direct construction uses fallible constructors.
Values returned by those constructors are structurally and semantically
validated according to the invariants below. Validation does not establish
authenticity and does not bind an identity to executable payload bytes. That
binding belongs to the future artifact container and loader.

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

Proof records, host launch integration, and runtime loading policy remain
outside this model layer.

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
