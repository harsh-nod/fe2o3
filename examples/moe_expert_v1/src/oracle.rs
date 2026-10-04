//! Independent direct CPU oracle for every expert and combined token output.
//!
//! Unlike the host schedule, this implementation evaluates accepted routes
//! directly in route-ID order and then places each result into its expert row
//! and compact slot. It does not execute the tile-compaction/GEMM loops from
//! `pipeline`.

use fe2o3_device::Bf16;
use fe2o3_moe_top2_v1::RoutingOutputsV1;

use crate::{
    DROP_ROUTE_V1, MOE_COMBINED_OUTPUT_ELEMENTS_V1, MOE_COMPACT_OUTPUT_ELEMENTS_V1,
    MOE_EXPERT_INPUT_WIDTH_V1, MOE_EXPERT_OUTPUT_WIDTH_V1, MOE_EXPERT_TILE_ELEMENTS_V1,
    MOE_EXPERTS_V1, MOE_ROUTES_PER_TOKEN_V1, MOE_ROUTES_V1, MoeExpertInputErrorV1,
    pipeline::validate_pipeline_inputs_v1,
};

/// Independent expected values for all schedule-visible outputs.
#[derive(Clone, Debug, PartialEq)]
pub struct MoeExpertOracleV1 {
    /// Expected complete expert tiles, including untouched zero padding rows.
    pub expert_output_tiles: [f32; MOE_EXPERTS_V1 * MOE_EXPERT_TILE_ELEMENTS_V1],
    /// Expected accepted outputs in compact routing-slot order.
    pub compact_output: [f32; MOE_COMPACT_OUTPUT_ELEMENTS_V1],
    /// Expected final token-major weighted output.
    pub combined_output: [f32; MOE_COMBINED_OUTPUT_ELEMENTS_V1],
    /// Expected route-major output before inverse permutation.
    pub route_output: [f32; MOE_COMPACT_OUTPUT_ELEMENTS_V1],
}

/// Computes every accepted expert row and weighted token output directly.
pub fn moe_expert_independent_oracle_v1(
    logits: &[f32],
    token_activations: &[u16],
    expert_weights: &[u16],
    route_weights: &[f32],
    routing: &RoutingOutputsV1,
) -> Result<MoeExpertOracleV1, MoeExpertInputErrorV1> {
    validate_pipeline_inputs_v1(
        logits,
        token_activations,
        expert_weights,
        route_weights,
        routing,
    )?;

    let mut expert_output_tiles = [0.0_f32; MOE_EXPERTS_V1 * MOE_EXPERT_TILE_ELEMENTS_V1];
    let mut compact_output = [0.0_f32; MOE_COMPACT_OUTPUT_ELEMENTS_V1];
    let mut route_output = [0.0_f32; MOE_COMPACT_OUTPUT_ELEMENTS_V1];
    let mut combined_output = [0.0_f32; MOE_COMBINED_OUTPUT_ELEMENTS_V1];

    for route in 0..MOE_ROUTES_V1 {
        let slot = routing.inverse[route];
        if slot == DROP_ROUTE_V1 {
            continue;
        }
        let token = route / MOE_ROUTES_PER_TOKEN_V1;
        let expert = routing.top2_experts[route] as usize;
        let expert_row = slot as usize - routing.expert_offsets[expert] as usize;
        for output in 0..MOE_EXPERT_OUTPUT_WIDTH_V1 {
            let mut value = 0.0_f32;
            for depth in 0..MOE_EXPERT_INPUT_WIDTH_V1 {
                let activation =
                    Bf16::from_bits(token_activations[token * MOE_EXPERT_INPUT_WIDTH_V1 + depth])
                        .to_f32();
                let weight = Bf16::from_bits(
                    expert_weights[expert * MOE_EXPERT_TILE_ELEMENTS_V1
                        + depth * MOE_EXPERT_OUTPUT_WIDTH_V1
                        + output],
                )
                .to_f32();
                value += activation * weight;
            }
            route_output[route * MOE_EXPERT_OUTPUT_WIDTH_V1 + output] = value;
            compact_output[slot as usize * MOE_EXPERT_OUTPUT_WIDTH_V1 + output] = value;
            expert_output_tiles[expert * MOE_EXPERT_TILE_ELEMENTS_V1
                + expert_row * MOE_EXPERT_OUTPUT_WIDTH_V1
                + output] = value;
            combined_output[token * MOE_EXPERT_OUTPUT_WIDTH_V1 + output] +=
                route_weights[route] * value;
        }
    }

    Ok(MoeExpertOracleV1 {
        expert_output_tiles,
        compact_output,
        combined_output,
        route_output,
    })
}
