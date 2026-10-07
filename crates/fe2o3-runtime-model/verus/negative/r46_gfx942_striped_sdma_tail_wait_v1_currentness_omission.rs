// Expected-negative R46 mutation: a round observes tails without currentness.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)] pub enum PhaseV1 { Pending, ValidationTerminal }
pub open spec fn mutated_round_v1(current: bool) -> PhaseV1 { PhaseV1::Pending }
pub proof fn mutated_currentness_omission_is_terminal_v1()
    ensures mutated_round_v1(false) == PhaseV1::ValidationTerminal,
{}
}
