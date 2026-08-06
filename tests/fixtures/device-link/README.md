# Bidirectional device FFI source fixture

`rust-device/src/lib.rs` imports `external_scale_bias_v1` and exports
`rust_accumulate_v1`. `external.amdgpu.ll` defines the former and declares and
calls the latter, representing both directions at source level. The Rust
kernel checks both input and output extents before either access.

`external.evidence.v1.json` is canonical JSON with one trailing newline. It
binds the exact external LLVM IR SHA-256 digest, target, code-object version,
definition, declaration, physical ABIs, effects, and semantic identities. Its
authority fields are false and its limitations are exact.

The LLVM IR uses `memory(none)` consistently with both FFI contracts declaring
`effects = "none"`. The optional runner verification assembles the IR and runs
LLVM's verifier; unavailable tools are reported as unavailable rather than a
pass.

These files are source-model inputs, not LLVM bitcode, relocatable objects,
HSACO, compiler-derived closure, authenticated artifacts, production load
authority, launch authority, or hardware-execution evidence.
