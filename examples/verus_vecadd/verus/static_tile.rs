use vstd::prelude::*;

verus! {

/// Ghost identity of one checked region within an allocation.
pub struct RegionIdentity {
    pub allocation_id: nat,
    pub allocation_len: nat,
    pub start: nat,
    pub len: nat,
}

/// Ghost counterpart of the private runtime tile witness.
pub struct StaticTileWitness {
    pub parent: RegionIdentity,
    pub tile_start: nat,
    pub tile_len: nat,
}

pub open spec fn parent_region_is_checked(parent: RegionIdentity) -> bool {
    parent.len > 0
        && parent.start + parent.len <= parent.allocation_len
}

pub open spec fn tile_witness_is_checked(witness: StaticTileWitness) -> bool {
    parent_region_is_checked(witness.parent)
        && witness.tile_len > 0
        && witness.tile_start + witness.tile_len <= witness.parent.len
}

pub open spec fn witness_matches_parent(
    witness: StaticTileWitness,
    parent: RegionIdentity,
) -> bool {
    witness.parent == parent
}

pub open spec fn allocation_relative_index(
    witness: StaticTileWitness,
    constant_index: nat,
) -> nat {
    witness.parent.start + witness.tile_start + constant_index
}

/// A single checked tile extent discharges every in-range constant access.
pub proof fn checked_tile_constant_access_is_in_allocation(
    witness: StaticTileWitness,
    constant_index: nat,
)
    requires
        tile_witness_is_checked(witness),
        constant_index < witness.tile_len,
    ensures
        witness.parent.start
            <= allocation_relative_index(witness, constant_index),
        allocation_relative_index(witness, constant_index)
            < witness.parent.start + witness.parent.len,
        allocation_relative_index(witness, constant_index)
            < witness.parent.allocation_len,
{
    assert(witness.tile_start + constant_index
        < witness.tile_start + witness.tile_len);
}

/// A witness carrying a different allocation/region identity cannot authorize
/// accesses through the requested parent.
pub proof fn different_parent_witness_is_rejected(
    witness: StaticTileWitness,
    requested_parent: RegionIdentity,
)
    requires
        tile_witness_is_checked(witness),
        requested_parent != witness.parent,
    ensures
        !witness_matches_parent(witness, requested_parent),
{
}

} // verus!
