// Expected-negative R62 host control mutation: timeout_drops_custody.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_timeout_v1(retained: bool) -> bool { false }
pub proof fn mutated_timeout_drops_custody_v1(retained: bool)
    requires retained,
    ensures mutated_timeout_v1(retained) == retained,
{}
}
