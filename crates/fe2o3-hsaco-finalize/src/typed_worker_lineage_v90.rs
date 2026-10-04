//! Content-only V90 predicated lineage with the V89 physical finalizer.
//! No receipt decoder or caller callback can construct execution custody.

use crate::{
    InertProtectedFirstBuildWorkerV3EvidenceV1 as Source,
    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V89 as DESCRIPTOR_SCRATCH,
    NominalWorkerFinalizationErrorV89 as FinalizationError,
    PreparedFinalizedNominalWorkerHsacoV89 as Artifact,
    finalize_protected_worker_nominal_hsaco_v89 as finalize_worker,
};
use fe2o3_compiler_lineage::MixedMiddleEndIdentityV90 as Identity;
use fe2o3_verifier::ExecutedPredicatedTypedSourceTailV90 as Executed;

macro_rules! replay_lineage {
    ($executed:expr, $receipts:expr, $budget:expr $(,)?) => {
        $executed.replay_lineage_capsule_v90($receipts, $budget)
    };
}
include!("typed_worker_lineage_family.rs");

/// Retains the exact executed predicated source owner through artifact use.
/// It grants no publication, load or launch authority.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::PreparedFinalizedPredicatedTypedContentV90 as Owner;
/// fn duplicate(owner: Owner<'_, '_, '_, '_, '_, '_, '_>) { let _ = owner.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{PreparedFinalizedPredicatedTypedContentV90 as New, PreparedFinalizedTypedContentV50 as Old};
/// fn substitute<'e, 'r, 'h, 'n, 'p, 'v, 's>(old: Old<'e, 'r, 'h, 'n, 'p, 'v, 's>) -> New<'e, 'r, 'h, 'n, 'p, 'v, 's> { old }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{PreparedFinalizedPredicatedTypedContentV90 as New, PreparedFinalizedTypedContentV50 as Old};
/// fn downgrade<'e, 'r, 'h, 'n, 'p, 'v, 's>(new: New<'e, 'r, 'h, 'n, 'p, 'v, 's>) -> Old<'e, 'r, 'h, 'n, 'p, 'v, 's> { new }
/// ```
pub use PreparedFinalizedTypedContent as PreparedFinalizedPredicatedTypedContentV90;
pub use TypedWorkerLineageError as PredicatedTypedWorkerLineageErrorV90;
/// Requires the real predicated execution owner, never a legacy owner or decoded receipt.
/// The V89 descriptor and physical artifact remain mandatory; there is no fallback.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{InertProtectedFirstBuildWorkerV3EvidenceV1 as Source, finalize_protected_worker_predicated_typed_content_v90};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// use fe2o3_verifier::ExecutedTypedSourceTailV50;
/// fn substitute(source: Source, old: &ExecutedTypedSourceTailV50<'_, '_, '_, '_, '_, '_>, budget: &mut Budget<'_>) {
///     let _ = finalize_protected_worker_predicated_typed_content_v90(source, old, budget);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{InertProtectedFirstBuildWorkerV3EvidenceV1 as Source, finalize_protected_worker_typed_content_v50};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// use fe2o3_verifier::ExecutedPredicatedTypedSourceTailV90;
/// fn downgrade(source: Source, new: &ExecutedPredicatedTypedSourceTailV90<'_, '_, '_, '_, '_, '_>, budget: &mut Budget<'_>) {
///     let _ = finalize_protected_worker_typed_content_v50(source, new, budget);
/// }
/// ```
pub use finalize_protected_worker_typed_content as finalize_protected_worker_predicated_typed_content_v90;

#[cfg(test)]
mod version_tests {
    use super::*;

    #[test]
    fn predicated_typed_finalizer_retains_distinct_execution_identity_and_artifact_families() {
        use std::any::TypeId;
        type LegacyOwner = crate::PreparedFinalizedTypedContentV50<
            'static,
            'static,
            'static,
            'static,
            'static,
            'static,
            'static,
        >;
        type LegacyExecution = fe2o3_verifier::ExecutedTypedSourceTailV50<
            'static,
            'static,
            'static,
            'static,
            'static,
            'static,
        >;
        assert_ne!(TypeId::of::<StaticOwner>(), TypeId::of::<LegacyOwner>());
        assert_ne!(
            TypeId::of::<StaticExecution>(),
            TypeId::of::<LegacyExecution>()
        );
        assert_ne!(
            TypeId::of::<Identity>(),
            TypeId::of::<fe2o3_compiler_lineage::MixedMiddleEndIdentityV50>()
        );
        assert_ne!(
            TypeId::of::<Artifact>(),
            TypeId::of::<crate::PreparedFinalizedNominalWorkerHsacoV3>()
        );
        assert_ne!(
            TypeId::of::<Artifact>(),
            TypeId::of::<crate::PreparedFinalizedNominalWorkerHsacoV53>()
        );
        assert_eq!(
            TypeId::of::<Artifact>(),
            TypeId::of::<crate::PreparedFinalizedNominalWorkerHsacoV89>()
        );
        assert_eq!(
            DESCRIPTOR_SCRATCH,
            crate::NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V89
        );
    }
}
