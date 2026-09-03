use vstd::prelude::*;

verus! {

pub open spec fn mutated_atomic_admitted_v1(stable: bool, execution: bool) -> bool {
    stable
}

pub proof fn mutated_atomic_execution_capability_fails_closed_v1()
    ensures !mutated_atomic_admitted_v1(true, false),
{
}

} // verus!
