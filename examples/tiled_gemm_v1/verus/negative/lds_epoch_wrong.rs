use vstd::prelude::*;

#[path = "../lds_tiled_slice1.rs"]
mod model;

verus! {

/// Expected failure: a read in the next epoch is not initialized by merely
/// having a write in the previous epoch.
pub proof fn mutated_cross_epoch_read_is_initialized_v1(write_epoch: nat)
    ensures model::write_barrier_epoch_v1(write_epoch)
        == model::read_barrier_epoch_v1(write_epoch + 1),
{
}

} // verus!
