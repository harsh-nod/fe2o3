// Expected-negative R40 mutation: a duplicate session-local queue ID passes admission.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_ids_v1() -> Seq<nat> { seq![7nat, 7nat] }
pub proof fn mutated_queue_ids_are_distinct_v1()
    ensures mutated_ids_v1()[0] != mutated_ids_v1()[1],
{}
}
