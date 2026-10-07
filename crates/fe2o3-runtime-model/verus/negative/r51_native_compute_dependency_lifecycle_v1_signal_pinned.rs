// Expected-negative R51 mutation: SignalPinned substitutes completed custody.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)]
pub struct CompletedSignalCustodyV1 { pub public_custody: nat, pub signal: nat }
pub open spec fn mutated_signal_pinned_result_v1(completed: CompletedSignalCustodyV1)
    -> CompletedSignalCustodyV1
{
    CompletedSignalCustodyV1 { public_custody: completed.public_custody + 1, ..completed }
}
pub proof fn mutated_signal_pinned_recycle_is_inert_v1(completed: CompletedSignalCustodyV1)
    ensures mutated_signal_pinned_result_v1(completed) == completed,
{}
}
