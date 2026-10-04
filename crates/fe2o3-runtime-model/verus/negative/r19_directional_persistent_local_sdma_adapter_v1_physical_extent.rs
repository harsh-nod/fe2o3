use vstd::prelude::*;
verus! {
pub open spec fn mutated_extent_valid_v1(logical: nat, physical: nat) -> bool {
    logical > 0 && physical <= 256 * 1024 * 1024
}
pub proof fn mutated_logical_may_not_exceed_physical_v1()
    ensures !mutated_extent_valid_v1(8193, 8192),
{}
}
