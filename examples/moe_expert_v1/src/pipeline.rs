//! Executable host schedule for the exact bounded expert-compute profile.
//!
//! This mirrors the four-dispatch schedule and combine source at the CPU level.
//! It is testable planning/arithmetic code, not GPU execution evidence.

use core::fmt;

use fe2o3_device::Bf16;
use fe2o3_moe_top2_v1::{ModelViolationV1, RoutingOutputsV1, validate_routing_model_v1};

use crate::contract::{
    DROP_ROUTE_V1, MOE_COMBINED_OUTPUT_ELEMENTS_V1, MOE_COMPACT_OUTPUT_ELEMENTS_V1,
    MOE_EXPERT_CAPACITY_V1, MOE_EXPERT_GEMM_DISPATCHES_V1, MOE_EXPERT_INPUT_WIDTH_V1,
    MOE_EXPERT_OUTPUT_WIDTH_V1, MOE_EXPERT_TILE_ELEMENTS_V1, MOE_EXPERT_TILE_ROWS_V1,
    MOE_EXPERT_WEIGHT_ELEMENTS_V1, MOE_EXPERTS_V1, MOE_ROUTES_PER_TOKEN_V1, MOE_ROUTES_V1,
    MOE_TOKEN_ACTIVATION_ELEMENTS_V1,
};

/// One exact host-scheduled expert GEMM dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpertDispatchV1 {
    /// Expert selected for this dispatch.
    pub expert: usize,
    /// Number of active compact rows before zero padding.
    pub active_rows: usize,
    /// First activation-tile element in the four-tile allocation.
    pub activation_offset: usize,
    /// First weight element in the expert-major allocation.
    pub weight_offset: usize,
    /// First output-tile element in the four-tile allocation.
    pub output_offset: usize,
}

/// Complete observable result of the exact host schedule.
#[derive(Clone, Debug, PartialEq)]
pub struct MoeExpertExecutionV1 {
    /// Four zero-padded expert activation tiles.
    pub activation_tiles: [u16; MOE_EXPERTS_V1 * MOE_EXPERT_TILE_ELEMENTS_V1],
    /// Four complete FP32 expert output tiles, including zero padding rows.
    pub expert_output_tiles: [f32; MOE_EXPERTS_V1 * MOE_EXPERT_TILE_ELEMENTS_V1],
    /// Accepted route outputs packed by routing compact slot.
    pub compact_output: [f32; MOE_COMPACT_OUTPUT_ELEMENTS_V1],
    /// Final token-major weighted output.
    pub combined_output: [f32; MOE_COMBINED_OUTPUT_ELEMENTS_V1],
    /// Exact one-dispatch-per-expert schedule.
    pub dispatches: [ExpertDispatchV1; MOE_EXPERT_GEMM_DISPATCHES_V1],
}

/// Input admission failure before schedule construction or arithmetic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MoeExpertInputErrorV1 {
    /// A slice did not have its exact fixed extent.
    WrongLength {
        /// Rejected input name.
        input: &'static str,
        /// Required element count.
        expected: usize,
        /// Supplied element count.
        actual: usize,
    },
    /// One activation or expert weight was not finite BF16.
    NonFiniteBf16 {
        /// Rejected input name.
        input: &'static str,
        /// Element index.
        index: usize,
        /// Exact BF16 bits.
        bits: u16,
    },
    /// Route weights violated finite, nonnegative, or pair-sum policy.
    RouteWeights {
        /// Token whose pair was invalid.
        token: usize,
    },
    /// The supplied routing record violated the public exact model.
    Routing(ModelViolationV1),
    /// An accepted route could not be reconciled with its expert segment.
    RouteSegment {
        /// Rejected route ID.
        route: usize,
    },
}

impl fmt::Display for MoeExpertInputErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid exact MoE expert input: {self:?}")
    }
}

impl std::error::Error for MoeExpertInputErrorV1 {}

fn require_len(
    input: &'static str,
    actual: usize,
    expected: usize,
) -> Result<(), MoeExpertInputErrorV1> {
    if actual == expected {
        Ok(())
    } else {
        Err(MoeExpertInputErrorV1::WrongLength {
            input,
            expected,
            actual,
        })
    }
}

fn validate_bf16(input: &'static str, values: &[u16]) -> Result<(), MoeExpertInputErrorV1> {
    for (index, bits) in values.iter().copied().enumerate() {
        if !Bf16::from_bits(bits).to_f32().is_finite() {
            return Err(MoeExpertInputErrorV1::NonFiniteBf16 { input, index, bits });
        }
    }
    Ok(())
}

pub(crate) fn validate_pipeline_inputs_v1(
    logits: &[f32],
    token_activations: &[u16],
    expert_weights: &[u16],
    route_weights: &[f32],
    routing: &RoutingOutputsV1,
) -> Result<(), MoeExpertInputErrorV1> {
    require_len(
        "token_activations",
        token_activations.len(),
        MOE_TOKEN_ACTIVATION_ELEMENTS_V1,
    )?;
    require_len(
        "expert_weights",
        expert_weights.len(),
        MOE_EXPERT_WEIGHT_ELEMENTS_V1,
    )?;
    require_len("route_weights", route_weights.len(), MOE_ROUTES_V1)?;
    validate_bf16("token_activations", token_activations)?;
    validate_bf16("expert_weights", expert_weights)?;
    for token in 0..crate::MOE_TOKENS_V1 {
        let route = token * MOE_ROUTES_PER_TOKEN_V1;
        let first = route_weights[route];
        let second = route_weights[route + 1];
        if !first.is_finite()
            || !second.is_finite()
            || first < 0.0
            || second < 0.0
            || first + second != 1.0
        {
            return Err(MoeExpertInputErrorV1::RouteWeights { token });
        }
    }
    validate_routing_model_v1(logits, routing).map_err(MoeExpertInputErrorV1::Routing)
}

