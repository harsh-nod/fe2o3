// Expected-negative R41 mutation: a coherent host allocation substitutes for device-local storage.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_device_is_local_v1() -> bool { false }
pub proof fn mutated_persistent_device_storage_is_local_v1()
    ensures mutated_device_is_local_v1(),
{}
}
