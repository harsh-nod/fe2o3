use vstd::prelude::*;

verus! {

pub open spec fn worker_system_atomic_valid_v1() -> bool { true }

pub open spec fn mutated_direct_sidecar_system_atomic_valid_v1() -> bool {
    worker_system_atomic_valid_v1()
}

pub proof fn mutated_worker_and_sidecar_scope_predicates_are_distinct_v1()
    ensures
        worker_system_atomic_valid_v1(),
        !mutated_direct_sidecar_system_atomic_valid_v1(),
{
}

}
