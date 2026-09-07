// Expected-negative R42 mutation: self-dependency rejection changes state.
use vstd::prelude::*;
verus! {
pub open spec fn state_before_v1() -> nat { 41 }
pub open spec fn mutated_state_after_self_dependency_failure_v1() -> nat { 42 }
pub proof fn mutated_self_dependency_failure_has_no_mutation_v1()
    ensures mutated_state_after_self_dependency_failure_v1() == state_before_v1(),
{}
}
