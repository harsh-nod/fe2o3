use vstd::prelude::*;
verus! {
pub open spec fn mutated_range_fits_v1(offset: nat, bytes: nat, extent: nat) -> bool {
    offset <= extent
}
pub proof fn mutated_out_of_extent_range_is_rejected_v1()
    ensures !mutated_range_fits_v1(7, 2, 8), {}
}
