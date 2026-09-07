// Expected-negative R45 mutation: an out-of-range fault can reach success.
use vstd::prelude::*;
verus! {
pub open spec fn barrier_count_v1() -> nat { 1 }
pub open spec fn fault_index_v1() -> nat { 1 }
pub open spec fn publication_succeeded_v1() -> bool { true }
pub proof fn mutated_malformed_fault_fails_closed_v1()
    ensures fault_index_v1() >= barrier_count_v1() ==> !publication_succeeded_v1(),
{}
}
