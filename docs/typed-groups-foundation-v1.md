# Typed groups foundation V1

This document records the corrected first source-level foundation for
cuda-oxide parity rows 68 and 69. It is an arithmetic snapshot and fail-closed
API contract, not parity evidence. Both rows remain `Missing`.

## Snapshot model

`fe2o3-device` exposes four sealed group snapshot types:

- `Grid<'invocation>` computes launch-grid size and global row-major rank from
  a borrowed `Invocation3D` snapshot;
- `Workgroup<'invocation>` computes workgroup size and local row-major rank
  from the same snapshot;
- `SubgroupTile<'wave, N>` computes contiguous tile rank and index from a
  borrowed `WaveLane<Wave64>` snapshot; and
- `ActiveLaneGroup<'wave>` computes cardinality and rank from a borrowed
  wave64 lane plus an unsafe caller assertion about one active-lane mask.

These values are not branded execution identities. They do not authenticate a
launch, target, current hardware state, wave mode, control-flow epoch, EXEC
value, or compiled artifact. The lifetimes only prevent a derived group from
outliving the invocation or lane snapshot it borrows. In particular,
`ActiveLaneGroup` is not persistent EXEC authority and cannot authorize a
collective or barrier after control flow changes.

All fields are private. The raw `Invocation3D`, `WaveLane`, and active-mask
snapshot constructors are unsafe caller-asserted boundaries. None of these
types or the derived group values is `Copy`, `Clone`, `Send`, or `Sync`.
Grid/workgroup construction returns `None` rather than truncate when a size or
rank does not fit in `u64`; active-lane construction returns `None` when the
asserted mask excludes the lane snapshot.

There is no `Cluster` type, constructor, synchronization marker, or
compatibility approximation. Importing `fe2o3_device::Cluster` fails.

## Universal arithmetic API

The sealed `Group` trait provides the same arithmetic operations for each
snapshot:

```rust,ignore
trait Group {
    type Synchronization: SynchronizationContract;
    fn size(&self) -> u64;
    fn thread_rank(&self) -> u64;
}
```

Grid and workgroup ranks enumerate x fastest, then y, then z. A static tile's
rank is its lane offset in the tile. An active-lane rank counts asserted member
bits with lower lane IDs. `SynchronizationContract` is descriptive metadata;
it grants no convergence, memory, execution, target, or compiler authority.

`Wave64TileWidth<N>: ValidWave64TileWidth` admits only the power-of-two
divisors of 64:

```text
1, 2, 4, 8, 16, 32, 64
```

This is a compile-time wave64 arithmetic restriction, not gfx942 target
admission. Zero, non-power-of-two, and greater-than-64 widths fail during type
checking. A `WaveLane<Wave32>` cannot form either wave64-derived group.

## Synchronization contract

`Grid`, `SubgroupTile`, and `ActiveLaneGroup` use
`UnsupportedSynchronization` and expose no synchronization operation. Grid
and cluster barriers need launch/occupancy authority that this branch does not
have; subgroup and active-lane barriers have no established source lowering.

`WorkgroupSynchronization` describes the intended CUDA `__syncthreads`
compatibility contract:

- workgroup execution scope;
- workgroup memory scope;
- acquire-release ordering;
- global and workgroup address-space visibility; and
- workgroup-uniform dynamic convergence.

Both `Workgroup::synchronize` and `sync::syncthreads` are unsafe. The caller
must establish that the snapshot still describes the current invocation, that
every participating work-item executes the same dynamic barrier exactly once
and in the same sequence, and that none conditionally skips or exits before
it. A movable Rust token is not proof of these CFG properties, so this
foundation exposes no convergence witness and no safe barrier operation.
Synchronization must remain unsafe until authenticated compiler CFG analysis
can prove the exact dynamic call and preserve that proof through lowering.

The host and unsupported compiler implementations retain `unreachable!` and
therefore panic closed. Kernel IR can represent a convergence-bearing
`WorkgroupBarrier` with an address-space set, ordering, and memory scope, and
the established AMD lowering has workgroup barrier/fence machinery. This
branch deliberately does not connect the source operation to that IR: the
frontend does not recognize these constructors or establish authenticated CFG
convergence, so emitting such an IR claim would be untruthful.

## Breaking correction

The rejected branch API is intentionally not source-compatible:

- `WorkgroupConvergence` and `Workgroup::assume_uniform` were removed;
- synchronization is now `unsafe { workgroup.synchronize() }`;
- `Grid::from_invocation` and `Workgroup::from_invocation` became
  `from_invocation_snapshot`;
- `Gfx942SubgroupWidth` and `ValidGfx942SubgroupWidth` became
  `Wave64TileWidth` and `ValidWave64TileWidth`;
- subgroup and active-lane groups now borrow their lane snapshot and use
  `from_wave64_snapshot` and unsafe `from_caller_asserted_snapshot`; and
- the draft LDS-only `syncthreads` description was corrected to global plus
  workgroup visibility to match the intended CUDA compatibility contract.

## Test boundary

The numeric tests use independent fixed tables and enumeration oracles for
workgroup/grid rank, every admitted wave64 tile width, and active-mask rank.
They also cover overflow rejection, invalid active membership, exact policy
constants, and host panic behavior. They are CPU arithmetic tests only.

UI tests cover private construction, snapshot lifetime escape, non-`Clone`,
non-`Send`, non-`Sync`, invalid widths, wave32 misuse, unsafe active-mask and
barrier calls, a conditionally executed barrier call, unsupported group
synchronization, and absent `Cluster` behavior. The conditional case documents
compiler behavior: a branch-local call still requires `unsafe`; branch
structure is not interpreted as convergence proof.

These tests provide no source compiler lowering, codegen, Verus, artifact, or
gfx942 hardware evidence.

## Remaining evidence

Rows 68 and 69 must not be promoted without all of the following:

1. diagnostic-item recognition and source-to-semantic-IR lowering for snapshot
   construction and arithmetic;
2. authenticated CFG uniformity/barrier-order analysis for synchronization;
3. target and launch binding for exact wave mode and active-lane observations;
4. Verus proofs for rank bounds/bijections, mask ranking, and synchronization
   obligations;
5. artifact evidence showing scopes, fences, convergence, address spaces, and
   wave mode survive codegen; and
6. gfx942 hardware tests with independent host oracles, including global and
   workgroup memory communication across the barrier.
