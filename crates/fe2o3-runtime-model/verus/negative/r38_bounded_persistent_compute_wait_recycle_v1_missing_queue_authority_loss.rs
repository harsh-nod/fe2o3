// Expected-negative R38 mutation: the missing-queue terminal path drops its
// only Published native authority.
use vstd::prelude::*;

verus! {
pub struct StateV1 { pub published_authority_count: nat }

pub open spec fn mutated_missing_queue_v1() -> StateV1 {
    StateV1 { published_authority_count: 0 }
}

pub proof fn mutated_missing_queue_retains_published_authority_v1()
    ensures mutated_missing_queue_v1().published_authority_count == 1,
{}
}
