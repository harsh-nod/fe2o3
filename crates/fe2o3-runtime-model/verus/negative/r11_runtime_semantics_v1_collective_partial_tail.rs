use vstd::prelude::*;

verus! {

pub open spec fn mutated_complete_workgroup_geometry_v1(grid: nat, group: nat) -> bool {
    grid > 0 && group > 0 && grid >= group
}

pub proof fn mutated_partial_tail_collective_geometry_is_rejected_v1()
    ensures !mutated_complete_workgroup_geometry_v1(65, 64),
{
}

} // verus!
