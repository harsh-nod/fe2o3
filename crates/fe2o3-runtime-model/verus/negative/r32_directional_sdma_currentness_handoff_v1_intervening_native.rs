// Expected-negative R32 mutation: one native action is inserted before publication.
use vstd::prelude::*;

verus! {
pub struct StateV1 {
    pub shared_index: nat,
    pub publication_index: nat,
    pub fallible_gap: nat,
    pub native_gap: nat,
}

pub open spec fn mutated_publish_handoff_v1() -> StateV1 {
    StateV1 { shared_index: 2, publication_index: 4, fallible_gap: 0, native_gap: 1 }
}

pub proof fn mutated_successful_handoff_publishes_immediately_v1()
    ensures mutated_publish_handoff_v1().publication_index
            == mutated_publish_handoff_v1().shared_index + 1,
        mutated_publish_handoff_v1().fallible_gap == 0,
        mutated_publish_handoff_v1().native_gap == 0,
{}

}
