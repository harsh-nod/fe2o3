//! Producer-owned mixed content joins the existing capsule and module handoff.

use super::*;
use fe2o3_compiler_ffi::{CompilerModuleKindV1, InertSemanticCompilerModuleHandoffV3 as Handoff};
use fe2o3_compiler_lineage::{
    MixedMiddleEndIdentityV50 as Identity, MixedMiddleEndLayoutV50 as Layout,
};
use fe2o3_hsaco_finalize::{
    InertProtectedFirstBuildWorkerV3EvidenceV1 as FirstBuild,
    PreparedFinalizedNominalWorkerHsacoV3 as Artifact, PreparedFinalizedTypedContentV50 as Content,
};
use fe2o3_verifier::ExecutedTypedSourceTailV50 as Executed;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

#[path = "production_pipeline_source_mixed_publish_v53.rs"]
mod publish_v53;

use fe2o3_hsaco_finalize::{
    TypedWorkerLineageErrorV50 as ContentError,
    finalize_protected_worker_typed_content_v50 as finalize_content,
};
const COMPLETE_VERSIONED_HANDOFF: bool = false;
include!("production_pipeline_source_mixed_lineage_family.rs");
mixed_lineage_pipeline_family!(
    Lineage,
    check_lineage_request_v50,
    lineage_layout_v50,
    encode_lineage_content_v50,
    replay_lineage_capsule_v50
);
