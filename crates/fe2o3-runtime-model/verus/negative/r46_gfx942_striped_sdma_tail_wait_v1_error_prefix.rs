// Expected-negative R46 mutation: a final-audit Error prefix is ignored.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)] pub enum PhaseV1 { Completed, AuditTerminal }
pub open spec fn mutated_error_prefix_outcome_v1(prefix_error: bool) -> PhaseV1 { PhaseV1::Completed }
pub proof fn mutated_error_prefix_is_terminal_v1()
    ensures mutated_error_prefix_outcome_v1(true) == PhaseV1::AuditTerminal,
{}
}
