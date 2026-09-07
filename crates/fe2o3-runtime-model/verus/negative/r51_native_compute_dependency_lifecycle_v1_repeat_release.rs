// Expected-negative R51 mutation: completion releases the same pair twice.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_release_count_v1() -> nat { 2 }
pub proof fn mutated_source_pair_is_released_once_v1()
    ensures mutated_release_count_v1() == 1,
{}
}
