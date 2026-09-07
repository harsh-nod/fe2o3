// Expected-negative R46 mutation: a ready tail hides a Pending prefix.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)] pub enum PhaseV1 { Completed, ContractTerminal }
pub open spec fn mutated_ready_tail_outcome_v1(prefix_pending: bool) -> PhaseV1 { PhaseV1::Completed }
pub proof fn mutated_pending_prefix_fails_closed_v1()
    ensures mutated_ready_tail_outcome_v1(true) == PhaseV1::ContractTerminal,
{}
}
