// Expected-negative R40 mutation: observation begins while currentness remains open.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_currentness_closed_v1() -> bool { false }
pub proof fn mutated_observation_requires_closed_currentness_v1()
    ensures mutated_currentness_closed_v1(),
{}
}
