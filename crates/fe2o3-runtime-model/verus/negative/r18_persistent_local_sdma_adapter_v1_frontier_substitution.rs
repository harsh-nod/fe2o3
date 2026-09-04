use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)]
pub struct FrontierV1 { pub generation: nat }
pub open spec fn mutated_retire_v1(
    current: FrontierV1,
    observed: FrontierV1,
) -> Option<FrontierV1> {
    if observed.generation > 0 { None } else { Some(current) }
}
pub proof fn mutated_stale_frontier_is_rejected_atomically_v1()
    ensures mutated_retire_v1(
        FrontierV1 { generation: 1 },
        FrontierV1 { generation: 2 },
    ) == Some(FrontierV1 { generation: 1 }),
{}
}
