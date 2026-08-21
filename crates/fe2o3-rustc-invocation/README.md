# fe2o3-rustc-invocation

This crate defines bounded canonical descriptors for coordinating one exact
rustc process assigned the fe2o3 backend. An invocation descriptor records
requested compiler inputs. It is not an artifact manifest, proof record, or
launch authorization.

An invocation digest does not attest source contents, dependency closure,
compiler output, a GPU binary, kernel ABI, Verus evidence, or the execution
device. Compiler-owned output descriptors must bind those results, and the
runtime must validate them against the loaded code object and observed device
before launch.

## Versions

V1 is the original frozen schema. Its types, encoder, decoder, digest domain,
and golden wire fixture remain available for compatibility. V1 permits callers
to supply overlapping semantic fields independently and therefore must not be
used as a trusted identity for new coordinated builds. Its normative field and
wire reference is retained in [`V1_FORMAT.md`](V1_FORMAT.md).

V2 represents each compiler-visible input once. V1 and V2 use explicit wire
versions and distinct digest domains; each decoder rejects the other version.

V3 preserves the exact V2 rustc unit and environment semantics and adds the
canonical `fe2o3_build_authority::CompilerClosureV2` identity preimage. All
three versions have disjoint wire versions and digest domains. V1 and V2 bytes,
APIs, and digest constructions remain frozen.

## V3 model

`RustcInvocationDescriptorV3` owns a complete `RustcInvocationDescriptorV2`
and a `CompilerClosureV2`. `from_v2_and_compiler_closure` is the explicit
upgrade operation. It rejects the upgrade unless the V2 rustc-executable and
codegen-backend digests equal the closure pins assigned those exact roles.
The closure's aggregate identity remains derived rather than independently
declared in the V3 wire format.

The V3 closure preimage is encoded in the same order used by
`derive_compiler_closure_identity_v2`:

```text
Cargo binding transition protocol version, u16 little-endian
Cargo executable SHA-256 digest
Cargo binding trampoline SHA-256 digest
cargo-fe2o3 binding wrapper SHA-256 digest
rustc executable SHA-256 digest
rustc runtime-tree SHA-256 digest
codegen backend SHA-256 digest
```

The fixed `COMPILER_CLOSURE_IDENTITY_DOMAIN_V2` is implicit in the typed
schema. These fields are followed by the V2 body byte-for-byte, including its
rustc/backend digest fields, working directory, exact argv, and complete sorted
environment. The intentional duplicate rustc/backend pins are cross-checked on
construction, encoding, and decoding. The V3 size bound is the V2 bound plus
the fixed 194-byte closure preimage, so every valid V2 descriptor remains
eligible for an upgrade.

## V2 model

`RustcInvocationDescriptorV2` records:

- caller-asserted SHA-256 digests for the rustc executable and codegen backend;
- rustc's lexically canonical absolute working directory;
- the exact ordered final rustc argument vector, including `argv[0]`; and
- the complete intended rustc process environment, sorted by key.

The rustc path appears only in `argv[0]`. The backend path appears only in the
wrapper-injected final `-Zcodegen-backend=<path>` argument. V2 rejects every
other hyphen/underscore and joined/split `-Z` backend-selector spelling
anywhere in argv. The one Linux capability spelling admitted by the backend
path type is exactly `/proc/./self/fd/198`; canonical `/proc/self/fd/198`, all
other descriptor numbers, and procfs/devfd aliases are rejected. Ordinary
lexically canonical non-proc absolute backend paths remain valid as pathname
identities, not descriptor capabilities. `/dev/fd`, `/dev/stdin`,
`/dev/stdout`, `/dev/stderr`, and all `/proc` spellings other than the exact
capability are rejected. V2 also rejects an option terminator before the final
managed selector because rustc would interpret that selector as an input path.
Crate name,
source, crate types, edition, features, target options, and all other
compiler-visible unit properties remain in argv or the environment. `FE2O3_TARGET`,
`FE2O3_HSACO_DIR`, and `FE2O3_VERIFY_KERNEL_IR` are validated in place and
exposed as derived views; they are not copied into parallel fields.

