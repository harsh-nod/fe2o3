use vstd::prelude::*;
verus! {
pub struct RangeV1 { pub offset: nat, pub len: nat }
pub open spec fn mutated_completion_range_matches_v1(left: RangeV1, right: RangeV1) -> bool {
    left.len == right.len
}
pub proof fn mutated_completion_range_offset_is_exact_v1()
    ensures !mutated_completion_range_matches_v1(
        RangeV1 { offset: 0, len: 64 },
        RangeV1 { offset: 1, len: 64 },
    ),
{}
}
