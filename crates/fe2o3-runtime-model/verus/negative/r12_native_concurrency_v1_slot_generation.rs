use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct SlotKeyV1 {
    pub queue_occurrence: nat,
    pub slot_index: nat,
    pub generation: nat,
}

pub open spec fn mutated_slot_matches_v1(a: SlotKeyV1, b: SlotKeyV1) -> bool {
    a.queue_occurrence == b.queue_occurrence && a.slot_index == b.slot_index
}

pub proof fn mutated_slot_generation_substitution_is_rejected_v1(slot: SlotKeyV1)
    ensures !mutated_slot_matches_v1(slot, SlotKeyV1 {
        generation: slot.generation + 1,
        ..slot
    }),
{
}

}
