# fe2o3-compiler-ffi

This crate defines the LLVM-free, authority-free envelope produced from a successful
`rustc-codegen-fe2o3` device FFI collection. The envelope commits to the exact target,
code-object version, canonical imports and exports, source ownership, physical ABI, declared
effects, effect-to-ABI compatibility, semantic identity, and required definition role.

Construction is bounded and deterministic. The rustc adapter first streams borrowed contract fields
through `preflight_compiler_ffi_envelope_v1`, which checks cardinality, grammar, aggregate text, and
the exact encoded size before reserving the contract vector or cloning text. The builder requires
one canonical contract order, rejects duplicate IDs, symbols, owners, and semantic identities,
rechecks preflight sizes, and hashes canonical bytes under a dedicated V1 domain. Contract grammar
and IDs come from `reserved-fe2o3-symbols`.

The constructors are public so tests and non-rustc producers can create structurally identical
values. The envelope binds bytes but does not authenticate that rustc produced them.

The finished envelope deliberately exposes no contract list, provider artifact, linker input kind,
expected final symbol set, bitcode claim, or Worker V1 conversion. It is an inert compiler
observation, not proof that a compiler module exists and not link, load, or launch authority.
