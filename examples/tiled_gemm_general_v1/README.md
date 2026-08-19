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
semantic mutations. Ten companion UI fixtures exercise local Rust typestate and
sealed-surface restrictions. Those rustc failures remain distinct from a
distributed safety proof.

The separate mutation-oracle corpus keeps the same `#![forbid(unsafe_code)]`
root and derives every negative kernel by one reversible source edit of a full
dynamic baseline. Authenticated optimized-MIR analysis rejects all 15 exact
mutations at compiler preflight with their frozen property, stage, and
`0x464701xx` diagnostic. Each diagnostic retains the kernel root, source and
terminal spans, and reachable call chain. The managed build leaves no current
or stale artifact output. This is source-to-diagnostic evidence for the exact
corpus, not proof authority for the positive kernel or arbitrary Rust source.

## Current boundary

Provider authentication covers the compiled semantic surface: imported source
hashes for all six terminals and the provider-owned context type, plus the
reviewed `fe2o3_device::DisjointSlice` dependency in the compiled store
signature. It does not authenticate Cargo-manifest authorship, package
publication, or publisher identity. An alternate manifest that selects the
exact reviewed source and dependency is therefore semantically equivalent;
package provenance needs a separate signature or transparency-log authority.

This is a compile-tested source contract, not GPU execution authority. The
safe compiler operations are panic stubs under host rustc. The positive source
can produce an opaque, non-Clone frontend correspondence whose 11 typed
property receipts revalidate the exact optimized-MIR, provider, source, ABI,
and symbolic plan/KIR binding.

The compiler also contains structural Pliron/GPU lowering, two separately
identified reference and vectorized machine schedules, measured Worker/finalizer
observations, and a private owner-retaining pair join. That join has no
production caller. Verus proof execution remains fail-closed until the exact
root-owned runtime closure is provisioned, so no proof, artifact publication,
load, launch, or protected general-GEMM hardware authority can be issued. The
existing exact `tiled_gemm_v1` Slice 1 source and identities are unchanged.
