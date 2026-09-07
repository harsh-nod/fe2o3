// Expected-negative R57 mutation: completion mutates an input generation.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_input_content_change_is_allowed_v1() -> bool { true }
pub proof fn mutated_input_content_change_is_rejected_v1()
    ensures !mutated_input_content_change_is_allowed_v1(),
{}
}
fn main() {}
