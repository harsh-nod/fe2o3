use vstd::prelude::*;

verus! {

pub open spec fn mutated_accepts_v1(stable_rank: nat) -> bool {
    stable_rank <= 4
}

pub proof fn mutated_rank_four_is_dropped_v1()
    ensures !mutated_accepts_v1(4),
{
}

}
