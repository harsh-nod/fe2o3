use vstd::prelude::*;
verus! {
pub open spec fn mutated_expected_event_v1() -> nat { 24 }
pub open spec fn mutated_visited_event_v1() -> nat { 25 }
pub proof fn mutated_cyclic_visit_preserves_registration_identity_v1()
    ensures mutated_expected_event_v1() == mutated_visited_event_v1(), {}
}
