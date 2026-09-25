//! Borrowed replay adapter; live source authentication stays with the binding.

use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_verifier::portable_reference_v1::{
    self as portable, ReferenceReplayInputV1, ReplayedCpuEffectsV1,
};
#[cfg(test)]
use std::panic::{AssertUnwindSafe, catch_unwind};

type Error = ReferenceBindingErrorV1;

impl AuthenticatedReferenceEffectBindingV1 {
    pub(crate) fn with_replayed_output_writes_v1<R>(
        &self,
        budget: &mut Budget<'_>,
        consume: impl for<'cpu> FnOnce(&'cpu ReplayedCpuEffectsV1, &mut Budget<'_>) -> R,
    ) -> Result<R, Error> {
        portable::with_replayed_output_writes_v1(
            ReferenceReplayInputV1 {
                signature_preimage: &self.signature_preimage,
                effect_ir: &self.effect_ir,
                effect_ir_sha256: self.effect_ir_sha256,
                observable_output_writes: &self.observable_output_writes,
            },
            budget,
            consume,
        )
    }
}

#[cfg(test)]
#[path = "production_reference_effect_join_v2_replay_tests.rs"]
mod tests;
