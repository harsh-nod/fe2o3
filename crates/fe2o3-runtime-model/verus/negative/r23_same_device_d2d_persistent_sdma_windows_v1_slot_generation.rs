use vstd::prelude::*;
verus! {
pub open spec fn mutated_ticket_generation_v1(
    _slot_generation: nat, global_generation: nat) -> nat
{
    global_generation + 1
}
pub proof fn mutated_d2d_ticket_generation_may_ignore_slot_counter_v1()
    ensures mutated_ticket_generation_v1(1, 7) == 2, {}
}
