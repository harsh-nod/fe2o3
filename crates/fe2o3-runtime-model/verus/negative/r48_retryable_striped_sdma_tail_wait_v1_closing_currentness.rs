// Expected-negative R48 mutation: timeout is returned after failed closing currentness.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)] pub enum PhaseV1 { ClosingTerminal, TimedOut }
pub open spec fn mutated_close_v1(_closing_current: bool) -> PhaseV1 { PhaseV1::TimedOut }
pub proof fn failed_close_is_terminal_v1()
    ensures mutated_close_v1(false) == PhaseV1::ClosingTerminal,
{}
}
