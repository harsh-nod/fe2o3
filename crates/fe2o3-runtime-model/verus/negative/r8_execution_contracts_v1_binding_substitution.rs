use vstd::prelude::*;

verus! {

pub open spec fn mutated_published_destination_v1(destination: nat) -> nat {
    destination + 1
}

pub proof fn mutated_ready_publication_retains_destination_v1(destination: nat)
    ensures mutated_published_destination_v1(destination) == destination,
{
}

} // verus!
