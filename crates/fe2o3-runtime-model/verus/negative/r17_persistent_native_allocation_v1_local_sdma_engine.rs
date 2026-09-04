use vstd::prelude::*;

verus! {
pub open spec fn mutated_local_sdma_engine_valid_v1(engine: nat) -> bool { engine <= 2 }
pub proof fn mutated_third_local_sdma_engine_is_rejected_v1()
    ensures !mutated_local_sdma_engine_valid_v1(2),
{}
}
