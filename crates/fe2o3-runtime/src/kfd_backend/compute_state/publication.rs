use super::{
    ActiveSubmissionV1, RuntimeComputePipelineEntryV1, RuntimeComputePipelineIdentityV1,
    RuntimeComputePipelinePhaseV1, RuntimeComputePipelineSlotV1,
};

include!("../compute_pipeline_publication_body.rs");

macro_rules! pipeline_rust_expr {
    ($body:expr) => {
        $body
    };
}

fn vacant_generation(slot: &RuntimeComputePipelineSlotV1) -> Option<u64> {
    compute_pipeline_vacant_generation_body!(pipeline_rust_expr, slot)
}

fn first_vacant(slots: &[RuntimeComputePipelineSlotV1]) -> Option<(usize, u64)> {
    compute_pipeline_first_vacant_body!(pipeline_rust_expr, slots)
}

pub(super) fn has_capacity(
    slots: &[RuntimeComputePipelineSlotV1],
    live: usize,
    next: Option<u64>,
    staged: Option<RuntimeComputePipelineIdentityV1>,
) -> bool {
    compute_pipeline_capacity_body!(pipeline_rust_expr, slots, live, next, staged)
}

pub(super) fn exact_entry(
    slots: &[RuntimeComputePipelineSlotV1],
    identity: RuntimeComputePipelineIdentityV1,
) -> Option<&RuntimeComputePipelineEntryV1> {
    compute_pipeline_entry_body!(pipeline_rust_expr, slots, identity)
}

pub(super) fn exact_entry_mut(
    slots: &mut [RuntimeComputePipelineSlotV1],
    identity: RuntimeComputePipelineIdentityV1,
) -> Option<&mut RuntimeComputePipelineEntryV1> {
    compute_pipeline_entry_mut_body!(pipeline_rust_expr, slots, identity)
}

pub(super) fn checked_frontier(
    slots: &[RuntimeComputePipelineSlotV1],
    live: usize,
    frontier: Option<u64>,
    staged: Option<RuntimeComputePipelineIdentityV1>,
) -> Result<Option<usize>, ()> {
    compute_pipeline_frontier_body!(pipeline_rust_expr, slots, live, frontier, staged)
}

pub(super) fn staged_intact(
    slots: &[RuntimeComputePipelineSlotV1],
    live: usize,
    next: Option<u64>,
    frontier: Option<u64>,
    staged: Option<RuntimeComputePipelineIdentityV1>,
    identity: RuntimeComputePipelineIdentityV1,
) -> bool {
    compute_pipeline_staged_intact_body!(
        pipeline_rust_expr,
        slots,
        live,
        next,
        frontier,
        staged,
        identity
    )
}

// Refusal returns the original linear owner without adding a recovery allocation.
#[allow(clippy::result_large_err)]
pub(super) fn stage(
    slots: &mut [RuntimeComputePipelineSlotV1],
    live: &mut usize,
    next: Option<u64>,
    frontier: Option<u64>,
    staged: &mut Option<RuntimeComputePipelineIdentityV1>,
    active: ActiveSubmissionV1,
) -> Result<RuntimeComputePipelineIdentityV1, ActiveSubmissionV1> {
    compute_pipeline_stage_body!(
        pipeline_rust_expr,
        slots,
        live,
        next,
        frontier,
        staged,
        active
    )
}

pub(super) fn confirm(
    slots: &mut [RuntimeComputePipelineSlotV1],
    live: usize,
    next: &mut Option<u64>,
    frontier: &mut Option<u64>,
    staged: &mut Option<RuntimeComputePipelineIdentityV1>,
    identity: RuntimeComputePipelineIdentityV1,
) -> Result<(), ()> {
    compute_pipeline_confirm_body!(
        pipeline_rust_expr,
        slots,
        live,
        next,
        frontier,
        staged,
        identity
    )
}

pub(super) fn withdraw(
    slots: &mut [RuntimeComputePipelineSlotV1],
    live: &mut usize,
    next: Option<u64>,
    frontier: Option<u64>,
    staged: &mut Option<RuntimeComputePipelineIdentityV1>,
    identity: RuntimeComputePipelineIdentityV1,
) -> Option<ActiveSubmissionV1> {
    compute_pipeline_withdraw_body!(
        pipeline_rust_expr,
        slots,
        live,
        next,
        frontier,
        staged,
        identity
    )
}
