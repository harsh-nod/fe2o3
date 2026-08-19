# General tiled GEMM safe-source fixture

This standalone crate contains the positive ordinary-Rust kernel source for
issue #138. The kernel accepts dynamic `M`, `N`, `K`, `lda`, `ldb`, and `ldc`,
uses 16x16x16 BF16/F32 phases across a 2D grid, zero-fills guarded operand
tails, carries its accumulator across all phases, and applies `alpha`/`beta`
only to valid output coordinates.

The kernel crate uses `#![forbid(unsafe_code)]`. Compiler-only operations are
exposed through the sealed linear `Gfx942TiledGemmWave64V1` typestate in the
standalone `fe2o3-gemm-device-v1` companion crate. Keeping the new intrinsics
outside `fe2o3-device` preserves the reviewed provider-tree identities used by
existing kernels. The capability hides wave identity, two separate XOR4 LDS
tiles, publish/reuse barriers, MFMA state, the accumulator, phase epochs, and
disjoint output addressing. Its only phase sequence is:

```text
Ready -> Staged -> Published -> Consumed -> Ready
```

## Compile-time enforcement boundary

The V1 contract records one enforcement owner for each of issue #138's 15
semantic mutations. Rust typestate directly rejects the three local lifecycle
errors: missing publish, missing reuse, and reuse of an expired LDS epoch.

The sealed surface also has compile-fail escape tests for unguarded arbitrary C
stores, duplicate same-capability C/LDS writes, forged workgroup coordinates,
pre-publication LDS reads, premature staged reads, and accumulator reset. Those
seven categories still require MIR/Pliron proof for dynamic bounds, lane and
workgroup injectivity, complete distributed initialization, wait epochs, or
cross-phase refinement. A rustc error for an API escape attempt is not evidence
that the corresponding distributed property has been proved.

Unguarded A/B loads, divergent barriers, incorrect K-tail zero fill, and an
incorrect alpha/beta epilogue are intentionally verifier-only. Safe ordinary
Rust can express those five mutations, so their semantic-corpus fixtures must
continue to typecheck and reach proof-required compiler analysis.

## Current boundary

Provider authentication covers the compiled semantic surface: imported source
hashes for all six terminals and the provider-owned context type, plus the
reviewed `fe2o3_device::DisjointSlice` dependency in the compiled store
signature. It does not authenticate Cargo-manifest authorship, package
publication, or publisher identity. An alternate manifest that selects the
exact reviewed source and dependency is therefore semantically equivalent;
package provenance needs a separate signature or transparency-log authority.

This is a compile-tested source contract, not GPU execution authority. The
safe compiler operations are panic stubs under host rustc. Authenticated MIR
can reach a non-authoritative semantic Kernel IR witness, but runtime-plan
binding, frontend promotion, LLVM lowering, protected publication, and GPU
execution remain unimplemented. A proof-required build must therefore reject
before emitting an artifact. The existing exact `tiled_gemm_v1` Slice 1 source
and identities are unchanged.
