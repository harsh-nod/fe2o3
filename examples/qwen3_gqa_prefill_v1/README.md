# Qwen3 causal GQA prefill V1 host/model foundation

This standalone crate admits only the exact Qwen3-8B target and Qwen3-0.6B
draft attention geometries and M1 B3 prefill buckets `S1T128`, `S8T128`,
`S1T512`, and `S1T2048`.

Q and O are contiguous BF16
`[sequence][token][query_head][feature]`; K and V are contiguous BF16
`[sequence][token][kv_head][feature]`. Target uses 32 query heads and 8 KV
heads (four query heads per KV head). Draft uses 16 query heads and 8 KV heads
(two query heads per KV head). Both use head dimension 128. Each query attends
only to key tokens `0..=query_token` in the same sequence.

The tensor-stage boundary is explicit: Q/K/V are already projected, Q/K are
already QK-normalized and rotary-position encoded, and O is the
pre-output-projection attention tensor. Projection, Q/K normalization, RoPE,
and output projection are separate operators and are not implemented here.

The intended host order uses ascending-feature FP32 QK multiplication and
addition, one post-dot FP32 `1/sqrt(128)` scale, materialized causal scores, an
ascending two-pass stable FP32 softmax using the Rust host `f32::exp`, and
ascending-key FP32 weighted-V multiplication/addition and division. Output is
BF16 round-to-nearest, ties-to-even. Every physical input and non-finite
intermediate rejects; exponential underflow to positive zero is allowed. Full
finite BF16 subnormals and signed zeros are admitted and decoded exactly; the
host model does not impose flush-to-zero. Full output is staged and published
only after every vector succeeds. The host evaluator retains only linear
score/weight scratch, while the resource record exposes the complete bounded
quadratic operation count.

## Assurance boundary

Algorithm, evaluation, and candidate identities cover canonical inert records.
They are not source, MIR, KIR, LLVM, object, or HSACO identities and carry no
Verus result or compiler custody. The crate contains no GPU code, compilation,
artifact publication, load, dispatch, or launch API. The FP32 reference and
independent `f64` oracle do not establish real-number, IEEE-754, OCML, ISA,
machine-safety, performance, or source-to-machine refinement.

Production remains blocked on the owner-consuming same-session Rust MIR
authority join in issue #174, complete Verus/property discharge, machine
refinement, artifact admission, and the protected runtime join.
