use vstd::prelude::*;

verus! {

pub struct SchedulerV1 {
    pub lane0_owner: Option<nat>,
    pub lane1_owner: Option<nat>,
}

pub open spec fn mutated_lease_v1(state: SchedulerV1, owner: nat) -> SchedulerV1 {
    SchedulerV1 { lane1_owner: Some(owner), ..state }
}

pub proof fn mutated_publication_preserves_unique_lane_owners_v1(
    state: SchedulerV1,
    owner: nat,
)
    requires state.lane0_owner == Some(owner),
    ensures {
        let after = mutated_lease_v1(state, owner);
        after.lane0_owner != after.lane1_owner
    },
{
}

}
