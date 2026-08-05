# fe2o3-rustc-invocation

This crate defines the bounded, canonical `RustcInvocationDescriptorV1`
schema used to coordinate one selected Cargo compilation with the fe2o3 rustc
backend. The descriptor identifies how that compilation was requested. It is
not an artifact manifest, proof record, or launch authorization.

In particular, an invocation digest does not attest source contents, the
dependency closure, compiler output, a GPU binary, kernel ABI, Verus evidence,
or the device on which code will execute. A later compiler-owned descriptor
must bind those results, and a trusted runtime must validate that descriptor
against the loaded code object and observed device before launch.

## V1 model

The descriptor records:

- the Cargo executable version and SHA-256 digest;
- the selected workspace package, manifest, Cargo target, crate types,
  edition, source path, and activated features;
- the rustc executable version and SHA-256 digest, crate name, host and
  effective target triples, test state, and exact ordered final argument list;
- role-specific identities for the backend, clang, linker, and optional
  inspector executables;
- a canonical concrete AMD target ID and the result-affecting verification
  mode;
- canonical absolute workspace and artifact-output paths; and
- the sorted, unique compile environment selected by the caller.

Tool roles are structural fields. V1 never accepts a free-form role name.
Callers must hash the executable bytes they actually run and must remove
session, attempt-token, descriptor-transport, and other non-semantic variables
before constructing the compile-environment set.

All constructors validate their inputs and all fields are private. Set-like
collections must be supplied in strictly increasing order and without
duplicates. Rustc arguments remain in their original order and may repeat.
V1 accepts only UTF-8 arguments, paths, and environment values; a caller that
encounters a non-UTF-8 compiler input must fail closed rather than describe a
different invocation.

## Canonical paths and text

Names are nonempty UTF-8, contain no NUL, and are at most 128 bytes. General
text, versions, arguments, and environment values contain no NUL and are at
most 4096 bytes; versions are additionally nonempty. Relative paths are
nonempty, do not start with `/`, and contain only nonempty components other
than `.` or `..`. Absolute paths start with `/` and obey the same component
rules. Backslashes and NUL are rejected in every path. `/` is the one valid
absolute path without a component.

The AMD target spelling contains one known concrete `gfx` processor followed
by optional `sramecc[+-]` and `xnack[+-]` modifiers in that order. Duplicate,
unsupported, unknown, or reordered modifiers are rejected.

## Wire format

Integers are little-endian. Every enum has an explicit nonzero V1 tag, and all
unknown tags are rejected. Names and versions use `u16` byte lengths. Paths,
arguments, and environment values use `u32` byte lengths. Counts are checked
against their public limits before allocation.

The fixed header is:

```text
offset  size  field
0       8     magic = "FE2O3RI\0"
8       2     version = 1
10      2     flags = 0
12      4     total byte length
16      4     reserved = 0
```

The body is encoded in this order:

```text
cargo executable identity
cargo package name, version, workspace-relative manifest
selected target name, kind, edition, zero flags
crate-type count, feature count, zero reserved field
workspace-relative source, crate-type tags, feature names

rustc executable identity
rustc crate name, host target, effective target
test-state tag, zero reserved fields
argument count, ordered length-prefixed arguments

backend identity, clang identity, linker identity
inspector presence tag, zero reserved fields, optional inspector identity

canonical AMD target ID
verification-mode tag, zero reserved fields
absolute workspace root, absolute artifact output directory

environment count, zero reserved field
sorted key and value records
```

A tool identity is a nonempty bounded version string followed by its 32-byte
SHA-256 executable digest. The complete descriptor is at most 256 KiB. V1
permits at most 4096 rustc arguments, 1024 environment entries, and 1024 Cargo
features. Decoding rejects truncation, trailing bytes, oversized fields,
invalid UTF-8, bad tags, nonzero flags or reserved fields, and noncanonical
sets. It then re-encodes the typed value and requires exact byte equality.

## Invocation digest

The coordination digest is exactly:

```text
SHA256(
    "FE2O3/RUSTC-BUILD-INVOCATION/V1\0" ||
    u64_le(encoded_descriptor_length) ||
    encoded_descriptor
)
```

The domain includes its terminating NUL. An all-zero digest is reserved by the
artifact transaction protocol and cannot be constructed through
`InvocationDigest`.
