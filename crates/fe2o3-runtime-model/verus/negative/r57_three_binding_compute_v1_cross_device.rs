// Expected-negative R57 mutation: admission checks A/B but omits C's device.
use vstd::prelude::*;
verus! {
pub struct OwnersV1 { pub a_device: nat, pub b_device: nat, pub c_device: nat }
pub open spec fn mutated_same_device_v1(owners: OwnersV1) -> bool {
    owners.a_device == owners.b_device
}
pub proof fn mutated_cross_device_is_rejected_v1(owners: OwnersV1)
    requires mutated_same_device_v1(owners), owners.c_device != owners.a_device,
    ensures owners.c_device == owners.a_device,
{}
}
fn main() {}
