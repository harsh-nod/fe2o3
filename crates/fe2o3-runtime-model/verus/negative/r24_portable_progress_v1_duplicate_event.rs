use vstd::prelude::*;
verus! {
// Mutation: registration increments the active count even when the event
// coordinate duplicates an existing registration.
pub open spec fn mutated_register_event_v1(active: nat, _event_duplicate: bool) -> nat {
    active + 1
}
pub proof fn mutated_duplicate_event_registration_is_atomic_v1()
    ensures mutated_register_event_v1(7, true) == 7, {}
}
