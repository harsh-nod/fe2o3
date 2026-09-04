use vstd::prelude::*;
verus! {
pub struct IdentityV1 {
    pub allocation: nat,
    pub mapping: nat,
    pub parent: nat,
    pub pair: nat,
    pub attachment: nat,
    pub pool_generation: nat,
    pub incarnation: nat,
    pub direction: nat,
}
// Mutation drops only pool_generation; every other identity stays exact.
pub open spec fn mutated_frontier_equal_v1(old: IdentityV1, next: IdentityV1) -> bool {
    old.allocation == next.allocation && old.mapping == next.mapping
        && old.parent == next.parent && old.pair == next.pair
        && old.attachment == next.attachment && old.incarnation == next.incarnation
        && old.direction == next.direction
}
pub proof fn mutated_old_pool_frontier_is_rejected_v1()
    ensures {
        let old = IdentityV1 { allocation: 1, mapping: 2, parent: 3, pair: 4,
            attachment: 5, pool_generation: 6, incarnation: 7, direction: 1 };
        let next = IdentityV1 { pool_generation: 8, ..old };
        !mutated_frontier_equal_v1(old, next)
    },
{}
}
