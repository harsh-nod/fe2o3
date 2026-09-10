// Omitting the member offset assigns two independent records the same owner.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_member_owner_v1(first_owner: u64, member: int) -> int {
    first_owner as int
}
pub proof fn mutated_duplicate_owner_v1(first_owner: u64, count: int, a: int, b: int)
    requires first_owner > 0, 0 < count <= 65536,
        (first_owner as int) + count <= (u64::MAX as int),
        0 <= a < count, 0 <= b < count, a != b,
    ensures mutated_member_owner_v1(first_owner, a) != mutated_member_owner_v1(first_owner, b),
{}
}
