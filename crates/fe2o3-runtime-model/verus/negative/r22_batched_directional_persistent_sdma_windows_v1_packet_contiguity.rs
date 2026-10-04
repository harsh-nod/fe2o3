use vstd::prelude::*;
verus! {
pub open spec fn mutated_next_packet_offset_v1() -> nat { 1025 }
pub proof fn mutated_adjacent_packets_may_have_a_gap_v1()
    ensures mutated_next_packet_offset_v1() == 1024, {}
}
