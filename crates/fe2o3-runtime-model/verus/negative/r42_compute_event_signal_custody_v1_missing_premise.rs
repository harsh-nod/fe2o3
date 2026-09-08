// Expected-negative R42 mutation: reader admission invents target uniqueness.
use vstd::prelude::*;
verus! {
pub open spec fn first_reader_owner_v1() -> nat { 73 }
pub open spec fn second_reader_owner_v1() -> nat { 73 }
pub proof fn mutated_missing_premise_is_admitted_v1()
    ensures first_reader_owner_v1() != second_reader_owner_v1(),
{}
}
