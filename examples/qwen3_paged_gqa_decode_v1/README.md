# Qwen3 paged causal GQA decode V1 host/model foundation

This standalone crate admits only the exact Qwen3-8B target and Qwen3-0.6B
draft attention geometries and M1 B3 decode/speculative buckets: decode
`S1/S8/S32 C8192` and speculative `S1K4`, `S8K4`, `S1K8`, and `S1K16 C8192`.
Target speculative widths are `K+1`; draft widths are `K`.

Q and O use BF16 `[sequence][active_token][query_head][feature]`. K and V use
BF16 `[physical_page][16][kv_head][feature]`. Every request supplies exactly
512 logical P16 entries, a nonzero request identity and generation, and
committed/resident token counts. The resident prefix must equal committed plus
the bucket's active width. Query `j` is at logical position `committed+j` and
reads exactly keys `0..=committed+j`. Thus tentative speculative K/V may be
read only by their causal query; the initialized suffix after that query and
all uninitialized final-page slots are masked.

Page-table entries are ordered by logical page, generation- and request-bound,
and form an injective permutation of the finite physical page pool. Per-page
initialized counts must encode the one exact resident prefix, including its
final-page tail. Key and value allocations are physically disjoint. Prefix
sharing and aliased pages are outside this initial contract; arbitrary
injective page fragmentation is admitted.

The tensor-stage boundary is explicit: Q/K/V are already projected, Q/K are
already QK-normalized and rotary-position encoded, and K/V have already been
written by the separate K3 operator. The output is pre-output-projection.
Projection, Q/K normalization, RoPE, KV writes, acceptance, rollback, and
output projection are not implemented here.

The host order is ascending-feature FP32 QK multiply/add, one post-dot FP32
`1/sqrt(128)` scale, ascending two-pass stable FP32 softmax using Rust host
`f32::exp`, ascending-key FP32 weighted-V multiply/add and division, then BF16
round-to-nearest, ties-to-even. Every logically read BF16 value and every
intermediate must be finite. Exponential underflow to positive zero is
allowed. Output is transactional and published only after complete success.

## Assurance boundary

Candidate and metadata identities cover canonical inert structural records.
The metadata identity does not authenticate Q/K/V contents. The crate has no
GPU code, compiler integration, artifact publication, load, dispatch, or
launch API. The references do not establish real-number, IEEE-754, OCML, ISA,
machine-safety, performance, or source-to-machine refinement. Production is
blocked on issue #174, complete proof/property discharge, machine refinement,
artifact admission, and the protected runtime join.
