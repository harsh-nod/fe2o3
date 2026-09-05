// Expected-negative R35 mutation: authenticated construction failure returns
// Data rather than the exact detached Storage authority.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum CustodyV1 { TerminalStorage, TerminalData }
pub struct StateV1 { pub construction_succeeded: bool, pub custody: CustodyV1 }

pub open spec fn mutated_construction_failure_v1() -> StateV1 {
    StateV1 { construction_succeeded: false, custody: CustodyV1::TerminalData }
}

pub proof fn mutated_construction_failure_retains_storage_v1()
    ensures mutated_construction_failure_v1().custody == CustodyV1::TerminalStorage,
{}
}
