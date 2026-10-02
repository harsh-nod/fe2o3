# Ferric B3 Linear Host Reference

This standalone package adapts the host model from
[PR #191](https://github.com/harsh-nod/fe2o3/pull/191). It retains the exact
historical Ferric B3 source identity and finite target/draft selections,
but does not restore the retired tiled-GEMM planner.

Numerical execution directly reuses
`../tiled_gemm_general_v1/src/reference.rs` as a private module, with
`fe2o3-device` for BF16 conversion. There is no compiler backend, host
runtime, HIP, HSA, or GPU execution dependency. The model checks all
candidate fields, exact contiguous input lengths, and the Qwen `[N,K]`
to reference `[K,N]` weight transposition. It returns an ordinary
`Vec<f32>`, not a kernel, plan, trace, or qualification receipt.

## Identity And Numerical Boundary

Family, schema, route, and selection hashes use new host-reference domains.
The selection preimage consists of its domain; length-prefixed Ferric source,
family/schema/route/numerical domain strings and shared source digest;
role/mode/bucket/operator bytes; a little-endian u16 layer; and eight
little-endian u32 values (sequence/token/context counts, M/N/K, alpha/beta
bits). Production tests pin an independently encoded golden digest and the
complete shared reference source. These hashes are reproducible structural
identities, not authentication.

GEMV and GEMM are shape classifications (`M=1` and `M>1`); both run
the sequential oracle. No old Wave64/XOR4 schedule, grid, LDS, barrier trace,
GPU-plan identity, or twelve-property planner obligation is retained.
The source widens BF16, accumulates in increasing K in FP32, and evaluates
the complete `alpha * AB + beta * C` expression. In particular, C is read
even when beta is zero: `0 * NaN` remains NaN. Nonfinite results propagate;
NaN payload identity, BF16 output narrowing, and ISA-level refinement are
not claimed. Bounded vectors use ordinary infallible Rust allocation.

The pinned B3 graph uses hidden-width attention-output inputs for both
roles. For the draft graph, this is 1024 even though its query-head width
is 2048. This package preserves that historical graph contract; it does
not claim compatibility with a model checkpoint's projection tensors.
Alias fields record inherited conditional premises, not dynamically
authenticated allocation ownership.

## Production Checks

```sh
bash scripts/ci-local.sh host-reference
cargo test --locked --manifest-path examples/qwen3_linear_reference_v1/Cargo.toml --all-targets
```

The shared lane runs format, strict Clippy, debug and release tests, and
strict documentation through `test`, `generic-core`, and `generic`.
The standalone lockfile participates in `standalone-locks`.

Tests enumerate all 330 first/last-layer B3 selections without allocating
their large tensors. Numerical fixtures execute real canonical draft
GEMV, speculative GEMM, residual, and rectangular query shapes without
dimension overrides. They also check malformed metadata, exact extents,
nonfinite epilogues, and cancellation-sensitive accumulation.
