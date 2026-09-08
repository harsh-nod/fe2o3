// Expected-negative R57 mutation: the packet plan contains only dispatch.
use vstd::prelude::*;
verus! {
pub struct PacketPlanV1 { pub has_wait_for_prior: bool, pub has_dispatch: bool, pub count: nat }
pub open spec fn mutated_packet_plan_v1() -> PacketPlanV1 {
    PacketPlanV1 { has_wait_for_prior: false, has_dispatch: true, count: 1 }
}
pub proof fn mutated_wait_omission_is_rejected_v1()
    ensures {
        let plan = mutated_packet_plan_v1();
        plan.has_wait_for_prior && plan.has_dispatch && plan.count == 2
    },
{}
}
fn main() {}
