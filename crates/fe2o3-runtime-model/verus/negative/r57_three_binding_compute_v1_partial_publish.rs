// Expected-negative R57 mutation: publication transfers only two of three owners.
use vstd::prelude::*;
verus! {
pub struct PublishedV1 { pub supplied: nat, pub custody: nat, pub packet_prefix: nat }
pub open spec fn mutated_publish_v1(supplied: nat) -> PublishedV1 {
    PublishedV1 { supplied, custody: 2, packet_prefix: 2 }
}
pub proof fn mutated_partial_publish_is_rejected_v1()
    ensures {
        let out = mutated_publish_v1(3);
        out.custody == out.supplied && out.packet_prefix == 2
    },
{}
}
fn main() {}
