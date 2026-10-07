// Expected-negative R48 mutation: an out-of-range panic prefix is retained as a valid stage.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)] pub enum PhaseV1 { PanicRetained, PanicStageTerminal }
pub open spec fn mutated_invalid_panic_stage_v1(_prefix: nat, _active: nat) -> PhaseV1 {
    PhaseV1::PanicRetained
}
pub proof fn invalid_panic_stage_is_terminal_v1()
    ensures mutated_invalid_panic_stage_v1(7, 6) == PhaseV1::PanicStageTerminal,
{}
}
