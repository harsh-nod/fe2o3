# Qwen3 logits and compact completion V1 host/model foundation

This standalone crate covers the exact B3 Qwen3 target/draft logits projection
and following argmax/compact-completion step. It admits all eleven B3 buckets:
four prefill, three decode, and four speculative buckets. Target hidden width
is 4096, draft hidden width is 1024, and both use vocabulary 151936. Flattened
rows are exactly `sequences * active_tokens`; speculative target uses `K+1`
rows per request and draft uses `K`, while declared `K` remains at most 16.

Activations are BF16 `[row][hidden]` and the bias-free LM-head weight is BF16
`[token_id][hidden]`. Each logit uses ascending-hidden FP32 multiplication and
addition with no contraction. Inputs and every intermediate must be finite.
Argmax scans token IDs ascending and replaces the winner only on strict greater
comparison, so all equal maxima, including signed-zero ties, select the lowest
token ID.

The combined reference accepts a bounded logit provider. The concrete BF16
projection provider implements the exact projection above without requiring a
second full weight or logits image. Output records are staged transactionally
and bind request slot/generation, submission epoch, plan identity, candidate
identity, row/local-token coordinates, and selected token ID. Record identities
are structural commitments, not authentication or completion authority.

## Assurance boundary

The crate is host-only and independent of the production compiler. It neither
imports nor grants general-GEMM authority. It has no GPU source or schedule,
Verus proof, compiler/MIR/KIR custody, artifact publication, load, dispatch,
launch, HSA quiescence, IEEE/ISA/machine refinement, or performance claim.
Issue #174 and the protected runtime completion join remain open.
