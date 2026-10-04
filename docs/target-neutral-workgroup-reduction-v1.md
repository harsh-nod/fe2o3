# Target-neutral workgroup reduction V1

`fe2o3_device::WorkgroupCollectives` is compiler-issued workgroup authority.
Ordinary kernel source obtains it with `WorkgroupCollectives::current()` and
calls `WorkgroupCollectives::reduce_sum_portable` with the compiler-issued
`DynamicLds` value. Neither terminal selects a GPU family.
The production importer authenticates their diagnostic identities, exact
provider definition paths, complete `fe2o3-device` source closure, Rust ABI,
semantic types, and source provenance before producing semantic MIR.

The closed V1 operation set is scalar sum over `u32`, `i32`, and `f32`.
Integer addition wraps at 32 bits. Floating-point addition follows the
authenticated target's strict scalar-add policy. The source launch contract
must require exactly `[N, 1, 1]`, where `N` is a power of two in `1..=256`.
The reduction consumes compiler-owned dynamic LDS with exactly one matching
scalar slot per work-item. The importer authenticates the exact
`DynamicLds<T, LdsUninitialized>` ADTs and the scalar type.

The semantic operation is target-neutral. Its semantic MIR record uses the
closed V9 schema. KIR lowering expands it into generic operations already
representable by canonical KIR V8; the independent semantic MIR and KIR
version numbers therefore need not match. Lowering publishes every work-item's
value to its own LDS slot, executes one uniform acquire-release workgroup
barrier, and performs a deterministic binary reduction tree. A
uniform acquire-release barrier separates every read and write phase. One
final barrier protects scratch reuse, and every work-item receives the result.

Only after neutral semantic MIR and canonical KIR admission does production
target binding add the exact gfx942 or gfx950 Wave64 profile. Both LLVM
backends preserve the workgroup fences and physical `s_barrier`. A different
target, nonuniform barrier placement, non-1D or non-power-of-two geometry,
oversized workgroup, wrong scalar, changed scratch element, stale provider
identity, or target substitution fails closed at its owning typed boundary.
Production ranked projection emits the complete generated LDS access and
barrier-phase schedule. For `N` work-items it records `3 * log2(N) + 2`
allocation-level memory effects and `2 * log2(N) + 2` barriers in exact order.
Each generated record carries its semantic-terminator origin, stable effect
ordinal, ranked location, and domain-separated recipe identity. It does not
claim a direct source span for compiler-generated accesses or barriers. The
translation validator independently replays the KIR rank, pair and guarded
pair indices, pointer bases, scalar operations, access contracts, barrier
contracts, producer dominance, and input/output SSA custody before target
binding.

The compiler path creates KIR, LLVM, and compiler evidence only. It grants no
artifact publication, device attachment, collection, loading, launch, debug,
profiling, proof, or hardware authority. Those remain separate production
admission boundaries.

The source example is
[`examples/workgroup_sync_v1/src/kernel.rs`](../examples/workgroup_sync_v1/src/kernel.rs).
Its `lds_publish_read_reduce_i32_v1` kernel uses an exact 64-element LDS
allocation and the neutral terminal without a target-specific intrinsic.
