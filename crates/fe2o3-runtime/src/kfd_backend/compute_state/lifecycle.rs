use super::{
    ActiveSubmissionV1, RuntimeComputePipelineIdentityV1, RuntimeComputePipelinePhaseV1,
    RuntimeComputePipelineSlotV1,
};

include!("../compute_pipeline_lifecycle_body.rs");

macro_rules! lifecycle_rust_expr {
    ($body:expr) => {
        $body
    };
}

fn first_epoch(slots: &[RuntimeComputePipelineSlotV1], epoch: u64) -> Option<usize> {
    compute_pipeline_first_epoch_body!(lifecycle_rust_expr, slots, epoch)
}

pub(super) fn take_frontier(
    slots: &mut [RuntimeComputePipelineSlotV1],
    live: &mut usize,
    frontier: &mut Option<u64>,
    staged: Option<RuntimeComputePipelineIdentityV1>,
) -> Option<(RuntimeComputePipelinePhaseV1, ActiveSubmissionV1)> {
    compute_pipeline_take_frontier_body!(lifecycle_rust_expr, slots, live, frontier, staged)
}

pub(super) fn quarantine(slots: &mut [RuntimeComputePipelineSlotV1]) {
    compute_pipeline_quarantine_body!(lifecycle_rust_expr, slots)
}
