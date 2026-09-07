// Expected-negative R56 mutation: partial publication commits the cursor.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_partial_commits_v1() -> bool { true }
pub proof fn mutated_partial_publish_never_commits_v1()
    ensures !mutated_partial_commits_v1(),
{}
}
fn main() {}
