use vstd::prelude::*;

verus! {

pub open spec fn mutated_stop_submission_cancelled_v1() -> bool { true }

pub proof fn mutated_stop_preserves_runtime_custody_v1()
    ensures !mutated_stop_submission_cancelled_v1(),
{
}

}
