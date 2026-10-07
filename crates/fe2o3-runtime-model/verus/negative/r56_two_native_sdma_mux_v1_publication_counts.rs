// Expected-negative R56 mutation: full active publication reports one doorbell.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_doorbells_v1() -> nat { 1 }
pub proof fn mutated_full_publication_count_is_exact_v1()
    ensures mutated_doorbells_v1() == 2,
{}
}
fn main() {}
