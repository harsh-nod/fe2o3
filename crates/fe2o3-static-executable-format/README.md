# Static Executable Format

One authority-free parser and canonical identity for loader-independent x86-64
ELF images. Used by protected executable custody and runtime handoff validation.
The runtime-protocol crate retains its existing public identity/error exports.

This crate has no compiler, verifier, OS-descriptor or deployment dependencies.
Accepted bytes alone do not establish immutable storage, approved code, secure
startup, process confinement, proof validity or launch authority.

```sh
cargo test --locked -p fe2o3-static-executable-format
```
