// Expected-negative R46 mutation: duplicate shard slots are admitted.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_shard_roster_is_exact_v1(left_slot: nat, right_slot: nat) -> bool {
    left_slot < 14 && right_slot < 14
}
pub proof fn mutated_duplicate_shard_is_rejected_v1(slot: nat)
    requires slot < 14,
    ensures !mutated_shard_roster_is_exact_v1(slot, slot),
{}
}
