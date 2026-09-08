use vstd::prelude::*;
verus! {
pub enum SubmissionCustodyV1 { RuntimeRetained, CallerReleased }
pub open spec fn mutated_custody_before_drop_v1() -> SubmissionCustodyV1 {
    SubmissionCustodyV1::RuntimeRetained
}
pub open spec fn mutated_custody_after_drop_v1() -> SubmissionCustodyV1 {
    SubmissionCustodyV1::CallerReleased
}
pub proof fn mutated_drop_preserves_custody_v1()
    ensures mutated_custody_before_drop_v1() == mutated_custody_after_drop_v1(), {}
}
