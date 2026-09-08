// Expected-negative R57 mutation: distinct allocations admit aliased storage.
use vstd::prelude::*;
verus! {
pub struct OwnersV1 {
    pub a_allocation: nat, pub b_allocation: nat, pub c_allocation: nat,
    pub a_storage: nat, pub b_storage: nat, pub c_storage: nat,
}
pub open spec fn mutated_distinct_v1(owners: OwnersV1) -> bool {
    owners.a_allocation != owners.b_allocation
        && owners.a_allocation != owners.c_allocation
        && owners.b_allocation != owners.c_allocation
}
pub proof fn mutated_storage_alias_is_rejected_v1(owners: OwnersV1)
    requires mutated_distinct_v1(owners), owners.a_storage == owners.c_storage,
    ensures owners.a_storage != owners.c_storage,
{}
}
fn main() {}
