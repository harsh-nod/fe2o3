use vstd::prelude::*;

verus! {

pub open spec fn mutated_abandon_submission_retained_v1() -> bool { false }

pub proof fn mutated_abandon_preserves_runtime_custody_v1()
    ensures mutated_abandon_submission_retained_v1(),
{
}

}
