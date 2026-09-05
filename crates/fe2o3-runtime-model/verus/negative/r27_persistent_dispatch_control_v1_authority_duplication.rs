use vstd::prelude::*;
verus! {
pub open spec fn mutated_queue_authorities_v1() -> nat { 1 }
pub open spec fn mutated_external_authorities_v1() -> nat { 1 }
pub proof fn mutated_replay_retains_one_authority_v1()
    ensures mutated_queue_authorities_v1() + mutated_external_authorities_v1() == 1, {}
}
