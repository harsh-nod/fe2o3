#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

mod attestation;
mod client_profile;
mod current_record_verification;
mod launch_manifest;
mod receipt_publication;
mod service;
mod service_ready;
mod supervisor_handoff;

/// Sole production Unix socket pathname for the protected compiler-execution supervisor.
pub const COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1: &str =
    "/run/fe2o3/compiler-execution-supervisor.sock";

/// Sole production public client-profile pathname.
pub const COMPILER_EXECUTION_CLIENT_PROFILE_PATH_V1: &str =
    "/etc/fe2o3/compiler-execution/client-profile-v1";

pub use attestation::{
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1, COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1,
    CompilerExecutionAttestationChallengeIdentityV1, CompilerExecutionAttestationChallengeV1,
    CompilerExecutionAttestationErrorV1, CompilerExecutionAttestationReceiptIdentityV1,
    CompilerExecutionAttestationReceiptV1, CompilerExecutionAttestationRequestIdentityV1,
    CompilerExecutionAttestationRequestV1, CompilerExecutionIssuerMeasurementV1,
    CompilerExecutionIssuerPolicyIdentityV1, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionSubjectBindingV1, VerifiedCompilerExecutionAttestationV1,
};
pub use client_profile::{
    COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V1, CompilerExecutionClientProfileErrorV1,
    CompilerExecutionClientProfileIdentityV1, CompilerExecutionClientProfileV1,
};
pub use current_record_verification::{
    COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V1,
    COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V1,
    CompilerExecutionCurrentRecordAttestationIdentityV1,
    CompilerExecutionCurrentRecordAttestationV1, CompilerExecutionCurrentRecordVerificationErrorV1,
    CompilerExecutionCurrentRecordVerificationIdentityV1,
    CompilerExecutionCurrentRecordVerificationV1, VerifiedCompilerExecutionCurrentRecordV1,
};
pub use launch_manifest::{
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V1, CompilerExecutionClientProcessIdentityV1,
    CompilerExecutionServiceLaunchManifestErrorV1,
    CompilerExecutionServiceLaunchManifestIdentityV1, CompilerExecutionServiceLaunchManifestV1,
};
pub use receipt_publication::{
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V1,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1, CompilerExecutionReceiptCarriageIdentityV1,
    CompilerExecutionReceiptCarriageV1, CompilerExecutionReceiptPublicationAckIdentityV1,
    CompilerExecutionReceiptPublicationAckV1, CompilerExecutionReceiptPublicationErrorV1,
    CompilerExecutionReceiptPublicationIdentityV1, CompilerExecutionReceiptPublicationV1,
};
pub use service::{
    COMPILER_EXECUTION_SERVICE_CONTROL_REQUEST_BYTES_V1,
    COMPILER_EXECUTION_SERVICE_CONTROL_RESPONSE_BYTES_V1,
    COMPILER_EXECUTION_SERVICE_ISSUE_REQUEST_BYTES_V1,
    COMPILER_EXECUTION_SERVICE_ISSUED_RESPONSE_BYTES_V1,
    COMPILER_EXECUTION_SERVICE_PREPARE_REQUEST_BYTES_V1,
    COMPILER_EXECUTION_SERVICE_PREPARED_RESPONSE_BYTES_V1,
    COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_BYTES_V1,
    COMPILER_EXECUTION_SERVICE_PUBLISHED_RESPONSE_BYTES_V1,
    COMPILER_EXECUTION_SERVICE_RECOVER_REQUEST_BYTES_V1,
    COMPILER_EXECUTION_SERVICE_RECOVERED_RESPONSE_BYTES_V1,
    COMPILER_EXECUTION_SERVICE_VERIFIED_CURRENT_RESPONSE_BYTES_V1,
    COMPILER_EXECUTION_SERVICE_VERIFY_CURRENT_REQUEST_BYTES_V1,
    CompilerExecutionServiceProtocolErrorV1, CompilerExecutionServicePublishDispositionV1,
    CompilerExecutionServiceRequestIdentityV1, CompilerExecutionServiceRequestKindV1,
    CompilerExecutionServiceRequestV1, CompilerExecutionServiceResponseIdentityV1,
    CompilerExecutionServiceResponseKindV1, CompilerExecutionServiceResponseV1,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V1,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V1,
};
pub use service_ready::{
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V1, CompilerExecutionServiceReadyErrorV1,
    CompilerExecutionServiceReadyIdentityV1, CompilerExecutionServiceReadyV1,
};
pub use supervisor_handoff::{
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V1, CompilerExecutionSupervisorHandoffErrorV1,
    CompilerExecutionSupervisorHandoffIdentityV1, CompilerExecutionSupervisorHandoffV1,
};
