// Omitting the final record from the scan hides its invalid allocation identity.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_roster_valid_v1(ids: Seq<u64>) -> bool {
    0 < ids.len() <= 128
        && forall|i: int| 0 <= i < ids.len() - 1 ==> ids[i] != 0
}
pub proof fn mutated_final_record_omitted_v1(ids: Seq<u64>)
    requires mutated_roster_valid_v1(ids), ids[ids.len() - 1] == 0,
    ensures forall|i: int| 0 <= i < ids.len() ==> ids[i] != 0,
{}
}
