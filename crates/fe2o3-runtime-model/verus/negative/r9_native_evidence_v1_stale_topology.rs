use vstd::prelude::*;

verus! {

pub open spec fn mutated_topology_generation_is_current_v1(
    topology: nat,
    current: nat,
) -> bool {
    topology > 0 && current > 0
}

pub proof fn mutated_stale_topology_generation_blocks_route_v1(topology: nat, current: nat)
    requires topology > 0, current > 0, topology != current,
    ensures !mutated_topology_generation_is_current_v1(topology, current),
{
}

} // verus!
