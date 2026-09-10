// Expected negative: scan acceptance covers only the prefix before the final
// retained endpoint, but claims the complete-roster nonaliasing postcondition.
use vstd::prelude::*;
verus! {
pub open spec fn omitted_endpoint_scan_v1(compute: u64, copies: Seq<u64>) -> bool {
    0 < copies.len() <= 258
        && (forall|i: int| 0 <= i < copies.len() - 1 ==> copies[i] != compute)
}
pub proof fn mutated_endpoint_omission_v1(compute: u64, copies: Seq<u64>)
    requires omitted_endpoint_scan_v1(compute, copies),
    ensures forall|i: int| 0 <= i < copies.len() ==> copies[i] != compute,
{}
}
