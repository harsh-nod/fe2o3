// Expected-negative R37 mutation: completing one window eagerly republishes
// its continuation instead of leaving Ready custody for explicit flush.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum PhaseV1 { Ready, Published }
pub struct StateV1 { pub phase: PhaseV1, pub published_index: bool, pub publications: nat }

pub open spec fn mutated_continuation_v1() -> StateV1 {
    StateV1 { phase: PhaseV1::Published, published_index: true, publications: 1 }
}

pub proof fn mutated_continuation_remains_ready_and_unpublished_v1()
    ensures
        mutated_continuation_v1().phase == PhaseV1::Ready,
        !mutated_continuation_v1().published_index,
        mutated_continuation_v1().publications == 0,
{}
}
