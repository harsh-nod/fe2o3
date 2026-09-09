// Expected-negative R61 policy mutation: future_drop_releases_custody.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_retained_v1(observing: bool, retained: bool) -> bool { observing && retained }
pub proof fn mutated_abandon_preserves_custody_v1()
    ensures mutated_retained_v1(false, true),
{}
}
