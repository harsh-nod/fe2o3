// Expected-negative R57 mutation: prepublication failure records one packet effect.
use vstd::prelude::*;
verus! {
pub struct RestoredV1 { pub owner_count: nat, pub packet_prefix: nat }
pub open spec fn mutated_prepublication_failure_v1(owner_count: nat) -> RestoredV1 {
    RestoredV1 { owner_count, packet_prefix: 1 }
}
pub proof fn mutated_prepublication_effect_is_rejected_v1()
    ensures {
        let out = mutated_prepublication_failure_v1(3);
        out.owner_count == 3 && out.packet_prefix == 0
    },
{}
}
fn main() {}
