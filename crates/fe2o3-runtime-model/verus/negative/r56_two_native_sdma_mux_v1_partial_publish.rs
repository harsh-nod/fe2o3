// Expected-negative R56 mutation: partial publication commits the cursor.
use vstd::prelude::*;
verus! {
// Mutation: any confirmed native publication commits the cursor instead of
// requiring the complete two-native prefix.
pub open spec fn mutated_cursor_committed_v1(
    confirmed_native_count: nat,
    _required_native_count: nat,
) -> bool {
    confirmed_native_count > 0
}
pub proof fn mutated_partial_publish_never_commits_v1()
    ensures !mutated_cursor_committed_v1(1, 2),
{}
}
fn main() {}
