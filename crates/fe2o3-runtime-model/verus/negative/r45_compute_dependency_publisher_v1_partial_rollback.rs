// Expected-negative R45 mutation: failed preflight releases a reader prefix.
use vstd::prelude::*;
verus! {
pub open spec fn preflight_passed_v1() -> bool { false }
pub open spec fn released_readers_v1() -> nat { 1 }
pub proof fn mutated_failed_preflight_releases_nothing_v1()
    ensures !preflight_passed_v1() ==> released_readers_v1() == 0,
{}
}
