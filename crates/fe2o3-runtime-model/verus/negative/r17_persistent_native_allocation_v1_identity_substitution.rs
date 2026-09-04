use vstd::prelude::*;

verus! {
pub open spec fn mutated_mapping_binds_allocation_v1(
    allocation_id: nat,
    mapping_allocation_id: nat,
) -> bool {
    allocation_id > 0 && mapping_allocation_id > 0
}
pub proof fn mutated_allocation_mapping_substitution_is_rejected_v1()
    ensures !mutated_mapping_binds_allocation_v1(1, 2),
{}
}
