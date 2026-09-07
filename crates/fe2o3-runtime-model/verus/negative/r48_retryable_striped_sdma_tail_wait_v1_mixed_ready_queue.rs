// Expected-negative R48 mutation: any globally pending tail suppresses a ready-queue violation.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)] pub enum PhaseV1 { TimedOut, TailOrderingTerminal }
pub open spec fn mutated_classify_v1(any_tail_pending: bool, _pending_on_ready_queue: bool) -> PhaseV1 {
    if any_tail_pending { PhaseV1::TimedOut } else { PhaseV1::TailOrderingTerminal }
}
pub proof fn mixed_ready_queue_pending_prefix_is_terminal_v1()
    ensures mutated_classify_v1(true, true) == PhaseV1::TailOrderingTerminal,
{}
}
