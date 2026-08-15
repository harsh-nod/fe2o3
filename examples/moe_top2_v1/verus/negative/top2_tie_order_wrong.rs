use vstd::prelude::*;

verus! {

pub open spec fn mutated_precedes_v1(left_score: int, left: nat, right_score: int, right: nat) -> bool {
    left_score > right_score || (left_score == right_score && left > right)
}

pub proof fn mutated_lower_expert_wins_equal_score_v1()
    ensures mutated_precedes_v1(7, 0, 7, 1),
{
}

}
