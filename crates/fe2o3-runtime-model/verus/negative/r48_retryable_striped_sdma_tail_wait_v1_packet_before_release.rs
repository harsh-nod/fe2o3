// Expected-negative R48 mutation: a complete packet body need not precede pointer release.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_publication_admitted_v1(
    packet_complete: bool,
    _packet_before_pointer_release: bool,
    pointer_release: bool,
) -> bool {
    packet_complete && pointer_release
}
pub proof fn packet_body_precedes_pointer_release_v1()
    ensures !mutated_publication_admitted_v1(true, false, true),
{}
}
