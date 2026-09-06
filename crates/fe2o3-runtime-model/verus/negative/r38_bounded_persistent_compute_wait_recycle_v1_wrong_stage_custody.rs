// Expected-negative R38 mutation: the terminal DispatchRecycle path records
// Completed rather than the exact internally retained Recycled native stage.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum FailureStageV1 { DispatchRecycle }
#[derive(PartialEq, Eq)] pub enum RetainedNativeStageV1 { Completed, Recycled }
#[derive(PartialEq, Eq)]
pub enum CustodyV1 {
    ProcessTeardown(FailureStageV1, RetainedNativeStageV1, nat),
}

pub open spec fn mutated_dispatch_recycle_custody_v1() -> CustodyV1 {
    CustodyV1::ProcessTeardown(
        FailureStageV1::DispatchRecycle,
        RetainedNativeStageV1::Completed,
        7,
    )
}

pub proof fn mutated_dispatch_recycle_retains_recycled_v1()
    ensures mutated_dispatch_recycle_custody_v1() == CustodyV1::ProcessTeardown(
        FailureStageV1::DispatchRecycle,
        RetainedNativeStageV1::Recycled,
        7,
    ),
{}
}
