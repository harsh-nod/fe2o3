//! Exact V90 executed source lineage and V89 physical finalizer custody.
use super::*;
use fe2o3_compiler_ffi::{CompilerModuleKindV1, InertSemanticCompilerModuleHandoffV3 as Handoff};
use fe2o3_compiler_lineage::{
    MixedMiddleEndIdentityV90 as Identity, MixedMiddleEndLayoutV90 as Layout,
};
use fe2o3_hsaco_finalize::{
    InertProtectedFirstBuildWorkerV3EvidenceV1 as FirstBuild,
    PredicatedTypedWorkerLineageErrorV90 as ContentError,
    PreparedFinalizedNominalWorkerHsacoV89 as Artifact,
    PreparedFinalizedPredicatedTypedContentV90 as Content,
    finalize_protected_worker_predicated_typed_content_v90 as finalize_content,
};
use fe2o3_verifier::ExecutedPredicatedTypedSourceTailV90 as Executed;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

#[path = "production_pipeline_source_predicated_publish_v90.rs"]
mod publish_v90;

const COMPLETE_VERSIONED_HANDOFF: bool = true;
include!("production_pipeline_source_mixed_lineage_family.rs");
mixed_lineage_pipeline_family!(
    PredicatedLineage,
    check_lineage_request_v90,
    lineage_layout_v90,
    encode_lineage_content_v90,
    replay_lineage_capsule_v90
);
