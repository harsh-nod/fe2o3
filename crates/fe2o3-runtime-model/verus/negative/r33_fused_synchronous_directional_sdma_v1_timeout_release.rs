// Expected-negative R33 mutation: timeout releases the exact published ticket.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum CustodyV1 { Request, Published }
pub struct StateV1 { pub custody: CustodyV1, pub ticket_present: bool }

pub open spec fn mutated_timeout_v1() -> StateV1 {
    StateV1 { custody: CustodyV1::Request, ticket_present: false }
}

pub proof fn mutated_timeout_retains_exact_published_custody_v1()
    ensures mutated_timeout_v1().custody == CustodyV1::Published,
        mutated_timeout_v1().ticket_present,
{}
}
