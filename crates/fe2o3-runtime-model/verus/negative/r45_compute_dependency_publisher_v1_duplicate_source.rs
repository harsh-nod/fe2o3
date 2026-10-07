// Expected-negative R45 mutation: duplicate occurrences remain distinct.
use vstd::prelude::*;
verus! {
pub open spec fn first_source_v1() -> nat { 43 }
pub open spec fn second_source_v1() -> nat { 43 }
pub proof fn mutated_duplicate_sources_are_distinct_v1()
    ensures first_source_v1() != second_source_v1(),
{}
}
