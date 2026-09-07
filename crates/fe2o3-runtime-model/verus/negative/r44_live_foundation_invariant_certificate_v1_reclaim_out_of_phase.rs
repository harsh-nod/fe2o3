// Expected-negative R44 mutation: reclaim succeeds outside a live loan.
use vstd::prelude::*;
verus! {
pub open spec fn phase_is_live_loan_v1() -> bool { false }
pub proof fn mutated_reclaim_out_of_phase_is_admitted_v1()
    ensures phase_is_live_loan_v1(),
{}
}
