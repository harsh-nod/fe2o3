// Expected-negative R34 mutation: a confirmed publication is released even
// though the enclosing fused loan was not retaken.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum CustodyV1 { Published, TerminalPublished }
pub struct StateV1 { pub retake_succeeded: bool, pub custody: CustodyV1 }

pub open spec fn mutated_confirmed_finish_v1() -> StateV1 {
    StateV1 { retake_succeeded: false, custody: CustodyV1::Published }
}

pub proof fn mutated_omitted_retake_retains_terminal_published_v1()
    ensures !mutated_confirmed_finish_v1().retake_succeeded,
        mutated_confirmed_finish_v1().custody == CustodyV1::TerminalPublished,
{}
}
