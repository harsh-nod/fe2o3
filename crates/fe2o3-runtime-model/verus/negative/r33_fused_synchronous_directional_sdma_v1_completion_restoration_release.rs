// Expected-negative R33 mutation: failed restoration after the lower completed
// record was removed releases custody instead of retaining CompletedUnrestored.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum CustodyV1 { Completed, CompletedUnrestored }
pub struct StateV1 {
    pub restoration_succeeded: bool,
    pub lower_record_retired: bool,
    pub custody: CustodyV1,
}

pub open spec fn mutated_failed_completion_restoration_v1() -> StateV1 {
    StateV1 {
        restoration_succeeded: false,
        lower_record_retired: true,
        custody: CustodyV1::Completed,
    }
}

pub proof fn mutated_failed_restoration_retains_completed_unrestored_v1()
    ensures mutated_failed_completion_restoration_v1().lower_record_retired,
        mutated_failed_completion_restoration_v1().custody == CustodyV1::CompletedUnrestored,
{}
}
