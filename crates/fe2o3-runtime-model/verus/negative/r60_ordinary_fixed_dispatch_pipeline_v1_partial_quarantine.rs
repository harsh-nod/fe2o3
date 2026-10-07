// Expected-negative R60 mutation: lane quarantine omits one live epoch.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_quarantined_v1(index: nat) -> bool { index == 0 }
pub proof fn mutated_lane_quarantine_is_complete_v1()
    ensures mutated_quarantined_v1(1),
{}
}
