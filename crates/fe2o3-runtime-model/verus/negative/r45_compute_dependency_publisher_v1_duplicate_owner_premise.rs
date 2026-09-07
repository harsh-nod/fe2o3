// Expected-negative R45 mutation: session uniqueness needs no owner premise.
use vstd::prelude::*;
verus! {
pub open spec fn exactly_one_owner_contracted_v1() -> bool { false }
pub proof fn mutated_duplicate_owner_premise_is_admitted_v1()
    ensures exactly_one_owner_contracted_v1(),
{}
}
