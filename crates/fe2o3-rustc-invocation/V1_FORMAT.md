# V1 invocation format

V1 is frozen for compatibility. New coordinated builds should use a later
version.

The V1 descriptor records Cargo executable, package, and target metadata; the
rustc executable, crate and target metadata, test state, and ordered arguments;
backend, clang, linker, and optional inspector identities; the AMD target and
verification mode; workspace and output paths; and a sorted compile environment.

Integers are little-endian. Names and versions use `u16` byte lengths. Paths,
arguments, and environment values use `u32` byte lengths. The fixed header is:

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

A V1 tool identity is a nonempty bounded version string followed by its 32-byte
SHA-256 digest. The complete descriptor is at most 256 KiB. Decoding rejects
truncation, trailing bytes, oversized fields, invalid UTF-8, bad tags, nonzero
flags or reserved fields, and noncanonical sets, then requires byte-identical
re-encoding.

The V1 coordination digest is:

```text
SHA256(
    "FE2O3/RUSTC-BUILD-INVOCATION/V1\0" ||
    u64_le(encoded_descriptor_length) ||
    encoded_descriptor
)
```

The terminating NUL is part of the domain. The all-zero digest is reserved.
