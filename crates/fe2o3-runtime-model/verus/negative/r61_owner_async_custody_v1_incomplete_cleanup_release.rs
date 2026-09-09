// Expected-negative R61 policy mutation: incomplete_cleanup_release.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_release_v1(n: bool, c: bool, a: bool, s: bool) -> bool { n && a && s }
pub proof fn mutated_incomplete_cleanup_retains_v1()
    ensures !mutated_release_v1(true, false, true, true),
{}
}
