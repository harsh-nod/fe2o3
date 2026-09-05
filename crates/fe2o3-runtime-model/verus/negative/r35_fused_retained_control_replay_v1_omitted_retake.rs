// Expected-negative R35 mutation: Ready is committed after a failed retake.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum OutcomeV1 { Prepared, Terminal }
#[derive(PartialEq, Eq)] pub enum CustodyV1 { PreparedAttachment, TerminalAttached }
pub struct StateV1 { pub retake_succeeded: bool, pub outcome: OutcomeV1, pub custody: CustodyV1 }

pub open spec fn mutated_ready_finish_v1() -> StateV1 {
    StateV1 { retake_succeeded: false, outcome: OutcomeV1::Prepared,
        custody: CustodyV1::PreparedAttachment }
}

pub proof fn mutated_omitted_retake_is_terminal_attached_v1()
    ensures mutated_ready_finish_v1().custody == CustodyV1::TerminalAttached,
{}
}
