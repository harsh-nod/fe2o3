use vstd::prelude::*;
verus! {
pub open spec fn mutated_doorbell_updates_v1(packet_count: nat) -> nat { packet_count }
pub proof fn mutated_d2d_window_may_ring_doorbell_per_packet_v1()
    ensures mutated_doorbell_updates_v1(3) == 1, {}
}
