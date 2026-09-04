use vstd::prelude::*;
verus! {
pub open spec fn mutated_retired_registration_allows_reuse_v1() -> bool { false }
pub proof fn mutated_retired_registration_frees_active_capacity_v1()
    ensures mutated_retired_registration_allows_reuse_v1(), {}
}