fn expert_for_slot(routing: &RoutingOutputsV1, slot: usize) -> Option<(usize, usize)> {
    (0..MOE_EXPERTS_V1).find_map(|expert| {
        let start = routing.expert_offsets[expert] as usize;
        let end = routing.expert_offsets[expert + 1] as usize;
        (start <= slot && slot < end).then_some((expert, slot - start))
    })
}

/// Executes the exact four-GEMM host schedule and deterministic combine model.
///
/// Arithmetic is sequential BF16-to-FP32 multiply/add in depth order. It is a
/// source-level reference for schedule testing and is not an MFMA numerical
/// refinement claim.
pub fn run_host_scheduled_moe_experts_v1(
    logits: &[f32],
    token_activations: &[u16],
    expert_weights: &[u16],
    route_weights: &[f32],
    routing: &RoutingOutputsV1,
) -> Result<MoeExpertExecutionV1, MoeExpertInputErrorV1> {
    validate_pipeline_inputs_v1(
        logits,
        token_activations,
        expert_weights,
        route_weights,
        routing,
    )?;

    let dispatches = core::array::from_fn(|expert| ExpertDispatchV1 {
        expert,
        active_rows: routing.admitted_counts[expert] as usize,
        activation_offset: expert * MOE_EXPERT_TILE_ELEMENTS_V1,
        weight_offset: expert * MOE_EXPERT_TILE_ELEMENTS_V1,
        output_offset: expert * MOE_EXPERT_TILE_ELEMENTS_V1,
    });
    let mut activation_tiles = [0_u16; MOE_EXPERTS_V1 * MOE_EXPERT_TILE_ELEMENTS_V1];

    let accepted = routing.expert_offsets[MOE_EXPERTS_V1] as usize;
    for slot in 0..accepted {
        let route = routing.permutation[slot] as usize;
        let Some((expert, expert_row)) = expert_for_slot(routing, slot) else {
            return Err(MoeExpertInputErrorV1::RouteSegment { route });
        };
        if route >= MOE_ROUTES_V1
            || expert_row >= MOE_EXPERT_CAPACITY_V1
            || routing.top2_experts[route] as usize != expert
            || routing.inverse[route] as usize != slot
        {
            return Err(MoeExpertInputErrorV1::RouteSegment { route });
        }
        let token = route / MOE_ROUTES_PER_TOKEN_V1;
        for depth in 0..MOE_EXPERT_INPUT_WIDTH_V1 {
            activation_tiles[expert * MOE_EXPERT_TILE_ELEMENTS_V1
                + expert_row * MOE_EXPERT_INPUT_WIDTH_V1
                + depth] = token_activations[token * MOE_EXPERT_INPUT_WIDTH_V1 + depth];
        }
    }

    let mut expert_output_tiles = [0.0_f32; MOE_EXPERTS_V1 * MOE_EXPERT_TILE_ELEMENTS_V1];
    for dispatch in dispatches {
        for row in 0..MOE_EXPERT_TILE_ROWS_V1 {
            for output in 0..MOE_EXPERT_OUTPUT_WIDTH_V1 {
                let mut accumulator = 0.0_f32;
                for depth in 0..MOE_EXPERT_INPUT_WIDTH_V1 {
                    let activation = Bf16::from_bits(
                        activation_tiles
                            [dispatch.activation_offset + row * MOE_EXPERT_INPUT_WIDTH_V1 + depth],
                    )
                    .to_f32();
                    let weight = Bf16::from_bits(
                        expert_weights
                            [dispatch.weight_offset + depth * MOE_EXPERT_OUTPUT_WIDTH_V1 + output],
                    )
                    .to_f32();
                    accumulator += activation * weight;
                }
                expert_output_tiles
                    [dispatch.output_offset + row * MOE_EXPERT_OUTPUT_WIDTH_V1 + output] =
                    accumulator;
            }
        }
    }

    let mut compact_output = [0.0_f32; MOE_COMPACT_OUTPUT_ELEMENTS_V1];
    for slot in 0..accepted {
        let route = routing.permutation[slot] as usize;
        let expert = routing.top2_experts[route] as usize;
        let row = slot - routing.expert_offsets[expert] as usize;
        for output in 0..MOE_EXPERT_OUTPUT_WIDTH_V1 {
            compact_output[slot * MOE_EXPERT_OUTPUT_WIDTH_V1 + output] = expert_output_tiles
                [expert * MOE_EXPERT_TILE_ELEMENTS_V1 + row * MOE_EXPERT_OUTPUT_WIDTH_V1 + output];
        }
    }

    let mut combined_output = [0.0_f32; MOE_COMBINED_OUTPUT_ELEMENTS_V1];
    for token in 0..crate::MOE_TOKENS_V1 {
        for output in 0..MOE_EXPERT_OUTPUT_WIDTH_V1 {
            let mut accumulator = 0.0_f32;
            for rank in 0..MOE_ROUTES_PER_TOKEN_V1 {
                let route = token * MOE_ROUTES_PER_TOKEN_V1 + rank;
                let slot = routing.inverse[route];
                if slot != DROP_ROUTE_V1 {
                    accumulator += route_weights[route]
                        * compact_output[slot as usize * MOE_EXPERT_OUTPUT_WIDTH_V1 + output];
                }
            }
            combined_output[token * MOE_EXPERT_OUTPUT_WIDTH_V1 + output] = accumulator;
        }
    }

    Ok(MoeExpertExecutionV1 {
        activation_tiles,
        expert_output_tiles,
        compact_output,
        combined_output,
        dispatches,
    })
}
