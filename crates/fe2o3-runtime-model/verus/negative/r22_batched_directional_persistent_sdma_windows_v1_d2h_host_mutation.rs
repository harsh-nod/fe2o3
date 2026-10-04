use vstd::prelude::*;
verus! {
pub open spec fn mutated_d2h_host_mutation_through_v1() -> nat { 0 }
pub proof fn mutated_d2h_recovery_may_erase_host_mutation_v1()
    ensures mutated_d2h_host_mutation_through_v1() == 4096, {}
}
