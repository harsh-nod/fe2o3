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

`DigestBytes` contains opaque identity bytes supplied by build tooling. This
crate does not hash payloads, compare payload contents with those bytes, or
claim that the identity uses a cryptographic digest.

## Identity and launch invariants

- Names and compiler, tool, and target identity text use bounded, explicit
  ASCII grammars.
- Target capabilities are unique and stored in canonical enum order.
- Code-object identities record an opaque digest, format, and nonzero payload
  length without claiming the referenced payload exists or is authentic.
- Launch rank is one through three. Inactive dimensions are one, dimensions
  are nonzero, dimension products do not overflow, and static plus dynamic
  shared-memory requirements fit in `u32`.

ABI layout, kernel records, serialization, payload bytes, proof records, host
launch integration, and runtime loading policy are intentionally outside this
initial model layer.
