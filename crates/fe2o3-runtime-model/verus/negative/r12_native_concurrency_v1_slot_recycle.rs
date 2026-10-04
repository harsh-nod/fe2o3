use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct StateV1 {
    pub slot_generation: nat,
    pub live_slot_generation: nat,
    pub reserved: bool,
}

pub open spec fn mutated_cancel_v1(state: StateV1) -> StateV1 {
    StateV1 { reserved: false, ..state }
}

pub proof fn mutated_cancel_advances_live_slot_generation_v1(state: StateV1)
    requires
        state.reserved,
        state.live_slot_generation == state.slot_generation,
    ensures mutated_cancel_v1(state).live_slot_generation == state.slot_generation + 1,
{
}

}
