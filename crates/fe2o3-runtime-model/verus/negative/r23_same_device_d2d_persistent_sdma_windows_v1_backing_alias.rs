use vstd::prelude::*;
verus! {
pub open spec fn mutated_backings_are_distinct_v1(source: nat, destination: nat) -> bool {
    source == destination
}
pub proof fn mutated_backing_alias_is_rejected_v1()
    ensures !mutated_backings_are_distinct_v1(9, 9), {}
}
