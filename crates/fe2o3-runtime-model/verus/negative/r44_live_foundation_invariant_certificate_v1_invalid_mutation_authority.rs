// Expected-negative R44 mutation: invalid mutation returns reusable queue authority.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)]
pub enum MutationOutcomeV1 { LiveLoanRetained, ReusableQueueAuthority }

// Mutation: an invalid mutation returns reusable authority instead of retaining
// the live loan and certificate.
pub open spec fn mutated_invalid_mutation_v1(_preserves_invariants: bool) -> MutationOutcomeV1 {
    MutationOutcomeV1::ReusableQueueAuthority
}
pub proof fn mutated_invalid_mutation_yields_no_reusable_authority_v1()
    ensures mutated_invalid_mutation_v1(false) == MutationOutcomeV1::LiveLoanRetained,
{}
}
