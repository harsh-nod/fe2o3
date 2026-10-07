// Expected-negative R60 mutation: an ordered successor publishes without a wait witness.
use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum WitnessV1 { NoPredecessor, WaitForPrior(nat) }

pub open spec fn mutated_ordering_authority_v1(
    predecessor: Option<nat>,
    witness: WitnessV1,
) -> bool {
    predecessor.is_some()
}

pub proof fn mutated_missing_wait_for_prior_is_rejected_v1()
    ensures !mutated_ordering_authority_v1(Some(1), WitnessV1::NoPredecessor),
{
}

}
