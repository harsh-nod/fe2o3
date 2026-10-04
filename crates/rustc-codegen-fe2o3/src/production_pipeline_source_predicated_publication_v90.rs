//! Predicated original-source and final-native custody for strict publication.
use super::super::mixed_worker_v28::publication::{
    MixedPublicationErrorV28, MixedPublicationOpenGateV28,
};
use super::*;
use crate::production_pipeline::{AuthenticatedProductionBindings, ProductionCompilerCustody};
use crate::protected_rustc_invocation::{
    AdmittedProtectedRustcInvocationV1, ProtectedRustcInvocationErrorV1,
};
use fe2o3_artifact_transaction::BuildAttempt;
use fe2o3_verifier::{
    PredicatedTypedSourceTailSubjectV90 as Subject,
    PreparedOriginalSemanticMirRefinementV36 as OriginalMir,
};

#[path = "production_pipeline_source_predicated_publication_execution_v90.rs"]
mod execution;
#[path = "production_pipeline_source_predicated_lineage_v90.rs"]
mod lineage;
#[path = "production_pipeline_source_original_mir_v30.rs"]
pub(super) mod original_mir_v30;
pub(crate) use lineage::{ExecutedProtectedMixedPublicationV29, FinalizedProtectedMixedLineageV29};

include!("production_pipeline_source_mixed_publication_family.rs");
mixed_publication_pipeline_family!(
    with_original_source_predicated_publication_on_account_v90,
    with_original_source_predicated_publication_v90,
    with_original_source_predicated_publication_test_limits_v90
);
