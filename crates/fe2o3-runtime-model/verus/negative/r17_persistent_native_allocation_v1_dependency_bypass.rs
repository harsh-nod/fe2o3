use vstd::prelude::*;

verus! {
pub open spec fn mutated_can_publish_v1(dependencies_ready: bool) -> bool { true }
pub proof fn mutated_unready_dependency_blocks_publication_v1()
    ensures !mutated_can_publish_v1(false),
{}
}
