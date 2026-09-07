// Expected-negative R42 mutation: reader admission invents target uniqueness.
use vstd::prelude::*;
verus! {
pub open spec fn caller_supplied_uniqueness_v1() -> bool { false }
pub proof fn mutated_missing_premise_is_admitted_v1()
    ensures caller_supplied_uniqueness_v1(),
{}
}
