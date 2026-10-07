use vstd::prelude::*;

verus! {

pub enum AtomicScopeV1 {
    WorkerDevice,
    System,
}

pub open spec fn worker_atomic_scope_v1() -> AtomicScopeV1 {
    AtomicScopeV1::System
}

pub open spec fn mutated_direct_sidecar_atomic_scope_v1() -> AtomicScopeV1 {
    worker_atomic_scope_v1()
}

pub proof fn mutated_worker_and_sidecar_scope_predicates_are_distinct_v1()
    ensures
        worker_atomic_scope_v1() == AtomicScopeV1::System,
        mutated_direct_sidecar_atomic_scope_v1() != worker_atomic_scope_v1(),
{
}

}
