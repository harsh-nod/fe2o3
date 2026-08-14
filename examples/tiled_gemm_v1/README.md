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

The CPU oracle widens each BF16 input exactly to FP32, evaluates products and
sums as separate FP32 operations, and accumulates in increasing `k` order from
positive zero. Its bit pattern is the V1 host reference. It is not yet a claim
about undocumented MFMA evaluation order. Tests pin deterministic generator
bytes and independently calculated FP32 output bits for rounding, recurrence
order, cancellation, and signed-zero behavior.

`src/kernel_face.rs` deliberately stops at the existing `fe2o3-device`
`DeviceMatrix` and fragment API. GPU frontend lowering, lane-to-fragment load
mapping, HSACO production, protected runtime admission, and hardware dispatch
remain pending. This crate makes no hardware, compiler-refinement, memory-
safety, or race-freedom claim.

The dedicated `Tiled GEMM V1 host scaffold` workflow exercises this standalone
manifest independently of the root workspace.

Run the host checks independently of the root workspace:

```text
cargo test --manifest-path examples/tiled_gemm_v1/Cargo.toml
cargo clippy --manifest-path examples/tiled_gemm_v1/Cargo.toml \
  --all-targets --all-features -- -D warnings
```
