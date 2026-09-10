// Expected-negative R62 host control mutation: stop_loses_custody.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_stop_v1(p: nat) -> nat { if p == 0 || p == 2 || p == 3 { 5 } else { p } }
pub proof fn mutated_stop_loses_custody_v1(p: nat)
    requires p == 2 || p == 3,
    ensures mutated_stop_v1(p) == 6,
{}
}
