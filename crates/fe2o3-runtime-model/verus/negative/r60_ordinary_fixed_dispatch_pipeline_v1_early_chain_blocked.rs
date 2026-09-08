// Expected-negative R60 mutation: publication incorrectly waits for completion.
use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum PhaseV1 { Published, Completed, PhysicallyRetired }

pub open spec fn mutated_chain_capable_v1(phase: PhaseV1) -> bool {
    phase == PhaseV1::Completed || phase == PhaseV1::PhysicallyRetired
}

pub proof fn mutated_published_predecessor_allows_early_chain_v1()
    ensures mutated_chain_capable_v1(PhaseV1::Published),
{
}

}
