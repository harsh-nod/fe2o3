# Qwen3 SwiGLU V1 foundation

This standalone crate defines a bounded, host-only foundation for the Qwen3
SwiGLU activation between the gate/up projections and down projection. It
admits only Qwen3-8B target intermediate width 12,288 and Qwen3-0.6B draft
width 3,072, with all and only the eleven Ferric M1 B3 prefill, decode, and
speculative buckets.

The exact schedule-model expression for each element is:

```text
sigmoid = gate >= 0
            ? 1_f32 / (1_f32 + exp_f32(-gate))
            : exp_f32(gate) / (1_f32 + exp_f32(gate))
silu    = BF16_to_FP32(gate) * sigmoid
output  = BF16_RNE(silu * BF16_to_FP32(up))
```

The inert gfx942 schedule assigns each output element to exactly one logical
owner in 256-thread workgroups, with eight contiguous elements per thread.
Inputs are read-only, output is disjoint, no LDS or barrier is required, and
the host model publishes output transactionally.

## Assurance boundary

Profile, algorithm, schedule, and candidate identities hash canonical inert
records. They are not source, MIR, KIR, LLVM, object, HSACO, proof, or runtime
identities and carry no compile, publication, load, dispatch, or launch
authority. Rust `f32::exp` is the pinned host schedule model and the separate
oracle uses Rust `f64`; neither proves OCML, IEEE-754, ISA, GPU memory,
performance, or machine refinement.

Production remains blocked on the same-session Rust MIR authority join in
issue #174, complete property discharge, machine refinement, exact artifact
admission, and protected runtime composition.
