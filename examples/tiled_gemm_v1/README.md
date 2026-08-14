# Tiled GEMM V1

This directory implements the host-only scaffold for one conservative gfx942
GEMM:

- row-major `A[M,K]` and `B[K,N]` stored as BF16;
- row-major `C[M,N]` stored as FP32;
- `16x16x16` reduction steps;
- one wave64 workgroup per `16x16` output tile;
- fail-closed admission of the repository's typed, canonical
  `gfx942:xnack-` target declaration;
- no transposition, bias, scaling, batching, split-K, or tails.

`ShapeV1` and `LaunchGeometryV1` have private fields; checked constructors and
read-only accessors are the only safe interface. For a nonempty output, `M` and
`N` must be multiples of 16. Positive `K` must also be a multiple of 16. An
empty output is a no-dispatch operation requiring no `A`, `B`, or `C` storage,
including when unused dimensions are `u32::MAX`. Nonempty `K=0` is a
no-dispatch host fill with FP32 positive zero. Other tails and unrepresentable
launch geometry are rejected before geometry is produced.

Planning requires an `AdmittedTargetV1` obtained from the canonical
`fe2o3_amd_target::AmdTargetId`. Generic `gfx942`, XNACK-enabled,
SRAM-ECC-qualified, and other processor declarations fail closed. This token
binds a declaration only: it does not attest installed hardware, executable
metadata, or executable bytes.

The bitwise evidence path first validates exact operand lengths and every BF16
encoding. It admits only `BF16_INPUT_PATTERN_V1`, the finite pinned generator
alphabet; NaNs, infinities, subnormals, negative zero, and every other encoding
fail closed with operand, index, and bit-pattern diagnostics. Validated values
widen exactly to FP32, products and sums are separate FP32 operations, and
accumulation visits increasing `k` from positive zero. Tests pin deterministic
generator bytes and independently calculated output bits.

`tiled_gemm_arithmetic_oracle_v1` remains available for general BF16 arithmetic
experiments, including out-of-corpus rounding, recurrence-order, cancellation,
and signed-zero cases. Its results are not finite-corpus bitwise evidence.
Neither oracle claims undocumented MFMA evaluation order or GPU equivalence.

The combined tree includes a bounded primitive frontend slice: genuine
`DeviceMatrix::from_compiler` and `DeviceMatrix::multiply_accumulate` calls in
the exact gfx942 wave64 context lower to a verified Kernel IR matrix operation,
while spoofed or wrong-target forms fail closed. Lane-to-fragment mapping, LDS
data movement, full GEMM loops, output stores, production export and HSACO
generation, protected runtime admission, hardware dispatch,
compiler-to-machine refinement, memory-safety proof, and race-freedom proof
remain pending.

The dedicated `Tiled GEMM V1 host scaffold` workflow exercises this standalone
manifest independently of the root workspace.

Run the host checks independently of the root workspace:

```text
cargo fmt --manifest-path examples/tiled_gemm_v1/Cargo.toml \
  --package fe2o3-tiled-gemm-v1 -- --check
cargo test --manifest-path examples/tiled_gemm_v1/Cargo.toml
cargo clippy --manifest-path examples/tiled_gemm_v1/Cargo.toml \
  --all-targets --all-features -- -D warnings
```
