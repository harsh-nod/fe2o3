// Expected-negative R41 mutation: two requests share one persistent device storage identity.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_device_storage_v1(_index: nat) -> nat { 13 }
pub proof fn mutated_device_storage_is_distinct_v1()
    ensures mutated_device_storage_v1(0) != mutated_device_storage_v1(1),
{}
}
