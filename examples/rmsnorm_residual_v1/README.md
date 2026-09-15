# Qwen3 RMSNorm plus residual V1 foundation

This standalone crate defines a bounded, host-only foundation for a fused
Qwen3 RMSNorm plus residual operator. It admits only hidden widths 4096
(Qwen3-8B target) and 1024 (Qwen3-0.6B draft), and only the exact M1 B3
prefill, decode, and speculative buckets. A bucket record binds the model role,
sequence count, role-dependent active-token count, flattened rows, and hidden
width. Adjacent and custom shapes fail closed.

The semantic outputs are:

```text
z_i = BF16_to_FP32(x_i) + BF16_to_FP32(residual_i)
r   = 1 / sqrt(reduce_FP32(z_i * z_i) / hidden + 1e-6_f32)
normalized_i = BF16_RNE((z_i * r) * BF16_to_FP32(weight_i))
residual_out_i = BF16_RNE(z_i)
```

The structural gfx942 schedule assigns one Wave64 to each flattened row. Each
lane accumulates ascending stride-64 columns in FP32, followed by the exact
halving reduction stages `[32, 16, 8, 4, 2, 1]`. It uses no LDS, assigns one
owner to every output, requires uniform participation in all wave collectives,
and publishes host-reference outputs transactionally.

## Assurance boundary

The candidate, algorithm, and schedule identities hash canonical inert records.
They are not source, MIR, KIR, LLVM, object, or HSACO identities. They carry no
proof evidence and expose no compilation, publication, load, dispatch, or
launch API. The host reference uses Rust `f32`; the independent oracle uses
Rust `f64`. Neither establishes real-number, IEEE-754, OCML, ISA, GPU memory,
performance, or source-to-machine refinement.

Production remains blocked on the owner-consuming same-session Rust MIR
authority join tracked in issue #174, complete property discharge, machine
refinement, exact artifact admission, and the protected runtime join. This
foundation cannot bypass any of those boundaries.
