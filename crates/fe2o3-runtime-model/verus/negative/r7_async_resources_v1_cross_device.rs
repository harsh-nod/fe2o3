use vstd::prelude::*;

verus! {

pub proof fn mutated_peer_copy_executes_on_source_v1(source: nat, destination: nat)
    requires source > 0, destination > 0, source != destination,
    ensures source == destination,
{
}

} // verus!
