# Tiled GEMM V1

This directory prepares the host contract for one conservative gfx942 GEMM:

- row-major `A[M,K]` and `B[K,N]` stored as BF16;
- row-major `C[M,N]` stored as FP32;
- `16x16x16` reduction steps;
- one wave64 workgroup per `16x16` output tile;
- exact `gfx942:xnack-` target policy;
- no transposition, bias, scaling, batching, split-K, or tails.

For a nonempty output, `M` and `N` must be multiples of 16. Positive `K`
must also be a multiple of 16. An empty output is a no-dispatch operation.
Nonempty `K=0` is a no-dispatch host fill with FP32 positive zero. Other tails
are rejected before launch geometry is produced.

The CPU oracle widens each BF16 input exactly to FP32, evaluates products and
sums as separate FP32 operations, and accumulates in increasing `k` order from
positive zero. Its bit pattern is the V1 host reference. It is not yet a claim
about undocumented MFMA evaluation order.

`src/kernel_face.rs` deliberately stops at the existing `fe2o3-device`
`DeviceMatrix` and fragment API. GPU frontend lowering, lane-to-fragment load
mapping, HSACO production, protected runtime admission, and hardware dispatch
remain pending. This crate makes no hardware, compiler-refinement, memory-
safety, or race-freedom claim.

Run the host checks independently of the root workspace:

```text
cargo test --manifest-path examples/tiled_gemm_v1/Cargo.toml
cargo clippy --manifest-path examples/tiled_gemm_v1/Cargo.toml \
  --all-targets --all-features -- -D warnings
```
