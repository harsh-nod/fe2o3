// Expected-negative R40 mutation: a duplicate request index also stands in for a missing index.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_indices_v1() -> Seq<nat> { seq![0nat, 0nat] }
pub proof fn mutated_indices_are_exact_v1()
    ensures mutated_indices_v1()[0] == 0 && mutated_indices_v1()[1] == 1,
{}
}
