use vstd::prelude::*;

verus! {
pub open spec fn mutated_two_device_set_is_valid_v1(first: nat, second: nat) -> bool {
    first == second
}
pub proof fn mutated_duplicate_devices_are_rejected_v1()
    ensures !mutated_two_device_set_is_valid_v1(7, 7),
{}
}
