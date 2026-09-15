# dialect-tile

`dialect-tile` is the target-neutral Pliron shell that owns bounded distributed
tile and layout materialization. Its D0 surface checks rank, lane distribution,
elements per lane, and total tile extent without naming a target address space,
instruction, or hardware resource.

The shell does not lower operations, select a compiler or hardware target,
produce artifacts, or grant proof, publication, load, tuning, or launch
authority. Its Pliron values and printed syntax are not durable fe2o3
identities.

## Explicit Rank-One Distribution

`MaterializeOp::new_rank_one` binds `DistributionOrderAttr::Blocked` or
`DistributionOrderAttr::Striped` to the existing distribution metadata. For
`L` lanes and `E` components per lane, the logical capacity is `L * E`:

| Order | Logical index of `(lane, component)` | Inverse `(lane, component)` |
| --- | --- | --- |
| Blocked | `lane * E + component` | `(index / E, index % E)` |
| Striped | `component * L + lane` | `(index % L, index / L)` |

Forward and inverse queries check rank, geometry and coordinate bounds before
arithmetic. The maps are constant-size and do not enumerate logical elements.
Operation queries reverify the complete operation, including its result type,
before returning coordinates. A missing order on a legacy `MaterializeOp::new`
remains unspecified: the order query returns `None`, and coordinate queries
reject. Wrong-typed attributes and explicit maps on higher-rank tiles reject.

These coordinates describe distribution only. They do not prove memory bounds,
initialization, collective participation, or cross-invocation ownership, and do
not confer a capability. Tile capacity is not an active-mask length; a zero
active extent does not imply a zero-capacity tile. A coordinate is not a byte
offset or a physical lane identifier.

This is a checked D4 dialect contract, not an executable tile implementation.
The production importer, consuming schedule transform, source lineage and
tile-to-SIMT lowering are not connected to it yet. No new KIR wire encoding,
public device API, simulator or launch path is introduced.

Its production registration adapter depends only on
`fe2o3-pliron-owner-core`; ownership of the full Pliron session remains in
`fe2o3-pliron`.
