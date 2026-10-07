use vstd::prelude::*;
verus! {
pub open spec fn mutated_same_device_v1(source: nat, destination: nat) -> bool { true }
pub proof fn mutated_cross_device_pair_is_rejected_v1()
    ensures !mutated_same_device_v1(1, 2), {}
}
