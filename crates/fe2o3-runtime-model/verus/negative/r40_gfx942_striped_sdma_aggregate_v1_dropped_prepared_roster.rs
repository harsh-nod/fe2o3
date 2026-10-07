// Expected-negative R40 mutation: Pending drops the prepublication validation roster.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_pending_roster_len_v1() -> nat { 0 }
pub proof fn mutated_pending_retains_prepared_roster_v1(request_count: nat)
    requires request_count > 0,
    ensures mutated_pending_roster_len_v1() == request_count,
{}
}
