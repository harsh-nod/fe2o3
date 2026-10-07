# Native Fill Worker Test Payload

`kernel.hsaco` is the exact payload extracted from the existing authenticated
worker request at
`crates/fe2o3-kernel-analysis/src/gfx942_fill_analysis_v1/fill.request`.
It is test data, not current compiler, prover, device, or execution authority.

Regenerate using the request's structured canonical decoder:

```sh
cargo run -p fe2o3-kernel-analysis --example extract-native-fill-fixture -- \
  crates/fe2o3-runtime/tests/fixtures/native-fill-worker/kernel.hsaco
```

The generator reports both request and extracted payload SHA-256 hashes and the
exact payload length. The runtime tests intentionally consume only these inert
ELF bytes, avoiding a runtime dependency on the analysis/compiler framework.
The extracted payload is 6160 bytes with SHA-256
`8b6c2e5b67ba2f76bb57d5f42aedbaf968f8dc12bb121daa5d7853cc55d158c6`.
The source request SHA-256 is
`513cb8bb370dca0f79746a68849d09fa02200337dca72d5bda75fec30c7154f1`.
The Worker metadata has a 272-byte kernarg image; the independent 16-byte
standalone Rust fixture remains an explicit rejection control.
