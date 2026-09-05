// Expected-negative R33 mutation: the completed record retires before a failed
// final-currentness observation.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum CustodyV1 { Completed, TerminalPublished }
pub struct StateV1 {
    pub final_current: bool,
    pub final_event: nat,
    pub retirement_event: nat,
    pub retired: bool,
    pub custody: CustodyV1,
}

pub open spec fn mutated_failed_final_currentness_v1() -> StateV1 {
    StateV1 {
        final_current: false,
        final_event: 7,
        retirement_event: 6,
        retired: true,
        custody: CustodyV1::Completed,
    }
}

pub proof fn mutated_retirement_follows_successful_final_currentness_v1()
    ensures !mutated_failed_final_currentness_v1().retired,
        mutated_failed_final_currentness_v1().custody == CustodyV1::TerminalPublished,
        mutated_failed_final_currentness_v1().final_event
            < mutated_failed_final_currentness_v1().retirement_event,
{}
}
