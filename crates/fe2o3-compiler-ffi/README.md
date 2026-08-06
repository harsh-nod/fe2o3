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

The finished envelope exposes only a borrowed opaque directional-symbol projection over its
retained validated contracts. The projection preserves canonical order and cannot be constructed,
mutated, or kept independently of its envelope. It exposes no complete contract list, provider
artifact, linker input kind, bitcode claim, or Worker V1 conversion. Both envelope and projection
remain inert compiler observations: neither authenticates compiler origin, proves that a compiler
module exists, nor grants compiler, link, load, or launch authority.
