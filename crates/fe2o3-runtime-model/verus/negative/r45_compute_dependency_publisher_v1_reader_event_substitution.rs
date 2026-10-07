// Expected-negative R45 mutation: rollback returns a substituted reader event.
use vstd::prelude::*;
verus! {
pub open spec fn arena_event_v1() -> nat { 79 }
pub open spec fn returned_event_v1() -> nat { 80 }
pub proof fn mutated_reader_event_substitution_is_exact_v1()
    ensures arena_event_v1() == returned_event_v1(),
{}
}
