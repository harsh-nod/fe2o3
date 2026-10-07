// Expected-negative R34 mutation: a fallible action is inserted after the
// prepared handoff and before lower publication.
use vstd::prelude::*;

verus! {
pub struct StateV1 {
    pub handoff_event: nat,
    pub publication_event: nat,
    pub intervening_fallible_actions: nat,
}

pub open spec fn mutated_handoff_v1() -> StateV1 {
    StateV1 {
        handoff_event: 6,
        publication_event: 8,
        intervening_fallible_actions: 1,
    }
}

pub proof fn mutated_handoff_publishes_immediately_v1()
    ensures mutated_handoff_v1().publication_event == mutated_handoff_v1().handoff_event + 1,
        mutated_handoff_v1().intervening_fallible_actions == 0,
{}
}
