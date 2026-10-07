// Expected-negative R41 mutation: publication reports two indeterminate shards.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_indeterminate_count_v1() -> nat { 2 }
pub proof fn mutated_publication_has_at_most_one_indeterminate_v1()
    ensures mutated_indeterminate_count_v1() <= 1,
{}
}
