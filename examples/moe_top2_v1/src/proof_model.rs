//! Executable proof-facing routing invariants.
//!
//! These checks make the intended obligations concrete and mutation-testable.
//! They are not a machine-checked Verus proof and do not establish refinement
//! from the attributed Rust source, compiler IR, or machine code.

use crate::{
    contract::{
        DROP_ROUTE_V1, MOE_EXPERT_CAPACITY_V1, MOE_EXPERTS_V1, MOE_LOGIT_ELEMENTS_V1,
        MOE_ROUTES_PER_TOKEN_V1, MOE_ROUTES_V1, MOE_TOKENS_V1,
    },
    oracle::RoutingOutputsV1,
};

/// Honest assurance statement for this Phase A executable model.
pub const MODEL_ASSURANCE_V1: &str = "executable bounded invariant model with hostile mutation tests; reviewed correspondence only, not a machine-checked Verus/source refinement proof";
/// Phase A deliberately does not claim source-to-model refinement.
pub const SOURCE_MODEL_REFINEMENT_PROVED_V1: bool = false;

/// A violated routing-model obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelViolationV1 {
    /// Input length differs from the exact profile.
    WrongLogitLength,
    /// One input logit is NaN or infinity.
    NonFiniteLogit,
    /// A selected expert is outside `0..4`.
    Top2Range {
        /// Invalid route ID.
        route: usize,
    },
    /// A token selected the same expert twice.
    Top2Distinctness {
        /// Invalid token.
        token: usize,
    },
    /// Top-2 ordering or its lower-ID tie-break is wrong.
    Top2Order {
        /// Invalid token.
        token: usize,
    },
    /// Requested expert counts do not equal selected-route counts.
    RequestedCounts,
    /// Admitted counts do not equal requested counts clamped to capacity.
    AdmittedCounts,
    /// One admitted count exceeds expert capacity.
    ExpertCapacity {
        /// Expert with the invalid count.
        expert: usize,
    },
    /// Offsets are not the exclusive scan of admitted counts.
    ExclusiveScan,
    /// A non-sentinel slot is outside the compact route extent.
    SlotRange {
        /// Route carrying the invalid slot.
        route: usize,
    },
    /// A route does not have its stable capacity-bounded slot or drop sentinel.
    StableSlot {
        /// Route with the invalid assignment.
        route: usize,
    },
    /// Two accepted routes claim the same slot.
    DuplicateSlot {
        /// Duplicated compact slot.
        slot: usize,
    },
    /// A compact permutation entry is missing or references the wrong route.
    Permutation {
        /// Invalid compact slot.
        slot: usize,
    },
    /// A route's inverse does not round trip through the permutation.
    Inverse {
        /// Invalid route ID.
        route: usize,
    },
    /// Unused permutation tail entries are not sentinels.
    PermutationTail {
        /// Invalid tail slot.
        slot: usize,
    },
}

fn model_precedes(logits: &[f32], token: usize, left: usize, right: usize) -> bool {
    let left_score = logits[token * MOE_EXPERTS_V1 + left];
    let right_score = logits[token * MOE_EXPERTS_V1 + right];
    left_score > right_score || (left_score == right_score && left < right)
}

fn model_top2(logits: &[f32], token: usize) -> [u32; 2] {
    let mut order = [0_usize, 1, 2, 3];
    let mut left = 0;
    while left < MOE_EXPERTS_V1 {
        let mut right = left + 1;
        while right < MOE_EXPERTS_V1 {
            if model_precedes(logits, token, order[right], order[left]) {
                order.swap(left, right);
            }
            right += 1;
        }
        left += 1;
    }
    [order[0] as u32, order[1] as u32]
}

