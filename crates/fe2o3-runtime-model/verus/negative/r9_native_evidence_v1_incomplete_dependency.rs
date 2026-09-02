use vstd::prelude::*;

verus! {

pub open spec fn mutated_dependency_allows_dispatch_v1(
    dependency: nat,
    completed: nat,
) -> bool {
    completed < dependency
}

pub proof fn mutated_incomplete_dependency_blocks_evidence_dispatch_v1(
    dependency: nat,
    completed: nat,
)
    requires completed < dependency,
    ensures !mutated_dependency_allows_dispatch_v1(dependency, completed),
{
}

} // verus!
