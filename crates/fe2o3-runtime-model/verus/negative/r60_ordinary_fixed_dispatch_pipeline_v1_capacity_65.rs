// Expected-negative R60 mutation: one lane admits a sixty-fifth live epoch.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_lane_capacity_v1(live: nat) -> bool { live <= 65 }
pub proof fn mutated_sixty_fifth_epoch_is_rejected_v1()
    ensures !mutated_lane_capacity_v1(65),
{}
}
