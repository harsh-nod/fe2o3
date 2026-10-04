use vstd::prelude::*;

verus! {

pub open spec fn mutated_exclusive_contributor_v1(output_lane: nat, contributor: nat) -> bool {
    contributor <= output_lane
}

pub proof fn mutated_exclusive_excludes_its_output_lane_v1(output_lane: nat)
    requires output_lane < 64,
    ensures !mutated_exclusive_contributor_v1(output_lane, output_lane),
{
}

}
