use vstd::prelude::*;
verus! {
pub struct StateV1 { pub occupied_slots: nat, pub retired_uses: nat }
pub open spec fn mutated_retire_one_v1(state: StateV1) -> StateV1 {
    StateV1 { occupied_slots: 1, retired_uses: state.retired_uses + 1 }
}
pub proof fn mutated_one_hundred_thirty_retired_uses_reuse_slots_v1()
    ensures mutated_retire_one_v1(
        StateV1 { occupied_slots: 1, retired_uses: 129 }).occupied_slots == 0,
{}
}
