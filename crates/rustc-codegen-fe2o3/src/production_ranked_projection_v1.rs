//! Compatibility facade for the frontend-neutral production middle end.
//!
//! rustc collection ends at admitted semantic MIR. Ranked projection and its
//! verification custody are owned by `fe2o3-middle-end`.

pub use fe2o3_middle_end::ProductionRankedSemanticProjectionRosterReceiptV1;
pub(crate) use fe2o3_middle_end::{
    AuthenticatedRankedVerificationRosterV1, ProductionRankedProjectionErrorV1,
    ProductionRankedRootInputV1, ProductionRankedRootProgramV1, ProductionRankedSemanticProgramV1,
    ProductionRankedVerificationErrorV1, project_and_verify_ranked_semantic_mir_v1,
};
