use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct ObserverStateV1 {
    pub submission_retained: bool,
    pub event_retained: bool,
    pub cancelled: bool,
}

// Mutation: Stop preserves both custody coordinates but marks the submission
// cancelled instead of leaving cancellation false.
pub open spec fn mutated_stop_v1(before: ObserverStateV1) -> ObserverStateV1 {
    ObserverStateV1 {
        submission_retained: before.submission_retained,
        event_retained: before.event_retained,
        cancelled: true,
    }
}

pub proof fn mutated_stop_preserves_runtime_custody_v1()
    ensures {
        let before = ObserverStateV1 {
            submission_retained: true,
            event_retained: true,
            cancelled: false,
        };
        let after = mutated_stop_v1(before);
        after.submission_retained && after.event_retained && !after.cancelled
    },
{
}

}
