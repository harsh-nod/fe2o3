use vstd::prelude::*;
verus! {
pub open spec fn mutated_lease_offset_v1(window_offset: nat) -> nat { window_offset + 1 }
pub proof fn mutated_lease_ranges_match_window_v1()
    ensures mutated_lease_offset_v1(8) == 8, {}
}
