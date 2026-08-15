use vstd::prelude::*;

#[path = "../lds_tiled_kphase.rs"]
mod model;

verus! {

/// Mutation: without the reuse barrier, the next phase may start writing at
/// the same event at which the prior phase is still reading LDS.
pub open spec fn mutated_next_stage_without_reuse_v1(phase: nat) -> nat {
    model::kphase_read_event_v1(phase)
}

/// Expected failure marker: mutated_missing_reuse_epoch_protects_prior_reads_v1.
pub proof fn mutated_missing_reuse_epoch_protects_prior_reads_v1(phase: nat)
    ensures model::kphase_read_event_v1(phase)
        < mutated_next_stage_without_reuse_v1(phase),
{
}

} // verus!
