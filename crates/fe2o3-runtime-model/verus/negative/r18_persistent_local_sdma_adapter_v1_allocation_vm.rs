use vstd::prelude::*;
verus! {
pub struct VmV1 { pub device: nat, pub id: nat }
pub open spec fn mutated_same_vm_v1(left: VmV1, right: VmV1) -> bool {
    left.device == right.device
}
pub proof fn mutated_allocation_vm_substitution_is_rejected_v1()
    ensures !mutated_same_vm_v1(
        VmV1 { device: 1, id: 2 },
        VmV1 { device: 1, id: 3 },
    ),
{}
}
