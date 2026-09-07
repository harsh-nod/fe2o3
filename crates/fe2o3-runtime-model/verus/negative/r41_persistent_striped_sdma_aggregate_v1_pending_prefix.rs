// Expected-negative R41 mutation: Pending returns after observing a proper prefix.
use vstd::prelude::*;
verus! {
pub open spec fn request_count_v1() -> nat { 8 }
pub open spec fn mutated_observation_count_v1() -> nat { 3 }
pub proof fn mutated_pending_scans_whole_roster_v1()
    ensures mutated_observation_count_v1() == request_count_v1(),
{}
}
