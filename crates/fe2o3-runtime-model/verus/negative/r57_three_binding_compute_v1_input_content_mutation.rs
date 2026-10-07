// Expected-negative R57 mutation: completion increments input A content generation.
use vstd::prelude::*;
verus! {
pub struct CompletedOwnersV1 { pub a_content_generation: nat, pub c_content_generation: nat }
pub open spec fn mutated_complete_v1(
    a_generation: nat, c_generation: nat,
) -> CompletedOwnersV1 {
    CompletedOwnersV1 {
        a_content_generation: a_generation + 1,
        c_content_generation: c_generation + 1,
    }
}
pub proof fn mutated_input_content_change_is_rejected_v1(
    a_generation: nat, c_generation: nat,
)
    ensures mutated_complete_v1(a_generation, c_generation).a_content_generation
        == a_generation,
{}
}
fn main() {}
