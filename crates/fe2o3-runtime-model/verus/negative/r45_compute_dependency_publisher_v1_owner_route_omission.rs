// Expected-negative R45 mutation: a missing arena still supplies a route.
use vstd::prelude::*;
verus! {
pub open spec fn matching_owner_count_v1() -> nat { 0 }
pub proof fn mutated_omitted_owner_has_exact_route_v1()
    ensures matching_owner_count_v1() == 1,
{}
}
