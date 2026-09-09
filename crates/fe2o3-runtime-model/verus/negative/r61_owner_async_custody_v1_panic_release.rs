// Expected-negative R61 policy mutation: panic_release.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_release_v1(n: bool, c: bool, a: bool, s: bool) -> bool { c && a && s }
pub proof fn mutated_panic_retains_v1()
    ensures !mutated_release_v1(false, true, true, true),
{}
}
