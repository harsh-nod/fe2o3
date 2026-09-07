// Expected-negative R48 mutation: physical engine is derived from normalized slot.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)] pub enum EngineV1 { Engine0, Engine1 }
pub open spec fn mutated_engine_v1(normalized_slot: nat) -> EngineV1 {
    if normalized_slot % 2 == 0 { EngineV1::Engine0 } else { EngineV1::Engine1 }
}
pub proof fn actual_queue_three_has_engine_one_v1()
    ensures mutated_engine_v1(0) == EngineV1::Engine1,
{}
}
