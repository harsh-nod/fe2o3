use vstd::prelude::*;
verus! {
pub struct HostV1 { pub allocation: nat, pub generation: nat }
pub open spec fn mutated_same_host_v1(left: HostV1, right: HostV1) -> bool {
    left.allocation == right.allocation
}
pub proof fn mutated_host_generation_substitution_is_rejected_v1()
    ensures !mutated_same_host_v1(
        HostV1 { allocation: 8, generation: 1 },
        HostV1 { allocation: 8, generation: 2 },
    ),
{}
}
