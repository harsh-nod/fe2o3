// Expected-negative R45 mutation: one live reader appears twice in an arena.
use vstd::prelude::*;
verus! {
pub open spec fn exact_live_reader_matches_v1() -> nat { 2 }
pub proof fn mutated_duplicate_arena_entry_is_unique_v1()
    ensures exact_live_reader_matches_v1() == 1,
{}
}
