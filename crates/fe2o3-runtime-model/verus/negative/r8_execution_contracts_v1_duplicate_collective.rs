use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum CollectivePhaseV1 {
    Gathering,
    Ready,
}

pub open spec fn mutated_duplicate_arrival_v1(
    phase: CollectivePhaseV1,
    already_arrived: bool,
) -> CollectivePhaseV1 {
    if already_arrived {
        CollectivePhaseV1::Ready
    } else {
        phase
    }
}

pub proof fn mutated_duplicate_collective_arrival_does_not_advance_v1()
    ensures mutated_duplicate_arrival_v1(
        CollectivePhaseV1::Gathering,
        true,
    ) == CollectivePhaseV1::Gathering,
{
}

} // verus!