Cargo provenance, package selection, workspace containment policy, and other
selection intent do not belong in an exact process identity. They require a
separate selection-intent descriptor.

`classify_rustc_invocation_v2` provides the lossless classifier intended for
the workspace wrapper. It is separate from frozen wire decoding and can never
authorize artifacts by itself. Terminal and query modes are recognized before
compile metadata. In particular, Cargo's `rustc - --crate-name ___
--print=file-names ...` probe is passed through rather than mistaken for a
compile. Known output-suppressing rustc modes, response files, and ambiguous or
partial compile shapes fail closed. A managed build succeeds only after the
backend independently claims the exact build attempt and publishes output.

## Environment

`CompileEnvironmentV2::capture_current` captures the current process set.
`CompileEnvironmentV2::from_child_environment` accepts a complete explicitly
prepared child set. A wrapper must call `configure_command` on the command it
executes; this clears inherited entries and installs exactly the recorded set.
The environment is an intended input until trusted execution is coupled to the
descriptor.

All `FE2O3_TRANSPORT_*` variables are reserved and rejected. Build sessions,
attempt tokens, descriptor transport, and other protocol state must use a
separate channel. Trusted execution must also close unrelated inherited file
descriptors and explicitly control any descriptors retained for pinned tools
or transport. Retained descriptors must be closed before proc macros or other
compiler children are launched.

V2 accepts only UTF-8 compile arguments, paths, environment keys, and
environment values. Query passthrough classification remains lossless for
non-UTF-8 platform-native arguments. Environment capture stops at its entry
bound without collecting unbounded input.

## Paths and tools

V2 model paths use `/`, contain no empty, `.` or `..` components, and reject
backslashes and NUL. Model path validation is lexical. It does not resolve
symlinks or prove filesystem containment.

Executable digests are caller assertions until a trusted wrapper binds them to
execution. The wrapper must canonicalize and open each tool, hash that opened
object, and execute or load the same pinned object. A path that is reopened
after hashing is vulnerable to replacement and is not a trusted binding.

For rustc, Linux execution can consume a pinned executable handle. For the
dynamically loaded backend, the wrapper must retain a pinned object until rustc
loads it, for example through a sealed immutable object or a deliberately
inherited descriptor path. The backend must apply equivalent pinning to tools
it selects internally.

## V2 wire format

Integers are little-endian. Unknown flags and nonzero reserved values are
rejected. Environment keys use `u16` byte lengths. Paths, arguments, and
environment values use `u32` byte lengths. Counts and lengths are checked
before allocation.

The fixed header is:

```text
offset  size  field
0       8     magic = "FE2O3RI\0"
8       2     version = 2
10      2     flags = 0
12      4     total byte length
16      4     reserved = 0
```

The V2 body is encoded in this order:

```text
rustc executable SHA-256 digest
codegen backend SHA-256 digest
absolute rustc working directory
argument count, ordered length-prefixed arguments including argv[0]

environment count, zero reserved field
sorted key and value records
```

The encoded descriptor is at most 256 KiB. V2 permits at most 4096 rustc
arguments and 1024 environment entries. Decoding checks every bound before
allocation, revalidates typed and cross-field invariants, and requires
byte-identical re-encoding.

## Digests

The V3 coordination digest is:

```text
SHA256(
    "FE2O3/RUSTC-BUILD-INVOCATION/V3\0" ||
    u64_le(encoded_descriptor_length) ||
    encoded_descriptor
)
```

The V2 coordination digest is:

```text
SHA256(
    "FE2O3/RUSTC-BUILD-INVOCATION/V2\0" ||
    u64_le(encoded_descriptor_length) ||
    encoded_descriptor
)
```

V1 uses the same construction with its frozen V1 encoding and V1 domain. The
terminating NUL is part of each domain. The all-zero digest is reserved by the
artifact transaction protocol.
