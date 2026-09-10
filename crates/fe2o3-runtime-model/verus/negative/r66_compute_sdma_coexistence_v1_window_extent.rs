// Expected negative: an inclusive window end admits one slot outside the record.
use vstd::prelude::*;
verus! {
pub open spec fn inclusive_window_v1(slot: usize, anchor: usize, count: usize) -> bool {
    slot < 64 && anchor < 64 && count != 0 && count < 64
        && (slot as int + 64 - anchor as int) % 64 <= count
}
pub proof fn mutated_window_extent_v1(slot: usize, anchor: usize, count: usize)
    requires inclusive_window_v1(slot, anchor, count),
    ensures (slot as int + 64 - anchor as int) % 64 < count,
{}
}
