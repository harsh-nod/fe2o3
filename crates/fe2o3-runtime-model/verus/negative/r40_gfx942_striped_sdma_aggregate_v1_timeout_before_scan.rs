// Expected-negative R40 mutation: an expired deadline is returned before scanning the roster
// and loses the whole submission owner.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_timeout_observations_v1() -> nat { 0 }
pub open spec fn mutated_timeout_owner_count_v1() -> nat { 0 }
pub proof fn mutated_timeout_scans_and_retains_owner_v1(request_count: nat)
    requires request_count > 0,
    ensures
        mutated_timeout_observations_v1() == request_count,
        mutated_timeout_owner_count_v1() == 1,
{}
}
