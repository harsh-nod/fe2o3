use vstd::prelude::*;
verus! {
pub open spec fn mutated_write_pointer_updates_v1(prior: nat) -> nat { prior + 63 }
pub proof fn mutated_window_may_update_pointer_per_packet_v1()
    ensures mutated_write_pointer_updates_v1(4) == 5, {}
}
