use vstd::prelude::*;
verus! {
pub open spec fn mutated_same_vm_v1(source: nat, destination: nat) -> bool { true }
pub proof fn mutated_cross_vm_pair_is_rejected_v1()
    ensures !mutated_same_vm_v1(3, 4), {}
}
