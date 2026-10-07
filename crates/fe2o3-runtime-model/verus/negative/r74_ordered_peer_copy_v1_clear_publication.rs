// Expected-negative R74 mutation: completing a packet clears publication history.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_complete_v1(ever: bool) -> bool { false }
pub proof fn mutated_clear_publication_v1()
    ensures mutated_complete_v1(true),
{}
}
