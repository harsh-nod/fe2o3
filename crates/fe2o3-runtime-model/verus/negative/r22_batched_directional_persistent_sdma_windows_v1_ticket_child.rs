use vstd::prelude::*;
verus! {
pub open spec fn mutated_h2d_child_v1() -> nat { 3 }
pub proof fn mutated_window_ticket_may_bind_wrong_child_v1()
    ensures mutated_h2d_child_v1() == 4, {}
}
