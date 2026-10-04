use vstd::prelude::*;
verus! {
pub struct RangeV1 { pub offset: nat, pub len: nat }
pub open spec fn mutated_same_range_v1(left: RangeV1, right: RangeV1) -> bool {
    left.len == right.len
}
pub proof fn mutated_persistent_range_offset_substitution_is_rejected_v1()
    ensures !mutated_same_range_v1(
        RangeV1 { offset: 0, len: 4096 },
        RangeV1 { offset: 4096, len: 4096 },
    ),
{}
}
