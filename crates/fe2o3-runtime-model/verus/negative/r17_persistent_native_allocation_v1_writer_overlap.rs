use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)]
pub enum AccessV1 { Read, Write }
pub open spec fn mutated_conflict_v1(left: AccessV1, right: AccessV1) -> bool {
    left == AccessV1::Write && right == AccessV1::Write
}
pub proof fn mutated_overlapping_read_write_conflicts_v1()
    ensures mutated_conflict_v1(AccessV1::Read, AccessV1::Write),
{}
}
