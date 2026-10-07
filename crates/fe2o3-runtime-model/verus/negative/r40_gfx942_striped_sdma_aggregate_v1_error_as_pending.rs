// Expected-negative R40 mutation: an observation error is classified as Pending.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)] pub enum PhaseV1 { Pending, Terminal }
pub open spec fn mutated_error_phase_v1() -> PhaseV1 { PhaseV1::Pending }
pub proof fn mutated_error_is_terminal_v1()
    ensures mutated_error_phase_v1() == PhaseV1::Terminal,
{}
}
