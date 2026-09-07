// Expected-negative R41 mutation: full publication commits while currentness is open.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_currentness_closed_v1() -> bool { false }
pub open spec fn mutated_cursor_committed_v1() -> bool { true }
pub proof fn mutated_cursor_commit_requires_close_v1()
    ensures mutated_cursor_committed_v1() ==> mutated_currentness_closed_v1(),
{}
}
