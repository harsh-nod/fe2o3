// Expected-negative R57 mutation: post-dispatch ambiguity becomes restoration.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)]
pub enum PhaseV1 { Restored, Quarantined }
pub struct OutcomeV1 { pub phase: PhaseV1, pub prefix: nat, pub custody: nat }
pub open spec fn mutated_dispatch_ambiguity_v1(owner_count: nat) -> OutcomeV1 {
    OutcomeV1 { phase: PhaseV1::Restored, prefix: 2, custody: owner_count }
}
pub proof fn mutated_dispatch_ambiguity_restore_is_rejected_v1()
    ensures {
        let out = mutated_dispatch_ambiguity_v1(3);
        out.phase == PhaseV1::Quarantined && out.prefix == 2 && out.custody == 3
    },
{}
}
fn main() {}
