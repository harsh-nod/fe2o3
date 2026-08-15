use vstd::prelude::*;

verus! {

pub open spec fn mutated_contribution_v1(input: Seq<int>, lane: nat) -> int
    recommends lane < input.len(),
{
    input[lane as int]
}

pub proof fn mutated_inactive_lane_contributes_zero_v1(
    input: Seq<int>,
    active: Seq<bool>,
    lane: nat,
)
    requires
        input.len() == 64,
        active.len() == 64,
        lane < 64,
        !active[lane as int],
        input[lane as int] == 1,
    ensures mutated_contribution_v1(input, lane) == 0,
{
}

}
