// Expected-negative R61 policy mutation: release_without_native_shutdown.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_release_v1(n: bool, c: bool, a: bool, s: bool) -> bool { n && c && s }
pub proof fn mutated_release_requires_attempt_v1()
    ensures !mutated_release_v1(true, true, false, true),
{}
}
