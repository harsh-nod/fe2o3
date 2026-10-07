use vstd::prelude::*;
verus! {
pub open spec fn active_registration_count_after_retire_v1() -> nat { 1 }
pub open spec fn registration_capacity_v1() -> nat { 1 }
pub open spec fn count_after_mutated_reuse_v1() -> nat {
    active_registration_count_after_retire_v1() + 1
}
pub proof fn mutated_retired_registration_frees_active_capacity_v1()
    ensures count_after_mutated_reuse_v1() <= registration_capacity_v1(), {}
}
