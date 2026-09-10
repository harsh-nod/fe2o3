// Expected negative: refund preflight omits the final dimension's underflow.
use vstd::prelude::*;
verus! {
pub open spec fn partial_release_v1(used: Seq<u64>, charge: Seq<u64>) -> bool {
    used.len() == 19 && charge.len() == 19
        && (forall|i: int| 0 <= i < 18 ==> charge[i] <= used[i])
}
pub proof fn mutated_partial_release_v1(used: Seq<u64>, charge: Seq<u64>)
    requires partial_release_v1(used, charge),
    ensures forall|i: int| 0 <= i < 19 ==> charge[i] <= used[i],
{}
}
