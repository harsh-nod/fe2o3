# Exact KIR V7 CPU simulation fixture

This versioned fixture is a reproducible, GPU-free standalone simulator input.
It fills four `u32` elements with `17` in one fixed 64-thread workgroup, so the
result records four executed invocations and 64 scheduled slots.

From the repository root:

```text
cargo run --locked -q -p fe2o3-kir-sim-cli --bin fe2o3-kir-sim -- \
  --kir-v7 crates/fe2o3-kir-sim-cli/tutorial/fill-v1/kernel.kir \
  --request crates/fe2o3-kir-sim-cli/tutorial/fill-v1/request.json
```

`examples/emit_tutorial_fill_v1_kir.rs` rebuilds the exact verified canonical
KIR owner. The regression test requires its bytes and the complete CLI output
to match `kernel.kir` and `expected-result.json`. The KIR V7 identity is
`e8f2c794a5dd4aeac63f5c820f9d5785b40b5aaff357e3f6726164fa4425f384`
over 245 canonical bytes. A semantic change requires a new fixture version.

This evidence begins at exact KIR V7. It does not associate Rust source with
the KIR and grants no proof, artifact, GPU-equivalence, timing, performance, or
performance-prediction authority.
