use vstd::prelude::*;

verus! {

pub open spec fn mutated_resources_available_v1(
    _candidate: Set<nat>,
    _active: Set<nat>,
) -> bool {
    true
}

pub proof fn mutated_resource_overlap_blocks_publication_v1(
    candidate: Set<nat>,
    active: Set<nat>,
    resource: nat,
)
    requires
        candidate.contains(resource),
        active.contains(resource),
    ensures !mutated_resources_available_v1(candidate, active),
{
}

}
