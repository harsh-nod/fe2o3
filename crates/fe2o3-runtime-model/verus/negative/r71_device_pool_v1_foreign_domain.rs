// Checking native generations but omitting physical-device/VM coordinates.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_domain_valid_v1(device: u64, expected_device: u64, vm: u64, expected_vm: u64, generation: u64, expected_generation: u64) -> bool {
    generation != 0 && generation == expected_generation
}
pub proof fn mutated_foreign_domain_v1(device: u64, expected_device: u64, vm: u64, expected_vm: u64, generation: u64, expected_generation: u64)
    requires generation != 0, generation == expected_generation,
        vm != 0, expected_vm != 0, device != expected_device || vm != expected_vm,
    ensures !mutated_domain_valid_v1(device, expected_device, vm, expected_vm, generation, expected_generation),
{}
}
