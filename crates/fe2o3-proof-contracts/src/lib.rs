#![no_std]
#![forbid(unsafe_code)]

//! Solver-neutral records for exact, independently scoped proof claims.
//!
//! This crate validates structural consistency only. A valid record does not
//! authenticate a digest, execute a checker, prove a property, authorize a GPU
//! launch, or promote one property status into another.

extern crate alloc;

mod capability;
mod identity;
mod model;
mod validation;

pub use capability::{
    CAPABILITY_DIAGNOSTIC_NAMESPACE_V1, CAPABILITY_OBLIGATION_SET_VERSION_V1,
    CAPABILITY_PROPERTY_NAMESPACE_V1, CAPABILITY_RESULT_SET_VERSION_V1, CapabilityCodecErrorV1,
    CapabilityCompositionErrorV1, CapabilityDiagnosticIdV1, CapabilityIdentityFieldV1,
    CapabilityObligationIdentityV1, CapabilityObligationSpecV1, CapabilityObligationV1,
    CapabilityOutcomeKindV1, CapabilityOutcomeV1, CapabilityPropertyIdV1, CapabilityRecordKindV1,
    CapabilityResourceV1, CapabilityResultIdentityV1, CapabilityResultSpecV1, CapabilityResultV1,
    CapabilitySubjectFieldV1, CapabilitySubjectV1, ExecutableKirIdentityV1,
    InertCapabilityObligationSetIdentityV1, InertCapabilityObligationSetV1,
    InertCapabilityResultSetIdentityV1, InertCapabilityResultSetV1, KernelIdentityV1,
    KernelRootIdentityV1, LaunchContractIdentityV1, MAX_CAPABILITY_OBLIGATION_SET_BYTES_V1,
    MAX_CAPABILITY_OBLIGATIONS_V1, MAX_CAPABILITY_REJECTED_WITNESS_BYTES_V1,
    MAX_CAPABILITY_RESULT_SET_BYTES_V1, MAX_CAPABILITY_WITNESS_BYTES_V1, TargetModelIdentityV1,
    validate_capability_composition_v1,
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
