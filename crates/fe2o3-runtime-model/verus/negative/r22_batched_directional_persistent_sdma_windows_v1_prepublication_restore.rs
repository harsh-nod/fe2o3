use vstd::prelude::*;
verus! {
pub enum CustodyV1 { Ready, PreparedWindow }
pub open spec fn mutated_prepublication_restore_v1() -> CustodyV1 {
    CustodyV1::PreparedWindow
}
pub proof fn mutated_prepublication_retry_may_retain_lease_v1()
    ensures mutated_prepublication_restore_v1() == CustodyV1::Ready, {}
}
