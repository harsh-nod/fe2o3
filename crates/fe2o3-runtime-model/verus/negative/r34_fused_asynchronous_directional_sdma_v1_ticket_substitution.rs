// Expected-negative R34 mutation: a substituted returned ticket is accepted as
// a successful public asynchronous submission.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum OutcomeV1 { Published, Terminal }
pub struct StateV1 {
    pub planned_sequence: nat,
    pub returned_sequence: nat,
    pub outcome: OutcomeV1,
}

pub open spec fn mutated_ticket_finish_v1() -> StateV1 {
    StateV1 { planned_sequence: 17, returned_sequence: 18, outcome: OutcomeV1::Published }
}

pub proof fn mutated_ticket_substitution_is_terminal_v1()
    ensures mutated_ticket_finish_v1().returned_sequence
            != mutated_ticket_finish_v1().planned_sequence,
        mutated_ticket_finish_v1().outcome == OutcomeV1::Terminal,
{}
}
