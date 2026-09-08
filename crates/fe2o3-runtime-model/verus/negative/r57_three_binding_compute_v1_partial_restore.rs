// Expected-negative R57 mutation: prepublication failure restores only two owners.
use vstd::prelude::*;
verus! {
pub struct RestoredV1 { pub supplied: nat, pub restored: nat, pub packet_prefix: nat }
pub open spec fn mutated_restore_v1(supplied: nat) -> RestoredV1 {
    RestoredV1 { supplied, restored: 2, packet_prefix: 0 }
}
pub proof fn mutated_partial_restore_is_rejected_v1()
    ensures {
        let out = mutated_restore_v1(3);
        out.restored == out.supplied && out.packet_prefix == 0
    },
{}
}
fn main() {}
