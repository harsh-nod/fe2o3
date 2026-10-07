use vstd::prelude::*;
verus! {
pub open spec fn mutated_distinct_allocations_v1(source: nat, destination: nat) -> bool {
    source == destination
}
pub proof fn mutated_allocation_alias_is_rejected_v1()
    ensures !mutated_distinct_allocations_v1(7, 7), {}
}
