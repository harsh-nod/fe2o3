//! Content-only V50 lineage with the original nominal V3 physical finalizer.
//! No receipt decoder or caller callback can construct execution custody.

use crate::{
    InertProtectedFirstBuildWorkerV3EvidenceV1 as Source,
    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3 as DESCRIPTOR_SCRATCH,
    NominalWorkerFinalizationErrorV3 as FinalizationError,
    PreparedFinalizedNominalWorkerHsacoV3 as Artifact,
    finalize_protected_worker_nominal_hsaco_v3 as finalize_worker,
};
use fe2o3_compiler_lineage::MixedMiddleEndIdentityV50 as Identity;
use fe2o3_verifier::ExecutedTypedSourceTailV50 as Executed;

macro_rules! replay_lineage {
    ($executed:expr, $receipts:expr, $budget:expr $(,)?) => {
        $executed.replay_lineage_capsule_v50($receipts, $budget)
    };
}
include!("typed_worker_lineage_family.rs");

/// Retains the exact executed source owner through artifact use.
/// It grants no publication, load or launch authority.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::PreparedFinalizedTypedContentV50 as Owner;
/// fn duplicate(owner: Owner<'_, '_, '_, '_, '_, '_, '_>) { let _ = owner.clone(); }
/// ```
pub use PreparedFinalizedTypedContent as PreparedFinalizedTypedContentV50;
pub use TypedWorkerLineageError as TypedWorkerLineageErrorV50;
pub use finalize_protected_worker_typed_content as finalize_protected_worker_typed_content_v50;
