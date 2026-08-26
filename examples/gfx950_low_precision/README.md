# gfx950 low-precision kernels

This standalone HIP example contains four fixed-shape, single-wave kernels for
AMD CDNA 4 (`gfx950`):

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
and attention references. It rejects execution when the selected HIP device is
not `gfx950`.

## Validation evidence

On 2026-08-26, `./build_and_test.sh --compile-only` passed through SSH host
alias `mi350-2` (remote hostname `splinter-ctr-bj78-04`) with ROCm 6.4.1,
HIP 6.4.43483, and AMD Clang 19.0.0git. Symbol-scoped disassembly reported:

| Kernel | Required gfx950 instructions |
| --- | --- |
| `gfx950_fp4_gemm` | `v_mfma_f32_16x16x128_f8f6f4` with `cbsz:4 blgp:4` |
| `gfx950_fp8_gemm` | `v_mfma_f32_16x16x128_f8f6f4` with the default FP8 format |
| `gfx950_fp4_flash_attention` | FP4 MFMA above plus two `ds_read_b64_tr_b4` |
| `gfx950_fp8_flash_attention` | FP8 MFMA above plus four `ds_read_b64_tr_b8` |

The remote runtime exposed only `AMD Radeon Graphics (gfx1036)`, despite the
host alias. The executable correctly refused that device. Therefore the CPU
reference harness and GPU kernels were compiled and the gfx950 artifacts were
validated, but numerical GPU execution was not possible in that environment.
