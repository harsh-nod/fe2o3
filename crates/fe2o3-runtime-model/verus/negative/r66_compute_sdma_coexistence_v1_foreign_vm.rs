// Expected negative: device equality substitutes for exact VM identity.
use vstd::prelude::*;
verus! {
pub open spec fn device_only_domain_v1(device: u64, expected_device: u64, vm: u64, expected_vm: u64) -> bool {
    device == expected_device
}
pub proof fn mutated_foreign_vm_v1(device: u64, expected_device: u64, vm: u64, expected_vm: u64)
    requires device_only_domain_v1(device, expected_device, vm, expected_vm),
    ensures vm == expected_vm,
{}
}
