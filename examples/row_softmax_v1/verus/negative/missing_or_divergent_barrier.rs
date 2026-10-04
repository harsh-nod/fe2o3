use vstd::prelude::*;

verus! {

pub open spec fn mutated_barrier_count_v1(lane: nat) -> nat {
    if lane == 0 { 0 } else { 1 }
}

pub proof fn mutated_participants_have_uniform_barrier_count_v1()
    ensures mutated_barrier_count_v1(0) == mutated_barrier_count_v1(1),
{
}

}
