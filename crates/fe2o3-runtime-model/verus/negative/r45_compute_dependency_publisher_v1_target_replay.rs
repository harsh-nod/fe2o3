// Expected-negative R45 mutation: cancelled target custody remains live.
use vstd::prelude::*;
verus! {
pub struct TargetStateV1 {
    pub cancelled: bool,
    pub live: bool,
    pub acceptance_mint_outstanding: bool,
}

// Mutation: rollback cancellation clears the mint but leaves target liveness
// available for replay.
pub open spec fn mutated_cancel_target_v1() -> TargetStateV1 {
    TargetStateV1 {
        cancelled: true,
        live: true,
        acceptance_mint_outstanding: false,
    }
}
pub proof fn mutated_cancelled_target_cannot_replay_v1()
    ensures {
        let after = mutated_cancel_target_v1();
        after.cancelled && !after.live && !after.acceptance_mint_outstanding
    },
{}
}
