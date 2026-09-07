// Expected-negative R44 mutation: a noninductive mutation changes revision.
use vstd::prelude::*;
verus! {
pub open spec fn revision_before_v1() -> nat { 43 }
pub open spec fn mutated_revision_after_v1() -> nat { 44 }
pub proof fn mutated_noninductive_mutation_preserves_state_v1()
    ensures mutated_revision_after_v1() == revision_before_v1(),
{}
}
