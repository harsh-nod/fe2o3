# gfx950 advanced attention kernels

The ordinary attributed Rust kernels in [`src/kernel.rs`](src/kernel.rs) are
the fe2o3 source for these tutorials. [`src/reference.rs`](src/reference.rs)
contains independent safe CPU references, and `cargo test --offline` checks
their fixed-shape numerical and selection contracts. The HIP program remains
a separate compiler, ISA, and MI350 hardware-validation companion.

The current rustc backend cannot lower these Rust sources to gfx950 HSACO: its
production profile is still exact `gfx942:xnack-`, and gfx950 low-precision
tensor, transpose, device-math, and control-flow lowering are pending. The
Rust kernels therefore use conservative scalar grid-leader algorithms and set
`GFX950_ADVANCED_ATTENTION_SOURCE_LOWERING_SUPPORTED_V1` to `false`. They
claim source and CPU-reference evidence only; the HIP ISA and runtime results
below do not become fe2o3 artifact or execution evidence.

This directory is a bounded educational validation suite for AMD CDNA 4
(`gfx950`). It is not a production implementation, performance claim, model
reproduction, or general-purpose operator library.

The fixed shapes are 16 channels, eight recurrence tokens, 16 attention
tokens, and attention head dimension 128. The suite contains:

- single-token KDA/GDN-style decode with a three-tap causal convolution,
  gated recurrent state update, and RMS normalization;
- two-chunk KDA/GDN-style prefill with four tokens per chunk and state carried
  across the chunk boundary;
- content-indexed sparse attention with top-two block selection, top-three
  token selection, and sparse normalization/value reduction over those tokens;
- compressed hybrid attention combining three compressed global blocks with a
  four-token sliding window;
- AttnRes four-depth softmax aggregation, four-branch gated residual mixing,
  and a four-stream mHC mixer with three Sinkhorn iterations.

Every output and sparse index is compared against an independently written CPU
oracle using deterministic inputs. Attention Q, K, and V use non-uniform,
exactly representable E4M3 values so token-dependent score and transpose errors
cannot cancel as a common softmax term. The executable rejects non-gfx950 devices.
The two attention kernels use a gfx950 FP8
`v_mfma_f32_16x16x128_f8f6f4` score tile whose K operand is supplied by four
`ds_read_b64_tr_b8` LDS transpose reads. `check_isa.sh` validates those
instructions, their exact counts, and transpose-before-MFMA ordering within
each kernel symbol. At this bounded shape, the score MFMA covers all 16 tokens;
the sparse kernel applies its selected ragged set to softmax and the value
reduction. This suite does not claim a production sparse-QK scheduling strategy.

Run the Rust source and independent CPU-reference checks:

```bash
cargo test --offline
```

Build, inspect, and run the companion HIP validation on a gfx950 system:

```bash
./build_and_test.sh
```

Compiler and ISA validation without execution is available with:

```bash
./build_and_test.sh --compile-only
```

## HIP validation evidence

On 2026-08-26, `./build_and_test.sh` passed via SSH alias `mi350` (remote
hostname `smci350-rck-g03-b19-03`) with ROCm 7.2.1, HIP 7.2.53211, AMD Clang
22.0.0git, and a visible gfx950 device. The code object metadata target was
`amdgcn-amd-amdhsa--gfx950`. FP8 register packing follows the documented CDNA
4 split: lane group `g` supplies K=`g*16..g*16+15` in v0-v3 and
K=`64+g*16..64+g*16+15` in v4-v7; transpose-source staging produces that same
layout after the four B8 transpose reads.

The sparse IDs were exactly `[7,1,4]`. Maximum errors were `2.98023e-08` for
decode state, `4.76837e-07` for decode normalization, `1.49012e-08` for prefill
state, `3.57628e-07` for prefill normalization, `2.98023e-08` for sparse attention,
`1.67638e-07` for compressed hybrid attention, `0` for AttnRes, `0` for the
four-branch residual, and `2.98023e-08` for mHC/Sinkhorn mixing.

Both attention symbols contained exactly four `ds_read_b64_tr_b8` followed by
one `v_mfma_f32_16x16x128_f8f6f4`. The tested HIP source SHA-256 was
`c44b4227c0ec525a367359bdc16aff69c3086676aa61def1b653266604d1ed1d`.
The validated code-object SHA-256 was
`dcfb1e00354ac14dffae5e069138c5e212b0906133838195dd717686af26ce84`;
the host executable SHA-256 was
`d741786606e6a4d05ab2fd5f0a411bbc696a3dc6b12bce87c98eb624be39901e`.
