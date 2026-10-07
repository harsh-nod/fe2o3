// Expected-negative R48 mutation: pointer and doorbell releases need not be ordered.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_publication_admitted_v1(
    pointer_release: bool,
    doorbell_release: bool,
    _pointer_before_doorbell: bool,
) -> bool {
    pointer_release && doorbell_release
}
pub proof fn doorbell_requires_pointer_release_v1()
    ensures !mutated_publication_admitted_v1(true, true, false),
{}
}
