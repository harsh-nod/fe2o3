// Expected-negative R48 mutation: classification ignores a tail that becomes ready in the audit.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)] pub enum PhaseV1 { TimedOut, TailOrderingTerminal }
pub open spec fn mutated_classify_v1(
    pending_on_queue: bool,
    initially_ready: bool,
    _finally_ready: bool,
) -> PhaseV1 {
    if pending_on_queue && initially_ready {
        PhaseV1::TailOrderingTerminal
    } else {
        PhaseV1::TimedOut
    }
}
pub proof fn final_ready_tail_participates_in_ordering_v1()
    ensures mutated_classify_v1(true, false, true) == PhaseV1::TailOrderingTerminal,
{}
}
