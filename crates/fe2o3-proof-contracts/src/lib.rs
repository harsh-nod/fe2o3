#![no_std]
#![forbid(unsafe_code)]

//! Solver-neutral records for exact, independently scoped proof claims.
//!
//! This crate validates structural consistency only. A valid record does not
//! authenticate a digest, execute a checker, prove a property, authorize a GPU
//! launch, or promote one property status into another.

extern crate alloc;

mod affine_bounds_v1;
mod constrained_affine_bounds_v2;
mod dynamic_constrained_affine_bounds_v3;
mod identity;
mod model;
mod validation;

pub use affine_bounds_v1::{
    AffineBoundsCertificateErrorV1, AffineBoundsCertificateV1, AffineBoundsQueryV1,
    CheckedAffineBoundsCertificateV1, MAX_AFFINE_BOUNDS_RANK_V1,
    check_affine_bounds_certificate_v1,
};
pub use constrained_affine_bounds_v2::{
    AffineInequalityV2, CheckedConstrainedAffineBoundsCertificateV2,
    ConstrainedAffineBoundsCertificateErrorV2, ConstrainedAffineBoundsCertificateV2,
    ConstrainedAffineBoundsQueryV2, MAX_CONSTRAINED_AFFINE_CONSTRAINTS_V2,
    MAX_CONSTRAINED_AFFINE_MULTIPLIER_V2, MAX_CONSTRAINED_AFFINE_RANK_V2,
    check_constrained_affine_bounds_certificate_v2,
};
pub use dynamic_constrained_affine_bounds_v3::{
    CheckedDynamicConstrainedAffineBoundsCertificateV3, DYNAMIC_AFFINE_COMPONENT_CEILING_V3,
    DynamicConstrainedAffineBoundsCertificateErrorV3, DynamicConstrainedAffineBoundsCertificateV3,
    check_dynamic_constrained_affine_bounds_certificate_v3,
};
pub use identity::{
    ArtifactIdentityV1, CorrespondenceIdentityV1, DIGEST_BYTES_V1, DigestV1, EvidenceIdentityV1,
    ExactInputIdentityV1, ExactModelIdentityV1, ExactToolIdentityV1, ObligationIdentityV1,
    PropertyIdentityV1, StatementIdentityV1, TcbEntryIdentityV1,
};
pub use model::{
    CheckedEvidenceV1, ContractSetV1, ContractedEvidenceV1, CorrespondenceKindV1,
    CorrespondenceReferenceV1, EvidenceBindingV1, ObligationRecordV1, ObligationSatisfactionV1,
    PropertyEvidenceV1, PropertyKindV1, PropertyRecordV1, PropertyStatusV1, ProvedEvidenceV1,
    TcbEntryKindV1, TcbEntryV1, UnsupportedEvidenceV1, UnsupportedReasonV1, ValidatedEvidenceV1,
};
pub use validation::{
    IdentityFieldV1, MAX_CORRESPONDENCES_V1, MAX_OBLIGATIONS_V1, MAX_PROPERTIES_V1,
    MAX_TCB_ENTRIES_V1, MAX_TCB_REFERENCES_PER_EVIDENCE_V1, SectionV1, ValidationErrorV1,
};
