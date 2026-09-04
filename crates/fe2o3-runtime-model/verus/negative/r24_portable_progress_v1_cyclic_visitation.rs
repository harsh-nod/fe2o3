use vstd::prelude::*;
verus! {
pub open spec fn mutated_cyclic_slot_v1() -> nat { 3 }
pub proof fn mutated_cyclic_visitation_stays_in_roster_v1()
    ensures mutated_cyclic_slot_v1() < 3, {}
}
