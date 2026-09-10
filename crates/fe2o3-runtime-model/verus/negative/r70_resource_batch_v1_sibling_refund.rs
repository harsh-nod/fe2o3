// Disposing the first member must not subtract its still-owned sibling's debit.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_first_disposal_v1(used: u64, first: u64, second: u64) -> int {
    (used as int) + (first as int) + (second as int) - (second as int)
}
pub proof fn mutated_sibling_refund_v1(used: u64, first: u64, second: u64)
    requires first != second,
        (used as int) + (first as int) + (second as int) <= (u64::MAX as int),
    ensures mutated_first_disposal_v1(used, first, second) == (used as int) + (second as int),
{}
}
