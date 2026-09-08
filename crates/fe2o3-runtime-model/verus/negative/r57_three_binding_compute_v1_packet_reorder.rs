// Expected-negative R57 mutation: dispatch is ordered before WaitForPrior.
use vstd::prelude::*;
verus! {
pub struct PacketOrderV1 { pub wait_order: nat, pub dispatch_order: nat }
pub open spec fn mutated_packet_order_v1() -> PacketOrderV1 {
    PacketOrderV1 { wait_order: 1, dispatch_order: 0 }
}
pub proof fn mutated_packet_reorder_is_rejected_v1()
    ensures {
        let order = mutated_packet_order_v1();
        order.wait_order == 0 && order.dispatch_order == 1
    },
{}
}
fn main() {}
