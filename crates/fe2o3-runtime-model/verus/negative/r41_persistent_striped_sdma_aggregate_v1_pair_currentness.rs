// Expected-negative R41 mutation: the request uses a substituted directional pair generation.
use vstd::prelude::*;
verus! {
pub open spec fn admitted_pair_generation_v1() -> nat { 11 }
pub open spec fn mutated_request_pair_generation_v1() -> nat { 12 }
pub proof fn mutated_request_pair_is_current_v1()
    ensures mutated_request_pair_generation_v1() == admitted_pair_generation_v1(),
{}
}
