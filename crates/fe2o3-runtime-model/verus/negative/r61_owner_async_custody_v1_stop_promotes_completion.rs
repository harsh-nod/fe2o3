// Expected-negative R61 policy mutation: stop_promotes_completion.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_completed_v1(stopped: bool, completed: bool) -> bool { stopped || completed }
pub proof fn mutated_stop_preserves_completion_v1()
    ensures !mutated_completed_v1(true, false),
{}
}
