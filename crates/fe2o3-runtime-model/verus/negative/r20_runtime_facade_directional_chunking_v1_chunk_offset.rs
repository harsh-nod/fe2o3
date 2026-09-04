use vstd::prelude::*;
verus! {
pub open spec fn mutated_packet_offset_v1(completed: nat) -> nat { completed + 1 }
pub proof fn mutated_packet_offset_equals_completed_v1()
    ensures mutated_packet_offset_v1(17) == 17, {}
}
