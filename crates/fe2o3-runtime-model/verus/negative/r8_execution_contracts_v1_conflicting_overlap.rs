use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct AccessV1 {
    pub resource: nat,
    pub write: bool,
}

pub open spec fn accesses_conflict_v1(left: AccessV1, right: AccessV1) -> bool {
    left.resource == right.resource && (left.write || right.write)
}

pub open spec fn mutated_overlap_admitted_v1(left: AccessV1, right: AccessV1) -> bool {
    left.resource == right.resource && left.write && right.write
}

pub proof fn mutated_conflicting_overlap_is_safe_v1(left: AccessV1, right: AccessV1)
    requires mutated_overlap_admitted_v1(left, right),
    ensures !accesses_conflict_v1(left, right),
{
}

} // verus!
