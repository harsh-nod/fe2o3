// Expected-negative R57 mutation: A and C alias storage.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_storage_alias_admitted_v1() -> bool { true }
pub proof fn mutated_storage_alias_is_rejected_v1()
    ensures !mutated_storage_alias_admitted_v1(),
{}
}
fn main() {}
