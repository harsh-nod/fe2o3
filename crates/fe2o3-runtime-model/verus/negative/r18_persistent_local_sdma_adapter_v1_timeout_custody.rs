use vstd::prelude::*;
verus! {
pub struct CustodyV1 { pub owns_native: bool, pub retains_ticket: bool }
pub open spec fn mutated_timeout_v1() -> CustodyV1 {
    CustodyV1 { owns_native: false, retains_ticket: false }
}
pub proof fn mutated_timeout_retains_ticket_and_native_custody_v1()
    ensures mutated_timeout_v1().owns_native && mutated_timeout_v1().retains_ticket,
{}
}
