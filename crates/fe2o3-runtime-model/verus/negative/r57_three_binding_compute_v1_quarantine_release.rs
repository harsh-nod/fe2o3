// Expected-negative R57 mutation: observing quarantine releases its custody.
use vstd::prelude::*;
verus! {
pub struct QuarantineV1 { pub custody: nat, pub packet_prefix: nat }
pub open spec fn mutated_observe_again_v1(before: QuarantineV1) -> QuarantineV1 {
    QuarantineV1 { custody: 0, packet_prefix: before.packet_prefix }
}
pub proof fn mutated_quarantine_release_is_rejected_v1(before: QuarantineV1)
    requires before.custody == 3,
    ensures mutated_observe_again_v1(before).custody == before.custody,
{}
}
fn main() {}
