// Expected-negative R57 mutation: one owner crosses devices.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_cross_device_admitted_v1() -> bool { true }
pub proof fn mutated_cross_device_is_rejected_v1()
    ensures !mutated_cross_device_admitted_v1(),
{}
}
fn main() {}
