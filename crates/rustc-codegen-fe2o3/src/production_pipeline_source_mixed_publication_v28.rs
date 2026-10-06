//! Nominal mixed inputs joined to the original protected compiler custody.
//! Shared capsule and sealed-verifier extensions still own publication admission.

use super::*;
use crate::production_pipeline::{AuthenticatedProductionBindings, ProductionCompilerCustody};
use crate::protected_rustc_invocation::{
    AdmittedProtectedRustcInvocationV1, ProtectedRustcInvocationErrorV1,
};
use fe2o3_artifact_transaction::BuildAttempt;
use fe2o3_verifier::PreparedOriginalSemanticMirRefinementV36 as OriginalMir;
use fe2o3_verifier::TypedSourceTailSubjectV50 as Subject;

#[path = "production_pipeline_source_original_mir_v30.rs"]
pub(super) mod original_mir_v30;

#[path = "production_pipeline_source_mixed_publication_execution_v29.rs"]
mod execution;

#[path = "production_pipeline_source_mixed_lineage_v29.rs"]
mod lineage;
pub(crate) use lineage::{ExecutedProtectedMixedPublicationV29, FinalizedProtectedMixedLineageV29};

/// These producer/consumer joins have no admitted mixed implementation yet.
/// They cannot be closed by caller flags, graph hashes or existing V12 receipts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MixedPublicationOpenGateV28 {
    OriginalMirToKirRefinement,
    ExecutedComposedRefinement,
    MixedSemanticCapsuleTransport,
    ProtectedCompilerExecutionJoin,
    // Existing inspected-artifact/descriptor admission; LLVM and LLD are trusted.
    FinalArtifactIdentityAndAdmission,
    SealedMixedWorkerFinalizerReplay,
    ConcreteRuntimePremiseDischarge,
}

#[derive(Debug)]
pub(crate) enum MixedPublicationErrorV28 {
    ExtractionOnly,
    LiveInvocation(ProtectedRustcInvocationErrorV1),
    Binding(&'static str),
    Lineage(fe2o3_hsaco_finalize::TypedWorkerLineageErrorV50),
    PredicatedLineage(fe2o3_hsaco_finalize::PredicatedTypedWorkerLineageErrorV90),
}
impl std::fmt::Display for MixedPublicationErrorV28 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "mixed publication custody: {self:?}")
    }
}
impl std::error::Error for MixedPublicationErrorV28 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::LiveInvocation(error) => Some(error),
            Self::Lineage(error) => Some(error),
            Self::PredicatedLineage(error) => Some(error),
            Self::ExtractionOnly | Self::Binding(_) => None,
        }
    }
}
impl From<MixedPublicationErrorV28> for Error {
    fn from(error: MixedPublicationErrorV28) -> Self {
        Self::MixedPublication(error)
    }
}

include!("production_pipeline_source_mixed_publication_family.rs");
mixed_publication_pipeline_family!(
    with_original_source_mixed_publication_on_account_v28,
    with_original_source_mixed_publication_v28,
    with_original_source_mixed_publication_test_limits_v28
);
