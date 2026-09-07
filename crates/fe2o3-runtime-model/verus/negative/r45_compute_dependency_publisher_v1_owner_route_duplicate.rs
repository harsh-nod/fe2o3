// Expected-negative R45 mutation: duplicate exact arena identities are unique.
use vstd::prelude::*;
verus! {
pub open spec fn matching_owner_count_v1() -> nat { 2 }
pub proof fn mutated_duplicate_owner_has_exact_route_v1()
    ensures matching_owner_count_v1() == 1,
{}
}
