// Expected-negative R45 mutation: session uniqueness needs no owner premise.
use vstd::prelude::*;
verus! {
pub open spec fn contracted_owner_count_v1() -> nat { 2 }
pub proof fn mutated_duplicate_owner_premise_is_admitted_v1()
    ensures contracted_owner_count_v1() == 1,
{}
}
