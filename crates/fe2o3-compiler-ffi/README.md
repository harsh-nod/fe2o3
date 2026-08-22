# fe2o3-compiler-ffi

This crate defines the LLVM-free, authority-free envelope produced from a successful
`rustc-codegen-fe2o3` device FFI collection. The envelope commits to the exact target,
code-object version, canonical imports and exports, source ownership, physical ABI, declared
effects, effect-to-ABI compatibility, semantic identity, and required definition role.

Construction is bounded and deterministic. The rustc adapter first streams borrowed contract fields
through `preflight_compiler_ffi_envelope_v1`, which checks cardinality, grammar, aggregate text, and
the exact encoded size before reserving the contract vector, cloning contract fields, or allocating
canonical envelope bytes. Canonical target formatting may allocate one bounded temporary string.
The builder requires one canonical contract order, rejects duplicate IDs, symbols, owners, and
semantic identities, rechecks preflight sizes, and hashes canonical bytes under a dedicated V1
domain. Contract grammar and IDs come from `reserved-fe2o3-symbols`.

The constructors are public so tests and non-rustc producers can create structurally identical
values. The envelope binds bytes but does not authenticate that rustc produced them.

`ExternalDeviceLibraryManifestV1` is the bounded, canonical contract for gfx942 device-function
libraries. `ExternalDeviceLibraryProviderV1::new` performs only byte-bound, declared-length, and
recognizable-header preflight over borrowed provider bytes. It does not invoke an LLVM bitcode
reader, parse complete LLVM bitcode, inspect ELF sections or symbols, or establish that content is
well-formed or linkable. Provider-set validation checks the aggregate content bound before hashing
or allocating closure maps, then checks exact digests, dependency identities, declared profiles,
and import/export contracts. LLVM profile compatibility is only declared major/triple/data-layout
agreement; it is not linker admission. These public structural values authenticate no producer,
proof, or provenance and grant no compiler, link, load, or launch authority.

`CompilerDescriptorSourceV1` retains one bounded canonical `DeviceDescriptorTableV1` whose
code-object digest is still zero. Its identity commits to the exact table bytes that a later
ELF-aware stage may embed and finalize. It rejects already-finalized, malformed, truncated,
trailing, or noncanonical input. Public construction remains structural and does not authenticate
rustc or grant link, load, or launch authority.

`CompilerModuleHandoffV1` is the corresponding bounded data container for one exact LLVM text-IR
or bitcode module and one envelope. Its canonical encoding commits to the module kind, SHA-256 and
byte length, target, code-object version, and exact canonical envelope bytes. Strict decoding
rejects oversized or truncated fields before retaining payload storage, malformed UTF-8, trailing
bytes, digest substitution, target or code-object disagreement, and noncanonical envelope roles or
contract order. Public construction and decoding remain authority-free: the handoff does not
authenticate its producer or grant compiler, link, load, or launch authority.
`CompilerModuleHandoffV1::into_parts` moves the retained envelope and exact module payload into
opaque owned components, allowing finalization to reuse both without interpreting wire offsets or
reconstructing envelope fields. The decomposition does not change that authority classification.

`InertSemanticCompilerModuleHandoffV3` is the strict, bounded outer content schema that joins one
exact `InertProductionSemanticCapsuleV3` to one exact `CompilerModuleHandoffV2`. It retains both
complete canonical byte strings and their native identities. A fixed pair-binding segment commits
only to those already-complete inner identities under its own V3 domain; the terminal outer
identity then commits to the header, both exact inner encodings, and the complete pair-binding
segment under a separate V3 domain. The pair segment never refers to the outer identity, so the
hash dependency graph is acyclic.

The V3 outer decoder accepts no earlier schema or fallback. It validates the outer version, flags,
reserved fields, both inner lengths, complete aggregate length, pair-segment length, and exported
resource limits before decoding either inner owner or allocating the outer canonical buffer. It
then requires strict canonical V3 capsule and V2 module-handoff decodes, exact native identity
matches, canonical target agreement, a valid pair-binding identity, a valid terminal outer
identity, and byte-for-byte canonical reconstruction. Truncation, trailing bytes, substitutions,
and noncanonical encodings are rejected.

The `Inert` prefix is a security boundary. Public construction can fully rehash any internally
valid target-compatible capsule/module pair, including a cross-producer splice. Therefore this
object establishes content identity only: it does not authenticate a producer, prove semantic
derivation or compiler origin, establish artifact freshness, or grant compiler, artifact, worker,
link, publication, load, or launch authority. Those capabilities require a later private
producer-owned admission boundary.

All integers below are unsigned little-endian. The canonical outer encoding is:

1. `F2O3IHV3`, version `3` (`u16`), zero flags (`u16`), total length (`u64`), and zero reserved
   bits (`u32`);
2. capsule length (`u64`) and V2 module-handoff length (`u64`);
3. exactly that many canonical capsule bytes and canonical V2 module-handoff bytes;
4. the fixed pair-binding segment described below; and
5. the terminal 32-byte outer identity.

The fixed pair-binding segment is `F2O3PBV3`, version `3` (`u16`), zero flags (`u16`), fixed
segment length (`u32`), zero reserved bits (`u32`), then the capsule SHA-256 and length, the V2
handoff SHA-256 and length, and the terminal 32-byte pair-binding identity. The pair identity is
`SHA-256("FE2O3/INERT-COMPILER-MODULE-PAIR-BINDING/V3\0" || u64(preimage-length) ||
pair-preimage)`. The outer identity is
`SHA-256("FE2O3/INERT-SEMANTIC-COMPILER-MODULE-HANDOFF/V3\0" || u64(preimage-length) ||
complete-outer-preimage)`. Neither hash preimage contains the terminal outer identity.

`CompilerModuleSymbolManifestV1` adds a bounded, canonical classification of kernel entries,
kernel descriptor symbols, device FFI exports, internal helpers, and unresolved external imports.
Entries use a fixed role order followed by bytewise symbol order. Construction and strict decoding
reject empty or NUL-bearing names, oversized fields, duplicate names, cross-role overlap,
noncanonical order, truncation, and trailing bytes. `CompilerModuleHandoffV2` embeds the exact
manifest identity and bytes in a separate wire domain, cross-checks its import and export roles
against the FFI envelope, and commits the complete encoding under a V2 handoff identity. V1 bytes
and decoding are unchanged. These values preserve compiler-supplied observations for an external
authenticated transaction; their public constructors do not themselves authenticate compiler
origin or confer execution authority.

The finished envelope exposes only a borrowed opaque directional-symbol projection over its
retained validated contracts. The projection preserves canonical order and cannot be constructed,
mutated, or kept independently of its envelope. It exposes no complete contract list, provider
artifact, linker input kind, bitcode claim, or Worker V1 conversion. Both envelope and projection
remain inert compiler observations: neither authenticates compiler origin, proves that a compiler
module exists, nor grants compiler, link, load, or launch authority.
