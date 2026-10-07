// Expected-negative R44 mutation: reclaim succeeds outside a live loan.
use vstd::prelude::*;
verus! {
pub enum LoanPhaseV1 { Idle, LiveLoan }
pub open spec fn mutated_reclaim_phase_v1() -> LoanPhaseV1 { LoanPhaseV1::Idle }
pub proof fn mutated_reclaim_out_of_phase_is_admitted_v1()
    ensures mutated_reclaim_phase_v1() == LoanPhaseV1::LiveLoan,
{}
}
