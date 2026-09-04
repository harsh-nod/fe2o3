use vstd::prelude::*;
verus! {
pub open spec fn mutated_first_window_packets_v1() -> nat { 64 }
pub open spec fn mutated_tail_window_packets_v1() -> nat { 1 }
pub proof fn mutated_sixty_five_packets_plan_as_sixty_three_plus_two_v1()
    ensures mutated_first_window_packets_v1() == 63
        && mutated_tail_window_packets_v1() == 2, {}
}
