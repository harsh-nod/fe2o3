# gfx950 low-precision kernels

The primary source examples in this directory are four ordinary safe Rust
`#[kernel(typed)]` functions in `src/kernel.rs`:

- `gfx950_fp4_gemm_rust`
- `gfx950_fp8_gemm_rust`
- `gfx950_fp4_attention_rust`
- `gfx950_fp8_attention_rust`

The GEMMs use format- and role-typed gfx950 `16x16x128` scaled-MFMA fragments.
Each logical FP4 code occupies the low nibble of one input byte; the typed
fragment load owns the dense register packing required by the instruction.
The attention sources additionally use the move-only
`Gfx950LdsTransposeTile` transition
`Uninitialized -> Staged -> Published` before reading the K fragment, then
perform stable softmax and value accumulation through typed fe2o3 operations.
`src/reference.rs` is an independent CPU oracle with deterministic,
axis-varying inputs and exact OCP E2M1/E4M3 decoders.

Run the Rust source and oracle checks with:

```bash
cargo test --manifest-path Cargo.toml
```

The production rustc importer, Kernel IR schema, gfx950 production target
profile, and full-module AMDGPU lowering do not yet consume these new device
terminals. `GFX950_RUST_TO_HSACO_LOWERING_SUPPORTED_V1` is therefore `false`;
the repository does not claim that these Rust sources currently produce the
HSACO described below.

## HIP compiler and hardware fixture

The neighboring standalone HIP fixture contains four fixed-shape, single-wave
kernels for AMD CDNA 4 (`gfx950`):

- E2M1 FP4 `16x16x128` GEMM
- E4M3 FP8 `16x16x128` GEMM
- fused FP4 attention over 16 tokens with head dimension 128
- fused FP8 attention over 16 tokens with head dimension 128

The attention kernels keep the score tile in registers, apply softmax, and
immediately accumulate the value tile. They do not materialize the `Q*K^T`
matrix. Their K fragments pass through the gfx950 LDS transpose-read
instructions before the MFMA.

Run the compiler, ISA, and numerical checks on a gfx950 machine:

```bash
./build_and_test.sh
```

On a compiler host that does not expose a gfx950 runtime device, the codegen and
disassembly checks can still run:

```bash
./build_and_test.sh --compile-only
```

`check_isa.sh` scopes every assertion to its kernel symbol. It requires
`v_mfma_f32_16x16x128_f8f6f4` in all four kernels, FP4 format selectors
`cbsz:4 blgp:4`, `ds_read_b64_tr_b4` in FP4 attention, and
`ds_read_b64_tr_b8` in FP8 attention. A scalar-only implementation cannot pass
these checks.

The executable independently decodes the packed inputs and computes CPU GEMM
and attention references. Inputs vary across both matrix axes, including
non-uniform attention K, so an incorrect token/depth permutation changes the
softmax result. The comparison rejects NaN and infinity before applying its
error tolerance. Execution is rejected when the selected HIP device is not
`gfx950`.

## Validation evidence

On 2026-08-26, the complete `./build_and_test.sh` path passed through SSH host
alias `mi350` (remote hostname `smci350-rck-g03-b19-03`) with ROCm 7.2.1,
HIP 7.2.53211, AMD Clang 22.0.0git, and an AMD Instinct MI350X reported as
`gfx950`. The LDS staging uses the measured hardware permutations: B4 is a
`16x16` nibble transpose per 16-lane group, while B8 is two interleaved `8x8`
byte transposes per group. FP8 register packing follows the documented CDNA 4
split: lane group `g` supplies K=`g*16..g*16+15` in v0-v3 and
K=`64+g*16..64+g*16+15` in v4-v7. All four deterministic CPU-oracle
comparisons passed:

```text
PASS FP4 GEMM       max_error=0
PASS FP8 GEMM       max_error=0
PASS FP4 attention  max_error=2.38419e-07
PASS FP8 attention  max_error=2.38419e-07
```

The tested HIP source SHA-256 was
`5ecfad224a691b61a07ef4aa16e144853bd3e8f53295a0e9c60404877356609a`.
The resulting gfx950 HSACO SHA-256 for the retained final run was
`ab39293c0f251678496cb5da026b8fb6ebbb4f6c96989ad5a2962d3ad6018379`;
the host executable SHA-256 was
`5a0ca9b3a72421824c6ff4e13c8294ac0b6922cb1a4f63f3b7619abc4c1ed45d`.
Symbol-scoped disassembly reported:

| Kernel | Required gfx950 instructions |
| --- | --- |
| `gfx950_fp4_gemm` | `v_mfma_f32_16x16x128_f8f6f4` with `cbsz:4 blgp:4` |
| `gfx950_fp8_gemm` | `v_mfma_f32_16x16x128_f8f6f4` with the default FP8 format |
| `gfx950_fp4_flash_attention` | FP4 MFMA above plus two `ds_read_b64_tr_b4` |
| `gfx950_fp8_flash_attention` | FP8 MFMA above plus four `ds_read_b64_tr_b8` |

An earlier compile-only run on SSH host `mi350-2` used ROCm 6.4.1 and produced
the same required instruction families, but that host exposed only
`AMD Radeon Graphics (gfx1036)`. The executable correctly refused that device.
