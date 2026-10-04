use vstd::prelude::*;

verus! {

pub open spec fn mutated_dependency_ready_v1(completed: bool) -> bool {
    !completed
}

pub proof fn mutated_incomplete_dependency_blocks_closed_publication_v1()
    ensures !mutated_dependency_ready_v1(false),
{
}

} // verus!
