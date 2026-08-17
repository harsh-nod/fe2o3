use vstd::prelude::*;

verus! {

pub open spec fn source_owner_v2(lane: nat) -> nat { lane }
pub open spec fn mutated_cpu_owner_v2(lane: nat) -> nat { (lane + 1) % 64 }

pub proof fn mutated_cpu_owner_matches_same_lane_source_v2(lane: nat)
    requires lane < 64,
    ensures source_owner_v2(lane) == mutated_cpu_owner_v2(lane),
{
}

}
