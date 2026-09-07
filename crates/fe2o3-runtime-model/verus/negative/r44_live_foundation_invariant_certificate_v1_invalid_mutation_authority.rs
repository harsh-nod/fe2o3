// Expected-negative R44 mutation: invalid mutation returns reusable queue authority.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_reusable_queue_authority_v1() -> bool { true }
pub proof fn mutated_invalid_mutation_yields_no_reusable_authority_v1()
    ensures !mutated_reusable_queue_authority_v1(),
{}
}
