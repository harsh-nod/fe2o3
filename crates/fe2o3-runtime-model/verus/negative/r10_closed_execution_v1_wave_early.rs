use vstd::prelude::*;

verus! {

pub open spec fn mutated_partial_wave_publishes_v1(arrivals: nat) -> bool {
    arrivals < 64
}

pub proof fn mutated_incomplete_wave64_cannot_publish_v1(arrivals: nat)
    requires arrivals < 64,
    ensures !mutated_partial_wave_publishes_v1(arrivals),
{
}

} // verus!
