use vstd::prelude::*;

verus! {
pub struct KeyV1 { pub slot: nat, pub generation: nat }
pub open spec fn mutated_recycle_v1(key: KeyV1) -> KeyV1 {
    KeyV1 { slot: key.slot, generation: key.generation }
}
pub proof fn mutated_slot_reuse_advances_generation_v1(key: KeyV1)
    ensures mutated_recycle_v1(key).generation > key.generation,
{}
}
