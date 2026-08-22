# fe2o3-compiler-lineage

This crate owns a canonical, bounded **inert content format**. It records one
caller-selected Rust compilation invocation beside fifteen caller-supplied
semantic compilation transcripts through the final LLVM module. That
association is data, not proof that the inputs share a producer or derivation.

`InertProductionSemanticCapsuleV3` is deliberately inert. Its name is an API
boundary: it must never be accepted where an authenticated producer-owned
capsule is required. Its hashes detect accidental corruption and byte
substitution relative to the bytes in one capsule. Public construction does
not authenticate who produced those bytes,
prove that a receipt is truthful, establish that one stage derived the next,
or grant compiler, artifact, publication, load, or launch authority. A later
producer-owned integration boundary must construct the receipts from retained
typed stage owners, authenticate that construction, bind this inert capsule to
the exact outer module handoff, and expose a distinct move-only admitted type.
The outer handoff and producer authentication are deliberately absent here.

The decoder is strict: it accepts only version 3, zero flags and reserved bits,
one exact total length, canonical V3 rustc invocation bytes, a canonical AMD
target spelling matching that invocation, nonempty bounded receipt preimages,
matching per-receipt inert content identities, and a matching terminal inert
capsule identity. It
never falls back to another schema. Receipt payloads other than the rustc
invocation and target remain opaque to this dependency-light crate; their
stage-specific producers are responsible for supplying canonical transcripts.

Resource limits are part of the wire contract:

- semantic MIR: at most 128 MiB;
- every other stage transcript, including LLVM: at most 4 MiB each;
- complete capsule: at most 160 MiB;
- rustc invocation: the bound exported by `fe2o3-rustc-invocation`;
- target spelling: at most 128 bytes.

Lengths are checked before inert receipt allocation. A successful decode may retain
up to the exported decoder-owned allocation bound because it keeps both stage
preimages and a complete canonical encoding. These bounds limit, but do not
eliminate CPU and memory denial-of-service risk when decoding untrusted input.
