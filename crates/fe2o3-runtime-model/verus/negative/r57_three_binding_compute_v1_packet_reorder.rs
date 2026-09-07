// Expected-negative R57 mutation: dispatch precedes WaitForPrior.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_packet_reorder_is_exact_v1() -> bool { true }
pub proof fn mutated_packet_reorder_is_rejected_v1()
    ensures !mutated_packet_reorder_is_exact_v1(),
{}
}
fn main() {}
