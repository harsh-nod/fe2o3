// Expected-negative R45 mutation: a header can overlap the final body step.
use vstd::prelude::*;
verus! {
pub open spec fn last_body_step_v1() -> nat { 47 }
pub open spec fn first_header_step_v1() -> nat { 47 }
pub proof fn mutated_all_bodies_precede_headers_v1()
    ensures last_body_step_v1() < first_header_step_v1(),
{}
}
