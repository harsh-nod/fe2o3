# fe2o3-compiler-ffi

This crate defines the LLVM-free, authority-free envelope produced from a successful
`rustc-codegen-fe2o3` device FFI collection. The envelope commits to the exact target,
code-object version, canonical imports and exports, source ownership, physical ABI, declared
effects, effect-to-ABI compatibility, semantic identity, and required definition role.

Construction is bounded and deterministic. The builder checks cardinality before reserving its
contract vector, validates borrowed text before cloning it, requires one canonical contract order,
computes the exact encoded size before allocating canonical bytes, and hashes those bytes under a
dedicated V1 domain. Contract grammar and IDs come from `reserved-fe2o3-symbols`.

The finished envelope deliberately exposes no contract list, provider artifact, linker input kind,
expected final symbol set, bitcode claim, or Worker V1 conversion. It is an inert compiler
observation, not proof that a compiler module exists and not link, load, or launch authority.
