use vstd::prelude::*;

verus! {

pub open spec fn mutated_peer_execution_device_v1(source: nat, destination: nat) -> nat {
    source
}

pub proof fn mutated_peer_copy_executes_on_destination_v1(source: nat, destination: nat)
    requires source > 0, destination > 0, source != destination,
    ensures mutated_peer_execution_device_v1(source, destination) == destination,
{
}

} // verus!
