use vstd::prelude::*;
verus! {
pub open spec fn mutated_ticket_valid_v1(owner_ok: bool, slot: nat, generation: nat) -> bool {
    owner_ok && slot < 64 && generation > 0
}
pub proof fn mutated_ticket_must_bind_selected_child_v1()
    ensures !mutated_ticket_valid_v1(true, 0, 1),
{}
}
