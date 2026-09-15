//! Independent per-invocation effects, including the unwritten output frame.

const ELEMENTS: usize = crate::COMBINE_BATCHES * crate::TOKENS * crate::OUTPUT;

/// Fixed-order rank addition at one logical point. Invalid extents and points
/// leave the complete output unchanged; a valid call changes only `point`.
pub fn combine_expert_ranks_point_v1(
    point: usize,
    rank0: &[f32],
    rank1: &[f32],
    output: &mut [f32],
) {
    if rank0.len() == ELEMENTS
        && rank1.len() == ELEMENTS
        && output.len() == ELEMENTS
        && point < ELEMENTS
    {
        output[point] = rank0[point] + rank1[point];
    }
}

/// Stages one element for each active physical lane. Inactive lanes, invalid
/// extents and out-of-domain invocations preserve the complete output frame.
pub fn stage_gradient_shard_point_v1(point: usize, input: &[f32], output: &mut [f32]) {
    let batch = point / 64;
    let lane = point % 64;
    if batch < crate::SYSTEM_BATCHES
        && input.len() == crate::SYSTEM_BATCHES * crate::MUON_ELEMENTS
        && output.len() == crate::SYSTEM_BATCHES * crate::MUON_ELEMENTS
        && lane < crate::MUON_ELEMENTS
    {
        // The batch and lane guards make this coordinate nonoverflowing.
        let coordinate = batch.wrapping_mul(crate::MUON_ELEMENTS).wrapping_add(lane);
        output[coordinate] = input[coordinate];
    }
}

#[cfg(test)]
#[path = "effect_reference/tests.rs"]
mod tests;
