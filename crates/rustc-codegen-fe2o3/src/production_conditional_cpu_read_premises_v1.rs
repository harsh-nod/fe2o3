//! Compatibility adapter to the SAME portable read-premise checker.
use super::*;
pub(crate) use fe2o3_verifier::portable_reference_v1::ReplayedCpuEffectsV1;

pub(crate) fn check_replay_v1(
    ir: &ReferenceEffectIrV1,
    replay: &ReplayedCpuEffectsV1,
    budget: &mut Budget<'_>,
    check_input: impl FnMut(u32, &mut Budget<'_>) -> Result<(), Error>,
) -> Result<(), Error> {
    portable::read_premises::check_replay_v1(ir, replay, budget, check_input)
}
