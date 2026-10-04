use vstd::prelude::*;

verus! {

pub struct ByteRegion {
    pub allocation_id: nat,
    pub byte_offset: nat,
    pub byte_length: nat,
}

pub open spec fn regions_overlap(left: ByteRegion, right: ByteRegion) -> bool {
    left.allocation_id == right.allocation_id
        && left.byte_offset < right.byte_offset + right.byte_length
        && right.byte_offset < left.byte_offset + left.byte_length
}

/// Mutation: every thread receives exclusive ownership of output byte zero.
pub open spec fn mutated_output_region(allocation_id: nat, element_size: nat) -> ByteRegion {
    ByteRegion { allocation_id, byte_offset: 0, byte_length: element_size }
}

/// Expected failure marker: mutated_overlapping_output_writes_are_disjoint.
pub proof fn mutated_overlapping_output_writes_are_disjoint(
    allocation_id: nat,
    left: nat,
    right: nat,
    element_size: nat,
)
    requires
        left != right,
        element_size > 0,
    ensures
        !regions_overlap(
            mutated_output_region(allocation_id, element_size),
            mutated_output_region(allocation_id, element_size),
        ),
{
}

} // verus!
