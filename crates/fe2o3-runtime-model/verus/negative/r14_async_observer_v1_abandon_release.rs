use vstd::prelude::*;

verus! {

pub enum SubmissionCustodyV1 {
    RuntimeRetained,
    CallerReleased,
}

pub open spec fn mutated_abandon_custody_v1() -> SubmissionCustodyV1 {
    SubmissionCustodyV1::CallerReleased
}

pub proof fn mutated_abandon_preserves_runtime_custody_v1()
    ensures mutated_abandon_custody_v1() == SubmissionCustodyV1::RuntimeRetained,
{
}

}
