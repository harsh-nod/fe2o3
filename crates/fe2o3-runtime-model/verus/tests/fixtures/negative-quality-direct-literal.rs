use vstd::prelude::*;

verus! {
pub open spec /* comment-separated tokens must not bypass the checker */
fn mutated_named_only_v1() -> bool { true }
pub proof fn named_only_contradiction_v1()
    ensures /* comment-separated negation must also be rejected */ !mutated_named_only_v1(),
{}
}
