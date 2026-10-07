// Expected-negative R41 mutation: completion-terminal custody rolls back a committed cursor.
use vstd::prelude::*;
verus! {
pub open spec fn committed_cursor_v1() -> nat { 3 }
pub open spec fn mutated_terminal_cursor_v1() -> nat { 1 }
pub proof fn mutated_completion_terminal_retains_cursor_v1()
    ensures mutated_terminal_cursor_v1() == committed_cursor_v1(),
{}
}