/// Validates all exact top-2, count, scan, capacity, packing, and inverse obligations.
pub fn validate_routing_model_v1(
    logits: &[f32],
    output: &RoutingOutputsV1,
) -> Result<(), ModelViolationV1> {
    if logits.len() != MOE_LOGIT_ELEMENTS_V1 {
        return Err(ModelViolationV1::WrongLogitLength);
    }
    if logits.iter().any(|logit| !logit.is_finite()) {
        return Err(ModelViolationV1::NonFiniteLogit);
    }

    let mut expected_requested = [0_u32; MOE_EXPERTS_V1];
    for token in 0..MOE_TOKENS_V1 {
        let route = token * MOE_ROUTES_PER_TOKEN_V1;
        let first = output.top2_experts[route];
        let second = output.top2_experts[route + 1];
        if first as usize >= MOE_EXPERTS_V1 {
            return Err(ModelViolationV1::Top2Range { route });
        }
        if second as usize >= MOE_EXPERTS_V1 {
            return Err(ModelViolationV1::Top2Range { route: route + 1 });
        }
        if first == second {
            return Err(ModelViolationV1::Top2Distinctness { token });
        }
        if [first, second] != model_top2(logits, token) {
            return Err(ModelViolationV1::Top2Order { token });
        }
        expected_requested[first as usize] += 1;
        expected_requested[second as usize] += 1;
    }
    if output.requested_counts != expected_requested {
        return Err(ModelViolationV1::RequestedCounts);
    }

    let mut expected_admitted = [0_u32; MOE_EXPERTS_V1];
    for expert in 0..MOE_EXPERTS_V1 {
        if output.admitted_counts[expert] > MOE_EXPERT_CAPACITY_V1 as u32 {
            return Err(ModelViolationV1::ExpertCapacity { expert });
        }
        expected_admitted[expert] = expected_requested[expert].min(MOE_EXPERT_CAPACITY_V1 as u32);
    }
    if output.admitted_counts != expected_admitted {
        return Err(ModelViolationV1::AdmittedCounts);
    }

    let mut expected_offsets = [0_u32; MOE_EXPERTS_V1 + 1];
    for expert in 0..MOE_EXPERTS_V1 {
        expected_offsets[expert + 1] = expected_offsets[expert] + expected_admitted[expert];
    }
    if output.expert_offsets != expected_offsets {
        return Err(ModelViolationV1::ExclusiveScan);
    }

    let mut seen_expert = [0_u32; MOE_EXPERTS_V1];
    let mut seen_slot = [false; MOE_ROUTES_V1];
    for route in 0..MOE_ROUTES_V1 {
        let expert = output.top2_experts[route] as usize;
        let stable_rank = seen_expert[expert];
        seen_expert[expert] += 1;
        let expected_slot = if stable_rank < MOE_EXPERT_CAPACITY_V1 as u32 {
            expected_offsets[expert] + stable_rank
        } else {
            DROP_ROUTE_V1
        };
        let actual_slot = output.route_slots[route];
        if actual_slot != DROP_ROUTE_V1 && actual_slot as usize >= MOE_ROUTES_V1 {
            return Err(ModelViolationV1::SlotRange { route });
        }
        if expected_slot != DROP_ROUTE_V1
            && actual_slot != DROP_ROUTE_V1
            && seen_slot[actual_slot as usize]
        {
            return Err(ModelViolationV1::DuplicateSlot {
                slot: actual_slot as usize,
            });
        }
        if actual_slot != expected_slot {
            return Err(ModelViolationV1::StableSlot { route });
        }
        if expected_slot == DROP_ROUTE_V1 {
            if output.inverse[route] != DROP_ROUTE_V1 {
                return Err(ModelViolationV1::Inverse { route });
            }
            continue;
        }

        let slot = expected_slot as usize;
        seen_slot[slot] = true;
        if output.permutation[slot] != route as u32 {
            return Err(ModelViolationV1::Permutation { slot });
        }
        if output.inverse[route] != expected_slot
            || output.permutation[output.inverse[route] as usize] != route as u32
        {
            return Err(ModelViolationV1::Inverse { route });
        }
    }

    let accepted = expected_offsets[MOE_EXPERTS_V1] as usize;
    for (slot, occupied) in seen_slot.iter().enumerate().take(accepted) {
        if !occupied {
            return Err(ModelViolationV1::Permutation { slot });
        }
    }
    for slot in accepted..MOE_ROUTES_V1 {
        if output.permutation[slot] != DROP_ROUTE_V1 {
            return Err(ModelViolationV1::PermutationTail { slot });
        }
    }
    Ok(())
}
