use vstd::prelude::*;
verus! {
pub open spec fn mutated_covered_bytes_v1() -> nat { 1023 }
pub proof fn mutated_window_packets_may_leave_gap_v1()
    ensures mutated_covered_bytes_v1() == 1024, {}
}
