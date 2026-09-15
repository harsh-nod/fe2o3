# Ordered Wave16 Maximum

The complete Max batch is MOUNTED: 71 edits across 26 existing files and nine
child Rust files. Terminal46/current V5 identity172 was recorded in Bern's
matrix handoff before the shared hooks; no direct worker-message tool exists
and no acknowledgment is claimed. Current allocations do not conflict.
All 35 mounted Rust files parse and git diff --check passes. No Max typecheck,
Cargo, SSH, network or semantic tests were run. Repository edits are held for
the central core28/28b build and sweep checkpoint.

Exact source/SSA Context entry transport is mounted separately. Parent AMD27f
reports actual borrowed KIR lowering passes; the next AMD target partition-width
failure has a mounted fix awaiting central tests. Actual callback failures take
priority. Parent approved the coherent Max integration and explicitly resumed
the mount after the loan checkpoint. See STAGED_CHECKPOINT.md for test commands.

## Exact Semantics

Reuse the existing `WaveF32ReductionKindV1::Maximum`, not sum, LLVM fmax/minnum,
or a reassociated reduction. Each contiguous Wave16 partition in a full Wave64
executes stage-synchronous XOR partners at distances 1, 2, 4, 8, in that order.
For each lane and stage the left operand is that lane's previous-stage value;
the right operand is its XOR partner's previous-stage value. Select right only
when IEEE ordered `lhs < rhs` is true. Equal or unordered inputs preserve the
exact left bits, including signed zero and NaN sign/payload. Different lanes
can therefore retain different NaNs or signs of zero. No broadcast from a
chosen lane, NaN canonicalization, operand swapping or tree reordering is valid.

Existing implementations agree: AMD `lowering.rs` uses `fcmp olt` plus `select`;
simulator `execute_wave_f32_reduction` uses software IEEE comparison; the final
wave-expression renderer uses the same stage-synchronous ascending-XOR tree.
The new capability operation must reach these implementations without dropping
its checked partition, source occurrence, epoch, owner or loan custody.

## Coordinated Allocation

Bern/Lagrange/Pascal: this sidecar adds `ReduceMaxF32` to the existing semantic
partition enum with the same fields as `ReduceSumF32`, a MIR V22 gate and nested
partition subtag3 under execution tag24. This consumes no new top-level compiler
intrinsic tag. The V22 minimum and encoder gates retain Pascal's mounted scoped
index cases, and BF16 intrinsic80 stays unchanged. Shared-schema changes were
mounted with all downstream consumers in the same batch.

Pascal's mounted allocation is MIR execution26/27 and KIR execution27/28 for
WG index issuance/conversion, with legacy producer18 unchanged. The Max patch
preserves these arms and catalog entries. Bern's mounted matrix allocation uses
defined-contract tags3/4, a distinct namespace from Max terminal46 / V5 identity172.

KIR: append nested partition subtag3 under existing operation24, retaining
contract revision2 (or existing source-occurrence revision5). Tags25/26 and
all existing partition, borrowed and Math payloads remain byte-identical.
The ordered comparison and tree are fixed by the new operation, not caller
supplied flags. Signature is `[partition_reference, f32] -> f32`; physical
operands remain `[issued_partition, f32_value]`. Workgroup identity, lifetime,
convergence and exact-participation obligations match the existing sum consumer.

Importer: map only the existing authenticated
`Gfx950SubgroupReduceMaxF32Wave16` item through `SubgroupPartitionReduceMaxF32`.
Execution terminal tag46 / CombinedV5 identity172 is now mounted. Historical
schema mappings stay unchanged.
The terminal retains shared/by-value source ABI and exact width64/partition16,
brand, epoch, root and actual scalar type. No per-kernel route is introduced.

## Required Coverage

MIR V22 round-trip and pre-V22 refusal; KIR nested tag3 round-trip and old-byte
preservation; importer nominal/ABI/brand/epoch mutations; exact same-owner loan
receipt at reduction; KIR missing/foreign issuer, changed width, stale epoch,
lost lifetime/convergence/source occurrence; backend ordered-compare/select
tree with no sum/fmax; simulator NaN payloads, signaling NaNs, both signed-zero
orders, infinities, finite maxima, contiguous partitions and tree-order probes.
The actual authenticated AMD full-import callback must cover the new terminal
through source collection, replayed SSA, canonical lowering, backend and sim.
Its negative replay kills the same observed Workgroup owner both before partition
derivation and, separately, immediately before max consumption. The positive
source and the old V20 Workgroup source are not rewritten or weakened.
