// Wrapping the next-owner interval admits owners that cannot be issued uniquely.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_owner_interval_v1(owner: u64, count: usize) -> Option<int> {
    if owner == 0 || count == 0 || count > 65536 { None }
    else {
        let end = (owner as int) + (count as int);
        Some(if end > (u64::MAX as int) { end - 18446744073709551616 } else { end })
    }
}
pub proof fn mutated_generation_wrap_v1(owner: u64, count: usize)
    requires owner > 0, 0 < count <= 65536,
        (owner as int) + (count as int) > (u64::MAX as int),
    ensures mutated_owner_interval_v1(owner, count).is_none(),
{}
}
