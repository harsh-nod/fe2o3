use vstd::prelude::*;

#[path = "../lds_tiled_kphase.rs"]
mod model;

verus! {

/// Mutation: phase one starts from zero instead of carrying phase zero's
/// accumulator. A concrete all-one 16x32 by 32x16 product exposes the reset.
pub open spec fn mutated_reset_phase_accumulator_v1(
    prior_accumulator: real,
    phase_contribution: real,
) -> real {
    phase_contribution
}

pub open spec fn correct_carried_phase_accumulator_v1(
    prior_accumulator: real,
    phase_contribution: real,
) -> real {
    prior_accumulator + phase_contribution
}

pub proof fn mutated_accumulator_reset_preserves_k_product_v1()
    ensures mutated_reset_phase_accumulator_v1(16real, 16real)
        == correct_carried_phase_accumulator_v1(16real, 16real),
{
}

} // verus!
