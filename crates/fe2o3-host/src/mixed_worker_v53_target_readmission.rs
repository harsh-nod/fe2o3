//! Independent target-selection reconstruction; never proof or dispatch authority.
use super::*;
use fe2o3_compiler_lineage::{InertCanonicalSemanticMirReceiptV3, TargetLineageIdentityV3};
use fe2o3_kernel_ir::{
    CanonicalKernelIrReplayAdmissionErrorV18, CanonicalKernelIrReplayStorageV18,
    StorageLayoutLimitsV1, VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use fe2o3_verifier::{MixedTargetSelectionSubjectV53, check_mixed_target_selection_v53};

// Match the production semantic lowerer's finite storage-layout policy. The
// receiver still enforces its own caller-supplied cumulative work/storage caps.
const LAYOUT_LIMITS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 1 << 20,
    edges: 1 << 22,
    containment_depth: 256,
    object_bytes: (1u64 << 61) - 1,
};

pub(super) fn readmit_target_selection_v53(
    forwarded: &[u8],
    semantic: &InertCanonicalSemanticMirReceiptV3,
    invocation: TargetLineageIdentityV3,
    descriptor: &[u8],
    profile: ProductionAmdTargetProfileV1,
    received: &[u8],
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.check_prior_denials_v1()?;
    let scratch = [
        forwarded.len(),
        semantic.canonical_preimage().len(),
        descriptor.len(),
        received.len(),
        size_of::<CanonicalKernelIrReplayStorageV18>(),
        size_of::<MixedTargetSelectionSubjectV53<'_>>(),
        size_of::<
            std::result::Result<
                (Owner, CanonicalKernelIrReplayStorageV18),
                CanonicalKernelIrReplayAdmissionErrorV18,
            >,
        >(),
    ]
    .into_iter()
    .try_fold(0usize, |total, next| total.checked_add(next))
    .ok_or(Resource::Arithmetic)?;
    codec_on_budget(budget, scratch, |budget| {
        let (owner, storage) = Owner::from_canonical_bytes_with_verification_budget_v18(
            forwarded,
            LAYOUT_LIMITS,
            budget,
        )
        .map_err(codec_error)?;
        budget.reserve_storage(storage.retained_storage())?;
        let subject = MixedTargetSelectionSubjectV53 {
            owner: &owner,
            invocation,
            semantic_mir: semantic,
            descriptor,
            profile,
        };
        let result =
            check_mixed_target_selection_v53(&subject, received, budget).map_err(codec_error);
        // Owner drops before the same ledger's enclosing scope refunds storage,
        // on success, refusal and unwind. No caller-created owner can substitute.
        drop(owner);
        result
    })
}

#[cfg(test)]
#[path = "mixed_worker_v53_target_readmission_tests.rs"]
mod tests;
