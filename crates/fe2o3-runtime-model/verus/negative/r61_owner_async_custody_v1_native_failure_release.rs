// Expected-negative R61 policy mutation: native_failure_release.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_release_v1(n: bool, c: bool, a: bool, s: bool) -> bool { n && c && a }
pub proof fn mutated_native_failure_retains_v1()
    ensures !mutated_release_v1(true, true, true, false),
{}
}
