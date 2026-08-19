#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]
#![doc = r#"
Repository-approved join for the exact Pliron scalar-add Worker V2 lineage.

This crate joins opaque source/request lineage, one exact measured worker
execution, and a checkout-embedded approval profile. The profile is
compiled from one canonical manifest and has no public raw constructor or
decoder. The final receipt retains the execution privately and exposes neither
HSACO bytes nor a generic payload consumer. The sole runtime transition
consumes that receipt, loads only its privately retained bytes on one pinned
MI300X lane, executes one fixed scalar operation, and returns bounded
post-unload evidence. This crate does not claim general memory safety, race
freedom, or CUDA-Oxide parity.
"#]

mod authority;
mod manifest;
mod runtime;
mod source;

pub use authority::{
    FinalizedRepositoryScalarAddV1, FinalizedRepositoryScalarAddV1Identity,
    ObservedRepositoryScalarAddV1, RepositoryApprovalFieldV1, RepositoryApprovalIdentityV1,
    RepositoryScalarAddProfileV1, ScalarAddFinalizationErrorV1, ScalarAddLineageFieldV1,
    ScalarAddLineageIdentityV1, ScalarAddObservationIdentityV1, finalize_repository_scalar_add_v1,
};
pub use manifest::{RepositoryManifestFieldV1, RepositoryProfileErrorV1, repository_profile_v1};
pub use runtime::{
    QUALIFIED_MI300X_HSA_UUID_OBSERVATION_V1, REQUIRED_MI300X_PHYSICAL_DEVICE_IDENTITY_V1,
    RuntimeEvidenceIdentityV1, RuntimeEvidenceMarkerErrorV1, RuntimeEvidenceV1,
    RuntimeExecutionErrorV1, RuntimeLaneFieldV1, RuntimeObservationFieldV1, RuntimeResultFieldV1,
    execute_repository_scalar_add_v1_on_mi300x,
};
pub use source::{
    CanonicalPreparedScalarAddV1, CanonicalSourceErrorV1, CanonicalSourceObservationV1,
    canonical_prepared_scalar_add_v1, canonical_source_observation_v1,
};
