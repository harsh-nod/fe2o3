// Expected-negative R41 mutation: a partial publication advances the round-robin cursor.
use vstd::prelude::*;
verus! {
pub open spec fn cursor_before_v1() -> nat { 3 }
pub open spec fn mutated_cursor_after_v1() -> nat { 1 }
pub proof fn mutated_partial_publication_preserves_cursor_v1()
    ensures mutated_cursor_after_v1() == cursor_before_v1(),
{}
}
