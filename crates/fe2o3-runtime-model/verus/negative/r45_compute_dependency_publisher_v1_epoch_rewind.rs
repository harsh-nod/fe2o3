// Expected-negative R45 mutation: rollback rewinds a burned epoch.
use vstd::prelude::*;
verus! {
pub open spec fn epoch_before_mint_v1() -> nat { 8 }
pub open spec fn epoch_after_rollback_v1() -> nat { 7 }
pub proof fn mutated_rollback_preserves_burned_epoch_v1()
    ensures epoch_after_rollback_v1() >= epoch_before_mint_v1(),
{}
}
