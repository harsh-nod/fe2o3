// Expected-negative R41 mutation: two requests share one host storage identity.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_host_storage_v1(_index: nat) -> nat { 10 }
pub proof fn mutated_host_storage_is_distinct_v1()
    ensures mutated_host_storage_v1(0) != mutated_host_storage_v1(1),
{}
}
