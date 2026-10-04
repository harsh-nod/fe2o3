use vstd::prelude::*;

verus! {
pub open spec fn mutated_compute_binding_valid_v1(
    device: nat,
    queue_device: nat,
    owner_vm: nat,
    queue_vm: nat,
) -> bool {
    device == queue_device && owner_vm > 0 && queue_vm > 0
}
pub proof fn mutated_compute_queue_substitution_is_rejected_v1()
    ensures !mutated_compute_binding_valid_v1(1, 1, 7, 8),
{}
}
