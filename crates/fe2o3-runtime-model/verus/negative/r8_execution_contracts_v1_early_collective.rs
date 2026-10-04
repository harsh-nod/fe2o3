use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum CollectivePhaseV1 {
    Gathering,
    Published,
}

pub open spec fn mutated_partial_arrival_v1(arrived: nat, members: nat) -> CollectivePhaseV1 {
    if arrived + 1 < members {
        CollectivePhaseV1::Published
    } else {
        CollectivePhaseV1::Gathering
    }
}

pub proof fn mutated_partial_collective_cannot_publish_v1(arrived: nat, members: nat)
    requires arrived + 1 < members,
    ensures mutated_partial_arrival_v1(arrived, members) == CollectivePhaseV1::Gathering,
{
}

} // verus!
