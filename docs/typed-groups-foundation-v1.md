# Typed groups foundation V1

This document records the first source-level foundation for cuda-oxide parity
rows 68 and 69. It is a capability and API contract, not parity evidence. The
authoritative status of both rows remains `Missing`.

## Handle set

`fe2o3-device` exposes four sealed group handles:

- `Grid<'invocation>` is derived from an authenticated `Invocation3D` and
  describes every work-item in that launch;
- `Workgroup<'invocation>` is derived from the same capability and describes
  the current workgroup;
- `SubgroupTile<N>` consumes a `WaveLane<Wave64>` and describes the static
  contiguous gfx942 wave64 tile containing that lane; and
- `ActiveLaneGroup` consumes a `WaveLane<Wave64>` plus an unsafe assertion that
  a `u64` mask is the exact gfx942 EXEC mask at the same convergent point.

All fields are private. Grid and workgroup handles borrow the invocation
capability, while subgroup and active-lane handles consume the lane capability.
None of the handles is `Copy`, `Clone`, `Send`, or `Sync`. They cannot be
constructed with `Default` or from an integer. The only raw identity inputs
remain the documented unsafe `Invocation3D` and `WaveLane` compiler boundaries.

Grid and workgroup construction returns `None` rather than truncate when size
or row-major rank does not fit in `u64`. Active-lane construction returns
`None` when the current lane is not a member of the asserted mask.

There is intentionally no `Cluster` type, constructor, synchronization marker,
or compatibility approximation. A source import of `fe2o3_device::Cluster`
fails to compile.

## Universal API

The sealed `Group` trait provides the same operations for every handle:

```rust,ignore
trait Group {
    type Synchronization: SynchronizationContract;
    fn size(&self) -> u64;
    fn thread_rank(&self) -> u64;
}
```

Grid and workgroup ranks linearize x fastest, then y, then z. A subgroup tile's
rank is the physical lane modulo `N`. An active-lane rank is the population
count of member lanes with lower physical lane IDs. `size` is the exact group
cardinality represented by the handle.

The associated synchronization type prevents a generic group API from implying
that all group kinds have an executable barrier. `SynchronizationContract`
records whether synchronization is supported and, when supported, its
execution scope, memory scope, ordering, address spaces, and uniform-convergence
requirement.

## gfx942 subgroup widths

`ValidGfx942SubgroupWidth` is sealed and implemented only for the power-of-two
divisors of a physical gfx942 wave64:

```text
1, 2, 4, 8, 16, 32, 64
```

A zero width, non-power-of-two width, width wider than 64, or a wave32 lane
fails during type checking. There is no dynamic tile-width constructor. This
matches the exact wave64 capability currently admitted for gfx942; it does not
claim a target-polymorphic subgroup implementation.

## Synchronization

`Grid`, `SubgroupTile<N>`, and `ActiveLaneGroup` use
`UnsupportedSynchronization` and expose no synchronization method. Grid-wide
barriers require authenticated cooperative-launch and occupancy evidence that
does not exist. The current Kernel IR wave subset accepts full-wave operations,
not a subgroup or partial-active-lane execution barrier. Those operations are
therefore absent rather than represented by a no-op.

`Workgroup` uses `WorkgroupSynchronization`, which fixes all of these source
semantics:

- workgroup execution scope;
- workgroup memory scope;
- acquire-release ordering;
- workgroup address-space visibility; and
- uniform workgroup convergence.

A caller must cross the unsafe `Workgroup::assume_uniform` boundary to create a
one-shot `WorkgroupConvergence` witness. The caller is responsible for proving
that every active work-item reaches the same dynamic barrier in the same order
and that none can skip or exit before it. Consuming the witness is the only
typed synchronization operation. The legacy `sync::syncthreads` entry point is
also unsafe and has the same requirements; there is no safe zero-argument
barrier.

The compiler does not yet recognize either source operation. Both paths retain
the host `unreachable!` implementation, so host execution and unsupported
compiler paths panic closed. Kernel IR and AMD lowering already have a truthful
convergence-bearing workgroup barrier representation, but this source API is
not connected to it until rustc recognition, uniformity analysis, and exact
semantic lowering can preserve every field in the contract.

## Tests in this foundation

`group_properties.rs` checks:

- row-major grid and workgroup rank bijections over broad 3D shape sets;
- every lane of every admitted gfx942 tile width;
- thousands of deterministic active-mask/rank cases;
- invalid active membership and arithmetic overflow rejection;
- exact synchronization-policy constants; and
- host barrier panic behavior.

The device UI suite checks private construction, lifetime escape, non-`Clone`,
non-`Send`, non-`Sync`, invalid const widths, wave32 misuse, unsafe mask and
convergence boundaries, raw barrier safety, unsupported group synchronization,
and absent cluster behavior.

## Remaining evidence

Rows 68 and 69 must not be promoted on this source foundation. Promotion needs
at least:

1. genuine diagnostic-item recognition and source-to-semantic-IR lowering for
   handle construction, rank/size, and the workgroup convergence witness;
2. uniformity and barrier-order analysis that rejects divergent source calls;
3. target admission binding gfx942 tile widths and active masks to exact kernel
   wave metadata;
4. Verus proofs for rank bounds/bijections, mask ranking, convergence, and LDS
   epoch transfer;
5. authenticated artifact evidence showing the required scopes, fences,
   convergent operation, and target mode survive lowering; and
6. gfx942 hardware tests with independent CPU oracles for group ranks, tiles,
   active masks, and workgroup memory communication across the barrier.
