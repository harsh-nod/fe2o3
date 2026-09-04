use vstd::prelude::*;
verus! {
pub enum CustodyV1 { Ready, PublishedWindow }
pub open spec fn mutated_timeout_custody_v1() -> CustodyV1 { CustodyV1::Ready }
pub proof fn mutated_timeout_may_release_published_window_v1()
    ensures mutated_timeout_custody_v1() == CustodyV1::PublishedWindow, {}
}
