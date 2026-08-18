use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct CurrentnessCommitV1 {
    pub initial_vram_lost_counter: nat,
    pub reset_subscription_established: bool,
    pub reset_event_mask_enabled: bool,
    pub reset_fence_clear_before_commit: bool,
}

pub proof fn mutated_projection_drops_reset_fence_v1(counter: nat)
    ensures
        (CurrentnessCommitV1 {
            initial_vram_lost_counter: counter,
            reset_subscription_established: true,
            reset_event_mask_enabled: true,
            reset_fence_clear_before_commit: false,
        }) == (CurrentnessCommitV1 {
            initial_vram_lost_counter: counter,
            reset_subscription_established: true,
            reset_event_mask_enabled: true,
            reset_fence_clear_before_commit: true,
        }),
{
}

} // verus!
