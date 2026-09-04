use vstd::prelude::*;

verus! {

pub open spec fn mutated_pending_observation_v1(registered: bool) -> bool { false }

pub proof fn mutated_pending_observation_preserves_waiter_v1()
    ensures mutated_pending_observation_v1(true),
{
}

}
