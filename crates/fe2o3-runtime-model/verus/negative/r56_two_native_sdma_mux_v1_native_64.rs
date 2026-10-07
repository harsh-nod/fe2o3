// Expected-negative R56 mutation: a native queue admits 64 packets.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_native_capacity_v1(load: nat) -> bool { load <= 64 }
pub proof fn mutated_native_64_is_rejected_v1()
    ensures !mutated_native_capacity_v1(64),
{}
}
fn main() {}
