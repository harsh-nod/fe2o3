use vstd::prelude::*;
verus! {
pub open spec fn mutated_window_packet_bound_v1() -> nat { 64 }
pub proof fn mutated_sixty_four_packets_fit_one_d2d_window_v1()
    ensures mutated_window_packet_bound_v1() <= 63, {}
}
