// Expected-negative R44 mutation: a second live loan is admitted.
use vstd::prelude::*;
verus! {
pub open spec fn live_loans_v1() -> nat { 2 }
pub proof fn mutated_duplicate_live_loan_is_linear_v1()
    ensures live_loans_v1() <= 1,
{}
}
