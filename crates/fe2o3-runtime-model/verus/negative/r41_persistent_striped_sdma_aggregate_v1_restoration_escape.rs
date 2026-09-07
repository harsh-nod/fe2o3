// Expected-negative R41 mutation: failed persistent restoration exposes a normal prefix.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_completed_prefix_len_v1() -> nat { 2 }
pub proof fn mutated_failed_restoration_has_no_normal_output_v1()
    ensures mutated_completed_prefix_len_v1() == 0,
{}
}
