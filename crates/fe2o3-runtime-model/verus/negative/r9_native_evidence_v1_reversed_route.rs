use vstd::prelude::*;

verus! {

pub open spec fn mutated_reversed_route_is_current_v1(
    source: nat,
    destination: nat,
    current_source: nat,
    current_destination: nat,
) -> bool {
    current_source == destination && current_destination == source
}

pub proof fn mutated_reversed_xgmi_direction_is_rejected_v1(source: nat, destination: nat)
    requires source != destination,
    ensures !mutated_reversed_route_is_current_v1(
        source,
        destination,
        destination,
        source,
    ),
{
}

} // verus!
