use vstd::prelude::*;

verus! {

pub open spec fn mutated_dependency_is_ready_v1(
    dependency_frontier: nat,
    completed_frontier: nat,
) -> bool {
    completed_frontier < dependency_frontier
}

pub proof fn mutated_incomplete_dependency_blocks_publication_v1(
    dependency_frontier: nat,
    completed_frontier: nat,
)
    requires completed_frontier < dependency_frontier,
    ensures !mutated_dependency_is_ready_v1(dependency_frontier, completed_frontier),
{
}

} // verus!
