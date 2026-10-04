use vstd::prelude::*;

verus! {

pub struct Allocation {
    pub id: nat,
    pub byte_length: nat,
}

pub struct ByteRegion {
    pub allocation_id: nat,
    pub byte_offset: nat,
    pub byte_length: nat,
}

pub open spec fn region_is_in_bounds(
    allocation: Allocation,
    region: ByteRegion,
) -> bool {
    allocation.id == region.allocation_id
        && region.byte_length > 0
        && region.byte_offset + region.byte_length <= allocation.byte_length
}

/// Expected failure marker: mutated_unbounded_region_is_in_bounds.
pub proof fn mutated_unbounded_region_is_in_bounds(
    allocation: Allocation,
    byte_offset: nat,
    byte_length: nat,
)
    requires
        byte_length > 0,
    ensures
        region_is_in_bounds(
            allocation,
            ByteRegion { allocation_id: allocation.id, byte_offset, byte_length },
        ),
{
}

} // verus!
