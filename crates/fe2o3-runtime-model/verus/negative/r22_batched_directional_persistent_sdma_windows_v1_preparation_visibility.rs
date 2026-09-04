use vstd::prelude::*;
verus! {
pub open spec fn mutated_preparation_publications_v1(prior: nat) -> nat { prior + 1 }
pub proof fn mutated_preparation_may_publish_v1()
    ensures mutated_preparation_publications_v1(9) == 9, {}
}
